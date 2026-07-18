use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use crate::db::repositories::media_generations::{
    MediaGenerationsRepo, PgMediaGenerationsRepo, UpdateGenerationErrorPayload,
    UpdateS3MetadataPayload, UpdateGenerationJobStatusPayload, UpdatePayloadsPayload,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

// ─── Webhook payload ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct MediaWebhookPayload {
    pub job_id: Uuid,
    pub generation_id: Uuid,
    pub status: String,
    pub s3_object_key: Option<String>,
    pub presigned_url: Option<String>,
    pub file_url: Option<String>,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub artifact_metadata: Option<serde_json::Value>,
}

// ─── Response ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub received: bool,
    pub generation_id: Uuid,
}

// ─── Handler ─────────────────────────────────────────────────────────────────

/// POST /internal/media-generations/webhook
///
/// Internal webhook receiver for Python Arq Worker to report job completion/failure.
/// - **HMAC Signature Validation**: Verifies `X-Webhook-Signature` header using `MEDIA_GEN_WEBHOOK_SECRET`
/// - **Idempotency**: Uses `job_id` as key, ignores duplicate callbacks
/// - **Update DB**: Updates `generation_status`, `s3_object_key`, `presigned_download_url`, etc.
///
/// Enhanced with observability (Sub-tasks 3.2.1, 3.2.3):
/// - `#[tracing::instrument]` for structured tracing spans
/// - Structured lifecycle log events for each status transition
#[tracing::instrument(
    name = "webhook.receive",
    skip(state, headers, body),
    fields(endpoint = "/internal/media-generations/webhook")
)]
pub async fn webhook_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    // ─── HMAC Signature Validation ───────────────────────────────────────────
    let signature_header = headers
        .get("X-Webhook-Signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing X-Webhook-Signature header".into()))?
        .to_string();

    let secret = &state.config.media_gen_webhook_secret;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| AppError::Internal("invalid HMAC key length".into()))?;
    mac.update(&body);
    let expected_sig = hex::encode(mac.finalize().into_bytes());

    if !constant_time_eq(&signature_header, &expected_sig) {
        return Err(AppError::Unauthorized(
            "Invalid X-Webhook-Signature — payload may have been tampered".into(),
        ));
    }

    let payload: MediaWebhookPayload = serde_json::from_slice(&body)
        .map_err(|e| AppError::Validation(format!("invalid JSON payload: {e}")))?;

    // Sub-task 3.2.1: Log webhook reception with structured fields
    tracing::info!(
        job_id = %payload.job_id,
        generation_id = %payload.generation_id,
        status = %payload.status,
        event = "webhook.received",
        "webhook: received callback"
    );

    // ─── Idempotency check: ignore duplicate callbacks ───────────────────────
    let repo = PgMediaGenerationsRepo::new(state.db_pool.clone());

    let existing = repo
        .find_by_job_id(payload.job_id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to find generation by job_id: {e}")))?;

    let generation_id = match existing {
        Some(ref gen) => gen.id,
        None => {
            return Err(AppError::NotFound(format!(
                "no generation found for job_id {}",
                payload.job_id
            )));
        }
    };

    // If already in a terminal status, ignore (idempotent)
    if let Some(status) = existing.as_ref().and_then(|g| g.generation_status.as_deref()) {
        if matches!(status, "completed" | "failed") {
            tracing::warn!(
                job_id = %payload.job_id,
                generation_id = %generation_id,
                current_status = %status,
                "ignoring duplicate webhook callback"
            );
            return Ok((
                StatusCode::OK,
                Json(serde_json::json!({
                    "received": true,
                    "generation_id": generation_id,
                    "message": "duplicate callback ignored"
                })),
            ));
        }
    }

    // ─── Process based on status ─────────────────────────────────────────────
    match payload.status.as_str() {
        "completed" => {
            // Update S3 metadata when at least an object key is present.
            // expires_at / presigned_url are optional — job_status can mint a
            // fresh presigned URL on demand from s3_object_key alone.
            if let Some(ref s3_key) = payload.s3_object_key {
                let expires_at = payload.expires_at
                    .map(|naive| naive.and_utc())
                    .unwrap_or_else(|| {
                        chrono::Utc::now() + chrono::Duration::hours(1)
                    });
                let presigned_url = payload
                    .presigned_url
                    .clone()
                    .unwrap_or_default();
                repo.update_s3_metadata(
                    generation_id,
                    &UpdateS3MetadataPayload {
                        s3_object_key: s3_key.clone(),
                        presigned_download_url: presigned_url,
                        presigned_url_expires_at: expires_at,
                    },
                )
                .await
                .map_err(|e| AppError::Internal(format!("failed to update S3 metadata: {e}")))?;
            } else {
                tracing::warn!(
                    job_id = %payload.job_id,
                    generation_id = %generation_id,
                    "webhook completed without s3_object_key — status will still be marked completed"
                );
            }

            // Persist the entire payload to generator_service_response so that publication service
            // can read `file_url`, `artifact_metadata`, etc.
            repo.update_payloads(
                generation_id,
                &UpdatePayloadsPayload {
                    generator_service_response: Some(serde_json::to_value(&payload).unwrap_or_default()),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| AppError::Internal(format!("failed to update payload: {e}")))?;

            // Update status to completed
            repo.update_generation_job_status(
                generation_id,
                &UpdateGenerationJobStatusPayload {
                    generation_job_id: Some(payload.job_id),
                    generation_status: "completed".to_string(),
                },
            )
            .await
            .map_err(|e| AppError::Internal(format!("failed to update generation status: {e}")))?;

            // Sub-task 3.2.1: Structured lifecycle log for completed transition
            tracing::info!(
                job_id = %payload.job_id,
                generation_id = %generation_id,
                event = "job.completed",
                s3_object_key = ?payload.s3_object_key,
                "webhook: generation completed successfully — lifecycle transition recorded"
            );
        }
        "failed" => {
            // Update error fields
            repo.update_generation_error(
                generation_id,
                &UpdateGenerationErrorPayload {
                    generation_error_code: payload
                        .error_code
                        .clone()
                        .unwrap_or_else(|| "UNKNOWN_ERROR".to_string()),
                    generation_error_message: payload
                        .error_message
                        .clone()
                        .unwrap_or_else(|| "no error message provided".to_string()),
                },
            )
            .await
            .map_err(|e| AppError::Internal(format!("failed to update generation error: {e}")))?;

            // Update status to failed
            repo.update_generation_job_status(
                generation_id,
                &UpdateGenerationJobStatusPayload {
                    generation_job_id: Some(payload.job_id),
                    generation_status: "failed".to_string(),
                },
            )
            .await
            .map_err(|e| AppError::Internal(format!("failed to update generation status: {e}")))?;

            // Sub-task 3.2.1: Structured lifecycle log for failed transition
            tracing::warn!(
                job_id = %payload.job_id,
                generation_id = %generation_id,
                event = "job.failed",
                error_code = ?payload.error_code,
                error_message = ?payload.error_message,
                "webhook: generation failed — lifecycle transition recorded"
            );
        }
        other => {
            return Err(AppError::Validation(format!(
                "unsupported webhook status '{}' — must be 'completed' or 'failed'",
                other
            )));
        }
    }

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "received": true,
            "generation_id": generation_id,
            "status": payload.status,
        })),
    ))
}

// ─── Constant-time comparison ────────────────────────────────────────────────

fn constant_time_eq(a: &str, b: &str) -> bool {
    // Use a simple constant-time comparison to prevent timing attacks
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (ca, cb) in a.bytes().zip(b.bytes()) {
        result |= ca ^ cb;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_eq_same() {
        assert!(constant_time_eq("abc123", "abc123"));
    }

    #[test]
    fn test_constant_time_eq_different() {
        assert!(!constant_time_eq("abc123", "abc456"));
    }

    #[test]
    fn test_constant_time_eq_diff_len() {
        assert!(!constant_time_eq("abc", "abcd"));
    }

    #[test]
    fn test_webhook_response_serialization() {
        let resp = WebhookResponse {
            received: true,
            generation_id: Uuid::new_v4(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["received"], true);
        assert!(json["generation_id"].is_string());
    }
}
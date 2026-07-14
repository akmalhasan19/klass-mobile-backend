//! Python media generator client (HF Space #3).
//!
//! Port of `PythonMediaGeneratorClient` from Laravel.
//!
//! Sends HMAC-signed generation requests to the Python renderer at
//! `{media_gen_url}/v1/generate`. Handles response validation, artifact
//! metadata decoding, result persistence, and error mapping.
//!
//! ## Request body
//!
//! ```json
//! {
//!   "generation_id": "<uuid>",
//!   "generation_spec": { ... GenerationSpec ... },
//!   "contracts": {
//!     "generation_spec": "media_generation_spec.v1",
//!     "artifact_metadata": "media_artifact_metadata.v1"
//!   }
//! }
//! ```
//!
//! ## Response shape
//!
//! The Python renderer returns `artifact_metadata` either at the top level
//! or nested under a `response` key:
//!
//! ```json
//! { "artifact_metadata": { ... ArtifactMetadata ... } }
//! // or
//! { "response": { "artifact_metadata": { ... } } }
//! ```

use async_trait::async_trait;
use std::time::Duration;

use reqwest::Client as HttpClient;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::signing::InterServiceRequestSigner;
use crate::config::AppConfig;
use crate::contracts::artifact_metadata::{self as artifact_metadata_contract, ArtifactMetadata};
use crate::contracts::generation_spec as spec_contract;
use crate::orchestrator::workflow::{GenerateStep, WorkflowError};

// ─── Constants ──────────────────────────────────────────────────────────────

/// Endpoint path on the Python renderer.
const GENERATE_PATH: &str = "/v1/generate";

/// Error codes matching the plan: sent to the orchestration audit trail.
const ERROR_PYTHON_SERVICE_UNAVAILABLE: &str = "PYTHON_SERVICE_UNAVAILABLE";
const ERROR_ARTIFACT_INVALID: &str = "ARTIFACT_INVALID";

// ─── Error type ─────────────────────────────────────────────────────────────

/// Error type for Python media generator client operations.
#[derive(Debug, thiserror::Error)]
pub enum PythonClientError {
    /// Generation not found in DB.
    #[error("generation not found: {0}")]
    NotFound(String),

    /// Generation spec payload is missing.
    #[error("generation spec payload missing for {0}")]
    MissingSpec(String),

    /// HTTP request failed (transport, timeout).
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// Python renderer returned an error response.
    #[error("Python renderer error ({code}): {message}")]
    RendererError {
        code: String,
        message: String,
        status: u16,
        raw_body: Value,
    },

    /// Artifact metadata validation failed.
    #[error("artifact metadata validation failed: {0}")]
    ArtifactValidation(String),

    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// JSON serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// UUID parse error.
    #[error("invalid UUID: {0}")]
    InvalidUuid(String),
}

impl From<PythonClientError> for WorkflowError {
    fn from(e: PythonClientError) -> Self {
        WorkflowError::StepProvider(e.to_string())
    }
}

// ─── Client ─────────────────────────────────────────────────────────────────

/// Client for communicating with the Python renderer (HF Space #3).
pub struct PythonMediaGeneratorClient {
    pool: PgPool,
    http: HttpClient,
    base_url: String,
    signer: InterServiceRequestSigner,
    timeout: Duration,
    retry_attempts: u32,
    retry_backoff: Duration,
}

impl PythonMediaGeneratorClient {
    /// Create a new client from shared infrastructure.
    pub fn new(pool: PgPool, http: HttpClient, config: &AppConfig) -> Self {
        let signer = InterServiceRequestSigner::new(config.media_gen_hmac_secret.clone());
        let python_cfg = &config.media_generation.python;
        Self {
            pool,
            http,
            base_url: config.media_gen_url.trim_end_matches('/').to_string(),
            signer,
            timeout: Duration::from_secs_f64(python_cfg.timeout_seconds.max(1.0)),
            retry_attempts: python_cfg.retry_attempts.max(1),
            retry_backoff: Duration::from_millis(python_cfg.retry_sleep_milliseconds),
        }
    }

    /// Create a client with explicit overrides (useful for tests).
    pub fn new_with(
        pool: PgPool,
        http: HttpClient,
        base_url: &str,
        hmac_secret: &str,
        timeout: Duration,
        retry_attempts: u32,
        retry_backoff: Duration,
    ) -> Self {
        let signer = InterServiceRequestSigner::new(hmac_secret.to_string());
        Self {
            pool,
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            signer,
            timeout,
            retry_attempts: retry_attempts.max(1),
            retry_backoff,
        }
    }

    /// Run the full generate pipeline for a generation.
    ///
    /// 1. Load the generation spec payload from DB.
    /// 2. Build the HMAC-signed request body.
    /// 3. POST to the Python renderer with retry logic.
    /// 4. Decode and validate the returned artifact metadata.
    /// 5. Persist the result to the media_generations row.
    pub async fn generate(&self, generation_id: &str) -> Result<GenerateResult, PythonClientError> {
        let gen_id = Uuid::parse_str(generation_id)
            .map_err(|e| PythonClientError::InvalidUuid(e.to_string()))?;

        // Step 1: Load the generation spec payload from DB
        let spec_value = self.load_spec(gen_id).await?;

        // Step 2: Parse as GenerationSpec to validate
        let spec_str = serde_json::to_string(&spec_value)?;
        let _spec = spec_contract::decode_and_validate(&spec_str).map_err(|e| {
            PythonClientError::ArtifactValidation(format!(
                "generation spec validation failed: {}",
                e.message
            ))
        })?;

        // Step 3: Build request body
        let body = build_request_body(generation_id, &spec_value);

        // Step 4: POST to Python renderer with retry
        let response = self.send_with_retry(&body).await?;

        // Step 5: Extract artifact metadata from response
        let (artifact_metadata, raw_response) = extract_artifact_metadata(&response)?;

        // Step 6: Persist result to DB
        self.persist_result(
            gen_id,
            &artifact_metadata,
            &raw_response,
        )
        .await?;

        Ok(GenerateResult {
            artifact_metadata,
            raw_response,
        })
    }

    // ── Internal: load spec from DB ───────────────────────────────────────

    async fn load_spec(&self, gen_id: Uuid) -> Result<Value, PythonClientError> {
        let spec: Option<Value> = sqlx::query_scalar(
            r#"SELECT generation_spec_payload FROM media_generations WHERE id = $1"#,
        )
        .bind(gen_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(PythonClientError::Database)?
        .flatten();

        let spec = spec.ok_or_else(|| PythonClientError::NotFound(gen_id.to_string()))?;
        if spec.is_null() || !spec.is_object() {
            return Err(PythonClientError::MissingSpec(gen_id.to_string()));
        }
        Ok(spec)
    }

    // ── Internal: send with retry ─────────────────────────────────────────

    async fn send_with_retry(
        &self,
        body: &Value,
    ) -> Result<Value, PythonClientError> {
        let url = format!("{}{}", self.base_url, GENERATE_PATH);
        let body_bytes = serde_json::to_vec(body)?;
        let generation_id = Uuid::new_v4().to_string();
        let signed = self.signer.build(&generation_id, &body_bytes);

        let mut last_error = None;

        for attempt in 1..=self.retry_attempts {
            let request = self
                .http
                .post(&url)
                .header("Content-Type", "application/json")
                .header("X-Request-Id", &signed.request_id)
                .header("X-Klass-Generation-Id", &signed.generation_id)
                .header("X-Klass-Request-Timestamp", &signed.timestamp)
                .header("X-Klass-Signature-Algorithm", &signed.signature_algorithm)
                .header("X-Klass-Signature", &signed.signature)
                .body(body_bytes.clone())
                .timeout(self.timeout);

            match request.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let body_value: Value = resp.json().await.map_err(|e| {
                        PythonClientError::Http(e)
                    })?;

                    if status.is_success() {
                        return Ok(body_value);
                    }

                    let error_code = map_error_code(status.as_u16(), &body_value);
                    let error_message = extract_error_message(&body_value);
                    let err = PythonClientError::RendererError {
                        code: error_code,
                        message: error_message,
                        status: status.as_u16(),
                        raw_body: body_value,
                    };

                    // Retry only on server errors (5xx) and 429
                    if status.is_server_error() || status.as_u16() == 429 {
                        if attempt < self.retry_attempts {
                            let delay = self.retry_backoff * (1u64 << (attempt - 1)) as u32;
                            tokio::time::sleep(delay).await;
                            last_error = Some(err);
                            continue;
                        }
                    }

                    return Err(err);
                }
                Err(e) => {
                    // Transport errors (connect, timeout) are retryable
                    if attempt < self.retry_attempts {
                        let delay = self.retry_backoff * (1u64 << (attempt - 1)) as u32;
                        tokio::time::sleep(delay).await;
                        last_error = Some(PythonClientError::Http(e));
                        continue;
                    }
                    return Err(PythonClientError::Http(e));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            PythonClientError::RendererError {
                code: ERROR_PYTHON_SERVICE_UNAVAILABLE.to_string(),
                message: "All retry attempts exhausted".to_string(),
                status: 0,
                raw_body: Value::Null,
            }
        }))
    }

    // ── Internal: persist result ──────────────────────────────────────────

    async fn persist_result(
        &self,
        gen_id: Uuid,
        artifact: &ArtifactMetadata,
        raw_response: &Value,
    ) -> Result<(), PythonClientError> {
        sqlx::query(
            r#"
            UPDATE media_generations
            SET generator_service_response = $1,
                generator_provider = $2,
                generator_model = $3,
                mime_type = $4,
                resolved_output_type = $5,
                updated_at = NOW()
            WHERE id = $6
            "#,
        )
        .bind(raw_response)
        .bind(&artifact.generator_provider)
        .bind(&artifact.generator_model)
        .bind(&artifact.mime_type)
        .bind(&artifact.output_type)
        .bind(gen_id)
        .execute(&self.pool)
        .await
        .map_err(PythonClientError::Database)?;

        Ok(())
    }
}

// ─── GenerateStep trait impl for workflow integration ───────────────────────

#[async_trait]
impl GenerateStep for PythonMediaGeneratorClient {
    async fn generate(&self, generation_id: &str) -> Result<Value, WorkflowError> {
        let result = PythonMediaGeneratorClient::generate(self, generation_id).await?;
        Ok(result.raw_response)
    }
}

// ─── Result type ────────────────────────────────────────────────────────────

/// Result of a successful generation request.
#[derive(Debug, Clone)]
pub struct GenerateResult {
    /// Validated artifact metadata from the Python renderer.
    pub artifact_metadata: ArtifactMetadata,
    /// Raw response body stored as `generator_service_response`.
    pub raw_response: Value,
}

// ─── Request body builder ───────────────────────────────────────────────────

fn build_request_body(generation_id: &str, generation_spec: &Value) -> Value {
    serde_json::json!({
        "generation_id": generation_id,
        "generation_spec": generation_spec,
        "contracts": {
            "generation_spec": "media_generation_spec.v1",
            "artifact_metadata": "media_artifact_metadata.v1",
        }
    })
}

// ─── Response parsing ───────────────────────────────────────────────────────

/// Extract the artifact metadata from the Python renderer response.
///
/// Supports two response shapes:
/// - `{ "artifact_metadata": { ... } }`  (top-level)
/// - `{ "response": { "artifact_metadata": { ... } } }`  (nested)
fn extract_artifact_metadata(
    response: &Value,
) -> Result<(ArtifactMetadata, Value), PythonClientError> {
    // Try top-level artifact_metadata first
    let metadata_value = response
        .get("artifact_metadata")
        .or_else(|| response.pointer("/response/artifact_metadata"))
        .ok_or_else(|| {
            PythonClientError::ArtifactValidation(
                "response missing artifact_metadata field".to_string(),
            )
        })?;

    let metadata_str = serde_json::to_string(metadata_value)?;
    let metadata = artifact_metadata_contract::decode_and_validate(&metadata_str).map_err(|e| {
        PythonClientError::ArtifactValidation(format!(
            "artifact metadata contract validation failed: {}",
            e.message
        ))
    })?;

    Ok((metadata, response.clone()))
}

// ─── Error mapping ─────────────────────────────────────────────────────────

/// Map the Python renderer's error response to a canonical error code.
///
/// Priority:
/// 1. `error.laravel_error_code_hint` if present in the response body
/// 2. 5xx / 429 → `PYTHON_SERVICE_UNAVAILABLE`
/// 3. else → `ARTIFACT_INVALID`
fn map_error_code(status: u16, body: &Value) -> String {
    // Check for Laravel-style error code hint
    if let Some(hint) = body
        .pointer("/error/laravel_error_code_hint")
        .and_then(|v| v.as_str())
    {
        if !hint.is_empty() {
            return hint.to_string();
        }
    }

    // Also check top-level error_code field
    if let Some(code) = body.get("error_code").and_then(|v| v.as_str()) {
        if !code.is_empty() {
            return code.to_string();
        }
    }

    match status {
        429 | 500..=599 => ERROR_PYTHON_SERVICE_UNAVAILABLE.to_string(),
        _ => ERROR_ARTIFACT_INVALID.to_string(),
    }
}

/// Extract a human-readable error message from the response body.
fn extract_error_message(body: &Value) -> String {
    body.get("error")
        .and_then(|e| e.get("message"))
        .and_then(|v| v.as_str())
        .or_else(|| body.get("message").and_then(|v| v.as_str()))
        .or_else(|| body.get("error_message").and_then(|v| v.as_str()))
        .unwrap_or("Unknown error from Python renderer")
        .to_string()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Request body builder ──────────────────────────────────────────────

    #[test]
    fn test_build_request_body_structure() {
        let spec = serde_json::json!({
            "schema_version": "media_generation_spec.v1",
            "output_type": "pdf",
            "content_draft": {
                "title": "Test",
                "summary": "Summary",
                "sections": []
            }
        });
        let body = build_request_body("gen-1", &spec);

        assert_eq!(body["generation_id"], "gen-1");
        assert_eq!(body["generation_spec"]["output_type"], "pdf");
        assert_eq!(
            body["contracts"]["generation_spec"],
            "media_generation_spec.v1"
        );
        assert_eq!(
            body["contracts"]["artifact_metadata"],
            "media_artifact_metadata.v1"
        );
    }

    // ── Extract artifact metadata ─────────────────────────────────────────

    #[test]
    fn test_extract_top_level_artifact_metadata() {
        let response = serde_json::json!({
            "artifact_metadata": {
                "schema_version": "media_artifact_metadata.v1",
                "filename": "output.pdf",
                "mime_type": "application/pdf",
                "size_bytes": 102400,
                "checksum_sha256": "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
                "output_type": "pdf",
                "generator_provider": "python-renderer",
                "generator_model": "v2"
            }
        });

        let (metadata, _) = extract_artifact_metadata(&response).unwrap();
        assert_eq!(metadata.filename, "output.pdf");
        assert_eq!(metadata.mime_type, "application/pdf");
        assert_eq!(metadata.output_type, "pdf");
        assert_eq!(
            metadata.generator_provider,
            Some("python-renderer".to_string())
        );
    }

    #[test]
    fn test_extract_nested_artifact_metadata() {
        let response = serde_json::json!({
            "response": {
                "artifact_metadata": {
                    "schema_version": "media_artifact_metadata.v1",
                    "filename": "slide-deck.pptx",
                    "mime_type": "application/vnd.openxmlformats-officedocument.presentationml.presentation",
                    "size_bytes": 512000,
                    "checksum_sha256": "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
                    "output_type": "pptx"
                }
            },
            "status": "completed"
        });

        let (metadata, _) = extract_artifact_metadata(&response).unwrap();
        assert_eq!(metadata.filename, "slide-deck.pptx");
        assert_eq!(metadata.output_type, "pptx");
    }

    #[test]
    fn test_extract_missing_metadata_returns_error() {
        let response = serde_json::json!({"status": "error"});
        let result = extract_artifact_metadata(&response);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing artifact_metadata"));
    }

    // ── Error code mapping ────────────────────────────────────────────────

    #[test]
    fn test_map_error_code_laravel_hint() {
        let body = serde_json::json!({
            "error": {
                "laravel_error_code_hint": "RENDERER_TIMEOUT"
            }
        });
        assert_eq!(map_error_code(500, &body), "RENDERER_TIMEOUT");
    }

    #[test]
    fn test_map_error_code_top_level_code() {
        let body = serde_json::json!({
            "error_code": "INVALID_INPUT"
        });
        assert_eq!(map_error_code(400, &body), "INVALID_INPUT");
    }

    #[test]
    fn test_map_error_code_5xx_returns_unavailable() {
        let body = serde_json::json!({});
        assert_eq!(map_error_code(503, &body), "PYTHON_SERVICE_UNAVAILABLE");
        assert_eq!(map_error_code(502, &body), "PYTHON_SERVICE_UNAVAILABLE");
    }

    #[test]
    fn test_map_error_code_429_returns_unavailable() {
        let body = serde_json::json!({});
        assert_eq!(map_error_code(429, &body), "PYTHON_SERVICE_UNAVAILABLE");
    }

    #[test]
    fn test_map_error_code_4xx_returns_invalid() {
        let body = serde_json::json!({});
        assert_eq!(map_error_code(400, &body), "ARTIFACT_INVALID");
        assert_eq!(map_error_code(422, &body), "ARTIFACT_INVALID");
    }

    // ── Extract error message ─────────────────────────────────────────────

    #[test]
    fn test_extract_error_message_nested_error() {
        let body = serde_json::json!({
            "error": { "message": "spec validation failed" }
        });
        assert_eq!(
            extract_error_message(&body),
            "spec validation failed"
        );
    }

    #[test]
    fn test_extract_error_message_top_level() {
        let body = serde_json::json!({
            "message": "Service unavailable"
        });
        assert_eq!(extract_error_message(&body), "Service unavailable");
    }

    #[test]
    fn test_extract_error_message_fallback() {
        let body = serde_json::json!({});
        assert_eq!(
            extract_error_message(&body),
            "Unknown error from Python renderer"
        );
    }

    // ── PythonClientError ─────────────────────────────────────────────────

    #[test]
    fn test_error_display_not_found() {
        let err = PythonClientError::NotFound("gen-1".to_string());
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_error_display_missing_spec() {
        let err = PythonClientError::MissingSpec("gen-1".to_string());
        assert!(err.to_string().contains("generation spec payload missing"));
    }

    #[test]
    fn test_error_display_renderer_error() {
        let err = PythonClientError::RendererError {
            code: "PYTHON_SERVICE_UNAVAILABLE".to_string(),
            message: "timeout".to_string(),
            status: 503,
            raw_body: Value::Null,
        };
        let msg = err.to_string();
        assert!(msg.contains("PYTHON_SERVICE_UNAVAILABLE"));
        assert!(msg.contains("timeout"));
    }

    #[test]
    fn test_error_display_artifact_validation() {
        let err =
            PythonClientError::ArtifactValidation("missing checksum".to_string());
        assert!(err.to_string().contains("artifact metadata validation"));
    }

    #[test]
    fn test_error_conversion_to_workflow_error() {
        let err = PythonClientError::NotFound("gen-1".to_string());
        let wf_err: WorkflowError = err.into();
        assert!(wf_err.to_string().contains("step provider error"));
        assert!(wf_err.to_string().contains("not found"));
    }

}

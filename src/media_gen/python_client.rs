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
//!
//! ## Response shape
//!
//! The Python renderer now returns HTTP 202 Accepted.

use async_trait::async_trait;
use std::time::Duration;

use reqwest::Client as HttpClient;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::signing::InterServiceRequestSigner;
use crate::config::AppConfig;

use crate::orchestrator::workflow::{GenerateStep, WorkflowError};

// ─── Constants ──────────────────────────────────────────────────────────────

/// Async job submission endpoint on the Python renderer (fire-and-forget, Task 1.4.4).
const JOBS_PATH: &str = "/v1/jobs";

/// Webhook callback path on the Rust Gateway (where Python sends completion updates).
const WEBHOOK_PATH: &str = "/internal/media-generations/webhook";

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
    webhook_base_url: String,
    signer: InterServiceRequestSigner,
    timeout: Duration,
    _retry_attempts: u32,
    _retry_backoff: Duration,
}

impl PythonMediaGeneratorClient {
    /// Create a new client from shared infrastructure.
    pub fn new(pool: PgPool, http: HttpClient, config: &AppConfig) -> Self {
        let signer = InterServiceRequestSigner::new(config.media_gen_hmac_secret.clone());
        let python_cfg = &config.media_generation.python;
        // Strip trailing /v1/jobs if present — JOBS_PATH will be appended later
        let raw_url = config.media_gen_url.trim_end_matches('/');
        let base_url = if raw_url.ends_with(JOBS_PATH) {
            raw_url[..raw_url.len() - JOBS_PATH.len()].trim_end_matches('/').to_string()
        } else {
            raw_url.to_string()
        };
        Self {
            pool,
            http,
            base_url,
            webhook_base_url: config.webhook_base_url.trim_end_matches('/').to_string(),
            signer,
            timeout: Duration::from_secs_f64(python_cfg.timeout_seconds.max(1.0)),
            _retry_attempts: python_cfg.retry_attempts.max(1),
            _retry_backoff: Duration::from_millis(python_cfg.retry_sleep_milliseconds),
        }
    }

    /// Create a client with explicit overrides (useful for tests).
    pub fn new_with(
        pool: PgPool,
        http: HttpClient,
        base_url: &str,
        webhook_base_url: &str,
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
            webhook_base_url: webhook_base_url.trim_end_matches('/').to_string(),
            signer,
            timeout,
            _retry_attempts: retry_attempts.max(1),
            _retry_backoff: retry_backoff,
        }
    }

    async fn load_spec(&self, generation_id: Uuid) -> Result<serde_json::Value, PythonClientError> {
        use sqlx::Row;
        let rec = sqlx::query(
            "SELECT generation_spec_payload FROM media_generations WHERE id = $1"
        )
        .bind(generation_id)
        .fetch_optional(&self.pool)
        .await?;

        let row = rec.ok_or_else(|| PythonClientError::NotFound(generation_id.to_string()))?;
        let spec: Option<serde_json::Value> = row.try_get("generation_spec_payload")?;
        
        let spec = spec.ok_or_else(|| PythonClientError::MissingSpec(generation_id.to_string()))?;
        
        Ok(spec)
    }
}

// ─── Fire-and-Forget: Submit job to Python async endpoint (Task 1.4.4) ───────

impl PythonMediaGeneratorClient {
    /// Submit a generation job to the Python renderer's async endpoint.
    ///
    /// This is the fire-and-forget method used by the new async workflow.
    /// It POSTs to `POST /v1/jobs` with the generation spec and a webhook URL.
    /// The Python Arq worker processes the job asynchronously and sends a
    /// webhook back to the Rust Gateway when completed or failed.
    ///
    /// Returns `Ok(())` on HTTP 202 Accepted — the job has been queued.
    ///
    /// Enhanced with observability (Sub-task 3.2.3):
    /// - `#[tracing::instrument]` for structured tracing span on job submission
    #[tracing::instrument(
        name = "generation.submit_job",
        skip(self),
        fields(generation_id = %generation_id, job_id = %job_id)
    )]
    pub async fn submit_job(&self, generation_id: &str, job_id: &str) -> Result<(), PythonClientError> {
        let gen_id = Uuid::parse_str(generation_id)
            .map_err(|e| PythonClientError::InvalidUuid(e.to_string()))?;

        // Step 1: Load the generation spec from DB
        let spec_value = self.load_spec(gen_id).await?;

        // Step 2: Build the fire-and-forget job request body
        let webhook_url = format!("{}{}", self.webhook_base_url, WEBHOOK_PATH);
        let body = build_job_request_body(generation_id, job_id, &spec_value, &webhook_url);

        // Step 3: POST to Python renderer's async endpoint with HMAC signing
        let url = format!("{}{}", self.base_url, JOBS_PATH);
        let body_bytes = serde_json::to_vec(&body)?;
        let signed = self.signer.build(generation_id, &body_bytes);

        let response = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-Request-Id", &signed.request_id)
            .header("X-Klass-Generation-Id", &signed.generation_id)
            .header("X-Klass-Request-Timestamp", &signed.timestamp)
            .header("X-Klass-Signature-Algorithm", &signed.signature_algorithm)
            .header("X-Klass-Signature", &signed.signature)
            .body(body_bytes)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(PythonClientError::Http)?;

        let status = response.status();
        let body_value: Value = response.json().await.map_err(PythonClientError::Http)?;

        if status == reqwest::StatusCode::ACCEPTED || status.is_success() {
            // Fire-and-forget: job accepted, return immediately
            Ok(())
        } else {
            let error_code = map_error_code(status.as_u16(), &body_value);
            let error_message = extract_error_message(&body_value);

            // Log full validation error details for debugging
            tracing::warn!(
                generation_id = %generation_id,
                job_id = %job_id,
                http_status = status.as_u16(),
                error_code = %error_code,
                error_message = %error_message,
                validation_details = %body_value,
                "submit_job: Python renderer rejected the request"
            );

            Err(PythonClientError::RendererError {
                code: error_code,
                message: error_message,
                status: status.as_u16(),
                raw_body: body_value,
            })
        }
    }
}

// ─── GenerateStep trait impl for workflow integration ───────────────────────

#[async_trait]
impl GenerateStep for PythonMediaGeneratorClient {
    async fn submit_job(&self, generation_id: &str, job_id: &str) -> Result<(), WorkflowError> {
        PythonMediaGeneratorClient::submit_job(self, generation_id, job_id)
            .await
            .map_err(WorkflowError::from)
    }
}

// ─── Request body builder ───────────────────────────────────────────────────

/// Build the request body for the async job submission endpoint (`POST /v1/jobs`).
///
/// Includes the `webhook_url` so the Python Arq worker knows where to send
/// the completion callback.
fn build_job_request_body(
    generation_id: &str,
    job_id: &str,
    generation_spec: &Value,
    webhook_url: &str,
) -> Value {
    serde_json::json!({
        "generation_id": generation_id,
        "job_id": job_id,
        "generation_spec": generation_spec,
        "webhook_url": webhook_url,
    })
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
    fn test_error_conversion_to_workflow_error() {
        let err = PythonClientError::NotFound("gen-1".to_string());
        let wf_err: WorkflowError = err.into();
        assert!(wf_err.to_string().contains("step provider error"));
        assert!(wf_err.to_string().contains("not found"));
    }

}

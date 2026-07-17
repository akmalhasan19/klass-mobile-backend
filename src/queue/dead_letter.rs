//! Dead-letter queue (DLQ) for failed media generation jobs.
//!
//! Port of `MediaGenerationDeadLetterService` from Laravel.
//!
//! After N retry attempts, the job is moved to the DLQ via `XADD` and the
//! generation is marked as `failed` via the audit trail service. An admin
//! manual retry helper is provided for Phase 7 to re-enqueue DLQ'd jobs.

use deadpool_redis::Pool as RedisPool;
use serde_json::Value;

use crate::orchestrator::audit_trail::AuditTrailService;
use crate::queue::redis_streams::QueueService;

// ─── Constants ──────────────────────────────────────────────────────────────

/// Redis stream key for the dead-letter queue.
pub const DLQ_STREAM_KEY: &str = "klass:media-gen-dlq";

/// Max retry attempts before moving to DLQ.
pub const MAX_RETRY_ATTEMPTS: i64 = 3;

// ─── Service ────────────────────────────────────────────────────────────────

/// Service for managing the dead-letter queue.
pub struct DeadLetterService {
    pool: RedisPool,
}

impl DeadLetterService {
    /// Create a new dead-letter service.
    pub fn new(pool: RedisPool) -> Self {
        Self { pool }
    }

    /// Move a failed job to the DLQ and mark the generation as failed.
    ///
    /// 1. XADD `klass:media-gen-dlq` with failure details
    /// 2. Call `AuditTrailService::mark_failed()` to update the generation status
    ///
    /// This is best-effort: if the Redis XADD fails, the error is logged but
    /// the audit trail failure is still attempted.
    pub async fn send_to_dlq(
        &self,
        generation_id: &str,
        attempt: i64,
        error_code: &str,
        error_message: &str,
        audit: &AuditTrailService,
    ) -> Result<(), DlqError> {
        let now = chrono::Utc::now().to_rfc3339();

        // Step 1: XADD to DLQ stream
        let dlq_payload = serde_json::json!({
            "generation_id": generation_id,
            "attempt": attempt,
            "error_code": error_code,
            "error_message": error_message,
            "moved_to_dlq_at": now,
            "retry_context": Self::build_retry_context(generation_id, attempt, error_message),
        });

        let dlq_result = self.xadd_dlq(generation_id, attempt, &dlq_payload).await;

        match &dlq_result {
            Ok(entry_id) => {
                tracing::error!(
                    generation_id = %generation_id,
                    attempt = attempt,
                    error_code = %error_code,
                    dlq_entry_id = %entry_id,
                    "DLQ: job moved to dead-letter queue"
                );
            }
            Err(e) => {
                tracing::error!(
                    generation_id = %generation_id,
                    error = %e,
                    "DLQ: failed to XADD to dead-letter queue"
                );
            }
        }

        // Step 2: Mark generation as failed via audit trail
        let job_context = serde_json::json!({
            "connection": "redis",
            "queue": "klass:media-gen-dlq",
            "tries": MAX_RETRY_ATTEMPTS,
            "dlq_entry_id": dlq_result.as_ref().ok(),
        });

        let _ = audit
            .mark_failed(
                generation_id,
                error_code,
                error_message,
                false,  // not retryable — already exhausted max tries
                Some(dlq_payload),
                Some(attempt as i32),
                Some(job_context),
            )
            .await;

        Ok(())
    }

    /// XADD a job to the dead-letter stream.
    async fn xadd_dlq(
        &self,
        generation_id: &str,
        attempt: i64,
        payload: &Value,
    ) -> Result<String, DlqError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| DlqError::Redis(e.to_string()))?;

        let entry_id: String = redis::cmd("XADD")
            .arg(DLQ_STREAM_KEY)
            .arg("*")
            .arg("generation_id")
            .arg(generation_id)
            .arg("attempt")
            .arg(attempt.to_string())
            .arg("payload")
            .arg(payload.to_string())
            .query_async(&mut *conn)
            .await
            .map_err(|e| DlqError::Redis(e.to_string()))?;

        Ok(entry_id)
    }

    /// Build retry context for the admin manual retry endpoint.
    ///
    /// Used in Phase 7 to allow admins to manually retry a failed generation.
    /// Returns a JSON object with the retry context that can be stored or
    /// returned to the admin API caller.
    pub fn build_retry_context(
        generation_id: &str,
        attempt: i64,
        error: &str,
    ) -> Value {
        serde_json::json!({
            "generation_id": generation_id,
            "attempt": attempt,
            "error": error,
            "dlq_moved_at": chrono::Utc::now().to_rfc3339(),
            "max_retries": MAX_RETRY_ATTEMPTS,
        })
    }

    /// Admin manual retry helper (used in Phase 7).
    ///
    /// Re-enqueues a generation that was moved to the DLQ back into the main
    /// processing stream with attempt=1. This effectively resets the retry
    /// counter so the worker will pick it up and process it again.
    ///
    /// Returns `true` if the re-enqueue was successful, `false` if the
    /// generation was not found in the DLQ.
    pub async fn retry_from_dlq(
        &self,
        generation_id: &str,
        queue: &QueueService,
    ) -> Result<bool, DlqError> {
        // Delete the DLQ entry and re-enqueue
        let deleted = self.delete_dlq_entry(generation_id).await?;

        if !deleted {
            return Ok(false);
        }

        // Re-enqueue to the main stream (no job_id for DLQ retries)
        queue
            .enqueue(generation_id, "", 1)
            .await
            .map_err(|e| DlqError::Redis(e.to_string()))?;

        tracing::info!(
            generation_id = %generation_id,
            "DLQ: job manually retried from dead-letter queue"
        );

        Ok(true)
    }

    /// Delete all DLQ entries for a given generation_id.
    ///
    /// Uses `XDEL` to remove entries from the DLQ stream. Since multiple
    /// entries may exist for the same generation, this iterates through
    /// pending DLQ entries and deletes matching ones.
    async fn delete_dlq_entry(&self, generation_id: &str) -> Result<bool, DlqError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| DlqError::Redis(e.to_string()))?;

        // Get entries pending in the DLQ for this generation
        let entries: Option<redis::Value> = redis::cmd("XRANGE")
            .arg(DLQ_STREAM_KEY)
            .arg("-")
            .arg("+")
            .query_async(&mut *conn)
            .await
            .map_err(|e| DlqError::Redis(e.to_string()))?;

        let entry_ids = match entries {
            Some(redis::Value::Array(items)) => items
                .iter()
                .filter_map(|item| match item {
                    redis::Value::Array(parts) if parts.len() >= 2 => {
                        let entry_id = value_to_str(&parts[0])?;
                        // Check if the entry matches our generation_id
                        let fields = &parts[1];
                        match fields {
                            redis::Value::Array(field_pairs) => {
                                for pair in field_pairs.chunks(2) {
                                    if pair.len() == 2 {
                                        let key = value_to_str(&pair[0])?;
                                        if key == "generation_id" {
                                            let val = value_to_str(&pair[1])?;
                                            if val == generation_id {
                                                return Some(entry_id);
                                            }
                                        }
                                    }
                                }
                                None
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };

        if entry_ids.is_empty() {
            return Ok(false);
        }

        // XDEL each matching entry
        for id in &entry_ids {
            let _: Result<(), _> = redis::cmd("XDEL")
                .arg(DLQ_STREAM_KEY)
                .arg(id)
                .query_async(&mut *conn)
                .await;
        }

        Ok(true)
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn value_to_str(v: &redis::Value) -> Option<String> {
    match v {
        redis::Value::BulkString(d) => Some(String::from_utf8_lossy(d).to_string()),
        redis::Value::SimpleString(s) => Some(s.clone()),
        redis::Value::Int(i) => Some(i.to_string()),
        _ => None,
    }
}

// ─── Error type ─────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum DlqError {
    #[error("Redis error: {0}")]
    Redis(String),
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dlq_stream_key_constant() {
        assert_eq!(DLQ_STREAM_KEY, "klass:media-gen-dlq");
    }

    #[test]
    fn test_max_retry_attempts() {
        assert_eq!(MAX_RETRY_ATTEMPTS, 3);
    }

    #[test]
    fn test_build_retry_context_shape() {
        let ctx = DeadLetterService::build_retry_context("gen-1", 3, "timeout");
        assert_eq!(ctx["generation_id"], "gen-1");
        assert_eq!(ctx["attempt"], 3);
        assert_eq!(ctx["error"], "timeout");
        assert_eq!(ctx["max_retries"], 3);
        assert!(ctx["dlq_moved_at"].is_string());
    }

    #[test]
    fn test_dlq_error_display() {
        let err = DlqError::Redis("connection lost".to_string());
        assert!(err.to_string().contains("Redis error"));
    }

    #[test]
    fn test_value_to_str_bulk_string() {
        let v = redis::Value::BulkString(b"hello".to_vec());
        assert_eq!(value_to_str(&v), Some("hello".to_string()));
    }

    #[test]
    fn test_value_to_str_simple_string() {
        let v = redis::Value::SimpleString("OK".to_string());
        assert_eq!(value_to_str(&v), Some("OK".to_string()));
    }

    #[test]
    fn test_value_to_str_int() {
        let v = redis::Value::Int(42);
        assert_eq!(value_to_str(&v), Some("42".to_string()));
    }

    #[test]
    fn test_value_to_str_nil() {
        let v = redis::Value::Nil;
        assert_eq!(value_to_str(&v), None);
    }
}

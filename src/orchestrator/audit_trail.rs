//! Media generation audit trail service.
//!
//! Port of `MediaGenerationAuditTrailService` from Laravel.
//!
//! Orchestrates the lifecycle audit payload stored in `media_generations.orchestration_audit_payload`:
//!
//! - `initialize()` — Seeds the base orchestration_audit_payload with schema version,
//!   timing defaults, and initial status_history entry. Idempotent: does nothing
//!   if payload already exists with the correct schema version.
//! - `transition()` — Validates the transition via `MediaGenerationStatus`, computes
//!   timing (status durations, total duration), appends to status_history (capped 50),
//!   updates the generation's `status`, `error_code`, `error_message`, and payload.
//! - `record_attempt_failure()` — Records an `attempt_failed` event in status_history
//!   without changing the generation's status. Updates latest_error in payload.
//! - `mark_failed()` — Transitions to FAILED if the current status allows it and
//!   the generation is not already COMPLETED or CANCELLED. Sets error_code/error_message.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::orchestrator::lifecycle::MediaGenerationStatus;

// ─── Constants ──────────────────────────────────────────────────────────────

/// Schema version for the orchestration audit payload.
pub const SCHEMA_VERSION: &str = "media_generation_orchestration_audit.v1";

/// Maximum entries in the status_history array (matching Laravel's HISTORY_LIMIT = 50).
const HISTORY_LIMIT: usize = 50;

// ─── Types ──────────────────────────────────────────────────────────────────

/// Error type for audit trail operations.
#[derive(Debug, thiserror::Error)]
pub enum AuditTrailError {
    /// Generation not found.
    #[error("generation not found: {0}")]
    NotFound(String),

    /// Invalid status transition.
    #[error("invalid transition from {from} to {to}")]
    InvalidTransition {
        from: MediaGenerationStatus,
        to: MediaGenerationStatus,
    },

    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Generation is in a terminal state that blocks the operation.
    #[error("generation {generation_id} is already in terminal state {status}")]
    Terminal {
        generation_id: String,
        status: MediaGenerationStatus,
    },

    /// JSON serialization/deserialization error.
    #[error("payload error: {0}")]
    Payload(String),
}

// ─── AuditTrailService ──────────────────────────────────────────────────────

/// Service for managing the orchestration audit trail of media generations.
///
/// All operations use `SELECT ... FOR UPDATE` within a transaction to ensure
/// consistency, matching Laravel's `DB::transaction` + `lockForUpdate` pattern.
pub struct AuditTrailService {
    pool: PgPool,
}

impl AuditTrailService {
    /// Create a new audit trail service.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Public API ────────────────────────────────────────────────────────

    /// Initialize the orchestration_audit_payload for a generation.
    ///
    /// Idempotent: if a payload with the correct schema version already exists,
    /// this is a no-op (matching Laravel's `hasPayload` check).
    ///
    /// Uses `SELECT ... FOR UPDATE` within an explicit transaction.
    pub async fn initialize(&self, generation_id: &str) -> Result<serde_json::Value, AuditTrailError> {
        let gen_id = parse_uuid(generation_id)?;
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(AuditTrailError::Database)?;

        // Lock the generation row
        let exists: bool = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM media_generations WHERE id = $1 FOR UPDATE)",
        )
        .bind(gen_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(AuditTrailError::Database)?;

        if !exists {
            return Err(AuditTrailError::NotFound(generation_id.to_string()));
        }

        // Check if payload already exists with correct schema
        let existing_payload: Option<serde_json::Value> = sqlx::query_scalar(
            r#"
            SELECT orchestration_audit_payload
            FROM media_generations
            WHERE id = $1
              AND orchestration_audit_payload IS NOT NULL
              AND orchestration_audit_payload->>'schema_version' = $2
              AND orchestration_audit_payload->'status_history' IS NOT NULL
            "#,
        )
        .bind(gen_id)
        .bind(SCHEMA_VERSION)
        .fetch_optional(&mut *tx)
        .await
        .map_err(AuditTrailError::Database)?;

        if let Some(payload) = existing_payload {
            tx.commit().await.map_err(AuditTrailError::Database)?;
            return Ok(payload);
        }

        // Initialize the payload
        let base = build_base_payload(generation_id, &now);
        sqlx::query(
            r#"
            UPDATE media_generations
            SET orchestration_audit_payload = $1,
                updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(&base)
        .bind(gen_id)
        .execute(&mut *tx)
        .await
        .map_err(AuditTrailError::Database)?;

        tx.commit().await.map_err(AuditTrailError::Database)?;
        Ok(base)
    }

    /// Transition the generation to a new status.
    ///
    /// Validates the transition via `MediaGenerationStatus::can_transition`,
    /// computes timing, appends to status_history, and updates the generation's
    /// `status`, `error_code`, `error_message`, and `orchestration_audit_payload`.
    ///
    /// All DB operations use `SELECT ... FOR UPDATE` within an explicit transaction.
    pub async fn transition(
        &self,
        generation_id: &str,
        to_status: MediaGenerationStatus,
        context: Option<serde_json::Value>,
        attempt: Option<i32>,
        job_context: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, AuditTrailError> {
        let gen_id = parse_uuid(generation_id)?;
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(AuditTrailError::Database)?;

        let row = sqlx::query_as::<_, (String, Option<serde_json::Value>, String)>(
            r#"
            SELECT status, orchestration_audit_payload, status
            FROM media_generations
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(gen_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(AuditTrailError::Database)?
        .ok_or_else(|| AuditTrailError::NotFound(generation_id.to_string()))?;

        let (_status, opt_payload, from_status_str) = row;
        let from_status = MediaGenerationStatus::from_str(&from_status_str)
            .ok_or_else(|| AuditTrailError::Payload(format!("Unknown status: {}", from_status_str)))?;

        let mut current_payload = match opt_payload {
            Some(v) if !v.is_null() => v,
            _ => build_base_payload(generation_id, &now),
        };

        // Apply runtime metadata
        apply_runtime_metadata(&mut current_payload, &now, attempt, job_context.as_ref());

        if from_status == to_status {
            // Idempotent re-run: clear errors but keep same status
            current_payload["current_status"] = serde_json::json!(to_status.as_str());
            current_payload["latest_error"] = serde_json::Value::Null;

            sqlx::query(
                r#"
                UPDATE media_generations
                SET orchestration_audit_payload = $1,
                    error_code = NULL,
                    error_message = NULL,
                    updated_at = NOW()
                WHERE id = $2
                "#,
            )
            .bind(&current_payload)
            .bind(gen_id)
            .execute(&mut *tx)
            .await
            .map_err(AuditTrailError::Database)?;

            tx.commit().await.map_err(AuditTrailError::Database)?;
            return Ok(current_payload);
        }

        // Validate the transition
        if !from_status.can_transition(to_status) {
            return Err(AuditTrailError::InvalidTransition {
                from: from_status,
                to: to_status,
            });
        }

        // Apply transition timing
        apply_transition_timing(
            &mut current_payload,
            &from_status,
            to_status,
            &now,
        );

        // Update status fields
        current_payload["current_status"] = serde_json::json!(to_status.as_str());
        update_provider_trace(&mut current_payload);
        current_payload["latest_error"] = serde_json::Value::Null;

        // Append to status_history
        let history_entry = serde_json::json!({
            "event_type": "status_transition",
            "from_status": from_status_str,
            "to_status": to_status.as_str(),
            "attempt": attempt.unwrap_or(0),
            "at": now.to_rfc3339(),
            "context": filter_context(context.as_ref()),
        });
        append_history(&mut current_payload, history_entry);

        // Update the generation row
        sqlx::query(
            r#"
            UPDATE media_generations
            SET status = $1,
                orchestration_audit_payload = $2,
                error_code = NULL,
                error_message = NULL,
                updated_at = NOW()
            WHERE id = $3
            "#,
        )
        .bind(to_status.as_str())
        .bind(&current_payload)
        .bind(gen_id)
        .execute(&mut *tx)
        .await
        .map_err(AuditTrailError::Database)?;

        tx.commit().await.map_err(AuditTrailError::Database)?;
        Ok(current_payload)
    }

    /// Record an attempt failure without changing the generation's status.
    ///
    /// Updates `latest_error`, computes `total_duration_ms`, and appends an
    /// `attempt_failed` event to status_history.
    /// Uses `SELECT ... FOR UPDATE` within an explicit transaction.
    pub async fn record_attempt_failure(
        &self,
        generation_id: &str,
        error_code: &str,
        error_message: &str,
        retryable: bool,
        context: Option<serde_json::Value>,
        attempt: Option<i32>,
        job_context: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, AuditTrailError> {
        let gen_id = parse_uuid(generation_id)?;
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(AuditTrailError::Database)?;

        let opt_payload: Option<serde_json::Value> = sqlx::query_scalar(
            r#"
            SELECT orchestration_audit_payload
            FROM media_generations
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(gen_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(AuditTrailError::Database)?;

        let mut payload = match opt_payload {
            Some(v) if !v.is_null() => v,
            _ => build_base_payload(generation_id, &now),
        };

        apply_runtime_metadata(&mut payload, &now, attempt, job_context.as_ref());

        let error = serde_json::json!({
            "error_code": error_code,
            "error_class": "orchestrator::AuditTrailService",
            "message": sanitize_message(error_message),
            "retryable": retryable,
            "safe_context": safe_throwable_context(context.as_ref()),
        });

        let total_dur = calculate_total_duration(&payload, &now);
        payload["latest_error"] = error.clone();
        payload["timing"]["total_duration_ms"] = serde_json::json!(total_dur);

        let history_entry = serde_json::json!({
            "event_type": "attempt_failed",
            "status": payload.get("current_status"),
            "attempt": attempt.unwrap_or(0),
            "at": now.to_rfc3339(),
            "context": filter_context(context.as_ref()),
            "error": error,
        });
        append_history(&mut payload, history_entry);

        sqlx::query(
            r#"
            UPDATE media_generations
            SET orchestration_audit_payload = $1,
                updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(&payload)
        .bind(gen_id)
        .execute(&mut *tx)
        .await
        .map_err(AuditTrailError::Database)?;

        tx.commit().await.map_err(AuditTrailError::Database)?;
        Ok(payload)
    }

    /// Mark the generation as FAILED.
    ///
    /// Only transitions to FAILED if the current status allows it and the
    /// generation is not already COMPLETED or CANCELLED (matching Laravel).
    ///
    /// Sets `error_code` and `error_message` on the generation row.
    /// Uses `SELECT ... FOR UPDATE` within an explicit transaction.
    pub async fn mark_failed(
        &self,
        generation_id: &str,
        error_code: &str,
        error_message: &str,
        retryable: bool,
        context: Option<serde_json::Value>,
        attempt: Option<i32>,
        job_context: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, AuditTrailError> {
        let gen_id = parse_uuid(generation_id)?;
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(AuditTrailError::Database)?;

        let row = sqlx::query_as::<_, (String, Option<serde_json::Value>)>(
            r#"
            SELECT status, orchestration_audit_payload
            FROM media_generations
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(gen_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(AuditTrailError::Database)?
        .ok_or_else(|| AuditTrailError::NotFound(generation_id.to_string()))?;

        let (current_status_str, opt_payload) = row;
        let current_status = MediaGenerationStatus::from_str(&current_status_str)
            .ok_or_else(|| AuditTrailError::Payload(format!("Unknown status: {}", current_status_str)))?;

        // If already COMPLETED or CANCELLED, do nothing (matching Laravel)
        if current_status == MediaGenerationStatus::Completed
            || current_status == MediaGenerationStatus::Cancelled
        {
            tx.commit().await.map_err(AuditTrailError::Database)?;
            return Err(AuditTrailError::Terminal {
                generation_id: generation_id.to_string(),
                status: current_status,
            });
        }

        let mut payload = match opt_payload {
            Some(v) if !v.is_null() => v,
            _ => build_base_payload(generation_id, &now),
        };

        apply_runtime_metadata(&mut payload, &now, attempt, job_context.as_ref());

        let error = serde_json::json!({
            "error_code": error_code,
            "error_class": "orchestrator::AuditTrailService",
            "message": sanitize_message(error_message),
            "retryable": retryable,
            "safe_context": safe_throwable_context(context.as_ref()),
        });

        // Only transition if not already FAILED and transition is valid
        if current_status != MediaGenerationStatus::Failed
            && current_status.can_transition(MediaGenerationStatus::Failed)
        {
            apply_transition_timing(
                &mut payload,
                &current_status,
                MediaGenerationStatus::Failed,
                &now,
            );

            let history_entry = serde_json::json!({
                "event_type": "status_transition",
                "from_status": current_status_str,
                "to_status": "failed",
                "attempt": attempt.unwrap_or(0),
                "at": now.to_rfc3339(),
                "context": filter_context(context.as_ref()),
                "error": error,
            });
            append_history(&mut payload, history_entry);
        }

        payload["current_status"] = serde_json::json!("failed");
        update_provider_trace(&mut payload);
        payload["latest_error"] = error.clone();

        sqlx::query(
            r#"
            UPDATE media_generations
            SET status = 'failed',
                orchestration_audit_payload = $1,
                error_code = $2,
                error_message = $3,
                updated_at = NOW()
            WHERE id = $4
            "#,
        )
        .bind(&payload)
        .bind(error_code)
        .bind(sanitize_message(error_message))
        .bind(gen_id)
        .execute(&mut *tx)
        .await
        .map_err(AuditTrailError::Database)?;

        tx.commit().await.map_err(AuditTrailError::Database)?;
        Ok(payload)
    }

    /// Get the current orchestration_audit_payload for a generation.
    pub async fn get_payload(&self, generation_id: &str) -> Result<serde_json::Value, AuditTrailError> {
        let gen_id = parse_uuid(generation_id)?;
        let maybe: Option<serde_json::Value> = sqlx::query_scalar(
            r#"SELECT orchestration_audit_payload FROM media_generations WHERE id = $1"#,
        )
        .bind(gen_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AuditTrailError::Database)?;

        match maybe {
            Some(v) => Ok(v),
            None => Err(AuditTrailError::NotFound(generation_id.to_string())),
        }
    }
}

// ─── Helper functions ───────────────────────────────────────────────────────

/// Build the initial base payload for a generation.
fn build_base_payload(_generation_id: &str, now: &DateTime<Utc>) -> serde_json::Value {
    let now_rfc = now.to_rfc3339();
    serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "current_status": "queued",
        "resolved_output_type": null,
        "provider_trace": {
            "interpretation": { "name": null, "model": null },
            "generator": { "name": null, "model": null },
            "delivery": { "name": null, "model": null },
        },
        "job": {
            "connection": null,
            "queue": null,
            "tries": null,
            "timeout_seconds": null,
            "backoff_seconds": null,
            "attempt": 0,
            "last_run_at": null,
        },
        "timing": {
            "queued_at": now_rfc,
            "processing_started_at": null,
            "last_transition_at": now_rfc,
            "completed_at": null,
            "total_duration_ms": null,
            "status_durations_ms": {},
        },
        "latest_error": null,
        "status_history": [
            {
                "event_type": "status_transition",
                "from_status": null,
                "to_status": "queued",
                "attempt": 0,
                "at": now_rfc,
                "context": { "reason": "generation_created" },
            }
        ],
    })
}

/// Apply runtime metadata (job context) to the payload.
fn apply_runtime_metadata(
    payload: &mut serde_json::Value,
    now: &DateTime<Utc>,
    attempt: Option<i32>,
    job_context: Option<&serde_json::Value>,
) {
    if let Some(jc) = job_context {
        let job = payload.get_mut("job").and_then(|j| j.as_object_mut());
        if let Some(job_map) = job {
            if let Some(v) = jc.get("connection").and_then(|v| v.as_str()) {
                job_map.insert("connection".to_string(), serde_json::json!(v));
            }
            if let Some(v) = jc.get("queue").and_then(|v| v.as_str()) {
                job_map.insert("queue".to_string(), serde_json::json!(v));
            }
            if let Some(v) = jc.get("tries") {
                job_map.insert("tries".to_string(), v.clone());
            }
            if let Some(v) = jc.get("timeout_seconds") {
                job_map.insert("timeout_seconds".to_string(), v.clone());
            }
            if let Some(v) = jc.get("backoff_seconds") {
                job_map.insert("backoff_seconds".to_string(), v.clone());
            }
        }
    }

    let current_attempt = attempt.unwrap_or_else(|| {
        payload
            .get("job")
            .and_then(|j| j.get("attempt"))
            .and_then(|a| a.as_i64())
            .unwrap_or(0) as i32
    });

    if let Some(job) = payload.get_mut("job") {
        job["attempt"] = serde_json::json!(current_attempt);
        job["last_run_at"] = serde_json::json!(now.to_rfc3339());
    }
}

/// Apply transition timing to the payload.
fn apply_transition_timing(
    payload: &mut serde_json::Value,
    from_status: &MediaGenerationStatus,
    to_status: MediaGenerationStatus,
    now: &DateTime<Utc>,
) {
    let last_transition_at_str = payload
        .pointer("/timing/last_transition_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let last_ts = parse_timestamp(&last_transition_at_str)
        .or_else(|| Some(*now));

    if let Some(ts) = last_ts {
        let duration_ms = (now.timestamp_millis() - ts.timestamp_millis()).max(0);

        let status_durations = payload
            .get_mut("timing")
            .and_then(|t| t.get_mut("status_durations_ms"));

        if let Some(sd) = status_durations {
            let from_key = from_status.as_str();
            let current = sd.get(from_key).and_then(|v| v.as_i64()).unwrap_or(0);
            sd[from_key] = serde_json::json!(current + duration_ms as i64);
        }
    }

    // Set processing_started_at if transitioning from QUEUED to something else
    let processing_started = payload
        .pointer("/timing/processing_started_at")
        .and_then(|v| v.as_str());

    if processing_started.is_none() && to_status != MediaGenerationStatus::Queued {
        if let Some(timing) = payload.get_mut("timing") {
            timing["processing_started_at"] = serde_json::json!(now.to_rfc3339());
        }
    }

    // Completed at only for terminal states
    let completed_at = if to_status.is_terminal() {
        serde_json::json!(now.to_rfc3339())
    } else {
        payload
            .pointer("/timing/completed_at")
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    };

    let queued_at = payload
        .pointer("/timing/queued_at")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| now.to_rfc3339());

    let total_duration = payload
        .pointer("/timing/queued_at")
        .and_then(|v| v.as_str())
        .and_then(parse_timestamp)
        .map(|ts| (now.timestamp_millis() - ts.timestamp_millis()).max(0));

    if let Some(timing) = payload.get_mut("timing") {
        timing["queued_at"] = serde_json::json!(queued_at);
        timing["last_transition_at"] = serde_json::json!(now.to_rfc3339());
        timing["completed_at"] = completed_at;
        timing["total_duration_ms"] = serde_json::json!(total_duration);
    }
}

/// Calculate total duration from queued_at to now.
fn calculate_total_duration(
    payload: &serde_json::Value,
    now: &DateTime<Utc>,
) -> Option<i64> {
    let queued_at_str = payload
        .pointer("/timing/queued_at")
        .and_then(|v| v.as_str())?;

    let queued_ts = parse_timestamp(queued_at_str)?;
    let ms = (now.timestamp_millis() - queued_ts.timestamp_millis()).max(0);
    Some(ms)
}

// (update_total_duration not needed — calculate_total_duration is called inline)

/// Parse an RFC3339 timestamp string.
fn parse_timestamp(s: &str) -> Option<DateTime<Utc>> {
    if s.is_empty() {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Append an event to the status_history (capped at HISTORY_LIMIT).
fn append_history(payload: &mut serde_json::Value, event: serde_json::Value) {
    let mut history = payload
        .get("status_history")
        .and_then(|h| h.as_array().cloned())
        .unwrap_or_default();

    history.push(event);

    if history.len() > HISTORY_LIMIT {
        history = history.split_off(history.len() - HISTORY_LIMIT);
    }

    payload["status_history"] = serde_json::json!(history);
}

/// Fill in provider_trace defaults if missing.
fn update_provider_trace(payload: &mut serde_json::Value) {
    // Ensure provider_trace exists with default structure
    if payload.get("provider_trace").is_none() {
        payload["provider_trace"] = serde_json::json!({
            "interpretation": { "name": null, "model": null },
            "generator": { "name": null, "model": null },
            "delivery": { "name": null, "model": null },
        });
    }
}

/// Compute an error summary for the given error parameters.
fn safe_throwable_context(context: Option<&serde_json::Value>) -> serde_json::Value {
    let mut safe = serde_json::Map::new();
    if let Some(ctx) = context {
        if let Some(obj) = ctx.as_object() {
            let allowed_keys = [
                "http_status",
                "config",
                "kind",
                "adapter_provider",
                "adapter_model",
                "adapter_primary_provider",
                "adapter_fallback_used",
                "adapter_fallback_reason",
            ];
            for key in &allowed_keys {
                if let Some(value) = obj.get(*key) {
                    if value.is_number() || value.is_boolean() || value.is_string() {
                        safe.insert(key.to_string(), value.clone());
                    }
                }
            }
        }
    }
    serde_json::Value::Object(safe)
}

/// Sanitize a message: collapse whitespace, trim, limit to 240 chars.
fn sanitize_message(message: &str) -> String {
    let collapsed: String = message
        .chars()
        .fold((String::with_capacity(message.len()), false), |(mut acc, prev_space), c| {
            if c.is_whitespace() {
                if !prev_space {
                    acc.push(' ');
                }
                (acc, true)
            } else {
                acc.push(c);
                (acc, false)
            }
        })
        .0
        .trim()
        .to_string();

    if collapsed.len() > 240 {
        collapsed[..240].to_string()
    } else {
        collapsed
    }
}

/// Filter context to depth-limited, scalar-only values (matching Laravel's filterContext).
fn filter_context(context: Option<&serde_json::Value>) -> serde_json::Value {
    let Some(ctx) = context else {
        return serde_json::Value::Null;
    };
    filter_context_inner(ctx, 0)
}

fn filter_context_inner(value: &serde_json::Value, depth: usize) -> serde_json::Value {
    if depth > 1 {
        return serde_json::Value::Null;
    }

    match value {
        serde_json::Value::Object(map) => {
            let mut filtered = serde_json::Map::new();
            for (key, val) in map {
                match val {
                    serde_json::Value::Null => continue,
                    serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
                        filtered.insert(key.clone(), val.clone());
                    }
                    serde_json::Value::String(s) => {
                        let trimmed = s.trim();
                        if !trimmed.is_empty() {
                            let truncated = if trimmed.len() > 160 {
                                format!("{}...", &trimmed[..157])
                            } else {
                                trimmed.to_string()
                            };
                            filtered.insert(key.clone(), serde_json::json!(sanitize_message(&truncated)));
                        }
                    }
                    serde_json::Value::Array(arr) => {
                        let items: Vec<serde_json::Value> = arr
                            .iter()
                            .filter_map(|item| match item {
                                serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
                                    Some(item.clone())
                                }
                                serde_json::Value::String(s) => {
                                    let t = s.trim();
                                    if !t.is_empty() {
                                        let truncated = if t.len() > 80 {
                                            format!("{}...", &t[..77])
                                        } else {
                                            t.to_string()
                                        };
                                        Some(serde_json::json!(sanitize_message(&truncated)))
                                    } else {
                                        None
                                    }
                                }
                                _ => None,
                            })
                            .take(8)
                            .collect();

                        if !items.is_empty() {
                            filtered.insert(key.clone(), serde_json::Value::Array(items));
                        }
                    }
                    serde_json::Value::Object(_) => {
                        let nested = filter_context_inner(val, depth + 1);
                        if !nested.is_null() {
                            filtered.insert(key.clone(), nested);
                        }
                    }
                }
            }
            if filtered.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::Object(filtered)
            }
        }
        _ => serde_json::Value::Null,
    }
}

/// Parse a UUID string.
fn parse_uuid(s: &str) -> Result<Uuid, AuditTrailError> {
    Uuid::parse_str(s).map_err(|_| {
        AuditTrailError::Payload(format!("Invalid UUID: {}", s))
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_version() {
        assert_eq!(SCHEMA_VERSION, "media_generation_orchestration_audit.v1");
    }

    #[test]
    fn test_build_base_payload_structure() {
        let now = Utc::now();
        let payload = build_base_payload("00000000-0000-0000-0000-000000000001", &now);

        assert_eq!(
            payload["schema_version"],
            "media_generation_orchestration_audit.v1"
        );
        assert_eq!(payload["current_status"], "queued");
        assert!(payload["resolved_output_type"].is_null());
        assert!(payload["latest_error"].is_null());

        // Provider trace
        assert!(payload["provider_trace"]["interpretation"].is_object());
        assert!(payload["provider_trace"]["generator"].is_object());
        assert!(payload["provider_trace"]["delivery"].is_object());

        // Job defaults
        assert_eq!(payload["job"]["attempt"], 0);
        assert!(payload["job"]["last_run_at"].is_null());
        assert!(payload["job"]["queue"].is_null());

        // Timing defaults
        assert!(payload["timing"]["queued_at"].is_string());
        assert!(payload["timing"]["processing_started_at"].is_null());
        assert!(payload["timing"]["completed_at"].is_null());
        assert!(payload["timing"]["total_duration_ms"].is_null());
        assert!(payload["timing"]["status_durations_ms"].is_object());

        // Status history
        let history = payload["status_history"].as_array().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0]["event_type"], "status_transition");
        assert_eq!(history[0]["to_status"], "queued");
        assert_eq!(history[0]["context"]["reason"], "generation_created");
    }

    #[test]
    fn test_sanitize_message_collapses_whitespace() {
        let msg = "Hello    World\nThis\t is  \n  test";
        assert_eq!(sanitize_message(msg), "Hello World This is test");
    }

    #[test]
    fn test_sanitize_message_trims() {
        let msg = "  hello world  ";
        assert_eq!(sanitize_message(msg), "hello world");
    }

    #[test]
    fn test_sanitize_message_truncates_long() {
        let long = "a".repeat(300);
        let result = sanitize_message(&long);
        assert_eq!(result.len(), 240);
    }

    #[test]
    fn test_sanitize_message_short_stays_same() {
        let msg = "short message";
        assert_eq!(sanitize_message(msg), "short message");
    }

    #[test]
    fn test_sanitize_message_empty() {
        assert_eq!(sanitize_message(""), "");
        assert_eq!(sanitize_message("   "), "");
    }

    #[test]
    fn test_filter_context_none_returns_null() {
        assert!(filter_context(None).is_null());
    }

    #[test]
    fn test_filter_context_scalars_preserved() {
        let ctx = serde_json::json!({
            "http_status": 503,
            "retryable": true,
            "message": "Service unavailable",
        });
        let filtered = filter_context(Some(&ctx));
        assert_eq!(filtered["http_status"], 503);
        assert_eq!(filtered["retryable"], true);
        assert_eq!(filtered["message"], "Service unavailable");
    }

    #[test]
    fn test_filter_context_nulls_removed() {
        let ctx = serde_json::json!({
            "key1": null,
            "key2": "value",
        });
        let filtered = filter_context(Some(&ctx));
        assert!(filtered.get("key1").is_none());
        assert_eq!(filtered["key2"], "value");
    }

    #[test]
    fn test_filter_context_depth_limited() {
        let ctx = serde_json::json!({
            "level1": {
                "level2": {
                    "level3": "deep"
                }
            }
        });
        let filtered = filter_context(Some(&ctx));
        // level3 is stripped because depth > 1. This leaves level2 empty.
        // Empty objects are stripped, which leaves level1 empty.
        // Finally level1 is stripped, leaving the whole object empty (null).
        assert!(filtered.is_null());
    }

    #[test]
    fn test_filter_context_string_truncation() {
        let long = "a".repeat(200);
        let ctx = serde_json::json!({"long": long});
        let filtered = filter_context(Some(&ctx));
        let val = filtered["long"].as_str().unwrap();
        assert!(val.len() <= 163); // 160 + "..."
    }

    #[test]
    fn test_filter_context_array_items() {
        let ctx = serde_json::json!({
            "items": ["short", 42, true, null, ""]
        });
        let filtered = filter_context(Some(&ctx));
        let arr = filtered["items"].as_array().unwrap();
        // null and empty string should be filtered out
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0], "short");
        assert_eq!(arr[1], 42);
        assert_eq!(arr[2], true);
    }

    #[test]
    fn test_filter_context_array_capped_at_8() {
        let items: Vec<String> = (0..20).map(|i| format!("item-{}", i)).collect();
        let ctx = serde_json::json!({"items": items});
        let filtered = filter_context(Some(&ctx));
        let arr = filtered["items"].as_array().unwrap();
        assert!(arr.len() <= 8);
    }

    #[test]
    fn test_safe_throwable_context_empty() {
        let result = safe_throwable_context(None);
        assert!(result.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_safe_throwable_context_filters_allowed_keys() {
        let ctx = serde_json::json!({
            "http_status": 503,
            "config": "default",
            "kind": "timeout",
            "adapter_provider": "xiaomi",
            "adapter_model": "mimo-v2.5",
            "adapter_fallback_used": true,
            "adapter_fallback_reason": "rate_limited",
            "secret_key": "should-not-appear",
            "password": "hidden",
            "response_body": {"nested": "data"},
        });
        let result = safe_throwable_context(Some(&ctx));
        let map = result.as_object().unwrap();
        assert!(map.contains_key("http_status"));
        assert!(map.contains_key("config"));
        assert!(map.contains_key("kind"));
        assert!(map.contains_key("adapter_provider"));
        assert!(map.contains_key("adapter_model"));
        assert!(map.contains_key("adapter_fallback_used"));
        assert!(map.contains_key("adapter_fallback_reason"));
        assert!(!map.contains_key("secret_key"));
        assert!(!map.contains_key("password"));
        assert!(!map.contains_key("response_body"));
    }

    #[test]
    fn test_append_history_capped_at_50() {
        let mut payload = build_base_payload("00000000-0000-0000-0000-000000000001", &Utc::now());
        for i in 0..60 {
            let event = serde_json::json!({"event_type": "test", "index": i});
            append_history(&mut payload, event);
        }
        let history = payload["status_history"].as_array().unwrap();
        assert_eq!(history.len(), 50);
        // Should have the last 50 entries (indices 10-59)
        assert_eq!(history[0]["index"], 10);
        assert_eq!(history[49]["index"], 59);
    }

    #[test]
    fn test_append_history_within_limit() {
        let mut payload = build_base_payload("00000000-0000-0000-0000-000000000001", &Utc::now());
        for i in 0..3 {
            let event = serde_json::json!({"event_type": "test", "index": i});
            append_history(&mut payload, event);
        }
        let history = payload["status_history"].as_array().unwrap();
        assert_eq!(history.len(), 4); // 1 initial + 3 added
    }

    #[test]
    fn test_apply_runtime_metadata_with_job_context() {
        let mut payload = build_base_payload("gen-1", &Utc::now());
        let now = Utc::now();
        let jc = serde_json::json!({
            "connection": "redis",
            "queue": "media-gen",
            "tries": 3,
            "timeout_seconds": 300,
            "backoff_seconds": 30,
        });

        apply_runtime_metadata(&mut payload, &now, Some(1), Some(&jc));

        assert_eq!(payload["job"]["connection"], "redis");
        assert_eq!(payload["job"]["queue"], "media-gen");
        assert_eq!(payload["job"]["tries"], 3);
        assert_eq!(payload["job"]["timeout_seconds"], 300);
        assert_eq!(payload["job"]["backoff_seconds"], 30);
        assert_eq!(payload["job"]["attempt"], 1);
        assert!(payload["job"]["last_run_at"].is_string());
    }

    #[test]
    fn test_calculate_total_duration() {
        let now = Utc::now();
        let payload = build_base_payload("gen-1", &now);

        // Total duration should be ~0 (just created)
        let dur = calculate_total_duration(&payload, &now);
        assert!(dur.is_some());
        assert!(dur.unwrap() >= 0);
    }

    #[test]
    fn test_parse_timestamp_valid() {
        let ts = parse_timestamp("2026-07-14T10:00:00Z");
        assert!(ts.is_some());
        assert_eq!(ts.unwrap().timestamp(), 1784023200);
    }

    #[test]
    fn test_parse_timestamp_empty() {
        assert!(parse_timestamp("").is_none());
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        assert!(parse_timestamp("not-a-timestamp").is_none());
    }

    #[test]
    fn test_parse_uuid_valid() {
        let id = "00000000-0000-0000-0000-000000000001";
        assert!(parse_uuid(id).is_ok());
    }

    #[test]
    fn test_parse_uuid_invalid() {
        assert!(parse_uuid("not-a-uuid").is_err());
    }

    #[test]
    fn test_audit_trail_error_display() {
        let err = AuditTrailError::NotFound("gen-1".to_string());
        assert!(err.to_string().contains("not found"));

        let err = AuditTrailError::InvalidTransition {
            from: MediaGenerationStatus::Queued,
            to: MediaGenerationStatus::Completed,
        };
        let msg = err.to_string();
        assert!(msg.contains("invalid transition"));
    }

    #[test]
    fn test_update_provider_trace_defaults() {
        let mut payload = build_base_payload("gen-1", &Utc::now());
        // Remove provider_trace to test default creation
        payload["provider_trace"] = serde_json::Value::Null;
        // update_provider_trace uses .get().is_none(), not .is_null(),
        // so set to Object::new() which represents "missing" in our payload
        payload.as_object_mut().unwrap().remove("provider_trace");
        update_provider_trace(&mut payload);
        assert!(payload["provider_trace"].is_object());
        assert!(payload["provider_trace"]["interpretation"].is_object());
    }

    #[test]
    fn test_apply_runtime_metadata_without_job_context() {
        let mut payload = build_base_payload("gen-1", &Utc::now());
        let now = Utc::now();
        apply_runtime_metadata(&mut payload, &now, Some(2), None);

        assert_eq!(payload["job"]["attempt"], 2);
        assert!(payload["job"]["last_run_at"].is_string());
        // Other job fields should still be null defaults
        assert!(payload["job"]["queue"].is_null());
    }

    #[test]
    fn test_filter_context_nested_object_preserved() {
        let ctx = serde_json::json!({
            "outer": {
                "inner_key": "inner_value",
                "inner_num": 42
            }
        });
        let filtered = filter_context(Some(&ctx));
        let outer = &filtered["outer"];
        assert_eq!(outer["inner_key"], "inner_value");
        assert_eq!(outer["inner_num"], 42);
    }

    #[test]
    fn test_apply_transition_timing_from_queued() {
        let now = Utc::now();
        let mut payload = build_base_payload("gen-1", &now);

        apply_transition_timing(
            &mut payload,
            &MediaGenerationStatus::Queued,
            MediaGenerationStatus::Interpreting,
            &now,
        );

        assert!(payload["timing"]["processing_started_at"].is_string());
        assert_eq!(
            payload["timing"]["processing_started_at"],
            now.to_rfc3339()
        );
        assert!(payload["timing"]["total_duration_ms"].is_number());
    }

    #[test]
    fn test_apply_transition_timing_terminal_sets_completed_at() {
        let now = Utc::now();
        let mut payload = build_base_payload("gen-1", &now);

        // Simulate some time passing
        let later = now + chrono::Duration::seconds(5);
        apply_transition_timing(
            &mut payload,
            &MediaGenerationStatus::Publishing,
            MediaGenerationStatus::Completed,
            &later,
        );

        assert_eq!(payload["timing"]["completed_at"], later.to_rfc3339());
    }
}

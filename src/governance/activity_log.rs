use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// Represents a row in the `activity_logs` table.
///
/// Mirrors the schema defined in `migrations/20260712000008_create_activity_logs_table.sql`:
/// ```sql
/// CREATE TABLE activity_logs (
///     id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
///     actor_id BIGINT NULL REFERENCES users (id) ON DELETE SET NULL,
///     action VARCHAR(255) NOT NULL,
///     subject_type VARCHAR(255) NULL,
///     subject_id BIGINT NULL,
///     metadata JSONB NULL,
///     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
///     updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
/// );
/// ```
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct ActivityLog {
    pub id: Uuid,
    pub actor_id: Option<i64>,
    pub action: String,
    pub subject_type: Option<String>,
    pub subject_id: Option<i64>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}

/// Record an activity log entry.
///
/// This is the central helper used by all admin write handlers to record
/// mutations such as CREATE, UPDATE, DELETE, PUBLISH, REORDER, etc.
///
/// # Parameters
///
/// * `pool`         — A reference to the PostgreSQL connection pool.
/// * `actor_id`     — The user ID performing the action (`None` for system/anonymous).
/// * `action`       — A machine-readable action label, e.g. `"create_topic"`,
///   `"update_content"`, `"publish_topic"`, `"delete_task"`.
/// * `subject_type` — The type of entity being acted upon, e.g. `"topic"`,
///   `"content"`, `"marketplace_task"` (`None` if not applicable).
/// * `subject_id`   — The ID of the entity being acted upon (`None` if not applicable).
/// * `metadata`     — Arbitrary JSON payload with additional context, such as
///   diffs, request details, or IP address (`None` if empty).
///
/// # Returns
///
/// The newly inserted `ActivityLog` row.
///
/// # Errors
///
/// Propagates `sqlx::Error` as `anyhow::Error` if the INSERT fails.
pub async fn record_activity(
    pool: &PgPool,
    actor_id: Option<i64>,
    action: &str,
    subject_type: Option<&str>,
    subject_id: Option<i64>,
    metadata: Option<serde_json::Value>,
) -> anyhow::Result<ActivityLog> {
    let id = Uuid::new_v4();

    let log = sqlx::query_as::<_, ActivityLog>(
        r#"
        INSERT INTO activity_logs
            (id, actor_id, action, subject_type, subject_id, metadata,
             created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
        RETURNING id, actor_id, action, subject_type, subject_id, metadata,
                  created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(actor_id)
    .bind(action)
    .bind(subject_type)
    .bind(subject_id)
    .bind(metadata)
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("failed to record activity log: {e}"))?;

    Ok(log)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_log_struct_debug() {
        let log = ActivityLog {
            id: Uuid::nil(),
            actor_id: Some(1),
            action: "test_action".to_string(),
            subject_type: Some("topic".to_string()),
            subject_id: Some(42),
            metadata: Some(serde_json::json!({"key": "value"})),
            created_at: chrono::NaiveDateTime::from_timestamp_opt(0, 0),
            updated_at: chrono::NaiveDateTime::from_timestamp_opt(0, 0),
        };

        let debug = format!("{log:?}");
        assert!(debug.contains("test_action"));
        assert!(debug.contains("topic"));
        assert!(debug.contains("42"));
    }

    #[test]
    fn test_activity_log_serialization() {
        let log = ActivityLog {
            id: Uuid::nil(),
            actor_id: None,
            action: "system_event".to_string(),
            subject_type: None,
            subject_id: None,
            metadata: None,
            created_at: chrono::NaiveDateTime::from_timestamp_opt(0, 0),
            updated_at: chrono::NaiveDateTime::from_timestamp_opt(0, 0),
        };

        let json = serde_json::to_value(&log).unwrap();
        assert_eq!(json["action"], "system_event");
        assert!(json["actor_id"].is_null());
        assert!(json["subject_type"].is_null());
        assert!(json["subject_id"].is_null());
        assert!(json["metadata"].is_null());
    }

    #[test]
    fn test_activity_log_deserialization_roundtrip() {
        let json = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "actor_id": 5,
            "action": "update_topic",
            "subject_type": "topic",
            "subject_id": 100,
            "metadata": {"changed_fields": ["title"]},
            "created_at": "2026-07-14T00:00:00",
            "updated_at": "2026-07-14T00:00:00"
        });

        let log: ActivityLog = serde_json::from_value(json).unwrap();
        assert_eq!(log.action, "update_topic");
        assert_eq!(log.actor_id, Some(5));
        assert_eq!(log.subject_id, Some(100));
    }

    #[test]
    fn test_activity_log_null_metadata() {
        let json = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "actor_id": null,
            "action": "system_cleanup",
            "subject_type": null,
            "subject_id": null,
            "metadata": null,
            "created_at": "2026-07-14T00:00:00",
            "updated_at": "2026-07-14T00:00:00"
        });

        let log: ActivityLog = serde_json::from_value(json).unwrap();
        assert!(log.actor_id.is_none());
        assert!(log.subject_type.is_none());
        assert!(log.subject_id.is_none());
        assert!(log.metadata.is_none());
    }

    #[test]
    fn test_activity_log_with_all_fields() {
        let metadata = serde_json::json!({
            "email": "test@example.com",
            "ip": "192.168.1.1",
            "user_agent": "test-agent",
            "attempted_at": "2026-07-14T12:00:00Z"
        });

        let log = ActivityLog {
            id: Uuid::new_v4(),
            actor_id: Some(10),
            action: "failed_login_attempt".to_string(),
            subject_type: Some("user".to_string()),
            subject_id: Some(10),
            metadata: Some(metadata.clone()),
            created_at: chrono::NaiveDateTime::from_timestamp_opt(1720953600, 0),
            updated_at: chrono::NaiveDateTime::from_timestamp_opt(1720953600, 0),
        };

        assert_eq!(log.action, "failed_login_attempt");
        assert_eq!(log.metadata, Some(metadata));
        assert_eq!(log.actor_id, Some(10));
    }
}

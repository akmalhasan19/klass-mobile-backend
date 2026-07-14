use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::db::pagination::PaginationQuery;
use crate::governance::activity_log::ActivityLog;

// ─── Filters ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ActivityLogFilters {
    /// Exact match on `action` column.
    pub action: Option<String>,

    /// Exact match on `actor_id` column.
    pub actor_id: Option<i64>,

    /// Exact match on `subject_type` column.
    pub subject_type: Option<String>,

    /// Lower bound for `created_at` (inclusive).
    pub date_from: Option<DateTime<Utc>>,

    /// Upper bound for `created_at` (inclusive).
    pub date_to: Option<DateTime<Utc>>,

    /// General search across `action`, `subject_id`, and actor
    /// `name`/`email` (via JOIN with `users`). Uses ILIKE.
    pub search: Option<String>,
}

// ─── Trait ───────────────────────────────────────────────────────────────────

#[async_trait]
pub trait ActivityLogsRepo: Send + Sync {
    /// Return paginated activity logs matching the given filters,
    /// together with the total count (before pagination).
    async fn find_many(
        &self,
        filters: &ActivityLogFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<ActivityLog>, i64)>;
}

// ─── Pg implementation ───────────────────────────────────────────────────────

pub struct PgActivityLogsRepo {
    pool: PgPool,
}

impl PgActivityLogsRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ActivityLogsRepo for PgActivityLogsRepo {
    async fn find_many(
        &self,
        filters: &ActivityLogFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<ActivityLog>, i64)> {
        let search_pattern = filters
            .search
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{}%", s));

        // ── Count query ──────────────────────────────────────────────────
        let count_sql = r#"
            SELECT COUNT(*)
            FROM activity_logs a
            LEFT JOIN users u ON a.actor_id = u.id
            WHERE ($1::text IS NULL OR a.action = $1)
              AND ($2::bigint IS NULL OR a.actor_id = $2)
              AND ($3::text IS NULL OR a.subject_type = $3)
              AND ($4::timestamptz IS NULL OR a.created_at >= $4)
              AND ($5::timestamptz IS NULL OR a.created_at <= $5)
              AND ($6::text IS NULL OR
                   a.action ILIKE $6 OR
                   a.subject_id::text ILIKE $6 OR
                   u.name ILIKE $6 OR
                   u.email ILIKE $6)
        "#;

        let total: i64 = sqlx::query_scalar(count_sql)
            .bind(filters.action.as_deref())
            .bind(filters.actor_id)
            .bind(filters.subject_type.as_deref())
            .bind(filters.date_from)
            .bind(filters.date_to)
            .bind(search_pattern.as_deref())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to count activity logs: {e}"))?;

        // ── Data query ───────────────────────────────────────────────────
        let data_sql = r#"
            SELECT a.id, a.actor_id, a.action, a.subject_type, a.subject_id,
                   a.metadata, a.created_at, a.updated_at
            FROM activity_logs a
            LEFT JOIN users u ON a.actor_id = u.id
            WHERE ($1::text IS NULL OR a.action = $1)
              AND ($2::bigint IS NULL OR a.actor_id = $2)
              AND ($3::text IS NULL OR a.subject_type = $3)
              AND ($4::timestamptz IS NULL OR a.created_at >= $4)
              AND ($5::timestamptz IS NULL OR a.created_at <= $5)
              AND ($6::text IS NULL OR
                   a.action ILIKE $6 OR
                   a.subject_id::text ILIKE $6 OR
                   u.name ILIKE $6 OR
                   u.email ILIKE $6)
            ORDER BY a.created_at DESC
            LIMIT $7 OFFSET $8
        "#;

        let logs = sqlx::query_as::<_, ActivityLog>(data_sql)
            .bind(filters.action.as_deref())
            .bind(filters.actor_id)
            .bind(filters.subject_type.as_deref())
            .bind(filters.date_from)
            .bind(filters.date_to)
            .bind(search_pattern.as_deref())
            .bind(pagination.limit())
            .bind(pagination.offset())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch activity logs: {e}"))?;

        Ok((logs, total))
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_log_filters_default() {
        let filters = ActivityLogFilters::default();
        assert!(filters.action.is_none());
        assert!(filters.actor_id.is_none());
        assert!(filters.subject_type.is_none());
        assert!(filters.date_from.is_none());
        assert!(filters.date_to.is_none());
        assert!(filters.search.is_none());
    }

    #[test]
    fn test_activity_log_filters_custom() {
        let now = Utc::now();
        let filters = ActivityLogFilters {
            action: Some("create_topic".to_string()),
            actor_id: Some(42),
            subject_type: Some("topic".to_string()),
            date_from: Some(now),
            date_to: Some(now),
            search: Some("math".to_string()),
        };

        assert_eq!(filters.action.as_deref(), Some("create_topic"));
        assert_eq!(filters.actor_id, Some(42));
        assert_eq!(filters.subject_type.as_deref(), Some("topic"));
        assert_eq!(filters.search.as_deref(), Some("math"));
    }

    #[test]
    fn test_filter_with_search_empty_string() {
        let filters = ActivityLogFilters {
            search: Some(String::new()),
            ..Default::default()
        };

        // Empty search should be treated as None (no filter)
        let search_pattern = filters
            .search
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{}%", s));
        assert!(search_pattern.is_none());
    }

    #[test]
    fn test_filter_with_search_non_empty() {
        let filters = ActivityLogFilters {
            search: Some("test".to_string()),
            ..Default::default()
        };

        let search_pattern = filters
            .search
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{}%", s));
        assert_eq!(search_pattern.as_deref(), Some("%test%"));
    }

    #[test]
    fn test_date_range_filters() {
        let from = DateTime::from_timestamp(1720000000, 0).unwrap();
        let to = DateTime::from_timestamp(1720100000, 0).unwrap();

        let filters = ActivityLogFilters {
            date_from: Some(from),
            date_to: Some(to),
            ..Default::default()
        };

        assert!(filters.date_from.unwrap() < filters.date_to.unwrap());
    }
}

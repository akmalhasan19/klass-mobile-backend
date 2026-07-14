use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::pagination::{PaginationMeta, PaginationParams, PaginationQuery};
use crate::db::repositories::activity_logs::{
    ActivityLogFilters, ActivityLogsRepo, PgActivityLogsRepo,
};
use crate::auth::middleware::Principal;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

use super::require_admin;
use super::super::response;

// ─── Resource ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ActivityLogResource {
    id: Uuid,
    actor_id: Option<i64>,
    action: String,
    subject_type: Option<String>,
    subject_id: Option<i64>,
    metadata: Option<serde_json::Value>,
    created_at: String,
    updated_at: String,
}

// ─── Query params ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ActivityLogQueryParams {
    /// Exact match on `action` column.
    pub action: Option<String>,

    /// Exact match on `actor_id` column.
    pub actor_id: Option<i64>,

    /// Exact match on `subject_type` column.
    pub subject_type: Option<String>,

    /// Lower bound for `created_at` (RFC 3339 / ISO 8601).
    pub date_from: Option<String>,

    /// Upper bound for `created_at` (RFC 3339 / ISO 8601).
    pub date_to: Option<String>,

    /// General search across `action`, `subject_id`, and actor name/email.
    pub search: Option<String>,

    /// Pagination
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// GET /admin/activity-logs
///
/// Returns paginated activity logs with optional filters.
/// All mutations (create, update, delete, publish, reorder, upload, etc.)
/// are recorded via `record_activity()` in their respective handlers, and
/// this endpoint allows admins to browse/search through them.
pub async fn index(
    State(state): State<AppState>,
    principal: Principal,
    Query(params): Query<ActivityLogQueryParams>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    let pq = PaginationQuery::parse(Query(PaginationParams {
        page: params.page,
        per_page: params.per_page,
    }));

    // Parse date filters from RFC 3339 strings
    let date_from = if let Some(ref s) = params.date_from {
        Some(
            DateTime::parse_from_rfc3339(s)
                .map_err(|_| {
                    AppError::Validation(
                        "Format 'date_from' tidak valid. Gunakan format RFC 3339 (ISO 8601)."
                            .into(),
                    )
                })?
                .with_timezone(&Utc),
        )
    } else {
        None
    };

    let date_to = if let Some(ref s) = params.date_to {
        Some(
            DateTime::parse_from_rfc3339(s)
                .map_err(|_| {
                    AppError::Validation(
                        "Format 'date_to' tidak valid. Gunakan format RFC 3339 (ISO 8601)."
                            .into(),
                    )
                })?
                .with_timezone(&Utc),
        )
    } else {
        None
    };

    let filters = ActivityLogFilters {
        action: params.action,
        actor_id: params.actor_id,
        subject_type: params.subject_type,
        date_from,
        date_to,
        search: params.search,
    };

    let repo = PgActivityLogsRepo::new(state.db_pool.clone());
    let (logs, total) = repo
        .find_many(&filters, &pq)
        .await
        .map_err(|e| AppError::Internal(format!("gagal mengambil activity logs: {e}")))?;

    let resources: Vec<ActivityLogResource> = logs.into_iter().map(build_resource).collect();

    let meta = PaginationMeta::from_query(&pq, total);
    Ok(response::paginated(resources, meta))
}

// ─── Resource builders ───────────────────────────────────────────────────────

fn build_resource(log: crate::governance::activity_log::ActivityLog) -> ActivityLogResource {
    ActivityLogResource {
        id: log.id,
        actor_id: log.actor_id,
        action: log.action,
        subject_type: log.subject_type,
        subject_id: log.subject_id,
        metadata: log.metadata,
        created_at: format_datetime(log.created_at),
        updated_at: format_datetime(log.updated_at),
    }
}

// ─── Formatting helpers ──────────────────────────────────────────────────────

fn format_datetime(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

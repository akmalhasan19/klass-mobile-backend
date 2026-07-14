use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::pagination::{PaginationMeta, PaginationParams, PaginationQuery};
use crate::db::repositories::student_progress::{
    PgStudentProgressRepo, StudentProgress, StudentProgressFilters, StudentProgressRepo,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

use super::response;

// ─── Resources ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct StudentProgressResource {
    id: Uuid,
    student_name: String,
    score: Option<i32>,
    completion_date: Option<String>,
    created_at: String,
    updated_at: String,
}

// ─── Query params ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct StudentProgressQueryParams {
    search: Option<String>,
    page: Option<i64>,
    per_page: Option<i64>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// GET /student-progress
pub async fn index(
    State(state): State<AppState>,
    Query(params): Query<StudentProgressQueryParams>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let pq = PaginationQuery::parse(Query(PaginationParams {
        page: params.page,
        per_page: params.per_page,
    }));

    let filters = StudentProgressFilters {
        search: params.search,
    };

    let repo = PgStudentProgressRepo::new(state.db_pool.clone());
    let (records, total) = repo
        .find_many(&filters, &pq)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let resources: Vec<StudentProgressResource> = records
        .into_iter()
        .map(build_student_progress_resource)
        .collect();

    let meta = PaginationMeta::from_query(&pq, total);
    Ok(response::paginated(resources, meta))
}

/// GET /student-progress/{id}
pub async fn show(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let repo = PgStudentProgressRepo::new(state.db_pool.clone());

    let record = repo
        .find_by_id(id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("student progress not found".into()))?;

    let resource = build_student_progress_resource(record);

    Ok(response::ok_with_message(
        "Detail progress siswa berhasil diambil.",
        resource,
    ))
}

// ─── Resource builders ───────────────────────────────────────────────────────

fn build_student_progress_resource(record: StudentProgress) -> StudentProgressResource {
    StudentProgressResource {
        id: record.id,
        student_name: record.student_name,
        score: record.score,
        completion_date: record.completion_date.map(format_datetime),
        created_at: format_datetime(record.created_at),
        updated_at: format_datetime(record.updated_at),
    }
}

// ─── Formatting helpers ──────────────────────────────────────────────────────

fn format_datetime(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

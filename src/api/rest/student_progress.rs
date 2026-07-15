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

#[derive(Serialize, utoipa::ToSchema)]
pub struct StudentProgressResource {
    id: Uuid,
    student_name: String,
    score: Option<i32>,
    completion_date: Option<String>,
    created_at: String,
    updated_at: String,
}

// ─── Query params ────────────────────────────────────────────────────────────

#[derive(Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct StudentProgressQueryParams {
    search: Option<String>,
    page: Option<i64>,
    per_page: Option<i64>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// GET /student-progress
#[utoipa::path(
    get,
    path = "/api/v1/student-progress",
    tag = "student-progress",
    params(StudentProgressQueryParams),
    responses(
        (status = 200, body = Vec<StudentProgressResource>),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/api/v1/student-progress/{id}",
    tag = "student-progress",
    params(
        ("id" = Uuid, Path, description = "Student progress ID"),
    ),
    responses(
        (status = 200, body = StudentProgressResource),
        (status = 404, description = "Student progress not found"),
    ),
)]
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
        completion_date: record.completion_date.map(|d| d.and_utc().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
        created_at: record.created_at.map(|d| d.and_utc().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)).unwrap_or_default(),
        updated_at: record.updated_at.map(|d| d.and_utc().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)).unwrap_or_default(),
    }
}



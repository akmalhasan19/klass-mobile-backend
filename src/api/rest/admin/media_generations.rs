use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::Principal;
use crate::db::pagination::{PaginationMeta, PaginationParams, PaginationQuery};
use crate::db::repositories::media_generations::{
    AdminMediaGenerationFilters, MediaGenerationAdminRow, PgMediaGenerationsRepo,
};
use crate::error::{AppError, AppResult};
use crate::orchestrator::lifecycle::MediaGenerationStatus;
use crate::state::AppState;

use super::require_admin;
use super::super::response;

#[derive(Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct AdminMediaGenerationQueryParams {
    pub status: Option<String>,
    pub search: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct AdminTeacherSummary {
    pub id: i64,
    pub name: String,
    pub email: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct AdminMediaGenerationResource {
    pub id: Uuid,
    pub generated_from_id: Option<Uuid>,
    pub is_regeneration: bool,
    pub raw_prompt: String,
    pub preferred_output_type: String,
    pub resolved_output_type: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub teacher: AdminTeacherSummary,
}

#[utoipa::path(get, path = "/api/v1/admin/media-generations", tag = "admin-media-generations", params(AdminMediaGenerationQueryParams), responses((status = 200, description = "Success", body = [AdminMediaGenerationResource])), security(("bearer_auth" = [])))]
pub async fn index(
    State(state): State<AppState>,
    principal: Principal,
    Query(params): Query<AdminMediaGenerationQueryParams>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    if let Some(ref status) = params.status {
        MediaGenerationStatus::from_str(status).ok_or_else(|| {
            AppError::Validation(format!(
                "Status '{}' tidak valid. Gunakan salah satu: {}",
                status,
                MediaGenerationStatus::ALL
                    .iter()
                    .map(MediaGenerationStatus::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;
    }

    let pq = PaginationQuery::parse(Query(PaginationParams {
        page: params.page,
        per_page: params.per_page,
    }));

    let repo = PgMediaGenerationsRepo::new(state.db_pool.clone());
    let (rows, total) = repo
        .find_all_admin(
            &AdminMediaGenerationFilters {
                status: params.status,
                search: params.search,
            },
            &pq,
        )
        .await
        .map_err(|e| AppError::Internal(format!("gagal mengambil media generations: {e}")))?;

    let resources: Vec<AdminMediaGenerationResource> =
        rows.into_iter().map(build_resource).collect();

    let meta = PaginationMeta::from_query(&pq, total);
    Ok(response::paginated(resources, meta))
}

fn build_resource(row: MediaGenerationAdminRow) -> AdminMediaGenerationResource {
    AdminMediaGenerationResource {
        id: row.id,
        generated_from_id: row.generated_from_id,
        is_regeneration: row.is_regeneration,
        raw_prompt: row.raw_prompt,
        preferred_output_type: row.preferred_output_type,
        resolved_output_type: row.resolved_output_type,
        status: row.status,
        error_code: row.error_code,
        error_message: row.error_message,
        created_at: row.created_at.map(format_naive_datetime),
        updated_at: row.updated_at.map(format_naive_datetime),
        teacher: AdminTeacherSummary {
            id: row.teacher_id,
            name: row.teacher_name,
            email: row.teacher_email,
        },
    }
}

fn format_naive_datetime(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}

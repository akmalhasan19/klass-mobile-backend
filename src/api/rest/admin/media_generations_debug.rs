use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

use crate::auth::middleware::Principal;
use crate::db::repositories::media_generations::PgMediaGenerationsRepo;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

use super::require_admin;
use super::super::response;

#[derive(Serialize, utoipa::ToSchema)]
pub struct MediaGenerationTaxonomyDebugResource {
    pub id: Uuid,
    pub status: String,
    pub raw_prompt: String,
    pub preferred_output_type: String,
    pub resolved_output_type: Option<String>,
    pub interpretation: Option<serde_json::Value>,
    pub taxonomy_inference: Option<serde_json::Value>,
    pub decision: Option<serde_json::Value>,
    pub interpretation_audit: Option<serde_json::Value>,
}

#[utoipa::path(get, path = "/api/v1/admin/media-generations/{id}/debug-taxonomy", tag = "admin-media-generations", params(("id" = Uuid, Path)), responses((status = 200, description = "Success", body = MediaGenerationTaxonomyDebugResource)), security(("bearer_auth" = [])))]
pub async fn debug_taxonomy(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    let repo = PgMediaGenerationsRepo::new(state.db_pool.clone());
    let generation = repo
        .find_raw(id)
        .await
        .map_err(|e| AppError::NotFound(format!("media generation not found: {e}")))?;

    let taxonomy_inference = generation
        .interpretation_audit_payload
        .as_ref()
        .and_then(|audit| audit.get("taxonomy_inference").cloned());

    let resource = MediaGenerationTaxonomyDebugResource {
        id: generation.id,
        status: generation.status,
        raw_prompt: generation.raw_prompt,
        preferred_output_type: generation.preferred_output_type,
        resolved_output_type: generation.resolved_output_type,
        interpretation: generation.interpretation_payload,
        taxonomy_inference,
        decision: generation.decision_payload,
        interpretation_audit: generation.interpretation_audit_payload,
    };

    Ok(response::ok(resource))
}

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

use crate::db::repositories::homepage_sections::{
    HomepageSection, HomepageSectionsRepo, PgHomepageSectionsRepo,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

// ─── Resources ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct HomepageSectionResource {
    id: Uuid,
    key: String,
    label: String,
    position: i32,
    is_enabled: bool,
    data_source: Option<String>,
    created_at: String,
    updated_at: String,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// GET /homepage-sections
pub async fn index(
    State(state): State<AppState>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let repo = PgHomepageSectionsRepo::new(state.db_pool.clone());

    let sections = repo
        .find_enabled_ordered()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let resources: Vec<HomepageSectionResource> = sections
        .into_iter()
        .map(build_homepage_section_resource)
        .collect();

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "data": resources
        })),
    ))
}

// ─── Resource builders ───────────────────────────────────────────────────────

fn build_homepage_section_resource(section: HomepageSection) -> HomepageSectionResource {
    HomepageSectionResource {
        id: section.id,
        key: section.key,
        label: section.label,
        position: section.position,
        is_enabled: section.is_enabled,
        data_source: section.data_source,
        created_at: section.created_at.to_string(),
        updated_at: section.updated_at.to_string(),
    }
}

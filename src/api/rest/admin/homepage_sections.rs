use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::middleware::Principal;
use crate::db::repositories::homepage_sections::{
    HomepageSectionUpdate, HomepageSectionsRepo, PgHomepageSectionsRepo,
};
use crate::error::{AppError, AppResult};
use crate::governance::activity_log::record_activity;
use crate::state::AppState;

use super::require_admin;
use super::super::response;

// ─── Request body ────────────────────────────────────────────────────────────

#[derive(Deserialize, utoipa::ToSchema)]
pub struct BulkUpdateSection {
    pub id: Uuid,
    pub position: i32,
    pub is_enabled: bool,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct BulkUpdateHomepageSectionsRequest {
    pub sections: Vec<BulkUpdateSection>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// PATCH /admin/homepage-sections
///
/// Bulk update homepage sections' `position` and `is_enabled` fields
/// inside a single DB transaction.
///
/// Body: `{ "sections": [{ "id": "...", "position": 1, "is_enabled": true }, ...] }`
///
/// Returns the number of updated rows and the current state of all sections
/// (so the admin can verify the result immediately).
#[utoipa::path(patch, path = "/api/v1/admin/homepage-sections", tag = "admin-homepage-sections", request_body = BulkUpdateHomepageSectionsRequest, responses((status = 200, description = "Success")), security(("bearer_auth" = [])))]
pub async fn bulk_update(
    State(state): State<AppState>,
    principal: Principal,
    Json(payload): Json<BulkUpdateHomepageSectionsRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    if payload.sections.is_empty() {
        return Err(AppError::Validation(
            "Daftar section tidak boleh kosong.".into(),
        ));
    }

    // Validate position >= 0
    for section in &payload.sections {
        if section.position < 0 {
            return Err(AppError::Validation(format!(
                "Posisi '{}' untuk section '{}' tidak valid. Posisi minimal 0.",
                section.position, section.id,
            )));
        }
    }

    let updates: Vec<HomepageSectionUpdate> = payload
        .sections
        .into_iter()
        .map(|s| HomepageSectionUpdate {
            id: s.id,
            position: s.position,
            is_enabled: s.is_enabled,
        })
        .collect();

    let repo = PgHomepageSectionsRepo::new(state.db_pool.clone());
    let affected = repo
        .bulk_update(&updates)
        .await
        .map_err(|e| AppError::Internal(format!("gagal memperbarui homepage sections: {e}")))?;

    if affected == 0 {
        return Err(AppError::NotFound(
            "Tidak ada homepage section yang ditemukan untuk diperbarui.".into(),
        ));
    }

    // Record activity
    let updated_ids: Vec<String> = updates.iter().map(|u| u.id.to_string()).collect();
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "update_homepage_sections",
        Some("homepage_section"),
        None,
        Some(serde_json::json!({
            "updated_count": affected,
            "updated_section_ids": updated_ids,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("gagal mencatat aktivitas: {e}")))?;

    Ok(response::ok_with_message(
        &format!("{} section berhasil diperbarui.", affected),
        serde_json::json!({
            "updated_count": affected,
            "updated_section_ids": updated_ids,
        }),
    ))
}

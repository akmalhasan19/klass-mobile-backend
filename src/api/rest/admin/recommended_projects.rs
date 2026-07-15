use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::middleware::Principal;
use crate::db::repositories::recommended_projects::{
    CreateRecommendedProject, PgRecommendedProjectsRepo, RecommendedProject,
    RecommendedProjectsRepo, SOURCE_ADMIN_UPLOAD, UpdateRecommendedProject,
};
use crate::error::{AppError, AppResult};
use crate::governance::activity_log::record_activity;
use crate::media_gen::publication::{mime_label, thumbnail_accent};
use crate::state::AppState;
use crate::storage::r2;

use super::require_admin;
use super::super::response;

const THUMBNAIL_WIDTH: u32 = 1280;
const THUMBNAIL_HEIGHT: u32 = 720;

#[derive(Serialize, utoipa::ToSchema)]
pub struct RecommendedProjectResource {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub project_file_url: Option<String>,
    pub ratio: String,
    pub project_type: Option<String>,
    pub tags: Option<serde_json::Value>,
    pub modules: Option<serde_json::Value>,
    pub source_type: String,
    pub source_reference: Option<String>,
    pub display_priority: i32,
    pub is_active: bool,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateRecommendedProjectRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub project_file_url: Option<String>,
    pub ratio: Option<String>,
    pub project_type: Option<String>,
    pub tags: Option<serde_json::Value>,
    pub modules: Option<serde_json::Value>,
    pub source_reference: Option<String>,
    pub source_payload: Option<serde_json::Value>,
    pub display_priority: Option<i32>,
    pub is_active: Option<bool>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
}

#[utoipa::path(post, path = "/api/v1/admin/homepage-sections/recommended-projects", tag = "admin-recommended-projects", responses((status = 201, description = "Created", body = RecommendedProjectResource)), security(("bearer_auth" = [])))]
pub async fn create(
    State(state): State<AppState>,
    principal: Principal,
    mut multipart: Multipart,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    let mut title: Option<String> = None;
    let mut description: Option<String> = None;
    let mut ratio: Option<String> = None;
    let mut project_type: Option<String> = None;
    let mut tags: Option<serde_json::Value> = None;
    let mut modules: Option<serde_json::Value> = None;
    let mut source_reference: Option<String> = None;
    let mut source_payload: Option<serde_json::Value> = None;
    let mut display_priority: Option<i32> = None;
    let mut is_active: Option<bool> = None;
    let mut starts_at: Option<String> = None;
    let mut ends_at: Option<String> = None;
    let mut thumbnail_bytes: Option<(Vec<u8>, String)> = None;
    let mut project_file_bytes: Option<(Vec<u8>, String)> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation(format!("gagal membaca field multipart: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "title" => {
                title = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::Validation(format!("gagal membaca title: {e}")))?,
                );
            }
            "description" => {
                description = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::Validation(format!("gagal membaca description: {e}")))?,
                );
            }
            "ratio" => {
                ratio = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::Validation(format!("gagal membaca ratio: {e}")))?,
                );
            }
            "project_type" => {
                project_type = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::Validation(format!("gagal membaca project_type: {e}")))?,
                );
            }
            "tags" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::Validation(format!("gagal membaca tags: {e}")))?;
                tags = Some(
                    serde_json::from_str(&text)
                        .map_err(|e| AppError::Validation(format!("tags harus berupa JSON array: {e}")))?,
                );
            }
            "modules" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::Validation(format!("gagal membaca modules: {e}")))?;
                modules = Some(
                    serde_json::from_str(&text)
                        .map_err(|e| AppError::Validation(format!("modules harus berupa JSON array: {e}")))?,
                );
            }
            "source_reference" => {
                source_reference = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::Validation(format!("gagal membaca source_reference: {e}")))?,
                );
            }
            "source_payload" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::Validation(format!("gagal membaca source_payload: {e}")))?;
                source_payload = Some(
                    serde_json::from_str(&text).map_err(|e| {
                        AppError::Validation(format!("source_payload harus berupa JSON object: {e}"))
                    })?,
                );
            }
            "display_priority" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::Validation(format!("gagal membaca display_priority: {e}")))?;
                display_priority = Some(text.parse().map_err(|_| {
                    AppError::Validation("display_priority harus berupa angka.".into())
                })?);
            }
            "is_active" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::Validation(format!("gagal membaca is_active: {e}")))?;
                is_active = Some(text == "true" || text == "1");
            }
            "starts_at" => {
                starts_at = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::Validation(format!("gagal membaca starts_at: {e}")))?,
                );
            }
            "ends_at" => {
                ends_at = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::Validation(format!("gagal membaca ends_at: {e}")))?,
                );
            }
            "thumbnail" => {
                let mime = field.content_type().unwrap_or("image/png").to_string();
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::Validation(format!("gagal membaca thumbnail: {e}")))?;
                thumbnail_bytes = Some((data.to_vec(), mime));
            }
            "project_file" => {
                let mime = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::Validation(format!("gagal membaca project_file: {e}")))?;
                project_file_bytes = Some((data.to_vec(), mime));
            }
            _ => {}
        }
    }

    let title = title.ok_or_else(|| AppError::Validation("Field 'title' wajib dikirim.".into()))?;

    let project_file_url = if let Some((bytes, mime)) = &project_file_bytes {
        let result = r2::upload(
            &state.s3_client,
            &state.config.r2_bucket_name,
            &state.config.r2_public_url,
            "materials",
            bytes.clone(),
            mime,
        )
        .await
        .map_err(|e| AppError::Internal(format!("Gagal meng-upload project file: {e}")))?;
        Some(result.public_url)
    } else {
        None
    };

    let thumbnail_url = if let Some((bytes, mime)) = thumbnail_bytes {
        let result = r2::upload(
            &state.s3_client,
            &state.config.r2_bucket_name,
            &state.config.r2_public_url,
            "gallery",
            bytes,
            &mime,
        )
        .await
        .map_err(|e| AppError::Internal(format!("Gagal meng-upload thumbnail: {e}")))?;
        Some(result.public_url)
    } else if project_file_bytes.is_some() {
        let mime = &project_file_bytes.as_ref().unwrap().1;
        let (thumb_bytes, thumb_mime) = generate_svg_thumbnail(mime);
        let result = r2::upload(
            &state.s3_client,
            &state.config.r2_bucket_name,
            &state.config.r2_public_url,
            "gallery",
            thumb_bytes,
            &thumb_mime,
        )
        .await
        .map_err(|e| AppError::Internal(format!("Gagal meng-upload thumbnail: {e}")))?;
        Some(result.public_url)
    } else {
        None
    };

    let parse_dt =
        |s: Option<String>| -> Result<Option<chrono::DateTime<chrono::Utc>>, AppError> {
            match s {
                Some(val) => {
                    let dt = chrono::DateTime::parse_from_rfc3339(&val).map_err(|_| {
                        AppError::Validation(format!(
                            "Format tanggal tidak valid: {val}. Gunakan format RFC 3339."
                        ))
                    })?;
                    Ok(Some(dt.with_timezone(&chrono::Utc)))
                }
                None => Ok(None),
            }
        };

    let payload = CreateRecommendedProject {
        title,
        description,
        thumbnail_url,
        project_file_url,
        ratio,
        project_type,
        tags,
        modules,
        source_type: SOURCE_ADMIN_UPLOAD.to_string(),
        source_reference,
        source_payload,
        display_priority,
        is_active,
        starts_at: parse_dt(starts_at)?,
        ends_at: parse_dt(ends_at)?,
        created_by: Some(principal.user_id),
    };

    let repo = PgRecommendedProjectsRepo::new(state.db_pool.clone());
    let project = repo
        .create(&payload)
        .await
        .map_err(|e| AppError::Internal(format!("gagal membuat recommended project: {e}")))?;

    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "create_recommended_project",
        Some("recommended_project"),
        Some(project.id),
        Some(serde_json::json!({
            "title": project.title,
            "thumbnail_url": project.thumbnail_url,
            "project_file_url": project.project_file_url,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("gagal mencatat aktivitas: {e}")))?;

    Ok(response::created(build_resource(&project)))
}

#[utoipa::path(put, path = "/api/v1/admin/homepage-sections/recommended-projects/{id}", tag = "admin-recommended-projects", params(("id" = i64, Path)), request_body = UpdateRecommendedProjectRequest, responses((status = 200, description = "Success", body = RecommendedProjectResource)), security(("bearer_auth" = [])))]
pub async fn update(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateRecommendedProjectRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    let repo = PgRecommendedProjectsRepo::new(state.db_pool.clone());

    let existing = repo
        .find_by_id(id)
        .await
        .map_err(|e| AppError::Internal(format!("gagal mencari recommended project: {e}")))?
        .ok_or_else(|| AppError::NotFound("Recommended project tidak ditemukan.".into()))?;

    let parse_dt =
        |s: Option<String>| -> Result<Option<chrono::DateTime<chrono::Utc>>, AppError> {
            match s {
                Some(val) if val.is_empty() => Ok(None),
                Some(val) => {
                    let dt = chrono::DateTime::parse_from_rfc3339(&val).map_err(|_| {
                        AppError::Validation(format!(
                            "Format tanggal tidak valid: {val}. Gunakan format RFC 3339."
                        ))
                    })?;
                    Ok(Some(dt.with_timezone(&chrono::Utc)))
                }
                None => Ok(existing.starts_at),
            }
        };

    let update = UpdateRecommendedProject {
        title: payload.title.or(Some(existing.title)),
        description: payload.description.or(existing.description),
        thumbnail_url: payload.thumbnail_url.or(existing.thumbnail_url),
        project_file_url: payload.project_file_url.or(existing.project_file_url),
        ratio: payload.ratio.or(Some(existing.ratio)),
        project_type: payload.project_type.or(existing.project_type),
        tags: payload.tags.or(existing.tags),
        modules: payload.modules.or(existing.modules),
        source_type: None,
        source_reference: payload.source_reference.or(existing.source_reference),
        source_payload: payload.source_payload.or(existing.source_payload),
        display_priority: payload.display_priority.or(Some(existing.display_priority)),
        is_active: payload.is_active.or(Some(existing.is_active)),
        starts_at: parse_dt(payload.starts_at)?,
        ends_at: parse_dt(payload.ends_at)?,
        updated_by: Some(principal.user_id),
    };

    let project = repo
        .update(id, &update)
        .await
        .map_err(|e| AppError::Internal(format!("gagal memperbarui recommended project: {e}")))?
        .ok_or_else(|| AppError::NotFound("Recommended project tidak ditemukan.".into()))?;

    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "update_recommended_project",
        Some("recommended_project"),
        Some(id),
        Some(serde_json::json!({
            "title": project.title,
            "thumbnail_url": project.thumbnail_url,
            "project_file_url": project.project_file_url,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("gagal mencatat aktivitas: {e}")))?;

    Ok(response::ok(build_resource(&project)))
}

#[utoipa::path(delete, path = "/api/v1/admin/homepage-sections/recommended-projects/{id}", tag = "admin-recommended-projects", params(("id" = i64, Path)), responses((status = 200, description = "Deleted", body = ())), security(("bearer_auth" = [])))]
pub async fn delete(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<i64>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    let repo = PgRecommendedProjectsRepo::new(state.db_pool.clone());

    let existing = repo
        .find_by_id(id)
        .await
        .map_err(|e| AppError::Internal(format!("gagal mencari recommended project: {e}")))?
        .ok_or_else(|| AppError::NotFound("Recommended project tidak ditemukan.".into()))?;

    let deleted = repo
        .delete(id)
        .await
        .map_err(|e| AppError::Internal(format!("gagal menghapus recommended project: {e}")))?;

    if !deleted {
        return Err(AppError::NotFound("Recommended project tidak ditemukan.".into()));
    }

    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "delete_recommended_project",
        Some("recommended_project"),
        Some(id),
        Some(serde_json::json!({
            "title": existing.title,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("gagal mencatat aktivitas: {e}")))?;

    Ok(response::ok_with_message("Project berhasil dihapus.", serde_json::json!({})))
}

#[utoipa::path(patch, path = "/api/v1/admin/homepage-sections/recommended-projects/{id}/toggle-active", tag = "admin-recommended-projects", params(("id" = i64, Path)), responses((status = 200, description = "Success", body = RecommendedProjectResource)), security(("bearer_auth" = [])))]
pub async fn toggle_active(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<i64>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    let repo = PgRecommendedProjectsRepo::new(state.db_pool.clone());
    let new_active = repo
        .toggle_active(id)
        .await
        .map_err(|e| AppError::Internal(format!("gagal toggle active recommended project: {e}")))?
        .ok_or_else(|| AppError::NotFound("Recommended project tidak ditemukan.".into()))?;

    let project = repo
        .find_by_id(id)
        .await
        .map_err(|e| AppError::Internal(format!("gagal memuat recommended project: {e}")))?
        .ok_or_else(|| AppError::NotFound("Recommended project tidak ditemukan.".into()))?;

    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "update_recommended_project",
        Some("recommended_project"),
        Some(id),
        Some(serde_json::json!({
            "action": "toggle_active",
            "is_active": new_active,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("gagal mencatat aktivitas: {e}")))?;

    Ok(response::ok(build_resource(&project)))
}

#[utoipa::path(patch, path = "/api/v1/admin/homepage-sections/recommended-projects/{id}/show-now", tag = "admin-recommended-projects", params(("id" = i64, Path)), responses((status = 200, description = "Success", body = RecommendedProjectResource)), security(("bearer_auth" = [])))]
pub async fn show_now(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<i64>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    let repo = PgRecommendedProjectsRepo::new(state.db_pool.clone());
    let project = repo
        .show_now(id, Some(principal.user_id))
        .await
        .map_err(|e| AppError::Internal(format!("gagal show_now recommended project: {e}")))?
        .ok_or_else(|| AppError::NotFound("Recommended project tidak ditemukan.".into()))?;

    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "update_recommended_project",
        Some("recommended_project"),
        Some(id),
        Some(serde_json::json!({
            "action": "show_now",
            "is_active": true,
            "starts_at": null,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("gagal mencatat aktivitas: {e}")))?;

    Ok(response::ok(build_resource(&project)))
}

fn build_resource(project: &RecommendedProject) -> RecommendedProjectResource {
    RecommendedProjectResource {
        id: project.id,
        title: project.title.clone(),
        description: project.description.clone(),
        thumbnail_url: project.thumbnail_url.clone(),
        project_file_url: project.project_file_url.clone(),
        ratio: project.ratio.clone(),
        project_type: project.project_type.clone(),
        tags: project.tags.clone(),
        modules: project.modules.clone(),
        source_type: project.source_type.clone(),
        source_reference: project.source_reference.clone(),
        display_priority: project.display_priority,
        is_active: project.is_active,
        starts_at: project.starts_at.map(|dt| dt.to_rfc3339()),
        ends_at: project.ends_at.map(|dt| dt.to_rfc3339()),
        created_at: project.created_at.to_rfc3339(),
        updated_at: project.updated_at.to_rfc3339(),
    }
}

fn generate_svg_thumbnail(mime_type: &str) -> (Vec<u8>, String) {
    let accent = thumbnail_accent(mime_type);
    let label = mime_label(mime_type);
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">
  <rect width="{w}" height="{h}" fill="{accent}22" rx="24"/>
  <rect x="60" y="60" width="{iw}" height="{ih}" rx="16" fill="white" stroke="{accent}" stroke-width="3"/>
  <text x="{cx}" y="320" font-family="Arial,sans-serif" font-size="64" fill="{accent}" text-anchor="middle" font-weight="bold">KLASS</text>
  <text x="{cx}" y="400" font-family="Arial,sans-serif" font-size="36" fill="#555" text-anchor="middle">Generated Media</text>
  <text x="{cx}" y="460" font-family="Arial,sans-serif" font-size="28" fill="#888" text-anchor="middle">{label}</text>
</svg>"##,
        w = THUMBNAIL_WIDTH,
        h = THUMBNAIL_HEIGHT,
        iw = THUMBNAIL_WIDTH - 120,
        ih = THUMBNAIL_HEIGHT - 120,
        cx = THUMBNAIL_WIDTH / 2,
        accent = accent,
        label = label,
    );
    (svg.into_bytes(), "image/svg+xml".to_string())
}

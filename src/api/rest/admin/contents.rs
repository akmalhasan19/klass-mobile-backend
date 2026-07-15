use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::middleware::Principal;
use crate::db::repositories::contents::{
    ContentsRepo, CreateContentPayload, PgContentsRepo, UpdateContentPayload,
};
use crate::error::{AppError, AppResult};
use crate::governance::activity_log::record_activity;
use crate::state::AppState;

use super::require_admin;
use super::super::response;

// ─── Request bodies ──────────────────────────────────────────────────────────

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateContentRequest {
    pub topic_id: Uuid,
    #[serde(rename = "type")]
    pub content_type: String,
    pub title: Option<String>,
    pub data: Option<serde_json::Value>,
    pub media_url: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateContentRequest {
    pub topic_id: Option<Uuid>,
    #[serde(rename = "type")]
    pub content_type: Option<String>,
    pub title: Option<Option<String>>,
    pub data: Option<Option<serde_json::Value>>,
    pub media_url: Option<Option<String>>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ReorderRequest {
    pub direction: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct PublishRequest {
    pub is_published: bool,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// POST /admin/contents
#[utoipa::path(post, path = "/api/v1/admin/contents", tag = "admin-contents", request_body = CreateContentRequest, responses((status = 201, description = "Created")), security(("bearer_auth" = [])))]
pub async fn create(
    State(state): State<AppState>,
    principal: Principal,
    Json(payload): Json<CreateContentRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    // Validate content_type
    let valid_types = ["module", "quiz", "brief"];
    if !valid_types.contains(&payload.content_type.as_str()) {
        return Err(AppError::Validation(format!(
            "invalid type '{}': must be one of module, quiz, brief",
            payload.content_type
        )));
    }

    let repo = PgContentsRepo::new(state.db_pool.clone());

    let create_payload = CreateContentPayload {
        topic_id: payload.topic_id,
        content_type: payload.content_type,
        title: payload.title,
        data: payload.data,
        media_url: payload.media_url,
    };

    let content = repo
        .insert(&create_payload)
        .await
        .map_err(|e| AppError::Internal(format!("failed to create content: {e}")))?;

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "create_content",
        Some("content"),
        None, // contents use UUID, not BIGINT
        Some(serde_json::json!({
            "content_id": content.id.to_string(),
            "topic_id": content.topic_id.to_string(),
            "content_type": content.content_type,
            "content_title": content.title,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to record activity: {e}")))?;

    Ok(response::created_with_message(
        "Konten berhasil dibuat.",
        serde_json::json!({
            "id": content.id,
            "topic_id": content.topic_id,
            "type": content.content_type,
            "title": content.title,
            "data": content.data,
            "media_url": content.media_url,
            "is_published": content.is_published,
            "order": content.order,
        }),
    ))
}

/// PATCH /admin/contents/{id}
#[utoipa::path(patch, path = "/api/v1/admin/contents/{id}", tag = "admin-contents", params(("id" = Uuid, Path)), request_body = UpdateContentRequest, responses((status = 200, description = "Success")), security(("bearer_auth" = [])))]
pub async fn update(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateContentRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    // Validate content_type if provided
    if let Some(ref content_type) = payload.content_type {
        let valid_types = ["module", "quiz", "brief"];
        if !valid_types.contains(&content_type.as_str()) {
            return Err(AppError::Validation(format!(
                "invalid type '{}': must be one of module, quiz, brief",
                content_type
            )));
        }
    }

    let repo = PgContentsRepo::new(state.db_pool.clone());

    let update_payload = UpdateContentPayload {
        topic_id: payload.topic_id,
        content_type: payload.content_type,
        title: payload.title,
        data: payload.data,
        media_url: payload.media_url,
    };

    let content = repo
        .update(id, &update_payload)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                AppError::NotFound("content not found".into())
            } else {
                AppError::Internal(format!("failed to update content: {e}"))
            }
        })?;

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "update_content",
        Some("content"),
        None, // contents use UUID, not BIGINT
        Some(serde_json::json!({
            "content_id": content.id.to_string(),
            "topic_id": content.topic_id.to_string(),
            "content_type": content.content_type,
            "content_title": content.title,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to record activity: {e}")))?;

    Ok(response::ok_with_message(
        "Konten berhasil diperbarui.",
        serde_json::json!({
            "id": content.id,
            "topic_id": content.topic_id,
            "type": content.content_type,
            "title": content.title,
            "data": content.data,
            "media_url": content.media_url,
            "is_published": content.is_published,
            "order": content.order,
        }),
    ))
}

/// DELETE /admin/contents/{id}
#[utoipa::path(delete, path = "/api/v1/admin/contents/{id}", tag = "admin-contents", params(("id" = Uuid, Path)), responses((status = 200, description = "Deleted", body = ())), security(("bearer_auth" = [])))]
pub async fn delete(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;
    let repo = PgContentsRepo::new(state.db_pool.clone());

    let deleted = repo
        .delete(id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to delete content: {e}")))?;

    if !deleted {
        return Err(AppError::NotFound("content not found".into()));
    }

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "delete_content",
        Some("content"),
        None,
        Some(serde_json::json!({
            "content_id": id.to_string(),
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to record activity: {e}")))?;

    Ok(response::ok_with_message(
        "Konten berhasil dihapus.",
        serde_json::json!({}),
    ))
}

/// PATCH /admin/contents/{id}/reorder
///
/// Body: `{ "direction": "up" | "down" }`
/// Swaps the content's `order` with the adjacent content within the same topic_id group.
/// Operations are performed inside a `BEGIN...SELECT FOR UPDATE...COMMIT` transaction.
#[utoipa::path(patch, path = "/api/v1/admin/contents/{id}/reorder", tag = "admin-contents", params(("id" = Uuid, Path)), request_body = ReorderRequest, responses((status = 200, description = "Success")), security(("bearer_auth" = [])))]
pub async fn reorder(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(payload): Json<ReorderRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;
    let direction = payload.direction.to_lowercase();

    if direction != "up" && direction != "down" {
        return Err(AppError::Validation(format!(
            "invalid direction '{}': must be 'up' or 'down'",
            payload.direction
        )));
    }

    let repo = PgContentsRepo::new(state.db_pool.clone());

    repo.reorder(id, &direction)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") || e.to_string().contains("already at the edge")
            {
                AppError::Validation(e.to_string())
            } else {
                AppError::Internal(format!("failed to reorder content: {e}"))
            }
        })?;

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "reorder_content",
        Some("content"),
        None,
        Some(serde_json::json!({
            "content_id": id.to_string(),
            "direction": direction,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to record activity: {e}")))?;

    Ok(response::ok_with_message(
        &format!(
            "Konten berhasil dipindahkan ke {}.",
            if direction == "up" { "atas" } else { "bawah" }
        ),
        serde_json::json!({}),
    ))
}

/// PATCH /admin/contents/{id}/publish
///
/// Body: `{ "is_published": true | false }`
/// Sets the content's `is_published` flag to the given value (idempotent).
#[utoipa::path(patch, path = "/api/v1/admin/contents/{id}/publish", tag = "admin-contents", params(("id" = Uuid, Path)), request_body = PublishRequest, responses((status = 200, description = "Success")), security(("bearer_auth" = [])))]
pub async fn publish(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(payload): Json<PublishRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;
    let repo = PgContentsRepo::new(state.db_pool.clone());

    let new_is_published = repo
        .set_publish(id, payload.is_published)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                AppError::NotFound("content not found".into())
            } else {
                AppError::Internal(format!("failed to set publish status: {e}"))
            }
        })?;

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        if new_is_published {
            "publish_content"
        } else {
            "unpublish_content"
        },
        Some("content"),
        None,
        Some(serde_json::json!({
            "content_id": id.to_string(),
            "is_published": new_is_published,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to record activity: {e}")))?;

    Ok(response::ok_with_message(
        &format!(
            "Konten berhasil {}.",
            if new_is_published {
                "dipublikasikan"
            } else {
                "ditangguhkan"
            }
        ),
        serde_json::json!({
            "is_published": new_is_published,
        }),
    ))
}

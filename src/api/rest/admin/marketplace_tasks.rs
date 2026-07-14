use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::middleware::Principal;
use crate::db::repositories::marketplace_tasks::{
    CreateMarketplaceTaskPayload, MarketplaceTasksRepo, PgMarketplaceTasksRepo,
    UpdateMarketplaceTaskPayload,
};
use crate::error::{AppError, AppResult};
use crate::governance::activity_log::record_activity;
use crate::state::AppState;

use super::require_admin;
use super::super::response;

// ─── Valid statuses and task types ──────────────────────────────────────────

const VALID_STATUSES: &[&str] = &["open", "taken", "done"];
const VALID_TASK_TYPES: &[&str] = &["bid", "suggestion"];

// ─── Request bodies ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateMarketplaceTaskRequest {
    pub content_id: Uuid,
    pub status: Option<String>,
    pub task_type: Option<String>,
    pub description: Option<String>,
    pub creator_id: Option<String>,
    pub suggested_freelancer_id: Option<i64>,
    pub attachment_url: Option<String>,
    pub media_generation_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct UpdateMarketplaceTaskRequest {
    pub content_id: Option<Uuid>,
    pub task_type: Option<String>,
    pub description: Option<Option<String>>,
    pub creator_id: Option<Option<String>>,
    pub suggested_freelancer_id: Option<Option<i64>>,
    pub attachment_url: Option<Option<String>>,
    pub media_generation_id: Option<Option<Uuid>>,
}

#[derive(Deserialize)]
pub struct UpdateStatusRequest {
    pub status: String,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// POST /admin/marketplace-tasks
pub async fn create(
    State(state): State<AppState>,
    principal: Principal,
    Json(payload): Json<CreateMarketplaceTaskRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    let status = payload.status.unwrap_or_else(|| "open".to_string());
    let task_type = payload.task_type.unwrap_or_else(|| "bid".to_string());

    // Validate status
    if !VALID_STATUSES.contains(&status.as_str()) {
        return Err(AppError::Validation(format!(
            "invalid status '{}': must be one of open, taken, done",
            status
        )));
    }

    // Validate task_type
    if !VALID_TASK_TYPES.contains(&task_type.as_str()) {
        return Err(AppError::Validation(format!(
            "invalid task_type '{}': must be one of bid, suggestion",
            task_type
        )));
    }

    let repo = PgMarketplaceTasksRepo::new(state.db_pool.clone());

    let create_payload = CreateMarketplaceTaskPayload {
        content_id: payload.content_id,
        status,
        task_type,
        description: payload.description,
        creator_id: payload.creator_id,
        suggested_freelancer_id: payload.suggested_freelancer_id,
        attachment_url: payload.attachment_url,
        media_generation_id: payload.media_generation_id,
    };

    let task = repo
        .insert(&create_payload)
        .await
        .map_err(|e| AppError::Internal(format!("failed to create marketplace task: {e}")))?;

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "create_marketplace_task",
        Some("marketplace_task"),
        None, // marketplace_tasks use UUID, not BIGINT
        Some(serde_json::json!({
            "task_id": task.id.to_string(),
            "content_id": task.content_id.to_string(),
            "status": task.status,
            "task_type": task.task_type,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to record activity: {e}")))?;

    Ok(response::created_with_message(
        "Task berhasil dibuat.",
        serde_json::json!({
            "id": task.id,
            "content_id": task.content_id,
            "status": task.status,
            "task_type": task.task_type,
            "description": task.description,
            "creator_id": task.creator_id,
            "suggested_freelancer_id": task.suggested_freelancer_id,
            "attachment_url": task.attachment_url,
            "media_generation_id": task.media_generation_id,
        }),
    ))
}

/// PATCH /admin/marketplace-tasks/{id}
pub async fn update(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateMarketplaceTaskRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    // Validate task_type if provided
    if let Some(ref task_type) = payload.task_type {
        if !VALID_TASK_TYPES.contains(&task_type.as_str()) {
            return Err(AppError::Validation(format!(
                "invalid task_type '{}': must be one of bid, suggestion",
                task_type
            )));
        }
    }

    let repo = PgMarketplaceTasksRepo::new(state.db_pool.clone());

    let update_payload = UpdateMarketplaceTaskPayload {
        content_id: payload.content_id,
        task_type: payload.task_type,
        description: payload.description,
        creator_id: payload.creator_id,
        suggested_freelancer_id: payload.suggested_freelancer_id,
        attachment_url: payload.attachment_url,
        media_generation_id: payload.media_generation_id,
    };

    let task = repo
        .update(id, &update_payload)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                AppError::NotFound("marketplace task not found".into())
            } else {
                AppError::Internal(format!("failed to update marketplace task: {e}"))
            }
        })?;

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "update_marketplace_task",
        Some("marketplace_task"),
        None,
        Some(serde_json::json!({
            "task_id": task.id.to_string(),
            "content_id": task.content_id.to_string(),
            "status": task.status,
            "task_type": task.task_type,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to record activity: {e}")))?;

    Ok(response::ok_with_message(
        "Task berhasil diperbarui.",
        serde_json::json!({
            "id": task.id,
            "content_id": task.content_id,
            "status": task.status,
            "task_type": task.task_type,
            "description": task.description,
            "creator_id": task.creator_id,
            "suggested_freelancer_id": task.suggested_freelancer_id,
            "attachment_url": task.attachment_url,
            "media_generation_id": task.media_generation_id,
        }),
    ))
}

/// DELETE /admin/marketplace-tasks/{id}
pub async fn delete(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;
    let repo = PgMarketplaceTasksRepo::new(state.db_pool.clone());

    let deleted = repo
        .delete(id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to delete marketplace task: {e}")))?;

    if !deleted {
        return Err(AppError::NotFound("marketplace task not found".into()));
    }

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "delete_marketplace_task",
        Some("marketplace_task"),
        None,
        Some(serde_json::json!({
            "task_id": id.to_string(),
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to record activity: {e}")))?;

    Ok(response::ok_with_message(
        "Task berhasil dihapus.",
        serde_json::json!({}),
    ))
}

/// PATCH /admin/marketplace-tasks/{id}/status
///
/// Body: `{ "status": "open" | "taken" | "done" }`
/// Updates only the status field of a marketplace task.
/// Writes `update_task_status` activity log.
pub async fn update_status(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateStatusRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;
    let new_status = payload.status.to_lowercase();

    // Validate status
    if !VALID_STATUSES.contains(&new_status.as_str()) {
        return Err(AppError::Validation(format!(
            "invalid status '{}': must be one of open, taken, done",
            payload.status
        )));
    }

    let repo = PgMarketplaceTasksRepo::new(state.db_pool.clone());

    // Fetch the current task first to capture the previous status for activity logging
    let current = repo
        .find_by_id(id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to fetch task: {e}")))?
        .ok_or_else(|| AppError::NotFound("marketplace task not found".into()))?;

    let previous_status = current.task.status;

    let task = repo
        .update_status(id, &new_status)
        .await
        .map_err(|e| AppError::Internal(format!("failed to update task status: {e}")))?;

    // Record activity — uses `update_task_status` action as specified in the plan
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "update_task_status",
        Some("marketplace_task"),
        None,
        Some(serde_json::json!({
            "task_id": task.id.to_string(),
            "content_id": task.content_id.to_string(),
            "previous_status": previous_status,
            "new_status": new_status,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to record activity: {e}")))?;

    Ok(response::ok_with_message(
        &format!("Status task berhasil diubah ke '{}'.", new_status),
        serde_json::json!({
            "id": task.id,
            "content_id": task.content_id,
            "status": task.status,
            "task_type": task.task_type,
        }),
    ))
}

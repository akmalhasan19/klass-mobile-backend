use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::middleware::Principal;
use crate::db::repositories::topics::{PgTopicsRepo, TopicsRepo, UpdateTopicPayload};
use crate::error::{AppError, AppResult};
use crate::governance::activity_log::record_activity;
use crate::state::AppState;

use super::require_admin;
use super::super::response;

// ─── Request bodies ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateTopicRequest {
    pub title: Option<String>,
    pub sub_subject_id: Option<Option<i64>>,
    pub thumbnail_url: Option<Option<String>>,
}

#[derive(Deserialize)]
pub struct ReorderRequest {
    pub direction: String,
}

#[derive(Deserialize)]
pub struct PublishRequest {
    pub is_published: bool,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// PATCH /admin/topics/{id}
pub async fn update(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTopicRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;
    let repo = PgTopicsRepo::new(state.db_pool.clone());

    let update_payload = UpdateTopicPayload {
        title: payload.title,
        sub_subject_id: payload.sub_subject_id,
        thumbnail_url: payload.thumbnail_url,
    };

    let topic = repo
        .update(id, &update_payload)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                AppError::NotFound("topic not found".into())
            } else {
                AppError::Internal(format!("failed to update topic: {e}"))
            }
        })?;

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "update_topic",
        Some("topic"),
        None, // topics use UUID, not BIGINT
        Some(serde_json::json!({
            "topic_id": topic.id.to_string(),
            "topic_title": topic.title,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to record activity: {e}")))?;

    Ok(response::ok_with_message(
        "Topik berhasil diperbarui.",
        serde_json::json!({
            "id": topic.id,
            "title": topic.title,
            "sub_subject_id": topic.sub_subject_id,
            "thumbnail_url": topic.thumbnail_url,
            "is_published": topic.is_published,
            "order": topic.order,
        }),
    ))
}

/// DELETE /admin/topics/{id}
pub async fn delete(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;
    let repo = PgTopicsRepo::new(state.db_pool.clone());

    let deleted = repo
        .delete(id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to delete topic: {e}")))?;

    if !deleted {
        return Err(AppError::NotFound("topic not found".into()));
    }

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "delete_topic",
        Some("topic"),
        None,
        Some(serde_json::json!({
            "topic_id": id.to_string(),
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to record activity: {e}")))?;

    Ok(response::ok_with_message(
        "Topik berhasil dihapus.",
        serde_json::json!({}),
    ))
}

/// PATCH /admin/topics/{id}/reorder
///
/// Body: `{ "direction": "up" | "down" }`
/// Swaps the topic's `order` with the adjacent topic in the given direction.
/// Operations are performed inside a `BEGIN...SELECT FOR UPDATE...COMMIT` transaction.
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

    let repo = PgTopicsRepo::new(state.db_pool.clone());

    repo.reorder(id, &direction)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") || e.to_string().contains("already at the edge") {
                AppError::Validation(e.to_string())
            } else {
                AppError::Internal(format!("failed to reorder topic: {e}"))
            }
        })?;

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "reorder_topic",
        Some("topic"),
        None,
        Some(serde_json::json!({
            "topic_id": id.to_string(),
            "direction": direction,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to record activity: {e}")))?;

    Ok(response::ok_with_message(
        &format!("Topik berhasil dipindahkan ke {}.", if direction == "up" { "atas" } else { "bawah" }),
        serde_json::json!({}),
    ))
}

/// PATCH /admin/topics/{id}/publish
///
/// Body: `{ "is_published": true | false }`
/// Sets the topic's `is_published` flag to the given value (idempotent).
pub async fn publish(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(payload): Json<PublishRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;
    let repo = PgTopicsRepo::new(state.db_pool.clone());

    let new_is_published = repo
        .set_publish(id, payload.is_published)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                AppError::NotFound("topic not found".into())
            } else {
                AppError::Internal(format!("failed to set publish status: {e}"))
            }
        })?;

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        if new_is_published {
            "publish_topic"
        } else {
            "unpublish_topic"
        },
        Some("topic"),
        None,
        Some(serde_json::json!({
            "topic_id": id.to_string(),
            "is_published": new_is_published,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to record activity: {e}")))?;

    Ok(response::ok_with_message(
        &format!(
            "Topik berhasil {}.",
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

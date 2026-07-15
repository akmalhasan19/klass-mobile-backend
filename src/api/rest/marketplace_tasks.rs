use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::pagination::{PaginationMeta, PaginationParams, PaginationQuery};
use crate::db::repositories::marketplace_tasks::{
    ContentSummary, MarketplaceTask, MarketplaceTaskFilters, MarketplaceTasksRepo,
    PgMarketplaceTasksRepo,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

use super::response;

// ─── Resources ───────────────────────────────────────────────────────────────

#[derive(Serialize, utoipa::ToSchema)]
pub struct ContentResource {
    id: Uuid,
    topic_id: Uuid,
    #[serde(rename = "type")]
    content_type: String,
    title: Option<String>,
    data: Option<serde_json::Value>,
    media_url: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct MarketplaceTaskResource {
    id: Uuid,
    content_id: Uuid,
    status: String,
    creator_id: Option<String>,
    attachment_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<ContentResource>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

// ─── Query params ────────────────────────────────────────────────────────────

#[derive(Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct MarketplaceTaskQueryParams {
    search: Option<String>,
    status: Option<String>,
    content_id: Option<Uuid>,
    page: Option<i64>,
    per_page: Option<i64>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// GET /marketplace-tasks
#[utoipa::path(
    get,
    path = "/api/v1/marketplace-tasks",
    tag = "marketplace-tasks",
    params(MarketplaceTaskQueryParams),
    responses(
        (status = 200, body = Vec<MarketplaceTaskResource>),
    ),
)]
pub async fn index(
    State(state): State<AppState>,
    Query(params): Query<MarketplaceTaskQueryParams>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let pq = PaginationQuery::parse(Query(PaginationParams {
        page: params.page,
        per_page: params.per_page,
    }));

    let filters = MarketplaceTaskFilters {
        search: params.search,
        status: params.status,
        content_id: params.content_id,
    };

    let repo = PgMarketplaceTasksRepo::new(state.db_pool.clone());
    let (tasks_with_content, total) = repo
        .find_many(&filters, &pq)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let resources: Vec<MarketplaceTaskResource> = tasks_with_content
        .into_iter()
        .map(|twc| build_marketplace_task_resource(twc.task, Some(twc.content)))
        .collect();

    let meta = PaginationMeta::from_query(&pq, total);
    Ok(response::paginated(resources, meta))
}

/// GET /marketplace-tasks/{id}
#[utoipa::path(
    get,
    path = "/api/v1/marketplace-tasks/{id}",
    tag = "marketplace-tasks",
    params(
        ("id" = Uuid, Path, description = "Marketplace task ID"),
    ),
    responses(
        (status = 200, body = MarketplaceTaskResource),
        (status = 404, description = "Marketplace task not found"),
    ),
)]
pub async fn show(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let repo = PgMarketplaceTasksRepo::new(state.db_pool.clone());

    let twc = repo
        .find_by_id(id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("marketplace task not found".into()))?;

    let resource = build_marketplace_task_resource(twc.task, Some(twc.content));

    Ok(response::ok_with_message(
        "Detail task berhasil diambil.",
        resource,
    ))
}

// ─── Resource builders ───────────────────────────────────────────────────────

fn build_marketplace_task_resource(
    task: MarketplaceTask,
    content: Option<ContentSummary>,
) -> MarketplaceTaskResource {
    let content_resource = content.map(build_content_resource);

    MarketplaceTaskResource {
        id: task.id,
        content_id: task.content_id,
        status: task.status,
        creator_id: task.creator_id,
        attachment_url: task.attachment_url,
        content: content_resource,
        created_at: task.created_at.map(format_naive_datetime),
        updated_at: task.updated_at.map(format_naive_datetime),
    }
}

fn build_content_resource(content: ContentSummary) -> ContentResource {
    ContentResource {
        id: content.id,
        topic_id: content.topic_id,
        content_type: content.content_type,
        title: content.title,
        data: content.data,
        media_url: content.media_url,
        created_at: content.created_at.map(format_naive_datetime),
        updated_at: content.updated_at.map(format_naive_datetime),
    }
}

// ─── Formatting helpers ──────────────────────────────────────────────────────

fn format_naive_datetime(dt: chrono::NaiveDateTime) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}

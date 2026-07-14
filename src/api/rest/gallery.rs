use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::pagination::{PaginationMeta, PaginationParams, PaginationQuery};
use crate::db::repositories::gallery::{
    GalleryFilters, GalleryItem, GalleryRepo, PgGalleryRepo, TopicSummary,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

use super::response;

// ─── Resources ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct TopicResource {
    id: Uuid,
    title: String,
    teacher_id: String,
    owner_user_id: Option<i64>,
    ownership_status: String,
    sub_subject_id: Option<i64>,
    thumbnail_url: Option<String>,
    is_published: bool,
    order: i32,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Serialize)]
struct ContentResource {
    id: Uuid,
    topic_id: Uuid,
    #[serde(rename = "type")]
    content_type: String,
    title: Option<String>,
    data: Option<serde_json::Value>,
    media_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<TopicResource>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

// ─── Query params ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct GalleryQueryParams {
    search: Option<String>,
    topic_id: Option<Uuid>,
    #[serde(rename = "type")]
    content_type: Option<String>,
    page: Option<i64>,
    per_page: Option<i64>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// GET /gallery
pub async fn index(
    State(state): State<AppState>,
    Query(params): Query<GalleryQueryParams>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let pq = PaginationQuery::parse(Query(PaginationParams {
        page: params.page,
        per_page: params.per_page,
    }));

    let filters = GalleryFilters {
        search: params.search,
        content_type: params.content_type,
        topic_id: params.topic_id,
    };

    let repo = PgGalleryRepo::new(state.db_pool.clone());
    let (items, total) = repo
        .find_many(&filters, &pq)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let resources: Vec<ContentResource> = items
        .into_iter()
        .map(build_content_resource_from_gallery_item)
        .collect();

    let meta = PaginationMeta::from_query(&pq, total);
    Ok(response::paginated(resources, meta))
}

// ─── Resource builders ───────────────────────────────────────────────────────

fn build_content_resource_from_gallery_item(item: GalleryItem) -> ContentResource {
    let topic_resource = item.topic.map(build_topic_resource);

    ContentResource {
        id: item.content.id,
        topic_id: item.content.topic_id,
        content_type: item.content.content_type,
        title: item.content.title,
        data: item.content.data,
        media_url: item.content.media_url,
        topic: topic_resource,
        created_at: item.content.created_at.map(format_naive_datetime),
        updated_at: item.content.updated_at.map(format_naive_datetime),
    }
}

fn build_topic_resource(topic: TopicSummary) -> TopicResource {
    TopicResource {
        id: topic.id,
        title: topic.title,
        teacher_id: topic.teacher_id,
        owner_user_id: topic.owner_user_id,
        ownership_status: topic.ownership_status,
        sub_subject_id: topic.sub_subject_id,
        thumbnail_url: topic.thumbnail_url,
        is_published: topic.is_published,
        order: topic.order,
        created_at: topic.created_at.map(format_naive_datetime),
        updated_at: topic.updated_at.map(format_naive_datetime),
    }
}

// ─── Formatting helpers ──────────────────────────────────────────────────────

fn format_naive_datetime(dt: chrono::NaiveDateTime) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::pagination::{PaginationMeta, PaginationParams, PaginationQuery};
use crate::db::repositories::contents::{
    Content, ContentFilters, ContentWithRelations, ContentsRepo, MarketplaceTaskSummary,
    PgContentsRepo, TopicSummary,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

use super::response;

// ─── Resources ───────────────────────────────────────────────────────────────

#[derive(Serialize, utoipa::ToSchema)]
pub struct MarketplaceTaskResource {
    id: Uuid,
    content_id: Uuid,
    status: String,
    creator_id: Option<String>,
    attachment_url: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct TopicResource {
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

#[derive(Serialize, utoipa::ToSchema)]
pub struct ContentResource {
    id: Uuid,
    topic_id: Uuid,
    #[serde(rename = "type")]
    content_type: String,
    title: Option<String>,
    data: Option<serde_json::Value>,
    media_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<TopicResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tasks: Option<Vec<MarketplaceTaskResource>>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

// ─── Query params ────────────────────────────────────────────────────────────

#[derive(Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct ContentQueryParams {
    search: Option<String>,
    topic_id: Option<Uuid>,
    #[serde(rename = "type")]
    content_type: Option<String>,
    page: Option<i64>,
    per_page: Option<i64>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// GET /contents
#[utoipa::path(
    get,
    path = "/api/v1/contents",
    tag = "contents",
    params(ContentQueryParams),
    responses(
        (status = 200, body = Vec<ContentResource>),
    ),
)]
pub async fn index(
    State(state): State<AppState>,
    Query(params): Query<ContentQueryParams>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let pq = PaginationQuery::parse(Query(PaginationParams {
        page: params.page,
        per_page: params.per_page,
    }));

    let filters = ContentFilters {
        search: params.search,
        topic_id: params.topic_id,
        content_type: params.content_type,
    };

    let repo = PgContentsRepo::new(state.db_pool.clone());
    let (contents, total) = repo
        .find_many(&filters, &pq)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let topic_ids: Vec<Uuid> = contents
        .iter()
        .map(|c| c.topic_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let topics = load_topics(&state.db_pool, &topic_ids).await?;

    let resources: Vec<ContentResource> = contents
        .into_iter()
        .map(|c| {
            let topic = topics.get(&c.topic_id).cloned();
            build_content_resource(c, topic, None)
        })
        .collect();

    let meta = PaginationMeta::from_query(&pq, total);
    Ok(response::paginated(resources, meta))
}

/// GET /contents/{id}
#[utoipa::path(
    get,
    path = "/api/v1/contents/{id}",
    tag = "contents",
    params(
        ("id" = Uuid, Path, description = "Content ID"),
    ),
    responses(
        (status = 200, body = ContentResource),
        (status = 404, description = "Content not found"),
    ),
)]
pub async fn show(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let repo = PgContentsRepo::new(state.db_pool.clone());

    let cwr = repo
        .find_by_id(id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("content not found".into()))?;

    let resource = build_content_resource_from_cwr(cwr);

    Ok(response::ok_with_message(
        "Detail konten berhasil diambil.",
        resource,
    ))
}

// ─── Resource builders ───────────────────────────────────────────────────────

fn build_content_resource(
    content: Content,
    topic: Option<TopicSummary>,
    tasks: Option<Vec<MarketplaceTaskSummary>>,
) -> ContentResource {
    let topic_resource = topic.map(build_topic_resource);

    let tasks_resource = tasks.map(|ts| {
        ts.into_iter()
            .map(build_marketplace_task_resource)
            .collect()
    });

    ContentResource {
        id: content.id,
        topic_id: content.topic_id,
        content_type: content.content_type,
        title: content.title,
        data: content.data,
        media_url: content.media_url,
        topic: topic_resource,
        tasks: tasks_resource,
        created_at: content.created_at.map(format_naive_datetime),
        updated_at: content.updated_at.map(format_naive_datetime),
    }
}

fn build_content_resource_from_cwr(cwr: ContentWithRelations) -> ContentResource {
    let topic_resource = Some(build_topic_resource(cwr.topic));

    let tasks_resource = Some(
        cwr.tasks
            .into_iter()
            .map(build_marketplace_task_resource)
            .collect(),
    );

    ContentResource {
        id: cwr.content.id,
        topic_id: cwr.content.topic_id,
        content_type: cwr.content.content_type,
        title: cwr.content.title,
        data: cwr.content.data,
        media_url: cwr.content.media_url,
        topic: topic_resource,
        tasks: tasks_resource,
        created_at: cwr.content.created_at.map(format_naive_datetime),
        updated_at: cwr.content.updated_at.map(format_naive_datetime),
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

fn build_marketplace_task_resource(task: MarketplaceTaskSummary) -> MarketplaceTaskResource {
    MarketplaceTaskResource {
        id: task.id,
        content_id: task.content_id,
        status: task.status,
        creator_id: task.creator_id,
        attachment_url: task.attachment_url,
        created_at: task.created_at.map(format_naive_datetime),
        updated_at: task.updated_at.map(format_naive_datetime),
    }
}

// ─── DB helpers ──────────────────────────────────────────────────────────────

async fn load_topics(
    pool: &sqlx::PgPool,
    topic_ids: &[Uuid],
) -> AppResult<HashMap<Uuid, TopicSummary>> {
    if topic_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let sql = r#"
        SELECT id, title, teacher_id, sub_subject_id, thumbnail_url,
               is_published, "order", owner_user_id, ownership_status,
               created_at, updated_at
        FROM topics
        WHERE id = ANY($1)
    "#;

    let rows = sqlx::query_as::<_, TopicSummary>(sql)
        .bind(topic_ids)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Internal(format!("failed to load topics: {e}")))?;

    Ok(rows.into_iter().map(|r| (r.id, r)).collect())
}

// ─── Formatting helpers ──────────────────────────────────────────────────────

fn format_naive_datetime(dt: chrono::NaiveDateTime) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}

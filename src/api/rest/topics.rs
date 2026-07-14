use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::pagination::{PaginationMeta, PaginationParams, PaginationQuery};
use crate::db::repositories::topics::{
    Content, ContentWithTasks, PgTopicsRepo, Topic, TopicFilters, TopicWithContents, TopicsRepo,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

use super::response;

// ─── DB helper rows ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
struct SubSubjectRow {
    id: i64,
    subject_id: i64,
    name: String,
    slug: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct SubjectRow {
    id: i64,
    name: String,
    slug: String,
}

// ─── Resources ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct SubjectResource {
    id: i64,
    name: String,
    slug: String,
}

#[derive(Serialize)]
struct SubSubjectResource {
    id: i64,
    subject_id: i64,
    name: String,
    slug: String,
}

#[derive(Serialize)]
struct TaxonomyResource {
    subject: Option<SubjectResource>,
    sub_subject: SubSubjectResource,
}

#[derive(Serialize)]
struct PersonalizationResource {
    eligible: bool,
    mode: String,
    has_adequate_taxonomy: bool,
    has_normalized_ownership: bool,
    excluded_reason: Option<String>,
}

#[derive(Serialize)]
struct MarketplaceTaskResource {
    id: Uuid,
    content_id: Uuid,
    status: String,
    creator_id: Option<String>,
    attachment_url: Option<String>,
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
    tasks: Option<Vec<MarketplaceTaskResource>>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Serialize)]
struct TopicResource {
    id: Uuid,
    title: String,
    teacher_id: String,
    owner_user_id: Option<i64>,
    ownership_status: String,
    sub_subject_id: Option<i64>,
    subject_id: Option<i64>,
    taxonomy: Option<TaxonomyResource>,
    personalization: PersonalizationResource,
    thumbnail_url: Option<String>,
    is_published: bool,
    order: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    contents_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contents: Option<Vec<ContentResource>>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

// ─── Query params ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TopicQueryParams {
    search: Option<String>,
    teacher_id: Option<String>,
    subject_id: Option<i64>,
    sub_subject_id: Option<i64>,
    is_published: Option<bool>,
    include_contents: Option<String>,
    page: Option<i64>,
    per_page: Option<i64>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// GET /topics
pub async fn index(
    State(state): State<AppState>,
    Query(params): Query<TopicQueryParams>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let pq = PaginationQuery::parse(Query(PaginationParams {
        page: params.page,
        per_page: params.per_page,
    }));

    let include_contents = params
        .include_contents
        .as_deref()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let filters = TopicFilters {
        search: params.search,
        teacher_id: params.teacher_id,
        subject_id: params.subject_id,
        sub_subject_id: params.sub_subject_id,
        is_published: params.is_published,
    };

    let repo = PgTopicsRepo::new(state.db_pool.clone());
    let (topics, total) = repo
        .find_many(&filters, &pq)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let topic_ids: Vec<Uuid> = topics.iter().map(|t| t.id).collect();
    let sub_subject_ids: Vec<i64> = topics
        .iter()
        .filter_map(|t| t.sub_subject_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let sub_subjects = load_sub_subjects(&state.db_pool, &sub_subject_ids).await?;
    let subject_ids: Vec<i64> = sub_subjects
        .values()
        .map(|ss| ss.subject_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let subjects = load_subjects(&state.db_pool, &subject_ids).await?;

    let resources = if include_contents {
        let contents_by_topic = load_contents_for_topics(&state.db_pool, &topic_ids).await?;
        topics
            .into_iter()
            .map(|t| {
                let ss = t
                    .sub_subject_id
                    .and_then(|id| sub_subjects.get(&id).cloned());
                let subj = ss
                    .as_ref()
                    .and_then(|s| subjects.get(&s.subject_id).cloned());
                let contents = contents_by_topic.get(&t.id).cloned().unwrap_or_default();
                build_topic_resource_with_contents(t, ss, subj, contents)
            })
            .collect()
    } else {
        let counts = load_contents_counts(&state.db_pool, &topic_ids).await?;
        topics
            .into_iter()
            .map(|t| {
                let ss = t
                    .sub_subject_id
                    .and_then(|id| sub_subjects.get(&id).cloned());
                let subj = ss
                    .as_ref()
                    .and_then(|s| subjects.get(&s.subject_id).cloned());
                let count = counts.get(&t.id).copied().unwrap_or(0);
                build_topic_resource_with_count(t, ss, subj, count)
            })
            .collect()
    };

    let meta = PaginationMeta::from_query(&pq, total);
    Ok(response::paginated(resources, meta))
}

/// GET /topics/{id}
pub async fn show(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let repo = PgTopicsRepo::new(state.db_pool.clone());

    let twc = repo
        .find_by_id(id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("topic not found".into()))?;

    let sub_subject = match twc.topic.sub_subject_id {
        Some(ss_id) => load_single_sub_subject(&state.db_pool, ss_id).await?,
        None => None,
    };

    let subject = match &sub_subject {
        Some(ss) => load_single_subject(&state.db_pool, ss.subject_id).await?,
        None => None,
    };

    let resource = build_topic_resource_from_twc(twc, sub_subject, subject);

    Ok(response::ok_with_message(
        "Detail topik berhasil diambil.",
        resource,
    ))
}

// ─── Resource builders ───────────────────────────────────────────────────────

fn build_topic_resource_with_count(
    topic: Topic,
    sub_subject: Option<SubSubjectRow>,
    subject: Option<SubjectRow>,
    contents_count: i64,
) -> TopicResource {
    let subject_id = subject
        .as_ref()
        .map(|s| s.id)
        .or(sub_subject.as_ref().map(|ss| ss.subject_id));

    let taxonomy = sub_subject.as_ref().map(|ss| TaxonomyResource {
        subject: subject.as_ref().map(|s| SubjectResource {
            id: s.id,
            name: s.name.clone(),
            slug: s.slug.clone(),
        }),
        sub_subject: SubSubjectResource {
            id: ss.id,
            subject_id: ss.subject_id,
            name: ss.name.clone(),
            slug: ss.slug.clone(),
        },
    });

    let personalization = build_personalization(&topic);

    TopicResource {
        id: topic.id,
        title: topic.title,
        teacher_id: topic.teacher_id,
        owner_user_id: topic.owner_user_id,
        ownership_status: topic.ownership_status,
        sub_subject_id: topic.sub_subject_id,
        subject_id,
        taxonomy,
        personalization,
        thumbnail_url: topic.thumbnail_url,
        is_published: topic.is_published,
        order: topic.order,
        contents_count: Some(contents_count),
        contents: None,
        created_at: topic.created_at.map(format_naive_datetime),
        updated_at: topic.updated_at.map(format_naive_datetime),
    }
}

fn build_topic_resource_with_contents(
    topic: Topic,
    sub_subject: Option<SubSubjectRow>,
    subject: Option<SubjectRow>,
    contents: Vec<Content>,
) -> TopicResource {
    let subject_id = subject
        .as_ref()
        .map(|s| s.id)
        .or(sub_subject.as_ref().map(|ss| ss.subject_id));

    let taxonomy = sub_subject.as_ref().map(|ss| TaxonomyResource {
        subject: subject.as_ref().map(|s| SubjectResource {
            id: s.id,
            name: s.name.clone(),
            slug: s.slug.clone(),
        }),
        sub_subject: SubSubjectResource {
            id: ss.id,
            subject_id: ss.subject_id,
            name: ss.name.clone(),
            slug: ss.slug.clone(),
        },
    });

    let personalization = build_personalization(&topic);

    let content_resources: Vec<ContentResource> = contents
        .into_iter()
        .map(|c| ContentResource {
            id: c.id,
            topic_id: c.topic_id,
            content_type: c.content_type,
            title: c.title,
            data: c.data,
            media_url: c.media_url,
            tasks: None,
            created_at: c.created_at.map(format_naive_datetime),
            updated_at: c.updated_at.map(format_naive_datetime),
        })
        .collect();

    TopicResource {
        id: topic.id,
        title: topic.title,
        teacher_id: topic.teacher_id,
        owner_user_id: topic.owner_user_id,
        ownership_status: topic.ownership_status,
        sub_subject_id: topic.sub_subject_id,
        subject_id,
        taxonomy,
        personalization,
        thumbnail_url: topic.thumbnail_url,
        is_published: topic.is_published,
        order: topic.order,
        contents_count: None,
        contents: Some(content_resources),
        created_at: topic.created_at.map(format_naive_datetime),
        updated_at: topic.updated_at.map(format_naive_datetime),
    }
}

fn build_topic_resource_from_twc(
    twc: TopicWithContents,
    sub_subject: Option<SubSubjectRow>,
    subject: Option<SubjectRow>,
) -> TopicResource {
    let topic = twc.topic;
    let subject_id = subject
        .as_ref()
        .map(|s| s.id)
        .or(sub_subject.as_ref().map(|ss| ss.subject_id));

    let taxonomy = sub_subject.as_ref().map(|ss| TaxonomyResource {
        subject: subject.as_ref().map(|s| SubjectResource {
            id: s.id,
            name: s.name.clone(),
            slug: s.slug.clone(),
        }),
        sub_subject: SubSubjectResource {
            id: ss.id,
            subject_id: ss.subject_id,
            name: ss.name.clone(),
            slug: ss.slug.clone(),
        },
    });

    let personalization = build_personalization(&topic);

    let content_resources: Vec<ContentResource> = twc
        .contents
        .into_iter()
        .map(build_content_resource_with_tasks)
        .collect();

    TopicResource {
        id: topic.id,
        title: topic.title,
        teacher_id: topic.teacher_id,
        owner_user_id: topic.owner_user_id,
        ownership_status: topic.ownership_status,
        sub_subject_id: topic.sub_subject_id,
        subject_id,
        taxonomy,
        personalization,
        thumbnail_url: topic.thumbnail_url,
        is_published: topic.is_published,
        order: topic.order,
        contents_count: None,
        contents: Some(content_resources),
        created_at: topic.created_at.map(format_naive_datetime),
        updated_at: topic.updated_at.map(format_naive_datetime),
    }
}

fn build_content_resource_with_tasks(cwt: ContentWithTasks) -> ContentResource {
    let tasks: Vec<MarketplaceTaskResource> = cwt
        .tasks
        .into_iter()
        .map(|t| MarketplaceTaskResource {
            id: t.id,
            content_id: t.content_id,
            status: t.status,
            creator_id: t.creator_id,
            attachment_url: t.attachment_url,
            created_at: t.created_at.map(format_naive_datetime),
            updated_at: t.updated_at.map(format_naive_datetime),
        })
        .collect();

    ContentResource {
        id: cwt.content.id,
        topic_id: cwt.content.topic_id,
        content_type: cwt.content.content_type,
        title: cwt.content.title,
        data: cwt.content.data,
        media_url: cwt.content.media_url,
        tasks: Some(tasks),
        created_at: cwt.content.created_at.map(format_naive_datetime),
        updated_at: cwt.content.updated_at.map(format_naive_datetime),
    }
}

fn build_personalization(topic: &Topic) -> PersonalizationResource {
    let has_adequate_taxonomy = topic.sub_subject_id.is_some();
    let has_normalized_ownership =
        topic.owner_user_id.is_some() && topic.ownership_status == "normalized";
    let eligible = has_adequate_taxonomy && has_normalized_ownership;

    let mode = if eligible {
        "candidate"
    } else {
        "general_feed_only"
    };

    let excluded_reason = if !has_adequate_taxonomy {
        Some("missing_sub_subject")
    } else if !has_normalized_ownership {
        Some("unresolved_ownership")
    } else {
        None
    };

    PersonalizationResource {
        eligible,
        mode: mode.to_string(),
        has_adequate_taxonomy,
        has_normalized_ownership,
        excluded_reason: excluded_reason.map(String::from),
    }
}

// ─── DB helpers ──────────────────────────────────────────────────────────────

async fn load_sub_subjects(
    pool: &sqlx::PgPool,
    ids: &[i64],
) -> AppResult<HashMap<i64, SubSubjectRow>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }

    let sql = r#"
        SELECT id, subject_id, name, slug
        FROM sub_subjects
        WHERE id = ANY($1)
    "#;

    let rows = sqlx::query_as::<_, SubSubjectRow>(sql)
        .bind(ids)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Internal(format!("failed to load sub_subjects: {e}")))?;

    Ok(rows.into_iter().map(|r| (r.id, r)).collect())
}

async fn load_subjects(pool: &sqlx::PgPool, ids: &[i64]) -> AppResult<HashMap<i64, SubjectRow>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }

    let sql = r#"
        SELECT id, name, slug
        FROM subjects
        WHERE id = ANY($1)
    "#;

    let rows = sqlx::query_as::<_, SubjectRow>(sql)
        .bind(ids)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Internal(format!("failed to load subjects: {e}")))?;

    Ok(rows.into_iter().map(|r| (r.id, r)).collect())
}

async fn load_contents_counts(
    pool: &sqlx::PgPool,
    topic_ids: &[Uuid],
) -> AppResult<HashMap<Uuid, i64>> {
    if topic_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let sql = r#"
        SELECT topic_id, COUNT(*) as count
        FROM contents
        WHERE topic_id = ANY($1)
        GROUP BY topic_id
    "#;

    #[derive(sqlx::FromRow)]
    struct CountRow {
        topic_id: Uuid,
        count: i64,
    }

    let rows = sqlx::query_as::<_, CountRow>(sql)
        .bind(topic_ids)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Internal(format!("failed to load contents counts: {e}")))?;

    Ok(rows.into_iter().map(|r| (r.topic_id, r.count)).collect())
}

async fn load_contents_for_topics(
    pool: &sqlx::PgPool,
    topic_ids: &[Uuid],
) -> AppResult<HashMap<Uuid, Vec<Content>>> {
    if topic_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let sql = r#"
        SELECT id, topic_id, type, title, data, media_url,
               is_published, "order", created_at, updated_at
        FROM contents
        WHERE topic_id = ANY($1)
        ORDER BY "order" ASC, created_at ASC
    "#;

    let rows = sqlx::query_as::<_, Content>(sql)
        .bind(topic_ids)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Internal(format!("failed to load contents: {e}")))?;

    let mut map: HashMap<Uuid, Vec<Content>> = HashMap::new();
    for c in rows {
        map.entry(c.topic_id).or_default().push(c);
    }
    Ok(map)
}

async fn load_single_sub_subject(pool: &sqlx::PgPool, id: i64) -> AppResult<Option<SubSubjectRow>> {
    let sql = r#"
        SELECT id, subject_id, name, slug
        FROM sub_subjects
        WHERE id = $1
    "#;

    sqlx::query_as::<_, SubSubjectRow>(sql)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::Internal(format!("failed to load sub_subject: {e}")))
}

async fn load_single_subject(pool: &sqlx::PgPool, id: i64) -> AppResult<Option<SubjectRow>> {
    let sql = r#"
        SELECT id, name, slug
        FROM subjects
        WHERE id = $1
    "#;

    sqlx::query_as::<_, SubjectRow>(sql)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::Internal(format!("failed to load subject: {e}")))
}

// ─── Formatting helpers ──────────────────────────────────────────────────────

fn format_naive_datetime(dt: chrono::NaiveDateTime) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}

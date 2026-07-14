use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::Principal;
use crate::db::repositories::media_generations::{
    MediaGeneration, MediaGenerationWithRelations, MediaGenerationsRepo, PgMediaGenerationsRepo,
};
use crate::error::{AppError, AppResult};
use crate::orchestrator::submission::{is_terminal_status, CreateInput, ProviderMetadata, SubmissionService};
use crate::queue::redis_streams::QueueService;
use crate::state::AppState;

use super::response;

// ─── Guard helper ─────────────────────────────────────────────────────────────

fn require_teacher(principal: &Principal) -> Result<(), AppError> {
    if principal.role != "teacher" {
        return Err(AppError::Forbidden(format!(
            "requires role 'teacher', user has '{}'",
            principal.role
        )));
    }
    Ok(())
}

// ─── Request bodies ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateMediaGenerationRequest {
    pub raw_prompt: String,
    pub preferred_output_type: Option<String>,
    pub subject_id: Option<i64>,
    pub sub_subject_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct MediaGenerationQueryParams {
    pub parent_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct RegenerateRequest {
    pub additional_prompt: String,
}

// ─── Resource ─────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SubjectResource {
    pub id: i64,
    pub name: String,
    pub slug: String,
}

#[derive(Serialize)]
pub struct SubSubjectResource {
    pub id: i64,
    pub subject_id: i64,
    pub name: String,
    pub slug: String,
    pub subject: Option<SubjectResource>,
}

#[derive(Serialize)]
pub struct TopicResource {
    pub id: Uuid,
    pub title: String,
    pub sub_subject_id: Option<i64>,
    pub thumbnail_url: Option<String>,
    pub is_published: bool,
}

#[derive(Serialize)]
pub struct ContentResource {
    pub id: Uuid,
    pub topic_id: Uuid,
    #[serde(rename = "type")]
    pub content_type: String,
    pub title: Option<String>,
    pub media_url: Option<String>,
    pub is_published: bool,
}

#[derive(Serialize)]
pub struct RecommendedProjectResource {
    pub id: i64,
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub project_file_url: Option<String>,
    pub source_type: String,
    pub is_active: bool,
}

#[derive(Serialize)]
pub struct MediaGenerationResource {
    pub id: Uuid,
    pub status: String,
    pub raw_prompt: String,
    pub preferred_output_type: String,
    pub resolved_output_type: Option<String>,
    pub subject_id: Option<i64>,
    pub sub_subject_id: Option<i64>,
    pub topic_id: Option<Uuid>,
    pub content_id: Option<Uuid>,
    pub recommended_project_id: Option<i64>,
    pub generated_from_id: Option<Uuid>,
    pub is_regeneration: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub subject: Option<SubjectResource>,
    pub sub_subject: Option<SubSubjectResource>,
    pub topic: Option<TopicResource>,
    pub content: Option<ContentResource>,
    pub recommended_project: Option<RecommendedProjectResource>,
}

#[derive(Serialize)]
pub struct MediaGenerationListResponse {
    pub generations: Vec<MediaGenerationResource>,
}

#[derive(Serialize)]
pub struct MediaGenerationChainResource {
    pub ancestors: Vec<MediaGenerationResource>,
    pub children: Vec<MediaGenerationResource>,
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// POST /media-generations
pub async fn create(
    State(state): State<AppState>,
    principal: Principal,
    Json(payload): Json<CreateMediaGenerationRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_teacher(&principal)?;

    let service = SubmissionService::new(state.db_pool.clone());

    let input = CreateInput {
        teacher_id: principal.user_id,
        raw_prompt: payload.raw_prompt,
        preferred_output_type: payload.preferred_output_type,
        subject_id: payload.subject_id,
        sub_subject_id: payload.sub_subject_id,
        provider_metadata: ProviderMetadata::default(),
    };

    let result = service
        .create_or_reuse(input)
        .await
        .map_err(|e| AppError::Internal(format!("failed to create media generation: {e}")))?;

    // Enqueue if freshly created
    if result.was_created {
        if let Some(ref redis_pool) = state.redis_pool {
            let queue = QueueService::new(redis_pool.clone(), 1);
            if let Err(e) = queue.enqueue(&result.id.to_string(), 1).await {
                tracing::warn!(error = %e, generation_id = %result.id, "failed to enqueue media generation");
            }
        } else {
            tracing::warn!("redis not available, skipping enqueue for media generation");
        }
    }

    let generation = PgMediaGenerationsRepo::new(state.db_pool.clone())
        .find_by_id_for_teacher(result.id, principal.user_id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to fetch created generation: {e}")))?
        .ok_or_else(|| AppError::Internal("created generation not found".into()))?;

    let resource = build_resource(&generation);

    Ok(response::accepted_with_message(
        "Generasi media berhasil dibuat.",
        resource,
    ))
}

/// GET /media-generations?parent_id=
pub async fn index(
    State(state): State<AppState>,
    principal: Principal,
    Query(params): Query<MediaGenerationQueryParams>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_teacher(&principal)?;

    let repo = PgMediaGenerationsRepo::new(state.db_pool.clone());

    match params.parent_id {
        Some(parent_id) => {
            // Chain walk from the given parent
            let chain = repo
                .find_chain(parent_id)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

            let chain_resource = MediaGenerationChainResource {
                ancestors: chain.ancestors.iter().map(build_resource_from_gen).collect(),
                children: chain.children.iter().map(build_resource_from_gen).collect(),
            };

            Ok(response::ok(chain_resource))
        }
        None => {
            // 20 most recent for this teacher
            let generations = repo
                .find_recent_for_teacher(principal.user_id, 20)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

            let resources: Vec<MediaGenerationResource> =
                generations.iter().map(build_resource).collect();

            Ok(response::ok(MediaGenerationListResponse {
                generations: resources,
            }))
        }
    }
}

/// GET /media-generations/{id}
pub async fn show(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_teacher(&principal)?;

    let repo = PgMediaGenerationsRepo::new(state.db_pool.clone());

    let generation = repo
        .find_by_id_for_teacher(id, principal.user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("media generation not found".into()))?;

    let resource = build_resource(&generation);
    Ok(response::ok_with_message(
        "Detail generasi media berhasil diambil.",
        resource,
    ))
}

/// POST /media-generations/{id}/regenerate
pub async fn regenerate(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(payload): Json<RegenerateRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_teacher(&principal)?;

    let repo = PgMediaGenerationsRepo::new(state.db_pool.clone());

    // Fetch the parent generation (scoped to teacher)
    let parent = repo
        .find_by_id_for_teacher(id, principal.user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("media generation not found".into()))?;

    // Parent must be in a terminal state
    if !is_terminal_status(&parent.generation.status) {
        return Err(AppError::Validation(format!(
            "parent generation status '{}' is not terminal — must be completed, failed, or cancelled",
            parent.generation.status
        )));
    }

    let service = SubmissionService::new(state.db_pool.clone());

    let new_id = service
        .create_regeneration(&id.to_string(), &payload.additional_prompt)
        .await
        .map_err(|e| AppError::Internal(format!("failed to create regeneration: {e}")))?;

    // Enqueue the new generation
    if let Some(ref redis_pool) = state.redis_pool {
        let queue = QueueService::new(redis_pool.clone(), 1);
        if let Err(e) = queue.enqueue(&new_id.to_string(), 1).await {
            tracing::warn!(error = %e, generation_id = %new_id, "failed to enqueue regeneration");
        }
    } else {
        tracing::warn!("redis not available, skipping enqueue for regeneration");
    }

    // Fetch the newly created regeneration
    let generation = repo
        .find_by_id_for_teacher(new_id, principal.user_id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to fetch regeneration: {e}")))?
        .ok_or_else(|| AppError::Internal("regeneration not found".into()))?;

    let resource = build_resource(&generation);
    Ok(response::accepted_with_message(
        "Regenerasi media berhasil dibuat.",
        resource,
    ))
}

// ─── Resource builders ────────────────────────────────────────────────────────

fn build_resource(generation: &MediaGenerationWithRelations) -> MediaGenerationResource {
    let mut res = build_resource_from_gen(&generation.generation);

    res.subject = generation.subject.as_ref().map(|s| SubjectResource {
        id: s.id,
        name: s.name.clone(),
        slug: s.slug.clone(),
    });

    res.sub_subject = generation.sub_subject.as_ref().map(|ss| SubSubjectResource {
        id: ss.sub_subject.id,
        subject_id: ss.sub_subject.subject_id,
        name: ss.sub_subject.name.clone(),
        slug: ss.sub_subject.slug.clone(),
        subject: Some(SubjectResource {
            id: ss.subject.id,
            name: ss.subject.name.clone(),
            slug: ss.subject.slug.clone(),
        }),
    });

    res.topic = generation.topic.as_ref().map(|t| TopicResource {
        id: t.id,
        title: t.title.clone(),
        sub_subject_id: t.sub_subject_id,
        thumbnail_url: t.thumbnail_url.clone(),
        is_published: t.is_published,
    });

    res.content = generation.content.as_ref().map(|c| ContentResource {
        id: c.id,
        topic_id: c.topic_id,
        content_type: c.content_type.clone(),
        title: c.title.clone(),
        media_url: c.media_url.clone(),
        is_published: c.is_published,
    });

    res.recommended_project = generation.recommended_project.as_ref().map(|r| RecommendedProjectResource {
        id: r.id,
        title: r.title.clone(),
        thumbnail_url: r.thumbnail_url.clone(),
        project_file_url: r.project_file_url.clone(),
        source_type: r.source_type.clone(),
        is_active: r.is_active,
    });

    res
}

fn build_resource_from_gen(gen: &MediaGeneration) -> MediaGenerationResource {
    MediaGenerationResource {
        id: gen.id,
        status: gen.status.clone(),
        raw_prompt: gen.raw_prompt.clone(),
        preferred_output_type: gen.preferred_output_type.clone(),
        resolved_output_type: gen.resolved_output_type.clone(),
        subject_id: gen.subject_id,
        sub_subject_id: gen.sub_subject_id,
        topic_id: gen.topic_id,
        content_id: gen.content_id,
        recommended_project_id: gen.recommended_project_id,
        generated_from_id: gen.generated_from_id,
        is_regeneration: gen.is_regeneration,
        created_at: gen.created_at.map(format_naive_datetime),
        updated_at: gen.updated_at.map(format_naive_datetime),
        subject: None,
        sub_subject: None,
        topic: None,
        content: None,
        recommended_project: None,
    }
}

fn format_naive_datetime(dt: chrono::NaiveDateTime) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}

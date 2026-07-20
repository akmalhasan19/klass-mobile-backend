use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::Principal;
use crate::contracts::prompt_interpretation as interp_contract;
use crate::db::repositories::media_generations::{
    MediaGeneration, MediaGenerationWithRelations, MediaGenerationsRepo, PgMediaGenerationsRepo,
    UpdatePayloadsPayload,
};
use crate::error::{AppError, AppResult};
use crate::media_gen::python_client::PythonMediaGeneratorClient;
use crate::orchestrator::submission::{is_terminal_status, CreateInput, ProviderMetadata, SubmissionService};
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

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateMediaGenerationRequest {
    pub raw_prompt: String,
    pub preferred_output_type: Option<String>,
    pub subject_id: Option<i64>,
    pub sub_subject_id: Option<i64>,
}

// ─── Async job tracking response (Task 1.3) ──────────────────────────────────

#[derive(Serialize, utoipa::ToSchema)]
pub struct CreateMediaGenerationResponse {
    pub generation_id: Uuid,
    pub job_id: Uuid,
    pub status: String,
    pub poll_url: String,
}

/// Response for polling the async media generation job status.
///
/// Fields are conditionally serialized based on the current job state so the
/// client only ever receives the data relevant to that state:
/// - `presigned_download_url` and `presigned_url_expires_at` are present only
///   when `status == "completed"`.
/// - `error_code` and `error_message` are present only when `status == "failed"`.
#[derive(Serialize, utoipa::ToSchema)]
pub struct JobStatusResponse {
    /// The media generation ID.
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub generation_id: Uuid,

    /// The async job ID (present once the generation has been enqueued).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<Uuid>,

    /// Current job status: `pending`, `processing`, `completed`, or `failed`.
    #[schema(example = "processing")]
    pub status: String,

    /// Presigned download URL for the generated artifact.
    /// Only present when `status == "completed"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presigned_download_url: Option<String>,

    /// ISO 8601 timestamp when the presigned download URL expires.
    /// Only present when `status == "completed"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presigned_url_expires_at: Option<String>,

    /// Machine-readable error code. Only present when `status == "failed"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,

    /// Human-readable error message. Only present when `status == "failed"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct MediaGenerationQueryParams {
    pub parent_id: Option<Uuid>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct RegenerateRequest {
    pub additional_prompt: String,
}

// ─── Resource ─────────────────────────────────────────────────────────────────

#[derive(Serialize, utoipa::ToSchema)]
pub struct SubjectResource {
    pub id: i64,
    pub name: String,
    pub slug: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SubSubjectResource {
    pub id: i64,
    pub subject_id: i64,
    pub name: String,
    pub slug: String,
    pub subject: Option<SubjectResource>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct TopicResource {
    pub id: Uuid,
    pub title: String,
    pub sub_subject_id: Option<i64>,
    pub thumbnail_url: Option<String>,
    pub is_published: bool,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ContentResource {
    pub id: Uuid,
    pub topic_id: Uuid,
    #[serde(rename = "type")]
    pub content_type: String,
    pub title: Option<String>,
    pub media_url: Option<String>,
    pub is_published: bool,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct RecommendedProjectResource {
    pub id: i64,
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub project_file_url: Option<String>,
    pub source_type: String,
    pub is_active: bool,
}

#[derive(Serialize, utoipa::ToSchema)]
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
    pub delivery_payload: Option<serde_json::Value>,
    pub presigned_download_url: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub subject: Option<SubjectResource>,
    pub sub_subject: Option<SubSubjectResource>,
    pub topic: Option<TopicResource>,
    pub content: Option<ContentResource>,
    pub recommended_project: Option<RecommendedProjectResource>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct MediaGenerationListResponse {
    pub generations: Vec<MediaGenerationResource>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct MediaGenerationChainResource {
    pub ancestors: Vec<MediaGenerationResource>,
    pub children: Vec<MediaGenerationResource>,
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// POST /media-generations
#[utoipa::path(
    post,
    path = "/api/v1/media-generations",
    tag = "media-generations",
    summary = "Create an async media generation job",
    description = "Creates (or reuses) a media generation and enqueues it for \
        asynchronous processing. Returns `202 Accepted` immediately with a \
        `job_id` and a `poll_url` the client can use to poll for job status. \
        The generated artifact is delivered out-of-band via an internal webhook \
        and exposed to clients through a presigned download URL once completed.",
    request_body = CreateMediaGenerationRequest,
    responses(
        (status = 202, description = "Generation accepted and enqueued for async processing", body = CreateMediaGenerationResponse),
        (status = 401, description = "Missing or invalid authentication token"),
        (status = 403, description = "Authenticated user is not a teacher"),
        (status = 422, description = "Validation error"),
    ),
    security(
        ("bearer_auth" = [])
    ),
)]
pub async fn create(
    State(state): State<AppState>,
    principal: Principal,
    Json(payload): Json<CreateMediaGenerationRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_teacher(&principal)?;

    let service = SubmissionService::new(state.db_pool.clone());

    let raw_prompt = payload.raw_prompt.clone();
    let preferred_output_type = payload.preferred_output_type.clone();

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

    // ─── Async job tracking (Task 1.3.1) ─────────────────────────────────────
    let repo = PgMediaGenerationsRepo::new(state.db_pool.clone());

    if result.was_created {
        // Generate a job_id (UUID) for async tracking
        let job_id = Uuid::new_v4();

        // Update DB with job_id and status='pending'
        repo.update_generation_job_status(
            result.id,
            &crate::db::repositories::media_generations::UpdateGenerationJobStatusPayload {
                generation_job_id: Some(job_id),
                generation_status: "pending".to_string(),
            },
        )
        .await
        .map_err(|e| AppError::Internal(format!("failed to set generation job status: {e}")))?;

        // Enqueue to Redis stream for async workflow processing.
        // The worker will run the full LLM pipeline:
        //   interpret (OpenRouter) → DecisionService → draft (OpenRouter) → generate (Python)
        if let Some(ref redis_pool) = state.redis_pool {
            let queue = crate::queue::redis_streams::QueueService::new(redis_pool.clone(), 1);
            if let Err(e) = queue
                .enqueue(&result.id.to_string(), &job_id.to_string(), 1)
                .await
            {
                tracing::warn!(
                    error = %e,
                    generation_id = %result.id,
                    job_id = %job_id,
                    "failed to enqueue generation job to Redis stream"
                );
            } else {
                tracing::info!(
                    generation_id = %result.id,
                    job_id = %job_id,
                    "enqueued generation job to Redis stream for LLM workflow"
                );
            }
        } else {
            tracing::warn!(
                generation_id = %result.id,
                job_id = %job_id,
                "Redis not configured — worker pipeline unavailable, job may not be processed"
            );
        }

        // Return 202 Accepted with async tracking info
        let response = CreateMediaGenerationResponse {
            generation_id: result.id,
            job_id,
            status: "pending".to_string(),
            poll_url: format!("/api/v1/media-generations/{}/job-status", result.id),
        };

        return Ok(response::accepted_with_message(
            "Generasi media berhasil dibuat dan sedang diproses.",
            response,
        ));
    }

    // ─── Existing generation (duplicate) → return existing data ──────────────
    let generation = repo
        .find_by_id_for_teacher(result.id, principal.user_id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to fetch created generation: {e}")))?
        .ok_or_else(|| AppError::Internal("created generation not found".into()))?;

    let job_id = generation.generation.generation_job_id.unwrap_or_else(Uuid::new_v4);

    let response = CreateMediaGenerationResponse {
        generation_id: result.id,
        job_id,
        status: generation.generation.status.clone(),
        poll_url: format!("/api/v1/media-generations/{}/job-status", result.id),
    };

    Ok(response::accepted_with_message(
        "Generasi media berhasil dibuat.",
        response,
    ))
}

/// GET /media-generations?parent_id=
#[utoipa::path(
    get,
    path = "/api/v1/media-generations",
    tag = "media-generations",
    params(
        ("parent_id" = Option<Uuid>, Query, description = "Parent generation ID"),
    ),
    responses(
        (status = 200, body = MediaGenerationListResponse),
    ),
    security(
        ("bearer_auth" = [])
    ),
)]
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
#[utoipa::path(
    get,
    path = "/api/v1/media-generations/{id}",
    tag = "media-generations",
    params(
        ("id" = Uuid, Path, description = "Media generation ID"),
    ),
    responses(
        (status = 200, body = MediaGenerationResource),
        (status = 404, description = "Media generation not found"),
    ),
    security(
        ("bearer_auth" = [])
    ),
)]
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

/// GET /media-generations/{id}/job-status (Task 1.3.2)
#[utoipa::path(
    get,
    path = "/api/v1/media-generations/{id}/job-status",
    tag = "media-generations",
    summary = "Poll async media generation job status",
    description = "Returns the current status of an async media generation job. \
        Clients should poll this endpoint with exponential backoff. When the job \
        is `completed`, a freshly generated presigned download URL (valid for 1 \
        hour) and its expiry timestamp are included. When the job is `failed`, an \
        error code and message are included instead.",
    params(
        ("id" = Uuid, Path, description = "Media generation ID"),
    ),
    responses(
        (status = 200, description = "Current job status", body = JobStatusResponse),
        (status = 401, description = "Missing or invalid authentication token"),
        (status = 403, description = "Authenticated user is not a teacher"),
        (status = 404, description = "Media generation not found"),
    ),
    security(
        ("bearer_auth" = [])
    ),
)]
/// Sub-task 3.2.3: Add tracing span for job status polling
#[tracing::instrument(
    name = "job_status.poll",
    skip(state, principal),
    fields(generation_id = %id)
)]
pub async fn job_status(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_teacher(&principal)?;

    let repo = PgMediaGenerationsRepo::new(state.db_pool.clone());

    // Fetch generation by ID, scoped to teacher
    let generation = repo
        .find_by_id_for_teacher(id, principal.user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("media generation not found".into()))?;

    let gen = &generation.generation;

    // Determine presigned_download_url + expiry: if completed, generate on-demand
    // with a 1 hour expiry, falling back to the persisted DB value on failure.
    const PRESIGNED_URL_TTL_SECS: u64 = 3600;
    let mut presigned_url_expires_at: Option<String> = None;

    let presigned_download_url = if gen.generation_status.as_deref() == Some("completed") {
        if let Some(ref s3_key) = gen.s3_object_key {
            match crate::storage::r2::generate_presigned_url(
                &state.s3_client,
                &state.config.r2_transit_bucket_name,
                s3_key,
                std::time::Duration::from_secs(PRESIGNED_URL_TTL_SECS),
            )
            .await
            {
                Ok(url) => {
                    // Freshly generated URL is valid for TTL from now.
                    presigned_url_expires_at = Some(format_naive_datetime(
                        chrono::Utc::now()
                            + chrono::Duration::seconds(PRESIGNED_URL_TTL_SECS as i64),
                    ));
                    Some(url)
                }
                Err(e) => {
                    tracing::error!(error = %e, s3_key = %s3_key, "failed to generate presigned URL on demand");
                    // Fallback to DB-persisted URL + its recorded expiry.
                    presigned_url_expires_at =
                        gen.presigned_url_expires_at.map(format_naive_datetime);
                    gen.presigned_download_url.clone()
                }
            }
        } else {
            presigned_url_expires_at = gen.presigned_url_expires_at.map(format_naive_datetime);
            gen.presigned_download_url.clone()
        }
    } else {
        None
    };

    let response = JobStatusResponse {
        generation_id: gen.id,
        job_id: gen.generation_job_id,
        status: gen.generation_status.clone().unwrap_or_else(|| gen.status.clone()),
        presigned_download_url,
        presigned_url_expires_at,
        error_code: gen.generation_error_code.clone(),
        error_message: gen.generation_error_message.clone(),
    };

    Ok(response::ok(response))
}

/// POST /media-generations/{id}/regenerate
#[utoipa::path(
    post,
    path = "/api/v1/media-generations/{id}/regenerate",
    tag = "media-generations",
    params(
        ("id" = Uuid, Path, description = "Media generation ID"),
    ),
    request_body = RegenerateRequest,
    responses(
        (status = 202, body = MediaGenerationResource),
    ),
    security(
        ("bearer_auth" = [])
    ),
)]
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

    // Submit directly to Python service (fire-and-forget)
    {
        let job_id = Uuid::new_v4();

        // Update DB with job_id and status='pending'
        if let Err(e) = repo.update_generation_job_status(
            new_id,
            &crate::db::repositories::media_generations::UpdateGenerationJobStatusPayload {
                generation_job_id: Some(job_id),
                generation_status: "pending".to_string(),
            },
        )
        .await
        {
            tracing::warn!(error = %e, generation_id = %new_id, "failed to set regeneration job status");
        }

        let python_client = PythonMediaGeneratorClient::new(
            state.db_pool.clone(),
            state.http.clone(),
            &state.config,
        );
        if let Err(e) = python_client
            .submit_job(&new_id.to_string(), &job_id.to_string())
            .await
        {
            tracing::warn!(
                error = %e,
                generation_id = %new_id,
                job_id = %job_id,
                "failed to submit regeneration job to Python service"
            );
        }
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
        delivery_payload: gen.delivery_payload.clone(),
        presigned_download_url: gen.presigned_download_url.clone(),
        error_code: gen.error_code.clone(),
        error_message: gen.error_message.clone(),
        subject: None,
        sub_subject: None,
        topic: None,
        content: None,
        recommended_project: None,
    }
}

fn format_naive_datetime(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}

//! Clarification API handlers.
//!
//! Endpoints:
//! - `POST /api/v1/media-generations/preflight` — Analyze prompt, return gaps
//! - `POST /api/v1/media-generations/confirm` — Submit enriched prompt
//! - `POST /api/v1/media-generations/{id}/skip-clarification` — Skip all, use enriched prompt

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::Principal;
use crate::db::repositories::media_generations::{
    MediaGenerationsRepo, PgMediaGenerationsRepo, UpdateClarificationStatePayload,
    UpdateGenerationJobStatusPayload,
};
use crate::error::{AppError, AppResult};
use crate::llm::clarification::{PreflightInput, ClarificationService};
use crate::orchestrator::submission::{CreateInput, ProviderMetadata, SubmissionService};
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
pub struct PreflightRequest {
    /// The raw teacher prompt to analyze.
    pub raw_prompt: String,
    /// Optional preferred output type (auto, pdf, docx, pptx).
    #[serde(default)]
    pub preferred_output_type: Option<String>,
    /// Optional subject ID for context.
    #[serde(default)]
    pub subject_id: Option<i64>,
    /// Optional sub-subject ID for context.
    #[serde(default)]
    pub sub_subject_id: Option<i64>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ConfirmRequest {
    /// The generation UUID from preflight.
    pub generation_id: String,
    /// The enriched prompt (original + clarification answers).
    pub enriched_prompt: String,
    /// Map of field_id → value (answers to clarification questions).
    #[serde(default)]
    pub answers: std::collections::HashMap<String, String>,
    /// Optional subject ID.
    #[serde(default)]
    pub subject_id: Option<i64>,
    /// Optional sub-subject ID.
    #[serde(default)]
    pub sub_subject_id: Option<i64>,
}

// ─── Response schemas ────────────────────────────────────────────────────────

#[derive(Serialize, utoipa::ToSchema)]
pub struct PreflightResponseData {
    pub generation_id: String,
    pub detected: DetectedInfoSchema,
    pub gaps: Vec<ContentGapSchema>,
    pub suggested_prompt: String,
    pub is_ready: bool,
    pub total_required_gaps: usize,
    pub total_recommended_gaps: usize,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct DetectedInfoSchema {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    pub content_type: String,
    pub confidence: f64,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ContentGapSchema {
    pub field_id: String,
    pub question: String,
    pub priority: String,
    pub input_type: String,
    pub suggestions: Vec<SuggestionChipSchema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_value: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SuggestionChipSchema {
    pub value: String,
    pub label: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ConfirmResponseData {
    pub generation_id: String,
    pub job_id: String,
    pub status: String,
    pub poll_url: String,
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// POST /media-generations/preflight
///
/// Analyzes the teacher's prompt, detects content type and auto-detected fields,
/// computes gaps against content standards, and returns clarification questions.
#[utoipa::path(
    post,
    path = "/api/v1/media-generations/preflight",
    tag = "media-generations",
    summary = "Analyze prompt and return clarification questions",
    description = "Analyzes a teacher's prompt to detect content type, auto-detect fields \
        (audience, output type, subject), and returns clarification questions for missing \
        required/recommended fields. Returns `is_ready: true` if all required fields are \
        already present in the prompt.",
    request_body = PreflightRequest,
    responses(
        (status = 200, description = "Preflight analysis with clarification gaps", body = PreflightResponseData),
        (status = 401, description = "Missing or invalid authentication token"),
        (status = 403, description = "Authenticated user is not a teacher"),
        (status = 422, description = "Validation error"),
    ),
    security(
        ("bearer_auth" = [])
    ),
)]
pub async fn preflight(
    State(state): State<AppState>,
    principal: Principal,
    Json(payload): Json<PreflightRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_teacher(&principal)?;

    // Validate input
    if payload.raw_prompt.trim().is_empty() {
        return Err(AppError::Validation(
            "raw_prompt tidak boleh kosong".to_string(),
        ));
    }

    // Run preflight analysis
    let input = PreflightInput {
        raw_prompt: payload.raw_prompt,
        preferred_output_type: payload.preferred_output_type,
        subject_id: payload.subject_id,
        sub_subject_id: payload.sub_subject_id,
    };

    let clarification_response = ClarificationService::preflight(input);

    // Build detected info, resolving subject name if subject_id is provided
    let mut detected = DetectedInfoSchema {
        output_type: clarification_response.detected.output_type,
        subject: clarification_response.detected.subject,
        subject_id: clarification_response.detected.subject_id,
        audience: clarification_response.detected.audience,
        topic: clarification_response.detected.topic,
        content_type: clarification_response.detected.content_type,
        confidence: clarification_response.detected.confidence,
    };

    if let Some(subject_id) = detected.subject_id {
        detected.subject = resolve_subject_name(&state, subject_id).await;
    }

    let response_data = PreflightResponseData {
        generation_id: clarification_response.generation_id,
        detected,
        gaps: clarification_response
            .gaps
            .into_iter()
            .map(|g| ContentGapSchema {
                field_id: g.field_id,
                question: g.question,
                priority: g.priority,
                input_type: g.input_type,
                suggestions: g
                    .suggestions
                    .into_iter()
                    .map(|s| SuggestionChipSchema {
                        value: s.value,
                        label: s.label,
                    })
                    .collect(),
                detected_value: g.detected_value,
            })
            .collect(),
        suggested_prompt: clarification_response.suggested_prompt,
        is_ready: clarification_response.is_ready,
        total_required_gaps: clarification_response.total_required_gaps,
        total_recommended_gaps: clarification_response.total_recommended_gaps,
    };

    Ok(response::ok(response_data))
}

/// POST /media-generations/confirm
///
/// Submits the enriched prompt for generation. Creates (or reuses) a media
/// generation, enqueues it for async processing, and returns 202 Accepted.
#[utoipa::path(
    post,
    path = "/api/v1/media-generations/confirm",
    tag = "media-generations",
    summary = "Submit enriched prompt for generation",
    description = "Creates a media generation with the enriched prompt from clarification, \
        enqueues it for async processing, and returns 202 Accepted with a job_id and poll_url.",
    request_body = ConfirmRequest,
    responses(
        (status = 202, description = "Generation accepted and enqueued", body = ConfirmResponseData),
        (status = 401, description = "Missing or invalid authentication token"),
        (status = 403, description = "Authenticated user is not a teacher"),
        (status = 422, description = "Validation error"),
    ),
    security(
        ("bearer_auth" = [])
    ),
)]
pub async fn confirm(
    State(state): State<AppState>,
    principal: Principal,
    Json(payload): Json<ConfirmRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_teacher(&principal)?;

    // Validate input
    if payload.enriched_prompt.trim().is_empty() {
        return Err(AppError::Validation(
            "enriched_prompt tidak boleh kosong".to_string(),
        ));
    }

    let generation_id = Uuid::parse_str(&payload.generation_id)
        .map_err(|e| AppError::Validation(format!("generation_id tidak valid: {}", e)))?;

    let repo = PgMediaGenerationsRepo::new(state.db_pool.clone());

    // Check if the generation exists (should have been created by preflight)
    let existing = repo
        .find_by_id_for_teacher(generation_id, principal.user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let gen_id = if let Some(gen) = existing {
        let g = &gen.generation;

        // If already in a terminal state or already processing, return existing
        if g.status == "completed" || g.status == "failed" || g.status == "cancelled" {
            let job_id = g.generation_job_id.unwrap_or_else(Uuid::new_v4);
            let response_data = ConfirmResponseData {
                generation_id: g.id.to_string(),
                job_id: job_id.to_string(),
                status: g.status.clone(),
                poll_url: format!("/api/v1/media-generations/{}/job-status", g.id),
            };
            return Ok(response::accepted_with_message(
                "Generasi media sudah dalam proses.",
                response_data,
            ));
        }

        g.id
    } else {
        // Generation not found — create a new one via SubmissionService
        let service = SubmissionService::new(state.db_pool.clone());

        let input = CreateInput {
            teacher_id: principal.user_id,
            raw_prompt: payload.enriched_prompt.clone(),
            preferred_output_type: payload
                .answers
                .get("output_type")
                .cloned()
                .or_else(|| Some("auto".to_string())),
            subject_id: payload.subject_id,
            sub_subject_id: payload.sub_subject_id,
            provider_metadata: ProviderMetadata::default(),
        };

        let result = service
            .create_or_reuse(input)
            .await
            .map_err(|e| AppError::Internal(format!("failed to create media generation: {e}")))?;

        result.id
    };

    // ── Reset generation for reprocessing via the full LLM workflow ─────
    // Instead of hardcoding interpretation/spec payloads and submitting directly
    // to Python (which bypassed the LLM and defaulted to "docx"), we now:
    // 1. Update raw_prompt to the enriched prompt
    // 2. Clear all classification payloads (forces the worker to re-classify)
    // 3. Reset status to 'queued'
    // 4. Enqueue to Redis stream for the worker to run the full LLM pipeline
    repo.reset_for_reprocessing(gen_id, &payload.enriched_prompt)
        .await
        .map_err(|e| AppError::Internal(format!("failed to reset generation for reprocessing: {e}")))?;

    // Save clarification state (Phase 2)
    let clarification_state = serde_json::json!({
        "answers": payload.answers,
        "suggested_prompt": payload.enriched_prompt,
        "generation_id": payload.generation_id,
    });
    repo.update_clarification_state(
        gen_id,
        &UpdateClarificationStatePayload {
            clarification_state: Some(clarification_state),
            clarified_at: Some(chrono::Utc::now()),
            clarification_skipped: false,
        },
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to update clarification state: {e}")))?;

    // Generate a new job_id and enqueue to Redis stream
    let job_id = Uuid::new_v4();

    // Update DB with job_id and status='pending'
    repo.update_generation_job_status(
        gen_id,
        &UpdateGenerationJobStatusPayload {
            generation_job_id: Some(job_id),
            generation_status: "pending".to_string(),
        },
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to set generation job status: {e}")))?;

    // Enqueue to Redis stream for the worker to run the full LLM workflow
    if let Some(ref redis_pool) = state.redis_pool {
        let queue = crate::queue::redis_streams::QueueService::new(redis_pool.clone(), 1);
        if let Err(e) = queue
            .enqueue(&gen_id.to_string(), &job_id.to_string(), 1)
            .await
        {
            tracing::warn!(
                error = %e,
                generation_id = %gen_id,
                job_id = %job_id,
                "failed to enqueue confirm job to Redis stream"
            );
        } else {
            tracing::info!(
                generation_id = %gen_id,
                job_id = %job_id,
                "enqueued confirm job to Redis stream for LLM workflow"
            );
        }
    } else {
        tracing::warn!(
            generation_id = %gen_id,
            job_id = %job_id,
            "Redis not configured — worker pipeline unavailable for confirm"
        );
    }

    let response_data = ConfirmResponseData {
        generation_id: gen_id.to_string(),
        job_id: job_id.to_string(),
        status: "pending".to_string(),
        poll_url: format!("/api/v1/media-generations/{}/job-status", gen_id),
    };

    Ok(response::accepted_with_message(
        "Generasi media berhasil dibuat dan sedang diproses.",
        response_data,
    ))
}

/// POST /media-generations/{id}/skip-clarification
///
/// Skips all clarification questions and generates with the enriched prompt.
/// Resets the generation for reprocessing via the full LLM workflow pipeline
/// (interpret → decide → draft → generate), ensuring the output format is
/// properly determined by the LLM interpretation + DecisionService rather than
/// hardcoded defaults.
#[utoipa::path(
    post,
    path = "/api/v1/media-generations/{id}/skip-clarification",
    tag = "media-generations",
    summary = "Skip clarification and generate with enriched prompt",
    description = "Skips all clarification questions and generates content using the \
        auto-enriched prompt via the full LLM workflow pipeline.",
    params(
        ("id" = Uuid, Path, description = "Generation ID from preflight"),
    ),
    responses(
        (status = 202, description = "Generation accepted and enqueued", body = ConfirmResponseData),
        (status = 401, description = "Missing or invalid authentication token"),
        (status = 403, description = "Authenticated user is not a teacher"),
        (status = 404, description = "Generation not found"),
    ),
    security(
        ("bearer_auth" = [])
    ),
)]
pub async fn skip_clarification(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_teacher(&principal)?;

    let repo = PgMediaGenerationsRepo::new(state.db_pool.clone());

    // Fetch the generation (should have been created by preflight)
    let generation = repo
        .find_by_id_for_teacher(id, principal.user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("media generation not found".into()))?;

    let gen = &generation.generation;

    // If already in a terminal state or already processing, return existing
    if gen.status == "completed" || gen.status == "failed" || gen.status == "cancelled" {
        let job_id = gen.generation_job_id.unwrap_or_else(Uuid::new_v4);
        let response_data = ConfirmResponseData {
            generation_id: gen.id.to_string(),
            job_id: job_id.to_string(),
            status: gen.status.clone(),
            poll_url: format!("/api/v1/media-generations/{}/job-status", gen.id),
        };
        return Ok(response::accepted_with_message(
            "Generasi media sudah dalam proses.",
            response_data,
        ));
    }

    // Build enriched prompt from original prompt (auto-detected values)
    let enriched_prompt = ClarificationService::enrich_prompt(&gen.raw_prompt, &Default::default());

    // ── Reset generation for reprocessing via the full LLM workflow ─────
    // Instead of hardcoding interpretation/spec payloads and submitting directly
    // to Python (which bypassed the LLM and defaulted to "docx"), we now:
    // 1. Update raw_prompt to the enriched prompt
    // 2. Clear all classification payloads (forces the worker to re-classify)
    // 3. Reset status to 'queued'
    // 4. Enqueue to Redis stream for the worker to run the full LLM pipeline
    repo.reset_for_reprocessing(id, &enriched_prompt)
        .await
        .map_err(|e| AppError::Internal(format!("failed to reset generation for reprocessing: {e}")))?;

    // Save clarification state as skipped (Phase 2)
    let clarification_state = serde_json::json!({
        "answers": {},
        "suggested_prompt": enriched_prompt,
        "skipped": true,
    });
    repo.update_clarification_state(
        id,
        &UpdateClarificationStatePayload {
            clarification_state: Some(clarification_state),
            clarified_at: Some(chrono::Utc::now()),
            clarification_skipped: true,
        },
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to update clarification state: {e}")))?;

    // Generate a new job_id and enqueue to Redis stream
    let job_id = Uuid::new_v4();

    // Update DB with job_id and status='pending'
    repo.update_generation_job_status(
        id,
        &UpdateGenerationJobStatusPayload {
            generation_job_id: Some(job_id),
            generation_status: "pending".to_string(),
        },
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to set generation job status: {e}")))?;

    // Enqueue to Redis stream for the worker to run the full LLM workflow
    if let Some(ref redis_pool) = state.redis_pool {
        let queue = crate::queue::redis_streams::QueueService::new(redis_pool.clone(), 1);
        if let Err(e) = queue
            .enqueue(&id.to_string(), &job_id.to_string(), 1)
            .await
        {
            tracing::warn!(
                error = %e,
                generation_id = %id,
                job_id = %job_id,
                "failed to enqueue skip-clarification job to Redis stream"
            );
        } else {
            tracing::info!(
                generation_id = %id,
                job_id = %job_id,
                "enqueued skip-clarification job to Redis stream for LLM workflow"
            );
        }
    } else {
        tracing::warn!(
            generation_id = %id,
            job_id = %job_id,
            "Redis not configured — worker pipeline unavailable for skip-clarification"
        );
    }

    let response_data = ConfirmResponseData {
        generation_id: id.to_string(),
        job_id: job_id.to_string(),
        status: "pending".to_string(),
        poll_url: format!("/api/v1/media-generations/{}/job-status", id),
    };

    Ok(response::accepted_with_message(
        "Generasi media berhasil dibuat (clarifikasi dilewati).",
        response_data,
    ))
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Resolve subject name from subject_id using a DB query.
async fn resolve_subject_name(state: &AppState, subject_id: i64) -> Option<String> {
    let result = sqlx::query_scalar::<_, String>("SELECT name FROM subjects WHERE id = $1")
        .bind(subject_id)
        .fetch_optional(&state.db_pool)
        .await;

    match result {
        Ok(name) => name,
        Err(e) => {
            tracing::warn!(error = %e, subject_id = subject_id, "failed to resolve subject name");
            None
        }
    }
}

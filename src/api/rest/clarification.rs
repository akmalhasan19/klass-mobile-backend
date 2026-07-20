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
    UpdateGenerationJobStatusPayload, UpdatePayloadsPayload,
};
use crate::error::{AppError, AppResult};
use crate::llm::clarification::{PreflightInput, ClarificationService};
use crate::media_gen::python_client::PythonMediaGeneratorClient;
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

    let _generation_id = Uuid::parse_str(&payload.generation_id)
        .map_err(|e| AppError::Validation(format!("generation_id tidak valid: {}", e)))?;

    // Use the submission service to create or reuse a generation
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

    let repo = PgMediaGenerationsRepo::new(state.db_pool.clone());

    if result.was_created {
        let job_id = Uuid::new_v4();

        // Update DB with job_id and status='pending'
        repo.update_generation_job_status(
            result.id,
            &UpdateGenerationJobStatusPayload {
                generation_job_id: Some(job_id),
                generation_status: "pending".to_string(),
            },
        )
        .await
        .map_err(|e| AppError::Internal(format!("failed to set generation job status: {e}")))?;

        // Determine output type for the generation spec
        let output_type_raw = payload
            .answers
            .get("output_type")
            .map(|s| s.as_str())
            .unwrap_or("docx");
        let output_type = match output_type_raw {
            "docx" | "pdf" | "pptx" => output_type_raw,
            "handout" => "docx",
            "worksheet" => "docx",
            "slide" | "slides" | "presentation" => "pptx",
            _ => "docx",
        };

        // Build interpretation payload (simplified for clarification flow)
        let interpretation_payload = serde_json::json!({
            "schema_version": crate::contracts::prompt_interpretation::SCHEMA_VERSION,
            "teacher_prompt": payload.enriched_prompt,
            "language": "id",
            "teacher_intent": {
                "type": "content_generation",
                "goal": payload.enriched_prompt,
                "preferred_delivery_mode": "async",
                "requires_clarification": false,
            },
            "learning_objectives": [],
            "constraints": {
                "preferred_output_type": output_type,
            },
            "output_type_candidates": [{
                "type": output_type,
                "score": 1.0,
                "reason": "Teacher confirmed via clarification.",
            }],
            "resolved_output_type_reasoning": "Teacher selected via clarification flow.",
            "document_blueprint": {
                "title": payload.enriched_prompt,
                "summary": payload.enriched_prompt,
                "sections": [{
                    "title": "Requested Content",
                    "purpose": "Deliver the requested learning material.",
                    "bullets": [],
                    "estimated_length": "standard",
                }],
            },
            "teacher_delivery_summary": "Delivered via async generation after clarification.",
            "confidence": {
                "score": 1.0,
                "label": "high",
            },
        });

        // Build generation spec
        let unit_type = if output_type == "pptx" { "slide" } else { "page" };
        let generation_spec_payload = serde_json::json!({
            "schema_version": "media_generation_spec.v1",
            "source_interpretation_schema_version": crate::contracts::prompt_interpretation::SCHEMA_VERSION,
            "export_format": output_type,
            "title": payload.enriched_prompt,
            "language": "id",
            "summary": payload.enriched_prompt,
            "learning_objectives": [],
            "sections": [{
                "title": "Requested Content",
                "purpose": "Deliver the requested learning material.",
                "body_blocks": [{
                    "type": "paragraph",
                    "content": payload.enriched_prompt,
                }],
                "emphasis": "short",
            }],
            "layout_hints": {
                "document_mode": if output_type == "pptx" { "slide_deck" } else { "document" },
                "visual_density": "medium",
                "section_count": 1,
                "asset_count": 0,
                "assessment_block_count": 0,
            },
            "style_hints": {
                "tone": "educational",
                "audience_level": "general",
                "format_preferences": [output_type],
            },
            "page_or_slide_structure": {
                "unit_type": unit_type,
                "total_units": 1,
                "opening_unit": false,
                "section_units": 1,
                "closing_unit": false,
            },
            "content_context": {},
            "assets": [],
            "assessment_or_activity_blocks": [],
            "teacher_delivery_summary": "Delivered via async generation after clarification.",
            "contract_versions": {
                "generator_output_metadata": "media_generator_output_metadata.v1",
            },
        });

        // Store payloads
        repo.update_payloads(
            result.id,
            &UpdatePayloadsPayload {
                interpretation_payload: Some(interpretation_payload),
                generation_spec_payload: Some(generation_spec_payload),
                resolved_output_type: Some(output_type.to_string()),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| AppError::Internal(format!("failed to update payloads: {e}")))?;

        // Save clarification state (Phase 2)
        let clarification_state = serde_json::json!({
            "answers": payload.answers,
            "suggested_prompt": payload.enriched_prompt,
            "generation_id": payload.generation_id,
        });
        repo.update_clarification_state(
            result.id,
            &UpdateClarificationStatePayload {
                clarification_state: Some(clarification_state),
                clarified_at: Some(chrono::Utc::now()),
                clarification_skipped: false,
            },
        )
        .await
        .map_err(|e| AppError::Internal(format!("failed to update clarification state: {e}")))?;

        // Submit to Python service (fire-and-forget)
        let python_client = PythonMediaGeneratorClient::new(
            state.db_pool.clone(),
            state.http.clone(),
            &state.config,
        );
        if let Err(e) = python_client
            .submit_job(&result.id.to_string(), &job_id.to_string())
            .await
        {
            tracing::warn!(
                error = %e,
                generation_id = %result.id,
                job_id = %job_id,
                "failed to submit job to Python service"
            );
        }

        let response_data = ConfirmResponseData {
            generation_id: result.id.to_string(),
            job_id: job_id.to_string(),
            status: "pending".to_string(),
            poll_url: format!("/api/v1/media-generations/{}/job-status", result.id),
        };

        return Ok(response::accepted_with_message(
            "Generasi media berhasil dibuat dan sedang diproses.",
            response_data,
        ));
    }

    // Existing generation (duplicate) → return existing data
    let generation = repo
        .find_by_id_for_teacher(result.id, principal.user_id)
        .await
        .map_err(|e| AppError::Internal(format!("failed to fetch created generation: {e}")))?
        .ok_or_else(|| AppError::Internal("created generation not found".into()))?;

    let job_id = generation.generation.generation_job_id.unwrap_or_else(Uuid::new_v4);

    let response_data = ConfirmResponseData {
        generation_id: result.id.to_string(),
        job_id: job_id.to_string(),
        status: generation.generation.status.clone(),
        poll_url: format!("/api/v1/media-generations/{}/job-status", result.id),
    };

    Ok(response::accepted_with_message(
        "Generasi media berhasil dibuat.",
        response_data,
    ))
}

/// POST /media-generations/{id}/skip-clarification
///
/// Skips all clarification questions and generates with the enriched prompt.
/// The enriched prompt is built from the original prompt + auto-detected values.
#[utoipa::path(
    post,
    path = "/api/v1/media-generations/{id}/skip-clarification",
    tag = "media-generations",
    summary = "Skip clarification and generate with enriched prompt",
    description = "Skips all clarification questions and generates content using the \
        auto-enriched prompt (original prompt + auto-detected values).",
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

    // Fetch the generation (should have been created by preflight or confirm)
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

    // Determine output type
    let output_type_raw = gen.preferred_output_type.as_str();
    let output_type = match output_type_raw {
        "docx" | "pdf" | "pptx" => output_type_raw,
        "handout" => "docx",
        "worksheet" => "docx",
        "slide" | "slides" | "presentation" => "pptx",
        _ => "docx",
    };

    // Build interpretation payload
    let interpretation_payload = serde_json::json!({
        "schema_version": crate::contracts::prompt_interpretation::SCHEMA_VERSION,
        "teacher_prompt": enriched_prompt,
        "language": "id",
        "teacher_intent": {
            "type": "content_generation",
            "goal": enriched_prompt,
            "preferred_delivery_mode": "async",
            "requires_clarification": false,
        },
        "learning_objectives": [],
        "constraints": {
            "preferred_output_type": output_type,
        },
        "output_type_candidates": [{
            "type": output_type,
            "score": 1.0,
            "reason": "Teacher skipped clarification.",
        }],
        "resolved_output_type_reasoning": "Teacher skipped clarification, using auto-enriched prompt.",
        "document_blueprint": {
            "title": enriched_prompt,
            "summary": enriched_prompt,
            "sections": [{
                "title": "Requested Content",
                "purpose": "Deliver the requested learning material.",
                "bullets": [],
                "estimated_length": "standard",
            }],
        },
        "teacher_delivery_summary": "Delivered via async generation (clarification skipped).",
        "confidence": {
            "score": 1.0,
            "label": "high",
        },
    });

    // Build generation spec
    let unit_type = if output_type == "pptx" { "slide" } else { "page" };
    let generation_spec_payload = serde_json::json!({
        "schema_version": "media_generation_spec.v1",
        "source_interpretation_schema_version": crate::contracts::prompt_interpretation::SCHEMA_VERSION,
        "export_format": output_type,
        "title": enriched_prompt,
        "language": "id",
        "summary": enriched_prompt,
        "learning_objectives": [],
        "sections": [{
            "title": "Requested Content",
            "purpose": "Deliver the requested learning material.",
            "body_blocks": [{
                "type": "paragraph",
                "content": enriched_prompt,
            }],
            "emphasis": "short",
        }],
        "layout_hints": {
            "document_mode": if output_type == "pptx" { "slide_deck" } else { "document" },
            "visual_density": "medium",
            "section_count": 1,
            "asset_count": 0,
            "assessment_block_count": 0,
        },
        "style_hints": {
            "tone": "educational",
            "audience_level": "general",
            "format_preferences": [output_type],
        },
        "page_or_slide_structure": {
            "unit_type": unit_type,
            "total_units": 1,
            "opening_unit": false,
            "section_units": 1,
            "closing_unit": false,
        },
        "content_context": {},
        "assets": [],
        "assessment_or_activity_blocks": [],
        "teacher_delivery_summary": "Delivered via async generation (clarification skipped).",
        "contract_versions": {
            "generator_output_metadata": "media_generator_output_metadata.v1",
        },
    });

    // Update payloads
    repo.update_payloads(
        id,
        &UpdatePayloadsPayload {
            interpretation_payload: Some(interpretation_payload),
            generation_spec_payload: Some(generation_spec_payload),
            resolved_output_type: Some(output_type.to_string()),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to update payloads: {e}")))?;

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

    // If no job_id yet, create one and submit
    let job_id = gen.generation_job_id.unwrap_or_else(|| {
        let new_job_id = Uuid::new_v4();
        // Fire-and-forget update
        let repo = PgMediaGenerationsRepo::new(state.db_pool.clone());
        let pool = state.db_pool.clone();
        let http = state.http.clone();
        let config = state.config.clone();
        let gen_id = id;
        let jid = new_job_id;

        tokio::spawn(async move {
            let _ = repo
                .update_generation_job_status(
                    gen_id,
                    &UpdateGenerationJobStatusPayload {
                        generation_job_id: Some(jid),
                        generation_status: "pending".to_string(),
                    },
                )
                .await;

            let python_client = PythonMediaGeneratorClient::new(pool, http, &config);
            if let Err(e) = python_client
                .submit_job(&gen_id.to_string(), &jid.to_string())
                .await
            {
                tracing::warn!(
                    error = %e,
                    generation_id = %gen_id,
                    job_id = %jid,
                    "failed to submit skip-clarification job to Python service"
                );
            }
        });

        new_job_id
    });

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

//! Adapters bridging the `InterpretStep` / `DraftStep` / `ComposeStep`
//! workflow traits to the fully-implemented services.
//!
//! These adapters are injected into `WorkflowService::process()` by the
//! worker so that the full LLM pipeline is executed via real services
//! instead of the hardcoded shortcut that previously bypassed classification.

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::llm::draft::{DraftError, DraftInput, DraftService};
use crate::llm::interpret::{InterpretError, InterpretInput, InterpretService};
use crate::llm::respond::{RespondError, RespondInput, RespondArtifact, RespondPublication, RespondService, EntityNode};
use crate::orchestrator::workflow::{ComposeStep, DraftStep, InterpretStep, WorkflowError};

// ─── InterpretStepAdapter ───────────────────────────────────────────────────

/// Implements `InterpretStep` by reading the generation row from the DB,
/// calling `InterpretService` (OpenRouter LLM), and persisting the result.
pub struct InterpretStepAdapter {
    pool: PgPool,
    interpret_service: InterpretService,
}

impl InterpretStepAdapter {
    pub fn new(pool: PgPool, interpret_service: InterpretService) -> Self {
        Self {
            pool,
            interpret_service,
        }
    }
}

#[async_trait]
impl InterpretStep for InterpretStepAdapter {
    async fn interpret(&self, generation_id: &str) -> Result<serde_json::Value, WorkflowError> {
        let gen_uuid = Uuid::parse_str(generation_id)
            .map_err(|e| WorkflowError::InvalidUuid(e.to_string()))?;

        // ── 1. Read generation from DB ────────────────────────────────────
        let row: (String, String, Option<i64>, Option<i64>) = sqlx::query_as(
            r#"
            SELECT raw_prompt, COALESCE(preferred_output_type, 'auto'), subject_id, sub_subject_id
            FROM media_generations
            WHERE id = $1
            "#,
        )
        .bind(gen_uuid)
        .fetch_one(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        let (raw_prompt, preferred_output_type, subject_id, sub_subject_id) = row;

        // ── 2. Resolve optional subject / sub_subject context ─────────────
        let subject_context = match subject_id {
            Some(sid) => {
                let ctx: Option<(String, Option<String>)> = sqlx::query_as(
                    "SELECT name, slug FROM subjects WHERE id = $1",
                )
                .bind(sid)
                .fetch_optional(&self.pool)
                .await
                .map_err(WorkflowError::Database)?;

                ctx.map(|(name, slug)| crate::llm::interpret::NamedContext {
                    id: sid,
                    name,
                    slug,
                })
            }
            None => None,
        };

        let sub_subject_context = match sub_subject_id {
            Some(sid) => {
                let ctx: Option<(String, Option<String>)> = sqlx::query_as(
                    "SELECT name, slug FROM sub_subjects WHERE id = $1",
                )
                .bind(sid)
                .fetch_optional(&self.pool)
                .await
                .map_err(WorkflowError::Database)?;

                ctx.map(|(name, slug)| crate::llm::interpret::NamedContext {
                    id: sid,
                    name,
                    slug,
                })
            }
            None => None,
        };

        // ── 3. Call InterpretService ──────────────────────────────────────
        let input = InterpretInput {
            generation_id: generation_id.to_string(),
            teacher_prompt: raw_prompt,
            preferred_output_type,
            subject_context,
            sub_subject_context,
            model: None,    // use default from config
            instruction: None, // use default
        };

        let result = self
            .interpret_service
            .interpret(input)
            .await
            .map_err(|e| match e {
                InterpretError::RateLimited { .. } => WorkflowError::StepProvider(format!(
                    "interpret: rate limited by governance — {}",
                    e
                )),
                InterpretError::Provider(pe) => {
                    WorkflowError::StepProvider(format!("interpret: provider error — {}", pe))
                }
                InterpretError::Cache(ce) => {
                    WorkflowError::StepProvider(format!("interpret: cache error — {}", ce))
                }
                InterpretError::Governance(ge) => {
                    WorkflowError::StepProvider(format!("interpret: governance error — {}", ge))
                }
                InterpretError::Contract(ce) => {
                    WorkflowError::StepProvider(format!("interpret: contract error — {}", ce))
                }
            })?;

        // ── 4. Persist interpretation_payload + provider metadata ─────────
        let payload_value = serde_json::to_value(&result.interpretation_payload)
            .map_err(|e| WorkflowError::StepProvider(format!("serialize interpretation: {}", e)))?;

        sqlx::query(
            r#"
            UPDATE media_generations
            SET interpretation_payload = $1,
                llm_provider = $2,
                llm_model   = $3,
                updated_at  = NOW()
            WHERE id = $4
            "#,
        )
        .bind(&payload_value)
        .bind(&result.llm_provider)
        .bind(&result.llm_model)
        .bind(gen_uuid)
        .execute(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        tracing::info!(
            generation_id = %generation_id,
            provider = %result.llm_provider,
            model = %result.llm_model,
            cache_hit = result.cache_hit,
            fallback_used = result.fallback_used,
            latency_ms = ?result.latency_ms,
            "interpret: completed"
        );

        Ok(payload_value)
    }
}

// ─── DraftStepAdapter ───────────────────────────────────────────────────────

/// Implements `DraftStep` by reading the interpretation + decision from the
/// DB, calling `DraftService` (OpenRouter LLM), and persisting the result.
pub struct DraftStepAdapter {
    pool: PgPool,
    draft_service: DraftService,
}

impl DraftStepAdapter {
    pub fn new(pool: PgPool, draft_service: DraftService) -> Self {
        Self {
            pool,
            draft_service,
        }
    }
}

#[async_trait]
impl DraftStep for DraftStepAdapter {
    async fn draft(&self, generation_id: &str) -> Result<serde_json::Value, WorkflowError> {
        let gen_uuid = Uuid::parse_str(generation_id)
            .map_err(|e| WorkflowError::InvalidUuid(e.to_string()))?;

        // ── 1. Read interpretation_payload + resolved_output_type ─────────
        let row: (Option<serde_json::Value>, Option<String>) = sqlx::query_as(
            r#"
            SELECT interpretation_payload, resolved_output_type
            FROM media_generations
            WHERE id = $1
            "#,
        )
        .bind(gen_uuid)
        .fetch_one(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        let interpretation_value = row.0.ok_or_else(|| WorkflowError::StepFailed {
            step: "draft",
            message: "interpretation_payload is NULL — interpret step must run first".into(),
            source: None,
        })?;

        let resolved_output_type = row.1.unwrap_or_else(|| "pdf".to_string());

        // ── 2. Deserialize InterpretationPayload ──────────────────────────
        let interpretation: crate::contracts::prompt_interpretation::InterpretationPayload =
            serde_json::from_value(interpretation_value).map_err(|e| WorkflowError::StepFailed {
                step: "draft",
                message: format!("failed to deserialize interpretation_payload: {}", e),
                source: None,
            })?;

        // ── 3. Call DraftService ──────────────────────────────────────────
        let input = DraftInput {
            generation_id: generation_id.to_string(),
            interpretation,
            resolved_output_type,
            taxonomy_hint: None,
            model: None,
            instruction: None,
        };

        let result = self
            .draft_service
            .draft(input)
            .await
            .map_err(|e| match e {
                DraftError::RateLimited { .. } => WorkflowError::StepProvider(format!(
                    "draft: rate limited by governance — {}",
                    e
                )),
                DraftError::Provider(pe) => {
                    WorkflowError::StepProvider(format!("draft: provider error — {}", pe))
                }
                DraftError::Cache(ce) => {
                    WorkflowError::StepProvider(format!("draft: cache error — {}", ce))
                }
                DraftError::Governance(ge) => {
                    WorkflowError::StepProvider(format!("draft: governance error — {}", ge))
                }
                DraftError::Contract(ce) => {
                    WorkflowError::StepProvider(format!("draft: contract error — {}", ce))
                }
            })?;

        // ── 4. Persist draft_payload ─────────────────────────────────────
        let draft_value = serde_json::to_value(&result.draft_payload)
            .map_err(|e| WorkflowError::StepProvider(format!("serialize draft: {}", e)))?;

        // Wrap in the envelope the generation_spec_builder expects
        let draft_envelope = serde_json::json!({
            "payload": draft_value,
            "source": result.source,
            "adapter_metadata": result.adapter_metadata,
        });

        sqlx::query(
            r#"
            UPDATE media_generations
            SET generation_spec_payload = COALESCE(generation_spec_payload, $1),
                updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(&draft_envelope)
        .bind(gen_uuid)
        .execute(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        tracing::info!(
            generation_id = %generation_id,
            source = %result.source,
            latency_ms = ?result.latency_ms,
            "draft: completed"
        );

        Ok(draft_value)
    }
}

// ─── ComposeStepAdapter ──────────────────────────────────────────────────────

/// Implements `ComposeStep` by reading the generation + publication data
/// from the DB, calling `RespondService` (OpenRouter LLM), and persisting
/// the delivery_payload.
pub struct ComposeStepAdapter {
    pool: PgPool,
    respond_service: RespondService,
}

impl ComposeStepAdapter {
    pub fn new(pool: PgPool, respond_service: RespondService) -> Self {
        Self {
            pool,
            respond_service,
        }
    }
}

#[async_trait]
impl ComposeStep for ComposeStepAdapter {
    async fn compose(&self, generation_id: &str) -> Result<serde_json::Value, WorkflowError> {
        let gen_uuid = Uuid::parse_str(generation_id)
            .map_err(|e| WorkflowError::InvalidUuid(e.to_string()))?;

        // ── 1. Read generation + publication data from DB ─────────────────
        let row: (
            Option<String>,   // title (from interpretation or draft)
            Option<String>,   // resolved_output_type
            Option<String>,   // file_url
            Option<String>,   // mime_type
            Option<String>,   // thumbnail_url
            Option<String>,   // filename
            Option<String>,   // preview summary (from draft)
            Option<i64>,      // topic_id
            Option<i64>,      // content_id
            Option<i64>,      // recommended_project_id
        ) = sqlx::query_as(
            r#"
            SELECT
                interpretation_payload->'document_blueprint'->>'title',
                resolved_output_type,
                file_url,
                mime_type,
                thumbnail_url,
                filename,
                interpretation_payload->'document_blueprint'->>'summary',
                topic_id,
                content_id,
                recommended_project_id
            FROM media_generations
            WHERE id = $1
            "#,
        )
        .bind(gen_uuid)
        .fetch_one(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        let title = row.0.unwrap_or_else(|| "Untitled".to_string());
        let resolved_output_type = row.1.unwrap_or_else(|| "pdf".to_string());
        let file_url = row.2.unwrap_or_default();
        let mime_type = row.3.unwrap_or_default();
        let thumbnail_url = row.4;
        let filename = row.5;
        let preview_summary = row.6.unwrap_or_default();
        let topic_id = row.7;
        let content_id = row.8;
        let recommended_project_id = row.9;

        // ── 2. Resolve publication entities ───────────────────────────────
        let topic = match topic_id {
            Some(tid) => {
                let t: Option<(i64, String)> = sqlx::query_as(
                    "SELECT id, title FROM topics WHERE id = $1",
                )
                .bind(tid)
                .fetch_optional(&self.pool)
                .await
                .map_err(WorkflowError::Database)?;
                t.map(|(id, title)| EntityNode {
                    id: id.to_string(),
                    title,
                })
            }
            None => None,
        };

        let content = match content_id {
            Some(cid) => {
                let c: Option<(i64, String)> = sqlx::query_as(
                    "SELECT id, title FROM contents WHERE id = $1",
                )
                .bind(cid)
                .fetch_optional(&self.pool)
                .await
                .map_err(WorkflowError::Database)?;
                c.map(|(id, title)| EntityNode {
                    id: id.to_string(),
                    title,
                })
            }
            None => None,
        };

        let recommended_project = match recommended_project_id {
            Some(rpid) => {
                let r: Option<(i64, String)> = sqlx::query_as(
                    "SELECT id, title FROM recommended_projects WHERE id = $1",
                )
                .bind(rpid)
                .fetch_optional(&self.pool)
                .await
                .map_err(WorkflowError::Database)?;
                r.map(|(id, title)| EntityNode {
                    id: id.to_string(),
                    title,
                })
            }
            None => None,
        };

        // ── 3. Call RespondService ────────────────────────────────────────
        let input = RespondInput {
            generation_id: generation_id.to_string(),
            title,
            preview_summary,
            artifact: RespondArtifact {
                output_type: resolved_output_type,
                file_url,
                mime_type,
                thumbnail_url,
                filename,
            },
            publication: RespondPublication {
                topic,
                content,
                recommended_project,
            },
            model: None,
            instruction: None,
        };

        let result = self
            .respond_service
            .respond(input)
            .await
            .map_err(|e| match e {
                RespondError::RateLimited { .. } => WorkflowError::StepProvider(format!(
                    "compose: rate limited by governance — {}",
                    e
                )),
                RespondError::Provider(pe) => {
                    WorkflowError::StepProvider(format!("compose: provider error — {}", pe))
                }
                RespondError::Cache(ce) => {
                    WorkflowError::StepProvider(format!("compose: cache error — {}", ce))
                }
                RespondError::Governance(ge) => {
                    WorkflowError::StepProvider(format!("compose: governance error — {}", ge))
                }
                RespondError::Contract(ce) => {
                    WorkflowError::StepProvider(format!("compose: contract error — {}", ce))
                }
            })?;

        // ── 4. Persist delivery_payload ───────────────────────────────────
        let delivery_value = serde_json::to_value(&result.delivery_payload)
            .map_err(|e| WorkflowError::StepProvider(format!("serialize delivery: {}", e)))?;

        sqlx::query(
            r#"
            UPDATE media_generations
            SET delivery_payload = $1,
                updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(&delivery_value)
        .bind(gen_uuid)
        .execute(&self.pool)
        .await
        .map_err(WorkflowError::Database)?;

        tracing::info!(
            generation_id = %generation_id,
            source = %result.source,
            llm_used = result.llm_used,
            latency_ms = ?result.latency_ms,
            "compose: completed"
        );

        Ok(delivery_value)
    }
}

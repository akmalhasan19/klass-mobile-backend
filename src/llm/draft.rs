//! Media content drafting service.
//!
//! Port of `ContentDraftWorkflowService` from the Python adapter
//! (`llm-adapter-service/app/draft.py`).
//!
//! The second stage of the LLM pipeline after interpretation:
//!
//! 1. Validate that the interpretation payload is present
//! 2. Preflight governance check (rate-limit, cost budget)
//! 3. Cache lookup (semantic cache shared with respond route)
//! 4. Cache-hit → record cache hit in ledger, return cached draft
//! 5. Cache-miss → call OpenRouter with interpretation context
//! 6. Validate response via `content_draft::decode_and_validate`
//! 7. On validation failure → `content_draft::fallback_from_interpretation`
//! 8. Build audit payload → persist to ledger + governance buckets

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::cache::{build_cache_key, CacheRoute, LlmCacheRepo};
use crate::contracts::content_draft::{
    decode_and_validate, fallback_from_interpretation, ContentDraftPayload,
};
use crate::contracts::prompt_interpretation::InterpretationPayload;
use crate::governance::ledger::{CacheStatus, LedgerRepo};
use serde::{Deserialize, Serialize};
use crate::governance::price_catalog::PriceCatalogRepo;
use crate::governance::rate_limit::{
    preflight_check, ExhaustionAction, RateLimitBucketsRepo, RateLimitPoliciesRepo,
};
use crate::providers::{json_mode_request, ProviderError, ProviderRouter};

// ─── Constants ──────────────────────────────────────────────────────────────

/// Route identifier used in governance and ledger.
/// Draft shares the "respond" route with delivery (per Python reference).
pub const DRAFT_ROUTE: &str = "respond";

/// Request type used in ledger entries.
pub const DRAFT_REQUEST_TYPE: &str = "media_content_draft";

/// Cache route — shares with respond (matching Python reference).
pub const DRAFT_CACHE_ROUTE: CacheRoute = CacheRoute::Respond;

/// Default system instruction sent to the LLM for content drafting.
pub const DEFAULT_DRAFT_INSTRUCTION: &str = "\
You are a media generation assistant helping teachers create classroom material. \
Given the following interpretation of a teacher's request, generate the actual \
content for the document. Return a valid JSON object following the \
media_content_draft.v1 schema. Include sections with body blocks, learning \
objectives, and a teacher delivery summary. Write in the same language as the \
interpretation.";

// ─── Types ──────────────────────────────────────────────────────────────────

/// Input for the content drafting service.
///
/// Analogous to `ContentDraftRequest` + `ContentDraftRequestInput` in Python.
#[derive(Debug, Clone)]
pub struct DraftInput {
    /// Unique identifier for the generation this draft belongs to.
    pub generation_id: String,
    /// The validated interpretation payload from the interpret step.
    pub interpretation: InterpretationPayload,
    /// Resolved output type: "pdf", "docx", "pptx".
    pub resolved_output_type: String,
    /// Optional taxonomy hint for the LLM.
    pub taxonomy_hint: Option<DraftTaxonomyHint>,
    /// Provider model override. If `None`, uses the default from config.
    pub model: Option<String>,
    /// System instruction / prompt for the LLM. If `None`, uses the default.
    pub instruction: Option<String>,
}

/// Taxonomy hint passed to the draft LLM for richer content generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftTaxonomyHint {
    pub schema_version: String,
    pub source: String,
    pub subject_name: Option<String>,
    pub subject_slug: Option<String>,
    pub sub_subject_name: Option<String>,
    pub sub_subject_slug: Option<String>,
    pub jenjang: Option<String>,
    pub kelas: Option<i32>,
    pub semester: Option<i32>,
    pub bab: Option<i32>,
    pub description: Option<String>,
    pub structure: Option<String>,
    pub matched_signals: Vec<String>,
}

/// The result of a successful content draft.
#[derive(Debug, Clone)]
pub struct DraftResult {
    /// Unique request ID used for ledger and governance tracking.
    pub request_id: String,
    /// The generation ID this draft belongs to.
    pub generation_id: String,
    /// Validated content draft payload.
    pub draft_payload: ContentDraftPayload,
    /// Source of this result: "cache", "provider", or "fallback".
    pub source: String,
    /// Rich adapter metadata (provider, model, usage).
    pub adapter_metadata: serde_json::Value,
    /// Optional fallback error details.
    pub fallback_error: Option<String>,
    /// HTTP-styled response headers with provider metadata.
    pub response_headers: HashMap<String, String>,
    /// Request latency in milliseconds.
    pub latency_ms: Option<Decimal>,
}

// ─── DraftService ───────────────────────────────────────────────────────────

/// Orchestrates content drafting by combining cache, governance, provider,
/// and contract validation.
pub struct DraftService {
    cache_repo: LlmCacheRepo,
    ledger_repo: LedgerRepo,
    price_catalog_repo: PriceCatalogRepo,
    policies_repo: RateLimitPoliciesRepo,
    buckets_repo: RateLimitBucketsRepo,
    provider_router: Arc<ProviderRouter>,
    default_model: String,
    default_instruction: String,
}

impl DraftService {
    /// Create a new content drafting service with all dependencies.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cache_repo: LlmCacheRepo,
        ledger_repo: LedgerRepo,
        price_catalog_repo: PriceCatalogRepo,
        policies_repo: RateLimitPoliciesRepo,
        buckets_repo: RateLimitBucketsRepo,
        provider_router: Arc<ProviderRouter>,
        default_model: String,
        default_instruction: String,
    ) -> Self {
        Self {
            cache_repo,
            ledger_repo,
            price_catalog_repo,
            policies_repo,
            buckets_repo,
            provider_router,
            default_model,
            default_instruction,
        }
    }

    // ─── Public API ───────────────────────────────────────────────────────

    /// Run the full content drafting pipeline.
    pub async fn draft(&self, input: DraftInput) -> Result<DraftResult, DraftError> {
        let request_id = Uuid::new_v4().to_string();
        let start = Instant::now();

        let model = input
            .model
            .as_deref()
            .unwrap_or(&self.default_model)
            .to_string();
        let instruction = input
            .instruction
            .as_deref()
            .unwrap_or(&self.default_instruction)
            .to_string();
        let provider = "openrouter";

        // ── Step 1: Build cache key ───────────────────────────────────────
        let cache_input_payload = serde_json::json!({
            "resolved_output_type": &input.resolved_output_type,
            "interpretation_title": &input.interpretation.document_blueprint.title,
            "interpretation_summary": &input.interpretation.document_blueprint.summary,
            "teacher_prompt": &input.interpretation.teacher_prompt,
            "subject_context": input.interpretation.subject_context.as_ref().map(|c| serde_json::json!({
                "name": &c.subject_name, "slug": &c.subject_slug,
            })),
            "sub_subject_context": input.interpretation.sub_subject_context.as_ref().map(|c| serde_json::json!({
                "name": &c.sub_subject_name, "slug": &c.sub_subject_slug,
            })),
        });
        let cache_key = build_cache_key(
            DRAFT_CACHE_ROUTE,
            provider,
            &model,
            DRAFT_REQUEST_TYPE,
            &instruction,
            &cache_input_payload,
        );

        // ── Step 2: Preflight governance check ────────────────────────────
        let projected_cost = self
            .price_catalog_repo
            .estimate_cost(provider, &model, 500, 3000)
            .await;
        let decision = preflight_check(
            &self.policies_repo,
            &self.buckets_repo,
            DRAFT_ROUTE,
            provider,
            &model,
            Some(projected_cost.total_cost_usd),
            &request_id,
            &input.generation_id,
        )
        .await
        .map_err(DraftError::Governance)?;

        match decision.action {
            ExhaustionAction::Deny => {
                return Err(DraftError::RateLimited {
                    request_id,
                    generation_id: input.generation_id,
                });
            }
            ExhaustionAction::Degrade => {
                return self
                    .execute_provider(
                        &input,
                        request_id,
                        provider,
                        &model,
                        &instruction,
                        &cache_key,
                        start,
                        false,
                    )
                    .await;
            }
            ExhaustionAction::Allow => {}
        }

        // ── Step 3: Cache lookup ──────────────────────────────────────────
        let cached = self
            .cache_repo
            .lookup(DRAFT_CACHE_ROUTE, &cache_key)
            .await
            .map_err(DraftError::Cache)?;

        if let Some(entry) = cached {
            let payload: ContentDraftPayload =
                serde_json::from_value(entry.response_payload.clone()).map_err(|e| {
                    DraftError::Contract(
                        crate::contracts::prompt_interpretation::ContractValidationError {
                            code: "cached_payload_corrupt",
                            message: format!("Cached draft payload failed deserialization: {}", e),
                            details: serde_json::json!({"deserialize_error": e.to_string()}),
                            raw_completion: entry.response_payload.to_string(),
                        },
                    )
                })?;

            let elapsed = start.elapsed();
            let latency_ms = Decimal::new(elapsed.as_millis() as i64, 3);

            let _ = self
                .ledger_repo
                .record_completed(
                    &request_id,
                    &input.generation_id,
                    DRAFT_ROUTE,
                    DRAFT_REQUEST_TYPE,
                    provider,
                    provider,
                    &model,
                    &model,
                    Some(latency_ms),
                    0,
                    CacheStatus::Hit,
                    None,
                    None,
                    None,
                    Some(Decimal::ZERO),
                    false,
                    None,
                    vec![provider.to_string()],
                    Some(&cache_key),
                    serde_json::json!({
                        "source": "draft_service",
                        "cache_hit": true,
                        "cache_source": "content_draft_cache",
                    }),
                )
                .await;

            return Ok(DraftResult {
                request_id,
                generation_id: input.generation_id,
                draft_payload: payload,
                source: "cache".to_string(),
                adapter_metadata: serde_json::json!({
                    "provider": provider,
                    "model": model,
                    "cache_hit": true,
                }),
                fallback_error: None,
                response_headers: headers_for(provider, &model, true, false),
                latency_ms: Some(latency_ms),
            });
        }

        // ── Step 4: Cache miss – anti-stampede lock ──────────────────────
        let lock_acquired = self
            .cache_repo
            .try_acquire_lock(DRAFT_CACHE_ROUTE, &cache_key)
            .await
            .map_err(DraftError::Cache)?;

        if !lock_acquired {
            let waited = self
                .cache_repo
                .wait_for_entry(DRAFT_CACHE_ROUTE, &cache_key, None, None)
                .await
                .map_err(DraftError::Cache)?;

            if let Some(entry) = waited {
                let payload: ContentDraftPayload =
                    serde_json::from_value(entry.response_payload.clone()).map_err(|e| {
                        DraftError::Contract(
                            crate::contracts::prompt_interpretation::ContractValidationError {
                                code: "cached_payload_corrupt",
                                message: format!(
                                    "Cached draft payload (waited) failed: {}",
                                    e
                                ),
                                details: serde_json::json!({"deserialize_error": e.to_string()}),
                                raw_completion: entry.response_payload.to_string(),
                            },
                        )
                    })?;

                let elapsed = start.elapsed();
                let latency_ms = Decimal::new(elapsed.as_millis() as i64, 3);

                let _ = self
                    .ledger_repo
                    .record_completed(
                        &request_id,
                        &input.generation_id,
                        DRAFT_ROUTE,
                        DRAFT_REQUEST_TYPE,
                        provider,
                        provider,
                        &model,
                        &model,
                        Some(latency_ms),
                        0,
                        CacheStatus::Hit,
                        None,
                        None,
                        None,
                        Some(Decimal::ZERO),
                        false,
                        None,
                        vec![provider.to_string()],
                        Some(&cache_key),
                        serde_json::json!({
                            "source": "draft_service",
                            "cache_hit": true,
                            "cache_source": "content_draft_cache_inflight_wait",
                        }),
                    )
                    .await;

                return Ok(DraftResult {
                    request_id,
                    generation_id: input.generation_id,
                    draft_payload: payload,
                    source: "cache".to_string(),
                    adapter_metadata: serde_json::json!({
                        "provider": provider,
                        "model": model,
                        "cache_hit": true,
                    }),
                    fallback_error: None,
                    response_headers: headers_for(provider, &model, true, false),
                    latency_ms: Some(latency_ms),
                });
            }

            tracing::warn!(
                request_id = %request_id,
                cache_key = %cache_key,
                "draft: inflight wait timed out, attempting provider directly"
            );
        }

        // ── Step 5: Execute provider call ─────────────────────────────────
        let result = self
            .execute_provider(
                &input,
                request_id,
                provider,
                &model,
                &instruction,
                &cache_key,
                start,
                true,
            )
            .await;

        if lock_acquired {
            if let Err(e) = self
                .cache_repo
                .release_lock(DRAFT_CACHE_ROUTE, &cache_key)
                .await
            {
                tracing::warn!(error = %e, cache_key = %cache_key, "draft: failed to release advisory lock");
            }
        }

        result
    }

    // ─── Internal: execute provider call ─────────────────────────────────

    async fn execute_provider(
        &self,
        input: &DraftInput,
        request_id: String,
        provider: &str,
        model: &str,
        instruction: &str,
        cache_key: &str,
        start: Instant,
        should_cache: bool,
    ) -> Result<DraftResult, DraftError> {
        let elapsed = start.elapsed();
        let latency_ms = Decimal::new(elapsed.as_millis() as i64, 3);

        // Build the user payload from the interpretation
        let user_payload = serde_json::json!({
            "output_type": input.resolved_output_type,
            "title": input.interpretation.document_blueprint.title,
            "summary": input.interpretation.document_blueprint.summary,
            "learning_objectives": input.interpretation.learning_objectives,
            "sections": input.interpretation.document_blueprint.sections.iter().map(|s| serde_json::json!({
                "title": s.title,
                "purpose": s.purpose,
                "bullets": s.bullets,
                "estimated_length": s.estimated_length,
            })).collect::<Vec<_>>(),
            "language": input.interpretation.language,
            "teacher_intent_goal": input.interpretation.teacher_intent.goal,
            "constraints": serde_json::json!({
                "preferred_output_type": input.interpretation.constraints.preferred_output_type,
                "must_include": input.interpretation.constraints.must_include,
                "avoid": input.interpretation.constraints.avoid,
                "tone": input.interpretation.constraints.tone,
            }),
            "taxonomy_hint": input.taxonomy_hint.as_ref().map(|h| serde_json::json!({
                "subject_name": h.subject_name,
                "sub_subject_name": h.sub_subject_name,
                "jenjang": h.jenjang,
                "kelas": h.kelas,
                "structure": h.structure,
                "matched_signals": h.matched_signals,
            })),
        });

        let user_message = serde_json::to_string_pretty(&user_payload)
            .unwrap_or_else(|_| user_payload.to_string());
        let completion_request = json_mode_request(model, instruction, &user_message);

        let provider_result = self.provider_router.complete(completion_request).await;

        match provider_result {
            Ok(response) => {
                let raw_completion = response.first_choice_content().unwrap_or("").to_string();

                let (payload, fallback_used, fallback_error) =
                    match decode_and_validate(&raw_completion) {
                        Ok(p) => (p, false, None),
                        Err(validation_err) => {
                            tracing::warn!(
                                request_id = %request_id,
                                error = %validation_err,
                                "draft: contract validation failed, using fallback"
                            );
                            let fb = fallback_from_interpretation(
                                &input.interpretation.document_blueprint.title,
                                &input.interpretation.document_blueprint.summary,
                                &input.interpretation.teacher_delivery_summary,
                            );
                            (fb, true, Some(validation_err.to_string()))
                        }
                    };

                // Cache the result
                if should_cache && !fallback_used {
                    let response_value =
                        serde_json::to_value(&payload).unwrap_or(serde_json::json!({}));
                    if let Err(e) = self
                        .cache_repo
                        .store(
                            DRAFT_CACHE_ROUTE,
                            cache_key,
                            &serde_json::json!({}),
                            &response_value,
                            None,
                        )
                        .await
                    {
                        tracing::warn!(error = %e, "draft: failed to cache result");
                    }
                }

                // Record in ledger
                let token_count = response
                    .usage
                    .as_ref()
                    .map(|u| u.total_tokens.unwrap_or(0) as i64);
                let input_tokens = response
                    .usage
                    .as_ref()
                    .map(|u| u.prompt_tokens.unwrap_or(0) as i64);
                let output_tokens = response
                    .usage
                    .as_ref()
                    .map(|u| u.completion_tokens.unwrap_or(0) as i64);

                let cost = self
                    .price_catalog_repo
                    .estimate_cost(
                        provider,
                        model,
                        input_tokens.unwrap_or(0),
                        output_tokens.unwrap_or(0),
                    )
                    .await;

                let _ = self
                    .ledger_repo
                    .record_completed(
                        &request_id,
                        &input.generation_id,
                        DRAFT_ROUTE,
                        DRAFT_REQUEST_TYPE,
                        response.model.as_deref().unwrap_or(provider),
                        provider,
                        response.model.as_deref().unwrap_or(model),
                        model,
                        Some(latency_ms),
                        0,
                        if fallback_used {
                            CacheStatus::Bypass
                        } else {
                            CacheStatus::Miss
                        },
                        input_tokens,
                        output_tokens,
                        token_count,
                        Some(cost.total_cost_usd),
                        fallback_used,
                        fallback_error.as_deref(),
                        vec![provider.to_string()],
                        Some(cache_key),
                        serde_json::json!({
                            "source": "draft_service",
                            "cache_hit": false,
                            "fallback_used": fallback_used,
                        }),
                    )
                    .await;

                // Record governance usage
                if let Ok(policies) = self
                    .policies_repo
                    .find_applicable(DRAFT_ROUTE, provider, model)
                    .await
                {
                    for policy in &policies {
                        let _ = self
                            .buckets_repo
                            .record_usage(
                                policy,
                                input_tokens.unwrap_or(0),
                                output_tokens.unwrap_or(0),
                                token_count.unwrap_or(0),
                                cost.total_cost_usd,
                                &request_id,
                                &input.generation_id,
                                Utc::now(),
                            )
                            .await;
                    }
                }

                Ok(DraftResult {
                    request_id,
                    generation_id: input.generation_id.clone(),
                    draft_payload: payload,
                    source: if fallback_used {
                        "fallback".to_string()
                    } else {
                        "provider".to_string()
                    },
                    adapter_metadata: serde_json::json!({
                        "provider": response.model.as_deref().unwrap_or(provider),
                        "model": response.model.as_deref().unwrap_or(model),
                        "usage": response.usage.as_ref().map(|u| serde_json::json!({
                            "prompt_tokens": u.prompt_tokens,
                            "completion_tokens": u.completion_tokens,
                            "total_tokens": u.total_tokens,
                        })),
                    }),
                    fallback_error,
                    response_headers: headers_for(
                        response.model.as_deref().unwrap_or(provider),
                        response.model.as_deref().unwrap_or(model),
                        false,
                        fallback_used,
                    ),
                    latency_ms: Some(latency_ms),
                })
            }
            Err(provider_err) => {
                let _ = self
                    .ledger_repo
                    .record_failure(
                        &request_id,
                        &input.generation_id,
                        DRAFT_ROUTE,
                        DRAFT_REQUEST_TYPE,
                        provider,
                        provider,
                        model,
                        model,
                        CacheStatus::Miss,
                        Some("ProviderError"),
                        Some(&provider_err.to_string()),
                        false,
                        None,
                        vec![provider.to_string()],
                        serde_json::json!({
                            "source": "draft_service",
                            "error": provider_err.to_string(),
                        }),
                    )
                    .await;

                Err(DraftError::Provider(provider_err))
            }
        }
    }
}

// ─── Error type ─────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum DraftError {
    #[error("rate limited by governance policy")]
    RateLimited {
        request_id: String,
        generation_id: String,
    },

    #[error("provider error: {0}")]
    Provider(#[from] ProviderError),

    #[error("cache error: {0}")]
    Cache(#[from] sqlx::Error),

    #[error("governance error: {0}")]
    Governance(sqlx::Error),

    #[error("contract validation error: {0}")]
    Contract(
        #[from]
        crate::contracts::prompt_interpretation::ContractValidationError,
    ),
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn headers_for(provider: &str, model: &str, cache_hit: bool, fallback_used: bool) -> HashMap<String, String> {
    let mut h = HashMap::new();
    h.insert("x-klass-provider".to_string(), provider.to_string());
    h.insert("x-klass-model".to_string(), model.to_string());
    h.insert(
        "x-klass-cache-status".to_string(),
        if cache_hit { "hit" } else { "miss" }.to_string(),
    );
    h.insert(
        "x-klass-fallback-used".to_string(),
        if fallback_used { "true" } else { "false" }.to_string(),
    );
    h
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_interpretation() -> InterpretationPayload {
        crate::contracts::prompt_interpretation::fallback("Buatkan materi pecahan kelas 5 SD")
    }

    fn sample_draft_input() -> DraftInput {
        DraftInput {
            generation_id: "gen-draft-1".to_string(),
            interpretation: sample_interpretation(),
            resolved_output_type: "pdf".to_string(),
            taxonomy_hint: None,
            model: None,
            instruction: None,
        }
    }

    #[test]
    fn test_draft_constants() {
        assert_eq!(DRAFT_ROUTE, "respond");
        assert_eq!(DRAFT_REQUEST_TYPE, "media_content_draft");
        assert_eq!(DRAFT_CACHE_ROUTE, CacheRoute::Respond);
        assert!(DEFAULT_DRAFT_INSTRUCTION.contains("media_content_draft.v1"));
    }

    #[test]
    fn test_draft_input_defaults() {
        let input = sample_draft_input();
        assert_eq!(input.generation_id, "gen-draft-1");
        assert_eq!(input.resolved_output_type, "pdf");
        assert!(input.taxonomy_hint.is_none());
        assert!(input.model.is_none());
    }

    #[test]
    fn test_draft_input_with_taxonomy_hint() {
        let hint = DraftTaxonomyHint {
            schema_version: "media_draft_taxonomy_hint.v1".to_string(),
            source: "interpretation_context".to_string(),
            subject_name: Some("Matematika".to_string()),
            subject_slug: Some("matematika".to_string()),
            sub_subject_name: Some("Pecahan".to_string()),
            sub_subject_slug: Some("pecahan".to_string()),
            jenjang: Some("SD".to_string()),
            kelas: Some(5),
            semester: Some(1),
            bab: None,
            description: Some("Pecahan adalah bagian dari keseluruhan".to_string()),
            structure: Some("Konsep, Contoh, Latihan".to_string()),
            matched_signals: vec!["subject_phrase".to_string(), "kelas".to_string()],
        };
        let input = DraftInput {
            taxonomy_hint: Some(hint),
            model: Some("tencent/hy3:free".to_string()),
            instruction: Some("Custom instruction".to_string()),
            ..sample_draft_input()
        };
        assert!(input.taxonomy_hint.is_some());
        assert_eq!(
            input.taxonomy_hint.as_ref().unwrap().subject_name.as_deref(),
            Some("Matematika")
        );
        assert_eq!(input.model.as_deref(), Some("tencent/hy3:free"));
    }

    #[test]
    fn test_draft_result_defaults() {
        let payload = sample_draft_input().interpretation;
        let result = DraftResult {
            request_id: "req-draft-1".to_string(),
            generation_id: "gen-draft-1".to_string(),
            draft_payload: fallback_from_interpretation(
                &payload.document_blueprint.title,
                &payload.document_blueprint.summary,
                &payload.teacher_delivery_summary,
            ),
            source: "provider".to_string(),
            adapter_metadata: serde_json::json!({
                "provider": "openrouter",
                "model": "tencent/hy3:free",
            }),
            fallback_error: None,
            response_headers: HashMap::new(),
            latency_ms: Some(Decimal::new(2500, 3)),
        };
        assert_eq!(result.request_id, "req-draft-1");
        assert_eq!(result.source, "provider");
        assert_eq!(
            result.adapter_metadata["provider"],
            "openrouter"
        );
    }

    #[test]
    fn test_draft_result_from_cache() {
        let payload = sample_draft_input().interpretation;
        let result = DraftResult {
            request_id: "req-cache".to_string(),
            generation_id: "gen-cache".to_string(),
            draft_payload: fallback_from_interpretation(
                &payload.document_blueprint.title,
                &payload.document_blueprint.summary,
                &payload.teacher_delivery_summary,
            ),
            source: "cache".to_string(),
            adapter_metadata: serde_json::json!({"cache_hit": true}),
            fallback_error: None,
            response_headers: headers_for("openrouter", "tencent/hy3:free", true, false),
            latency_ms: Some(Decimal::new(300, 3)),
        };
        assert_eq!(result.source, "cache");
        assert_eq!(
            result.response_headers.get("x-klass-cache-status"),
            Some(&"hit".to_string())
        );
    }

    #[test]
    fn test_draft_result_with_fallback() {
        let payload = sample_draft_input().interpretation;
        let result = DraftResult {
            request_id: "req-fallback".to_string(),
            generation_id: "gen-fallback".to_string(),
            draft_payload: fallback_from_interpretation(
                &payload.document_blueprint.title,
                &payload.document_blueprint.summary,
                &payload.teacher_delivery_summary,
            ),
            source: "fallback".to_string(),
            adapter_metadata: serde_json::json!({"fallback": true}),
            fallback_error: Some("provider_response_contract_invalid".to_string()),
            response_headers: headers_for("openrouter", "tencent/hy3:free", false, true),
            latency_ms: None,
        };
        assert_eq!(result.source, "fallback");
        assert!(result.fallback_error.is_some());
        assert_eq!(
            result.response_headers.get("x-klass-fallback-used"),
            Some(&"true".to_string())
        );
    }

    #[test]
    fn test_draft_error_display() {
        let err = DraftError::RateLimited {
            request_id: "req-1".to_string(),
            generation_id: "gen-1".to_string(),
        };
        assert!(err.to_string().contains("rate limited"));
    }

    #[test]
    fn test_draft_error_provider() {
        let err = DraftError::Provider(ProviderError::Api {
            status: 503,
            body: "Service unavailable".to_string(),
        });
        let msg = err.to_string();
        assert!(msg.contains("503"));
        assert!(msg.contains("Service unavailable"));
    }

    #[test]
    fn test_draft_taxonomy_hint_serialization() {
        let hint = DraftTaxonomyHint {
            schema_version: "media_draft_taxonomy_hint.v1".to_string(),
            source: "interpretation_context".to_string(),
            subject_name: Some("IPA".to_string()),
            subject_slug: Some("ipa".to_string()),
            sub_subject_name: None,
            sub_subject_slug: None,
            jenjang: Some("SMP".to_string()),
            kelas: Some(7),
            semester: None,
            bab: None,
            description: None,
            structure: None,
            matched_signals: vec!["subject_phrase".to_string()],
        };
        let json = serde_json::json!(hint);
        assert_eq!(json["subject_name"], "IPA");
        assert_eq!(json["jenjang"], "SMP");
        assert_eq!(json["matched_signals"][0], "subject_phrase");
    }

    #[test]
    fn test_headers_for() {
        let h = headers_for("openrouter", "tencent/hy3:free", true, false);
        assert_eq!(h.get("x-klass-provider"), Some(&"openrouter".to_string()));
        assert_eq!(h.get("x-klass-cache-status"), Some(&"hit".to_string()));
        assert_eq!(h.get("x-klass-fallback-used"), Some(&"false".to_string()));

        let h2 = headers_for("openrouter", "tencent/hy3:free", false, true);
        assert_eq!(h2.get("x-klass-cache-status"), Some(&"miss".to_string()));
        assert_eq!(h2.get("x-klass-fallback-used"), Some(&"true".to_string()));
    }

    #[test]
    fn test_fallback_from_interpretation() {
        let payload = sample_draft_input().interpretation;
        let draft = fallback_from_interpretation(
            &payload.document_blueprint.title,
            &payload.document_blueprint.summary,
            &payload.teacher_delivery_summary,
        );
        assert_eq!(draft.schema_version, "media_content_draft.v1");
        assert!(draft.fallback.triggered);
        assert_eq!(draft.sections.len(), 1);
    }
}

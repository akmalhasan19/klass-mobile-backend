//! Media delivery response service.
//!
//! Port of `DeliveryWorkflowService` from the Python adapter
//! (`llm-adapter-service/app/delivery.py`).
//!
//! The third and final stage of the LLM pipeline after drafting:
//!
//! 1. Preflight governance check (rate-limit, cost budget)
//! 2. Cache lookup (semantic cache over `llm_cache_entries`)
//! 3. Cache-hit → record cache hit in ledger, return cached response
//! 4. Cache-miss → call OpenRouter with artifact + publication context
//! 5. Validate via `delivery::decode_and_validate`
//! 6. On validation failure → `delivery::fallback()`
//! 7. Build delivery payload with `response_meta` (provider, model, llm_used)
//! 8. Persist to ledger + governance buckets

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::cache::{build_cache_key, CacheRoute, LlmCacheRepo};
use crate::contracts::delivery::{
    decode_and_validate, fallback, DeliveryResponsePayload,
};
use crate::governance::ledger::{CacheStatus, LedgerRepo};
use crate::governance::price_catalog::PriceCatalogRepo;
use crate::governance::rate_limit::{
    preflight_check, ExhaustionAction, RateLimitBucketsRepo, RateLimitPoliciesRepo,
};
use crate::providers::{json_mode_request, ProviderError, ProviderRouter};

// ─── Constants ──────────────────────────────────────────────────────────────

/// Route identifier used in governance and ledger.
pub const RESPOND_ROUTE: &str = "respond";

/// Request type used in ledger entries.
pub const RESPOND_REQUEST_TYPE: &str = "media_delivery_response";

/// Cache route.
pub const RESPOND_CACHE_ROUTE: CacheRoute = CacheRoute::Respond;

/// Default system instruction for the delivery (respond) LLM call.
pub const DEFAULT_RESPOND_INSTRUCTION: &str = "\
You are a media generation assistant helping teachers prepare their generated \
classroom material for delivery. Given the artifact details and publication \
context, compose a delivery response for the teacher. Return a valid JSON \
object following the media_delivery_response.v1 schema. Include a teacher \
message, recommended next steps, classroom tips, and ensure the response_meta \
accurately reflects the provider and model used. Write in the same language \
as the original prompt.";

// ─── Types ──────────────────────────────────────────────────────────────────

/// Input for the delivery response service.
///
/// Analogous to `DeliveryRequest` + `DeliveryRequestInput` in Python.
#[derive(Debug, Clone)]
pub struct RespondInput {
    /// Unique identifier for the generation this response belongs to.
    pub generation_id: String,
    /// Title of the generated material.
    pub title: String,
    /// Short preview/summary of the generated content.
    pub preview_summary: String,
    /// The generated artifact details (file URL, type, MIME, etc.).
    pub artifact: RespondArtifact,
    /// Publication entities (topic, content, recommended_project).
    pub publication: RespondPublication,
    /// Provider model override.
    pub model: Option<String>,
    /// System instruction override.
    pub instruction: Option<String>,
}

/// Artifact details for the delivery response.
#[derive(Debug, Clone)]
pub struct RespondArtifact {
    /// Output type: "pdf", "docx", "pptx".
    pub output_type: String,
    /// Public URL of the generated file.
    pub file_url: String,
    /// MIME type.
    pub mime_type: String,
    /// Optional thumbnail URL.
    pub thumbnail_url: Option<String>,
    /// Optional filename.
    pub filename: Option<String>,
}

/// Publication entities for the delivery response.
#[derive(Debug, Clone, Default)]
pub struct RespondPublication {
    /// Optional topic node.
    pub topic: Option<EntityNode>,
    /// Optional content node.
    pub content: Option<EntityNode>,
    /// Optional recommended project node.
    pub recommended_project: Option<EntityNode>,
}

/// A publication entity node (topic, content, or recommended project).
#[derive(Debug, Clone)]
pub struct EntityNode {
    pub id: String,
    pub title: String,
}

/// The result of a successful delivery response composition.
#[derive(Debug, Clone)]
pub struct RespondResult {
    /// Unique request ID used for ledger and governance tracking.
    pub request_id: String,
    /// The generation ID this response belongs to.
    pub generation_id: String,
    /// Validated delivery response payload.
    pub delivery_payload: DeliveryResponsePayload,
    /// Source: "cache", "provider", or "fallback".
    pub source: String,
    /// LLM provider that served the response.
    pub llm_provider: Option<String>,
    /// LLM model that served the response.
    pub llm_model: Option<String>,
    /// Whether the LLM was used to compose the response.
    pub llm_used: bool,
    /// Whether fallback was triggered.
    pub fallback_used: bool,
    /// HTTP-styled response headers.
    pub response_headers: HashMap<String, String>,
    /// Request latency in milliseconds.
    pub latency_ms: Option<Decimal>,
}

// ─── RespondService ─────────────────────────────────────────────────────────

/// Orchestrates delivery response composition by combining cache, governance,
/// provider, and contract validation.
pub struct RespondService {
    cache_repo: LlmCacheRepo,
    ledger_repo: LedgerRepo,
    price_catalog_repo: PriceCatalogRepo,
    policies_repo: RateLimitPoliciesRepo,
    buckets_repo: RateLimitBucketsRepo,
    provider_router: Arc<ProviderRouter>,
    default_model: String,
    default_instruction: String,
}

impl RespondService {
    /// Create a new delivery response service with all dependencies.
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

    /// Run the full delivery response pipeline.
    pub async fn respond(&self, input: RespondInput) -> Result<RespondResult, RespondError> {
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
            "title": &input.title,
            "preview_summary": &input.preview_summary,
            "artifact": {
                "output_type": &input.artifact.output_type,
                "file_url": &input.artifact.file_url,
                "mime_type": &input.artifact.mime_type,
            },
            "topic": input.publication.topic.as_ref().map(|t| serde_json::json!({
                "id": t.id, "title": t.title,
            })),
            "content": input.publication.content.as_ref().map(|c| serde_json::json!({
                "id": c.id, "title": c.title,
            })),
        });
        let cache_key = build_cache_key(
            RESPOND_CACHE_ROUTE,
            provider,
            &model,
            RESPOND_REQUEST_TYPE,
            &instruction,
            &cache_input_payload,
        );

        // ── Step 2: Preflight governance check ────────────────────────────
        let projected_cost = self
            .price_catalog_repo
            .estimate_cost(provider, &model, 300, 1000)
            .await;
        let decision = preflight_check(
            &self.policies_repo,
            &self.buckets_repo,
            RESPOND_ROUTE,
            provider,
            &model,
            Some(projected_cost.total_cost_usd),
            &request_id,
            &input.generation_id,
        )
        .await
        .map_err(RespondError::Governance)?;

        match decision.action {
            ExhaustionAction::Deny => {
                return Err(RespondError::RateLimited {
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
            .lookup(RESPOND_CACHE_ROUTE, &cache_key)
            .await
            .map_err(RespondError::Cache)?;

        if let Some(entry) = cached {
            let payload: DeliveryResponsePayload =
                serde_json::from_value(entry.response_payload.clone()).map_err(|e| {
                    RespondError::Contract(
                        crate::contracts::prompt_interpretation::ContractValidationError {
                            code: "cached_payload_corrupt",
                            message: format!("Cached delivery payload failed deserialization: {}", e),
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
                    RESPOND_ROUTE,
                    RESPOND_REQUEST_TYPE,
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
                        "source": "respond_service",
                        "cache_hit": true,
                        "cache_source": "delivery_cache",
                    }),
                )
                .await;

            return Ok(RespondResult {
                request_id,
                generation_id: input.generation_id,
                delivery_payload: payload.clone(),
                source: "cache".to_string(),
                llm_provider: payload.response_meta.provider.clone(),
                llm_model: payload.response_meta.model.clone(),
                llm_used: payload.response_meta.llm_used,
                fallback_used: payload.fallback.triggered,
                response_headers: respond_headers(provider, &model, true, false),
                latency_ms: Some(latency_ms),
            });
        }

        // ── Step 4: Cache miss – anti-stampede lock ──────────────────────
        let lock_acquired = self
            .cache_repo
            .try_acquire_lock(RESPOND_CACHE_ROUTE, &cache_key)
            .await
            .map_err(RespondError::Cache)?;

        if !lock_acquired {
            let waited = self
                .cache_repo
                .wait_for_entry(RESPOND_CACHE_ROUTE, &cache_key, None, None)
                .await
                .map_err(RespondError::Cache)?;

            if let Some(entry) = waited {
                let payload: DeliveryResponsePayload =
                    serde_json::from_value(entry.response_payload.clone()).map_err(|e| {
                        RespondError::Contract(
                            crate::contracts::prompt_interpretation::ContractValidationError {
                                code: "cached_payload_corrupt",
                                message: format!("Cached delivery payload (waited) failed: {}", e),
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
                        RESPOND_ROUTE,
                        RESPOND_REQUEST_TYPE,
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
                            "source": "respond_service",
                            "cache_hit": true,
                            "cache_source": "delivery_cache_inflight_wait",
                        }),
                    )
                    .await;

                return Ok(RespondResult {
                    request_id,
                    generation_id: input.generation_id,
                    delivery_payload: payload.clone(),
                    source: "cache".to_string(),
                    llm_provider: payload.response_meta.provider.clone(),
                    llm_model: payload.response_meta.model.clone(),
                    llm_used: payload.response_meta.llm_used,
                    fallback_used: payload.fallback.triggered,
                    response_headers: respond_headers(provider, &model, true, false),
                    latency_ms: Some(latency_ms),
                });
            }

            tracing::warn!(
                request_id = %request_id,
                cache_key = %cache_key,
                "respond: inflight wait timed out, attempting provider directly"
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
                .release_lock(RESPOND_CACHE_ROUTE, &cache_key)
                .await
            {
                tracing::warn!(error = %e, cache_key = %cache_key, "respond: failed to release advisory lock");
            }
        }

        result
    }

    // ─── Internal: execute provider call ─────────────────────────────────

    async fn execute_provider(
        &self,
        input: &RespondInput,
        request_id: String,
        provider: &str,
        model: &str,
        instruction: &str,
        cache_key: &str,
        start: Instant,
        should_cache: bool,
    ) -> Result<RespondResult, RespondError> {
        let elapsed = start.elapsed();
        let latency_ms = Decimal::new(elapsed.as_millis() as i64, 3);

        // Build the provider request payload
        let user_payload = serde_json::json!({
            "title": input.title,
            "preview_summary": input.preview_summary,
            "artifact": {
                "output_type": input.artifact.output_type,
                "title": input.title,
                "file_url": input.artifact.file_url,
                "thumbnail_url": input.artifact.thumbnail_url,
                "mime_type": input.artifact.mime_type,
                "filename": input.artifact.filename,
            },
            "publication": {
                "topic": input.publication.topic.as_ref().map(|t| serde_json::json!({
                    "id": t.id, "title": t.title,
                })),
                "content": input.publication.content.as_ref().map(|c| serde_json::json!({
                    "id": c.id, "title": c.title,
                })),
                "recommended_project": input.publication.recommended_project.as_ref().map(|r| serde_json::json!({
                    "id": r.id, "title": r.title,
                })),
            },
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
                                "respond: contract validation failed, using fallback"
                            );
                            let fb = fallback(
                                &input.title,
                                &input.preview_summary,
                                &input.artifact.output_type,
                                &input.artifact.file_url,
                                &input.artifact.mime_type,
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
                            RESPOND_CACHE_ROUTE,
                            cache_key,
                            &serde_json::json!({}),
                            &response_value,
                            None,
                        )
                        .await
                    {
                        tracing::warn!(error = %e, "respond: failed to cache result");
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
                        RESPOND_ROUTE,
                        RESPOND_REQUEST_TYPE,
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
                            "source": "respond_service",
                            "cache_hit": false,
                            "fallback_used": fallback_used,
                        }),
                    )
                    .await;

                // Record governance usage
                if let Ok(policies) = self
                    .policies_repo
                    .find_applicable(RESPOND_ROUTE, provider, model)
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

                let llm_provider = response.model.clone().unwrap_or_else(|| provider.to_string());
                let llm_model = response.model.clone().unwrap_or_else(|| model.to_string());

                Ok(RespondResult {
                    request_id,
                    generation_id: input.generation_id.clone(),
                    delivery_payload: payload,
                    source: if fallback_used {
                        "fallback".to_string()
                    } else {
                        "provider".to_string()
                    },
                    llm_provider: Some(llm_provider.clone()),
                    llm_model: Some(llm_model.clone()),
                    llm_used: !fallback_used,
                    fallback_used,
                    response_headers: respond_headers(&llm_provider, &llm_model, false, fallback_used),
                    latency_ms: Some(latency_ms),
                })
            }
            Err(provider_err) => {
                let _ = self
                    .ledger_repo
                    .record_failure(
                        &request_id,
                        &input.generation_id,
                        RESPOND_ROUTE,
                        RESPOND_REQUEST_TYPE,
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
                            "source": "respond_service",
                            "error": provider_err.to_string(),
                        }),
                    )
                    .await;

                Err(RespondError::Provider(provider_err))
            }
        }
    }
}

// ─── Error type ─────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum RespondError {
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

fn respond_headers(provider: &str, model: &str, cache_hit: bool, fallback_used: bool) -> HashMap<String, String> {
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
    fn sample_input() -> RespondInput {
        RespondInput {
            generation_id: "gen-respond-1".to_string(),
            title: "Materi Pecahan Kelas 5".to_string(),
            preview_summary: "PDF materi pecahan untuk kelas 5 SD".to_string(),
            artifact: RespondArtifact {
                output_type: "pdf".to_string(),
                file_url: "https://storage.example.com/materi-pecahan.pdf".to_string(),
                mime_type: "application/pdf".to_string(),
                thumbnail_url: None,
                filename: Some("materi-pecahan.pdf".to_string()),
            },
            publication: RespondPublication {
                topic: Some(EntityNode {
                    id: "topic-1".to_string(),
                    title: "Pecahan".to_string(),
                }),
                content: Some(EntityNode {
                    id: "content-1".to_string(),
                    title: "Materi Pecahan".to_string(),
                }),
                recommended_project: None,
            },
            model: None,
            instruction: None,
        }
    }

    #[test]
    fn test_respond_constants() {
        assert_eq!(RESPOND_ROUTE, "respond");
        assert_eq!(RESPOND_REQUEST_TYPE, "media_delivery_response");
        assert_eq!(RESPOND_CACHE_ROUTE, CacheRoute::Respond);
        assert!(DEFAULT_RESPOND_INSTRUCTION.contains("media_delivery_response.v1"));
    }

    #[test]
    fn test_respond_input_defaults() {
        let input = sample_input();
        assert_eq!(input.generation_id, "gen-respond-1");
        assert_eq!(input.title, "Materi Pecahan Kelas 5");
        assert_eq!(input.artifact.output_type, "pdf");
        assert_eq!(input.artifact.file_url, "https://storage.example.com/materi-pecahan.pdf");
    }

    #[test]
    fn test_respond_input_with_model() {
        let input = RespondInput {
            model: Some("tencent/hy3:free".to_string()),
            instruction: Some("Custom respond instruction".to_string()),
            ..sample_input()
        };
        assert_eq!(input.model.as_deref(), Some("tencent/hy3:free"));
    }

    #[test]
    fn test_respond_artifact_basic() {
        let art = RespondArtifact {
            output_type: "pptx".to_string(),
            file_url: "https://storage.example.com/slides.pptx".to_string(),
            mime_type: "application/vnd.openxmlformats-officedocument.presentationml.presentation".to_string(),
            thumbnail_url: Some("https://storage.example.com/slides-thumb.png".to_string()),
            filename: Some("slides.pptx".to_string()),
        };
        assert_eq!(art.output_type, "pptx");
        assert!(art.thumbnail_url.is_some());
    }

    #[test]
    fn test_respond_publication_default() {
        let pub_entity: RespondPublication = Default::default();
        assert!(pub_entity.topic.is_none());
        assert!(pub_entity.content.is_none());
        assert!(pub_entity.recommended_project.is_none());
    }

    #[test]
    fn test_respond_publication_with_nodes() {
        let pub_entity = RespondPublication {
            topic: Some(EntityNode { id: "t1".to_string(), title: "Topic 1".to_string() }),
            content: Some(EntityNode { id: "c1".to_string(), title: "Content 1".to_string() }),
            recommended_project: Some(EntityNode { id: "r1".to_string(), title: "Project 1".to_string() }),
        };
        assert_eq!(pub_entity.topic.as_ref().unwrap().id, "t1");
        assert_eq!(pub_entity.content.as_ref().unwrap().title, "Content 1");
        assert_eq!(pub_entity.recommended_project.as_ref().unwrap().id, "r1");
    }

    #[test]
    fn test_respond_result_defaults() {
        let payload = fallback(
            "Materi Pecahan",
            "Preview summary",
            "pdf",
            "https://example.com/file.pdf",
            "application/pdf",
        );
        let result = RespondResult {
            request_id: "req-resp-1".to_string(),
            generation_id: "gen-resp-1".to_string(),
            delivery_payload: payload,
            source: "provider".to_string(),
            llm_provider: Some("openrouter".to_string()),
            llm_model: Some("tencent/hy3:free".to_string()),
            llm_used: true,
            fallback_used: false,
            response_headers: HashMap::new(),
            latency_ms: Some(Decimal::new(1800, 3)),
        };
        assert_eq!(result.request_id, "req-resp-1");
        assert_eq!(result.source, "provider");
        assert!(result.llm_used);
        assert!(!result.fallback_used);
    }

    #[test]
    fn test_respond_result_fallback() {
        let payload = fallback(
            "Test", "Summary", "pdf", "https://example.com/f.pdf", "application/pdf",
        );
        let result = RespondResult {
            request_id: "req-fb".to_string(),
            generation_id: "gen-fb".to_string(),
            delivery_payload: payload,
            source: "fallback".to_string(),
            llm_provider: None,
            llm_model: None,
            llm_used: false,
            fallback_used: true,
            response_headers: respond_headers("openrouter", "tencent/hy3:free", false, true),
            latency_ms: None,
        };
        assert_eq!(result.source, "fallback");
        assert!(!result.llm_used);
        assert!(result.fallback_used);
        assert_eq!(
            result.response_headers.get("x-klass-fallback-used"),
            Some(&"true".to_string())
        );
    }

    #[test]
    fn test_respond_result_cache_hit() {
        let payload = fallback(
            "Cached Title", "Cached summary", "docx",
            "https://example.com/cached.docx", "application/docx",
        );
        let result = RespondResult {
            request_id: "req-cache".to_string(),
            generation_id: "gen-cache".to_string(),
            delivery_payload: payload,
            source: "cache".to_string(),
            llm_provider: Some("openrouter".to_string()),
            llm_model: Some("tencent/hy3:free".to_string()),
            llm_used: true,
            fallback_used: false,
            response_headers: respond_headers("openrouter", "tencent/hy3:free", true, false),
            latency_ms: Some(Decimal::new(200, 3)),
        };
        assert_eq!(result.source, "cache");
        assert_eq!(
            result.response_headers.get("x-klass-cache-status"),
            Some(&"hit".to_string())
        );
    }

    #[test]
    fn test_respond_error_display() {
        let err = RespondError::RateLimited {
            request_id: "req-1".to_string(),
            generation_id: "gen-1".to_string(),
        };
        assert!(err.to_string().contains("rate limited"));
    }

    #[test]
    fn test_respond_error_provider() {
        let err = RespondError::Provider(ProviderError::Api {
            status: 502,
            body: "Bad gateway".to_string(),
        });
        let msg = err.to_string();
        assert!(msg.contains("502"));
        assert!(msg.contains("Bad gateway"));
    }

    #[test]
    fn test_entity_node_construction() {
        let node = EntityNode {
            id: "topic-abc".to_string(),
            title: "Matematika".to_string(),
        };
        assert_eq!(node.id, "topic-abc");
        assert_eq!(node.title, "Matematika");
    }

    #[test]
    fn test_respond_payload_meta_in_fallback() {
        let payload = fallback(
            "Materi", "Summary", "pdf",
            "https://example.com/materi.pdf", "application/pdf",
        );
        // Fallback has llm_used: false, provider: None, model: None
        assert!(!payload.response_meta.llm_used);
        assert!(payload.response_meta.provider.is_none());
        assert!(payload.response_meta.model.is_none());
        assert!(payload.fallback.triggered);
        assert_eq!(payload.schema_version, "media_delivery_response.v1");
    }

    #[test]
    fn test_respond_headers() {
        let h = respond_headers("openrouter", "tencent/hy3:free", true, false);
        assert_eq!(h.get("x-klass-provider"), Some(&"openrouter".to_string()));
        assert_eq!(h.get("x-klass-cache-status"), Some(&"hit".to_string()));
        assert_eq!(h.get("x-klass-fallback-used"), Some(&"false".to_string()));

        let h2 = respond_headers("openrouter", "tencent/hy3:free", false, true);
        assert_eq!(h2.get("x-klass-cache-status"), Some(&"miss".to_string()));
        assert_eq!(h2.get("x-klass-fallback-used"), Some(&"true".to_string()));
    }
}

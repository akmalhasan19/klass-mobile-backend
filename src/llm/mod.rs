//! LLM orchestration module.
//!
//! Orchestrates the three-stage LLM pipeline:
//!
//! 1. **`interpret`** — Analyze a media-generation prompt to extract intent,
//!    taxonomy, constraints, and enrich with subject/sub\_subject context.
//! 2. **`draft`** — Given an interpretation, generate the actual media content
//!    (text/OOXML payload) via the LLM provider.
//! 3. **`respond`** — Compose the final delivery response including title,
//!    summary, teacher messages, artifact metadata, and publication entities.
//!
//! Each stage uses the shared infrastructure:
//! - [`LlmCacheRepo`](crate::cache::LlmCacheRepo) for semantic caching
//! - [`LedgerRepo`](crate::governance::ledger::LedgerRepo) for audit logging
//! - [`PriceCatalogRepo`](crate::governance::price_catalog::PriceCatalogRepo) for cost tracking
//! - [`ProviderRouter`](crate::providers::ProviderRouter) for primary/fallback provider selection
//! - Contract schemas from [`crate::contracts`] for decode-validate-fallback

pub mod clarification;
pub mod draft;
pub mod interpret;
pub mod respond;

//! Tests for governance modules: rate limiting, ledger, price catalog.
//!
//! Covers:
//! - Rate limit policy and bucket SQL constants
//! - Preflight decision logic (interpret=deny, respond=degrade)
//! - Ledger entry input types and cache status
//! - Price catalog cost estimation
//! - Module type signatures (compile-time checks)

use klass_gateway::governance::ledger::{CacheStatus, LedgerRecordInput, LedgerRepo};
use klass_gateway::governance::price_catalog::{estimate_cost_inner, CostEstimate, PriceEntry, PriceCatalogRepo};
use klass_gateway::governance::rate_limit::{
    exhausted_decision, ExhaustionAction, RateLimitBucketsRepo,
    RateLimitPoliciesRepo, RateLimitRoute, ScopeType, WindowUnit,
};
use rust_decimal::Decimal;

// ═════════════════════════════════════════════════════════════════════════════
// 1. Rate Limit — Route, Scope, Window Types
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_rate_limit_route_from_str() {
    assert_eq!(RateLimitRoute::from("interpret"), RateLimitRoute::Interpret);
    assert_eq!(RateLimitRoute::from("respond"), RateLimitRoute::Respond);
    assert_eq!(RateLimitRoute::from("all"), RateLimitRoute::All);
    assert_eq!(RateLimitRoute::from("unknown"), RateLimitRoute::All);
}

#[test]
fn test_scope_type_as_str() {
    assert_eq!(ScopeType::Global.as_str(), "global");
    assert_eq!(ScopeType::Route.as_str(), "route");
    assert_eq!(ScopeType::Provider.as_str(), "provider");
    assert_eq!(ScopeType::Model.as_str(), "model");
}

#[test]
fn test_window_unit_as_str() {
    assert_eq!(WindowUnit::Minute.as_str(), "minute");
    assert_eq!(WindowUnit::Hour.as_str(), "hour");
    assert_eq!(WindowUnit::Day.as_str(), "day");
}

#[test]
fn test_rate_limit_route_eq() {
    assert_eq!(RateLimitRoute::Interpret, RateLimitRoute::Interpret);
    assert_ne!(RateLimitRoute::Interpret, RateLimitRoute::Respond);
}

#[test]
fn test_scope_type_eq() {
    assert_eq!(ScopeType::Global, ScopeType::Global);
    assert_ne!(ScopeType::Global, ScopeType::Route);
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. Rate Limit — Exhaustion Decisions
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_exhausted_decision_interpret_is_deny() {
    let d = exhausted_decision("interpret", "deepseek", "deepseek-v4-flash");
    assert!(!d.allowed);
    assert_eq!(d.action, ExhaustionAction::Deny);
}

#[test]
fn test_exhausted_decision_respond_is_degrade() {
    let d = exhausted_decision("respond", "deepseek", "deepseek-v4-flash");
    assert!(d.allowed);
    assert_eq!(d.action, ExhaustionAction::Degrade);
}

#[test]
fn test_exhausted_decision_unknown_defaults_to_deny() {
    let d = exhausted_decision("draft", "dp", "m");
    assert!(!d.allowed);
    assert_eq!(d.action, ExhaustionAction::Deny);
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. Rate Limit — Repository Type Signatures
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_repo_types_are_send_sync() {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<RateLimitPoliciesRepo>();
    _assert_send_sync::<RateLimitBucketsRepo>();
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. Ledger — Cache Status
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_cache_status_as_str() {
    assert_eq!(CacheStatus::Hit.as_str(), "hit");
    assert_eq!(CacheStatus::Miss.as_str(), "miss");
    assert_eq!(CacheStatus::Bypass.as_str(), "bypass");
}

#[test]
fn test_cache_status_eq() {
    assert_eq!(CacheStatus::Hit, CacheStatus::Hit);
    assert_ne!(CacheStatus::Hit, CacheStatus::Miss);
}

// ═════════════════════════════════════════════════════════════════════════════
// 5. Ledger — Record Input Types
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_ledger_record_input_completed() {
    let input = LedgerRecordInput {
        request_id: "req-1".to_string(),
        generation_id: "gen-1".to_string(),
        route: "interpret".to_string(),
        request_type: "media_prompt_interpretation".to_string(),
        provider: "deepseek".to_string(),
        primary_provider: "deepseek".to_string(),
        model: "deepseek-v4-flash".to_string(),
        requested_model: "deepseek-v4-flash".to_string(),
        latency_ms: Some(Decimal::new(1234, 2)),
        retry_count: 0,
        cache_status: CacheStatus::Miss,
        final_status: "completed".to_string(),
        error_class: None,
        error_code: None,
        fallback_used: false,
        fallback_reason: None,
        attempted_providers: vec!["deepseek".to_string()],
        upstream_request_id: None,
        provider_response_id: Some("resp-abc".to_string()),
        provider_model_version: None,
        finish_reason: Some("stop".to_string()),
        candidate_index: Some(0),
        input_tokens: Some(150),
        output_tokens: Some(200),
        total_tokens: Some(350),
        estimated_cost_usd: Some(Decimal::new(42, 6)),
        cache_key: Some("abc123...".to_string()),
        metadata: serde_json::json!({"source": "test"}),
        completed_at: None,
    };
    assert_eq!(input.route, "interpret");
    assert_eq!(input.cache_status.as_str(), "miss");
    assert_eq!(input.total_tokens, Some(350));
}

#[test]
fn test_ledger_record_input_cache_hit() {
    let input = LedgerRecordInput {
        cache_status: CacheStatus::Hit,
        final_status: "completed".to_string(),
        input_tokens: None,
        output_tokens: None,
        total_tokens: None,
        estimated_cost_usd: Some(Decimal::ZERO),
        fallback_used: false,
        cache_key: Some("hit-key".to_string()),
        latency_ms: Some(Decimal::new(500, 3)),
        retry_count: 0,
        attempted_providers: vec!["deepseek".to_string()],
        request_id: "req-hit".to_string(),
        generation_id: "gen-hit".to_string(),
        route: "interpret".to_string(),
        request_type: "media_prompt_interpretation".to_string(),
        provider: "deepseek".to_string(),
        primary_provider: "deepseek".to_string(),
        model: "deepseek-v4-flash".to_string(),
        requested_model: "deepseek-v4-flash".to_string(),
        error_class: None,
        error_code: None,
        fallback_reason: None,
        upstream_request_id: None,
        provider_response_id: None,
        provider_model_version: None,
        finish_reason: None,
        candidate_index: None,
        metadata: serde_json::json!({"cache_hit": true}),
        completed_at: None,
    };
    assert_eq!(input.cache_status.as_str(), "hit");
    assert_eq!(input.estimated_cost_usd, Some(Decimal::ZERO));
}

#[test]
fn test_ledger_record_input_failure() {
    let input = LedgerRecordInput {
        request_id: "req-fail".to_string(),
        generation_id: "gen-fail".to_string(),
        route: "respond".to_string(),
        request_type: "media_delivery_response".to_string(),
        provider: "deepseek".to_string(),
        primary_provider: "deepseek".to_string(),
        model: "deepseek-v4-flash".to_string(),
        requested_model: "deepseek-v4-flash".to_string(),
        latency_ms: None,
        retry_count: 2,
        cache_status: CacheStatus::Bypass,
        final_status: "failed".to_string(),
        error_class: Some("ProviderError".to_string()),
        error_code: Some("provider_unavailable".to_string()),
        fallback_used: true,
        fallback_reason: Some("primary failed".to_string()),
        attempted_providers: vec!["deepseek".to_string(), "gemini".to_string()],
        upstream_request_id: None,
        provider_response_id: None,
        provider_model_version: None,
        finish_reason: None,
        candidate_index: None,
        input_tokens: None,
        output_tokens: None,
        total_tokens: None,
        estimated_cost_usd: None,
        cache_key: None,
        metadata: serde_json::json!({"error": "timeout"}),
        completed_at: None,
    };
    assert_eq!(input.final_status, "failed");
    assert!(input.fallback_used);
    assert_eq!(input.attempted_providers.len(), 2);
}

// ═════════════════════════════════════════════════════════════════════════════
// 6. Ledger — Repository Type
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_ledger_repo_type_exists() {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<LedgerRepo>();
}

// ═════════════════════════════════════════════════════════════════════════════
// 7. Price Catalog — Cost Estimation
// ═════════════════════════════════════════════════════════════════════════════

fn make_price_entry(input_rate: Decimal, output_rate: Decimal) -> PriceEntry {
    use chrono::DateTime;
    PriceEntry {
        id: 1,
        provider: "deepseek".to_string(),
        model: "deepseek-v4-flash".to_string(),
        input_cost_per_unit_usd: Some(input_rate),
        output_cost_per_unit_usd: Some(output_rate),
        effective_from: DateTime::from_timestamp(0, 0).unwrap(),
        is_active: true,
        created_at: DateTime::from_timestamp(0, 0).unwrap(),
        updated_at: DateTime::from_timestamp(0, 0).unwrap(),
    }
}

#[test]
fn test_estimate_cost_with_exact_rates() {
    let entry = Some(make_price_entry(
        Decimal::new(10, 2),  // $0.10 / 1M input
        Decimal::new(40, 2),  // $0.40 / 1M output
    ));
    let cost = estimate_cost_inner(&entry, 1_000_000, 500_000);
    assert_eq!(cost.input_cost_usd, Decimal::new(10, 2));
    assert_eq!(cost.output_cost_usd, Decimal::new(20, 2));
    assert_eq!(cost.total_cost_usd, Decimal::new(30, 2));
}

#[test]
fn test_estimate_cost_without_pricing_entry() {
    let cost = estimate_cost_inner(&None, 1_000_000, 500_000);
    assert_eq!(cost.input_cost_usd, Decimal::new(10, 2));
    assert_eq!(cost.output_cost_usd, Decimal::new(20, 2));
    assert_eq!(cost.total_cost_usd, Decimal::new(30, 2));
}

#[test]
fn test_estimate_cost_zero_tokens() {
    let cost = estimate_cost_inner(&None, 0, 0);
    assert_eq!(cost.total_cost_usd, Decimal::ZERO);
    assert_eq!(cost.input_cost_usd, Decimal::ZERO);
    assert_eq!(cost.output_cost_usd, Decimal::ZERO);
}

#[test]
fn test_estimate_cost_small_request() {
    let entry = Some(make_price_entry(
        Decimal::new(10, 2),  // $0.10 / 1M
        Decimal::new(40, 2),  // $0.40 / 1M
    ));
    let cost = estimate_cost_inner(&entry, 500, 1000);
    let expected_total = Decimal::new(45, 5); // 0.00045
    assert!(
        (cost.total_cost_usd - expected_total).abs() < Decimal::new(1, 6),
        "expected ~0.00045, got {}",
        cost.total_cost_usd
    );
}

#[test]
fn test_estimate_cost_null_rates_fallback_to_defaults() {
    let entry = Some(PriceEntry {
        id: 2,
        provider: "unknown".to_string(),
        model: "unknown".to_string(),
        input_cost_per_unit_usd: None,
        output_cost_per_unit_usd: None,
        effective_from: chrono::DateTime::from_timestamp(0, 0).unwrap(),
        is_active: true,
        created_at: chrono::DateTime::from_timestamp(0, 0).unwrap(),
        updated_at: chrono::DateTime::from_timestamp(0, 0).unwrap(),
    });
    let cost = estimate_cost_inner(&entry, 1_000_000, 1_000_000);
    assert_eq!(cost.total_cost_usd, Decimal::new(50, 2)); // $0.10 + $0.40 = $0.50
}

#[test]
fn test_cost_estimate_roundtrip() {
    let cost = CostEstimate {
        input_cost_usd: Decimal::new(1, 4),
        output_cost_usd: Decimal::new(2, 4),
        total_cost_usd: Decimal::new(3, 4),
    };
    assert_eq!(cost.input_cost_usd + cost.output_cost_usd, cost.total_cost_usd);
}

// ═════════════════════════════════════════════════════════════════════════════
// 8. Price Catalog — Repository Type
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_price_catalog_repo_type_exists() {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<PriceCatalogRepo>();
}

#[test]
fn test_price_entry_defaults() {
    let entry = PriceEntry {
        id: 1,
        provider: "deepseek".to_string(),
        model: "deepseek-v4-flash".to_string(),
        input_cost_per_unit_usd: Some(Decimal::new(10, 2)),
        output_cost_per_unit_usd: Some(Decimal::new(40, 2)),
        effective_from: chrono::DateTime::from_timestamp(0, 0).unwrap(),
        is_active: true,
        created_at: chrono::DateTime::from_timestamp(0, 0).unwrap(),
        updated_at: chrono::DateTime::from_timestamp(0, 0).unwrap(),
    };
    assert_eq!(entry.provider, "deepseek");
    assert!(entry.is_active);
}

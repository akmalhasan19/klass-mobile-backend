//! LLM request ledger over the `llm_request_ledger` table.
//!
//! Records every LLM provider call with request metadata, token usage,
//! cache status, fallback signals, and final status.
//!
//! Port of `app/costs.py` — `AdapterCostService` recording logic.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

// ─── Types ──────────────────────────────────────────────────────────────────

/// A row in the `llm_request_ledger` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LedgerEntry {
    pub id: i64,
    pub request_id: String,
    pub generation_id: String,
    pub route: String,
    pub request_type: String,
    pub provider: String,
    pub primary_provider: String,
    pub model: String,
    pub requested_model: String,
    pub latency_ms: Option<rust_decimal::Decimal>,
    pub retry_count: i32,
    pub cache_status: String,
    pub final_status: String,
    pub error_class: Option<String>,
    pub error_code: Option<String>,
    pub fallback_used: bool,
    pub fallback_reason: Option<String>,
    pub attempted_providers: serde_json::Value,
    pub upstream_request_id: Option<String>,
    pub provider_response_id: Option<String>,
    pub provider_model_version: Option<String>,
    pub finish_reason: Option<String>,
    pub candidate_index: Option<i32>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub estimated_cost_usd: Option<rust_decimal::Decimal>,
    pub cache_key: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Input parameters for recording a new ledger entry.
#[derive(Debug, Clone)]
pub struct LedgerRecordInput {
    pub request_id: String,
    pub generation_id: String,
    pub route: String,
    pub request_type: String,
    pub provider: String,
    pub primary_provider: String,
    pub model: String,
    pub requested_model: String,
    pub latency_ms: Option<rust_decimal::Decimal>,
    pub retry_count: i32,
    pub cache_status: CacheStatus,
    pub final_status: String,
    pub error_class: Option<String>,
    pub error_code: Option<String>,
    pub fallback_used: bool,
    pub fallback_reason: Option<String>,
    pub attempted_providers: Vec<String>,
    pub upstream_request_id: Option<String>,
    pub provider_response_id: Option<String>,
    pub provider_model_version: Option<String>,
    pub finish_reason: Option<String>,
    pub candidate_index: Option<i32>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub estimated_cost_usd: Option<rust_decimal::Decimal>,
    pub cache_key: Option<String>,
    pub metadata: serde_json::Value,
    /// When the request completed (set on success or failure).
    pub completed_at: Option<DateTime<Utc>>,
}

/// Cache status for a ledger entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheStatus {
    Hit,
    Miss,
    Bypass,
}

impl CacheStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CacheStatus::Hit => "hit",
            CacheStatus::Miss => "miss",
            CacheStatus::Bypass => "bypass",
        }
    }
}

// ─── SQL ─────────────────────────────────────────────────────────────────────

const INSERT_SQL: &str = r#"
INSERT INTO llm_request_ledger (
    request_id, generation_id, route, request_type,
    provider, primary_provider, model, requested_model,
    latency_ms, retry_count, cache_status, final_status,
    error_class, error_code, fallback_used, fallback_reason,
    attempted_providers, upstream_request_id, provider_response_id,
    provider_model_version, finish_reason, candidate_index,
    input_tokens, output_tokens, total_tokens, estimated_cost_usd,
    cache_key, metadata, created_at, completed_at
) VALUES (
    $1, $2, $3, $4,
    $5, $6, $7, $8,
    $9, $10, $11, $12,
    $13, $14, $15, $16,
    $17, $18, $19,
    $20, $21, $22,
    $23, $24, $25, $26,
    $27, $28, NOW(), $29
)
RETURNING id, request_id, generation_id, route, request_type,
          provider, primary_provider, model, requested_model,
          latency_ms, retry_count, cache_status, final_status,
          error_class, error_code, fallback_used, fallback_reason,
          attempted_providers, upstream_request_id, provider_response_id,
          provider_model_version, finish_reason, candidate_index,
          input_tokens, output_tokens, total_tokens, estimated_cost_usd,
          cache_key, metadata, created_at, completed_at
"#;

// ─── Repository ─────────────────────────────────────────────────────────────

/// Repository over `llm_request_ledger` for recording LLM provider requests.
pub struct LedgerRepo {
    pool: PgPool,
}

impl LedgerRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Record a new ledger entry.
    ///
    /// Returns the inserted `LedgerEntry` with all fields populated.
    pub async fn record(&self, input: &LedgerRecordInput) -> Result<LedgerEntry, sqlx::Error> {
        let providers_json = serde_json::to_value(&input.attempted_providers)
            .unwrap_or_else(|_| serde_json::Value::Array(vec![]));

        let row = sqlx::query_as::<_, LedgerEntry>(INSERT_SQL)
            .bind(&input.request_id)
            .bind(&input.generation_id)
            .bind(&input.route)
            .bind(&input.request_type)
            .bind(&input.provider)
            .bind(&input.primary_provider)
            .bind(&input.model)
            .bind(&input.requested_model)
            .bind(input.latency_ms)
            .bind(input.retry_count)
            .bind(input.cache_status.as_str())
            .bind(&input.final_status)
            .bind(&input.error_class)
            .bind(&input.error_code)
            .bind(input.fallback_used)
            .bind(&input.fallback_reason)
            .bind(&providers_json)
            .bind(&input.upstream_request_id)
            .bind(&input.provider_response_id)
            .bind(&input.provider_model_version)
            .bind(&input.finish_reason)
            .bind(input.candidate_index)
            .bind(input.input_tokens)
            .bind(input.output_tokens)
            .bind(input.total_tokens)
            .bind(input.estimated_cost_usd)
            .bind(&input.cache_key)
            .bind(&input.metadata)
            .bind(input.completed_at)  // completed_at can be NULL if still in-flight
            .fetch_one(&self.pool)
            .await?;
        Ok(row)
    }

    /// Record a fully completed request with all metrics.
    pub async fn record_completed(
        &self,
        request_id: &str,
        generation_id: &str,
        route: &str,
        request_type: &str,
        provider: &str,
        primary_provider: &str,
        model: &str,
        requested_model: &str,
        latency_ms: Option<rust_decimal::Decimal>,
        retry_count: i32,
        cache_status: CacheStatus,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
        total_tokens: Option<i64>,
        estimated_cost_usd: Option<rust_decimal::Decimal>,
        fallback_used: bool,
        fallback_reason: Option<&str>,
        attempted_providers: Vec<String>,
        cache_key: Option<&str>,
        metadata: serde_json::Value,
    ) -> Result<LedgerEntry, sqlx::Error> {
        let now = Utc::now();
        let input = LedgerRecordInput {
            request_id: request_id.to_string(),
            generation_id: generation_id.to_string(),
            route: route.to_string(),
            request_type: request_type.to_string(),
            provider: provider.to_string(),
            primary_provider: primary_provider.to_string(),
            model: model.to_string(),
            requested_model: requested_model.to_string(),
            latency_ms,
            retry_count,
            cache_status,
            final_status: "completed".to_string(),
            error_class: None,
            error_code: None,
            fallback_used,
            fallback_reason: fallback_reason.map(|s| s.to_string()),
            attempted_providers,
            upstream_request_id: None,
            provider_response_id: None,
            provider_model_version: None,
            finish_reason: None,
            candidate_index: None,
            input_tokens,
            output_tokens,
            total_tokens,
            estimated_cost_usd,
            cache_key: cache_key.map(|s| s.to_string()),
            metadata,
            completed_at: Some(now),
        };
        self.record(&input).await
    }

    /// Record a failed request (provider error, timeout, etc.).
    pub async fn record_failure(
        &self,
        request_id: &str,
        generation_id: &str,
        route: &str,
        request_type: &str,
        provider: &str,
        primary_provider: &str,
        model: &str,
        requested_model: &str,
        cache_status: CacheStatus,
        error_class: Option<&str>,
        error_code: Option<&str>,
        fallback_used: bool,
        fallback_reason: Option<&str>,
        attempted_providers: Vec<String>,
        metadata: serde_json::Value,
    ) -> Result<LedgerEntry, sqlx::Error> {
        let input = LedgerRecordInput {
            request_id: request_id.to_string(),
            generation_id: generation_id.to_string(),
            route: route.to_string(),
            request_type: request_type.to_string(),
            provider: provider.to_string(),
            primary_provider: primary_provider.to_string(),
            model: model.to_string(),
            requested_model: requested_model.to_string(),
            latency_ms: None,
            retry_count: 0,
            cache_status,
            final_status: "failed".to_string(),
            error_class: error_class.map(|s| s.to_string()),
            error_code: error_code.map(|s| s.to_string()),
            fallback_used,
            fallback_reason: fallback_reason.map(|s| s.to_string()),
            attempted_providers,
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
            metadata,
            completed_at: Some(Utc::now()),
        };
        self.record(&input).await
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_status_as_str() {
        assert_eq!(CacheStatus::Hit.as_str(), "hit");
        assert_eq!(CacheStatus::Miss.as_str(), "miss");
        assert_eq!(CacheStatus::Bypass.as_str(), "bypass");
    }

    #[test]
    fn test_ledger_record_input_all_fields() {
        let input = LedgerRecordInput {
            request_id: "req-1".to_string(),
            generation_id: "gen-1".to_string(),
            route: "interpret".to_string(),
            request_type: "media_prompt_interpretation".to_string(),
            provider: "tencent".to_string(),
            primary_provider: "tencent".to_string(),
            model: "hy3:free".to_string(),
            requested_model: "hy3:free".to_string(),
            latency_ms: Some(rust_decimal::Decimal::new(1234, 2)), // 12.34 ms
            retry_count: 0,
            cache_status: CacheStatus::Miss,
            final_status: "completed".to_string(),
            error_class: None,
            error_code: None,
            fallback_used: false,
            fallback_reason: None,
            attempted_providers: vec!["tencent".to_string()],
            upstream_request_id: None,
            provider_response_id: Some("resp-abc".to_string()),
            provider_model_version: None,
            finish_reason: Some("stop".to_string()),
            candidate_index: Some(0),
            input_tokens: Some(150),
            output_tokens: Some(200),
            total_tokens: Some(350),
            estimated_cost_usd: Some(rust_decimal::Decimal::new(42, 6)), // 0.000042
            cache_key: Some("abc123...".to_string()),
            metadata: serde_json::json!({"source": "test"}),
            completed_at: None,
        };

        assert_eq!(input.route, "interpret");
        assert_eq!(input.request_type, "media_prompt_interpretation");
        assert_eq!(input.cache_status.as_str(), "miss");
        assert_eq!(input.total_tokens, Some(350));
    }

    #[test]
    fn test_ledger_record_input_failure() {
        let input = LedgerRecordInput {
            request_id: "req-2".to_string(),
            generation_id: "gen-2".to_string(),
            route: "respond".to_string(),
            request_type: "media_delivery_response".to_string(),
            provider: "tencent".to_string(),
            primary_provider: "tencent".to_string(),
            model: "hy3:free".to_string(),
            requested_model: "hy3:free".to_string(),
            latency_ms: None,
            retry_count: 2,
            cache_status: CacheStatus::Bypass,
            final_status: "failed".to_string(),
            error_class: Some("ProviderError".to_string()),
            error_code: Some("provider_unavailable".to_string()),
            fallback_used: true,
            fallback_reason: Some("primary provider failed".to_string()),
            attempted_providers: vec!["tencent".to_string(), "gemini".to_string()],
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
            metadata: serde_json::json!({"attempted_fallback": true}),
            completed_at: None,
        };

        assert_eq!(input.final_status, "failed");
        assert!(input.fallback_used);
        assert_eq!(input.attempted_providers.len(), 2);
    }

    #[test]
    fn test_cache_hit_input() {
        let input = LedgerRecordInput {
            request_id: "req-3".to_string(),
            generation_id: "gen-3".to_string(),
            route: "interpret".to_string(),
            request_type: "media_prompt_interpretation".to_string(),
            provider: "tencent".to_string(),
            primary_provider: "tencent".to_string(),
            model: "hy3:free".to_string(),
            requested_model: "hy3:free".to_string(),
            latency_ms: Some(rust_decimal::Decimal::new(5, 1)), // 0.5 ms
            retry_count: 0,
            cache_status: CacheStatus::Hit,
            final_status: "completed".to_string(),
            error_class: None,
            error_code: None,
            fallback_used: false,
            fallback_reason: None,
            attempted_providers: vec!["tencent".to_string()],
            upstream_request_id: None,
            provider_response_id: None,
            provider_model_version: None,
            finish_reason: Some("stop".to_string()),
            candidate_index: Some(0),
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            estimated_cost_usd: Some(rust_decimal::Decimal::ZERO),
            cache_key: Some("def456...".to_string()),
            metadata: serde_json::json!({"cache_source": "interpretation_cache"}),
            completed_at: None,
        };

        assert_eq!(input.cache_status.as_str(), "hit");
        assert_eq!(input.estimated_cost_usd, Some(rust_decimal::Decimal::ZERO));
    }

    #[test]
    fn test_sql_literal() {
        assert!(INSERT_SQL.contains("INSERT INTO llm_request_ledger"));
        assert!(INSERT_SQL.contains("RETURNING id"));
    }
}

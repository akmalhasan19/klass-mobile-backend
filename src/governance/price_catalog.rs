//! LLM price catalog over `llm_price_catalog` table.
//!
//! - `PriceCatalogRepo` — lookup active pricing for a provider/model pair
//! - `estimate_cost()` — compute estimated USD cost from token counts

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

// ─── Types ──────────────────────────────────────────────────────────────────

/// A row in the `llm_price_catalog` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PriceEntry {
    pub id: i64,
    pub provider: String,
    pub model: String,
    pub input_cost_per_unit_usd: Option<Decimal>,
    pub output_cost_per_unit_usd: Option<Decimal>,
    pub effective_from: DateTime<Utc>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Result of a cost estimate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CostEstimate {
    pub input_cost_usd: Decimal,
    pub output_cost_usd: Decimal,
    pub total_cost_usd: Decimal,
}

// ─── SQL ─────────────────────────────────────────────────────────────────────

const FIND_ACTIVE_SQL: &str = r#"
SELECT id, provider, model, input_cost_per_unit_usd, output_cost_per_unit_usd,
       effective_from, is_active, created_at, updated_at
FROM llm_price_catalog
WHERE provider = $1
  AND model = $2
  AND is_active = TRUE
  AND effective_from <= NOW()
ORDER BY effective_from DESC
LIMIT 1
"#;

// ─── Repository ──────────────────────────────────────────────────────────────

/// Repository over `llm_price_catalog`.
pub struct PriceCatalogRepo {
    pool: PgPool,
}

impl PriceCatalogRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Find the currently active price entry for a provider + model.
    pub async fn find_active(
        &self,
        provider: &str,
        model: &str,
    ) -> Result<Option<PriceEntry>, sqlx::Error> {
        sqlx::query_as::<_, PriceEntry>(FIND_ACTIVE_SQL)
            .bind(provider)
            .bind(model)
            .fetch_optional(&self.pool)
            .await
    }

    /// Estimate the cost of a request given token counts and optional unit costs.
    ///
    /// If `price_entry` is `None`, uses the default fallback costs.
    /// The unit is *per 1M tokens* (matching the DB column convention).
    pub async fn estimate_cost(
        &self,
        provider: &str,
        model: &str,
        input_tokens: i64,
        output_tokens: i64,
    ) -> CostEstimate {
        let entry = self.find_active(provider, model).await.ok().flatten();
        estimate_cost_inner(&entry, input_tokens, output_tokens)
    }
}

// ─── Pure cost calculation ──────────────────────────────────────────────────

/// Pure function — estimate cost from token counts and optional pricing.
///
/// Defaults (when no PriceEntry exists):
/// - Input: $0.10 / 1M tokens (xiaomi V4 Flash typical)
/// - Output: $0.40 / 1M tokens
pub fn estimate_cost_inner(
    price_entry: &Option<PriceEntry>,
    input_tokens: i64,
    output_tokens: i64,
) -> CostEstimate {
    let (input_rate, output_rate) = match price_entry {
        Some(entry) => (
            entry.input_cost_per_unit_usd.unwrap_or(Decimal::new(10, 2)), // $0.10 default
            entry.output_cost_per_unit_usd.unwrap_or(Decimal::new(40, 2)), // $0.40 default
        ),
        None => (
            Decimal::new(10, 2),  // $0.10 / 1M tokens
            Decimal::new(40, 2),  // $0.40 / 1M tokens
        ),
    };

    let tokens_per_unit = Decimal::new(1_000_000, 0);
    let input_tokens_dec = Decimal::new(input_tokens, 0);
    let output_tokens_dec = Decimal::new(output_tokens, 0);

    let input_cost = if input_tokens > 0 {
        (input_rate * input_tokens_dec) / tokens_per_unit
    } else {
        Decimal::ZERO
    };

    let output_cost = if output_tokens > 0 {
        (output_rate * output_tokens_dec) / tokens_per_unit
    } else {
        Decimal::ZERO
    };

    let total_cost = input_cost + output_cost;

    CostEstimate {
        input_cost_usd: input_cost,
        output_cost_usd: output_cost,
        total_cost_usd: total_cost,
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_price(input_rate: Decimal, output_rate: Decimal) -> PriceEntry {
        PriceEntry {
            id: 1,
            provider: "xiaomi".to_string(),
            model: "mimo-v2.5-pro".to_string(),
            input_cost_per_unit_usd: Some(input_rate),
            output_cost_per_unit_usd: Some(output_rate),
            effective_from: DateTime::from_timestamp(0, 0).unwrap(),
            is_active: true,
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
            updated_at: DateTime::from_timestamp(0, 0).unwrap(),
        }
    }

    #[test]
    fn test_estimate_cost_with_pricing() {
        // $0.10 / 1M input, $0.40 / 1M output
        let entry = Some(make_price(
            Decimal::new(10, 2),  // $0.10
            Decimal::new(40, 2),  // $0.40
        ));
        let cost = estimate_cost_inner(&entry, 1_000_000, 500_000);
        assert_eq!(cost.input_cost_usd, Decimal::new(10, 2));       // $0.10
        assert_eq!(cost.output_cost_usd, Decimal::new(20, 2));      // $0.20
        assert_eq!(cost.total_cost_usd, Decimal::new(30, 2));       // $0.30
    }

    #[test]
    fn test_estimate_cost_with_defaults() {
        // No price entry → use hardcoded defaults
        let cost = estimate_cost_inner(&None, 1_000_000, 500_000);
        assert_eq!(cost.input_cost_usd, Decimal::new(10, 2));       // $0.10
        assert_eq!(cost.output_cost_usd, Decimal::new(20, 2));      // $0.20
        assert_eq!(cost.total_cost_usd, Decimal::new(30, 2));       // $0.30
    }

    #[test]
    fn test_estimate_cost_zero_tokens() {
        let cost = estimate_cost_inner(&None, 0, 0);
        assert_eq!(cost.total_cost_usd, Decimal::ZERO);
    }

    #[test]
    fn test_estimate_cost_small_request() {
        let entry = Some(make_price(
            Decimal::new(10, 2),  // $0.10 / 1M
            Decimal::new(40, 2),  // $0.40 / 1M
        ));
        // 500 input tokens → $0.00005
        // 1000 output tokens → $0.0004
        let cost = estimate_cost_inner(&entry, 500, 1000);
        assert!(cost.input_cost_usd > Decimal::ZERO);
        assert!(cost.output_cost_usd > Decimal::ZERO);
        let expected_total = Decimal::new(45, 5); // 0.00045
        assert!((cost.total_cost_usd - expected_total).abs() < Decimal::new(1, 6));
    }

    #[test]
    fn test_estimate_cost_with_null_rates() {
        // If price entry has None for both rates, defaults kick in
        let entry = Some(PriceEntry {
            id: 2,
            provider: "unknown".to_string(),
            model: "unknown".to_string(),
            input_cost_per_unit_usd: None,
            output_cost_per_unit_usd: None,
            effective_from: DateTime::from_timestamp(0, 0).unwrap(),
            is_active: true,
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
            updated_at: DateTime::from_timestamp(0, 0).unwrap(),
        });
        let cost = estimate_cost_inner(&entry, 1_000_000, 1_000_000);
        assert_eq!(cost.total_cost_usd, Decimal::new(50, 2)); // $0.10 + $0.40 = $0.50
    }

    #[test]
    fn test_sql_literal() {
        assert!(FIND_ACTIVE_SQL.contains("SELECT"));
        assert!(FIND_ACTIVE_SQL.contains("llm_price_catalog"));
    }
}

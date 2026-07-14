//! LLM rate-limit governance over `llm_rate_limit_policies` and
//! `llm_rate_limit_buckets`.
//!
//! - `RateLimitPoliciesRepo` — CRUD over `llm_rate_limit_policies`
//! - `RateLimitBucketsRepo` — fixed-window counter, upsert with
//!   `ON CONFLICT DO UPDATE SET request_count = ...`
//! - `preflight_check()` — per-route budget check before LLM calls
//! - Exhaustion actions: `deny` (reject) or `degrade` (allow with warning)
//!
//! Port of `app/governance.py` — `AdapterGovernanceService`.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

// ─── Constants ──────────────────────────────────────────────────────────────

const ROUTE_ALL: &str = "all";
const ROUTE_INTERPRET: &str = "interpret";
const ROUTE_RESPOND: &str = "respond";
const DIMENSION_WILDCARD: &str = "*";

// ─── Types ──────────────────────────────────────────────────────────────────

/// The route discriminator for rate-limit policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RateLimitRoute {
    All,
    Interpret,
    Respond,
}

impl RateLimitRoute {
    pub fn as_str(&self) -> &'static str {
        match self {
            RateLimitRoute::All => ROUTE_ALL,
            RateLimitRoute::Interpret => ROUTE_INTERPRET,
            RateLimitRoute::Respond => ROUTE_RESPOND,
        }
    }
}

impl From<&str> for RateLimitRoute {
    fn from(s: &str) -> Self {
        match s {
            ROUTE_INTERPRET => RateLimitRoute::Interpret,
            ROUTE_RESPOND => RateLimitRoute::Respond,
            _ => RateLimitRoute::All,
        }
    }
}

/// Scope type for a rate-limit policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeType {
    Global,
    Route,
    Provider,
    Model,
}

impl ScopeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScopeType::Global => "global",
            ScopeType::Route => "route",
            ScopeType::Provider => "provider",
            ScopeType::Model => "model",
        }
    }
}

/// Window unit for a rate-limit policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowUnit {
    Minute,
    Hour,
    Day,
}

impl WindowUnit {
    pub fn as_str(&self) -> &'static str {
        match self {
            WindowUnit::Minute => "minute",
            WindowUnit::Hour => "hour",
            WindowUnit::Day => "day",
        }
    }
}

/// A rate-limit policy row from `llm_rate_limit_policies`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RateLimitPolicy {
    pub id: i64,
    pub scope_type: String,
    pub strategy: String,
    pub route: String,
    pub provider: String,
    pub model: String,
    pub window_unit: String,
    pub max_requests: Option<i64>,
    pub max_input_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub max_total_tokens: Option<i64>,
    pub max_estimated_cost_usd: Option<rust_decimal::Decimal>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A rate-limit bucket row from `llm_rate_limit_buckets`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RateLimitBucket {
    pub id: i64,
    pub policy_id: i64,
    pub scope_type: String,
    pub strategy: String,
    pub route: String,
    pub provider: String,
    pub model: String,
    pub window_unit: String,
    pub window_started_at: DateTime<Utc>,
    pub window_ends_at: DateTime<Utc>,
    pub request_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub estimated_cost_usd: rust_decimal::Decimal,
    pub deny_count: i64,
    pub last_request_id: Option<String>,
    pub last_generation_id: Option<String>,
    pub last_seen_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Result of a preflight check.
#[derive(Debug, Clone)]
pub struct PreflightDecision {
    pub allowed: bool,
    pub action: ExhaustionAction,
}

/// Exhaustion action when limits are exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExhaustionAction {
    Allow,
    Deny,
    Degrade,
}

/// Parameters to build a fixed-window bucket mutation.
#[derive(Debug, Clone)]
pub struct BucketMutation {
    pub policy_id: i64,
    pub route: String,
    pub provider: String,
    pub model: String,
    pub window_unit: String,
    pub request_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub estimated_cost_usd: rust_decimal::Decimal,
    pub deny_count: i64,
    pub last_request_id: Option<String>,
    pub last_generation_id: Option<String>,
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Compute the start of the fixed window for the given time and unit.
fn window_start(at: DateTime<Utc>, unit: WindowUnit) -> DateTime<Utc> {
    use chrono::Timelike;
    match unit {
        WindowUnit::Minute => {
            at.with_second(0).unwrap().with_nanosecond(0).unwrap()
        }
        WindowUnit::Hour => {
            at.with_minute(0).unwrap().with_second(0).unwrap().with_nanosecond(0).unwrap()
        }
        WindowUnit::Day => {
            at.with_hour(0).unwrap().with_minute(0).unwrap()
                .with_second(0).unwrap().with_nanosecond(0).unwrap()
        }
    }
}

/// Compute the end of the fixed window given its start and unit.
fn window_end(start: DateTime<Utc>, unit: WindowUnit) -> DateTime<Utc> {
    match unit {
        WindowUnit::Minute => start + chrono::Duration::minutes(1),
        WindowUnit::Hour => start + chrono::Duration::hours(1),
        WindowUnit::Day => start + chrono::Duration::days(1),
    }
}

// ─── SQL ─────────────────────────────────────────────────────────────────────

const POLICY_UPSERT_SQL: &str = r#"
INSERT INTO llm_rate_limit_policies
    (scope_type, strategy, route, provider, model, window_unit,
     max_requests, max_input_tokens, max_output_tokens, max_total_tokens,
     max_estimated_cost_usd, enabled, updated_at)
VALUES ($1, 'fixed_window', $2, $3, $4, $5,
        $6, $7, $8, $9, $10, TRUE, NOW())
ON CONFLICT (scope_type, route, provider, model, window_unit)
DO UPDATE SET
    strategy        = EXCLUDED.strategy,
    max_requests    = EXCLUDED.max_requests,
    max_input_tokens  = EXCLUDED.max_input_tokens,
    max_output_tokens = EXCLUDED.max_output_tokens,
    max_total_tokens  = EXCLUDED.max_total_tokens,
    max_estimated_cost_usd = EXCLUDED.max_estimated_cost_usd,
    enabled         = EXCLUDED.enabled,
    updated_at      = NOW()
RETURNING *
"#;

/// Fetch applicable enabled policies for the given route / provider / model.
const APPLICABLE_POLICIES_SQL: &str = r#"
SELECT id, scope_type, strategy, route, provider, model, window_unit,
       max_requests, max_input_tokens, max_output_tokens, max_total_tokens,
       max_estimated_cost_usd, enabled, created_at, updated_at
FROM llm_rate_limit_policies
WHERE enabled = TRUE
  AND (route = $1 OR route = 'all')
  AND (provider = $2 OR provider = '*')
  AND (model = $3 OR model = '*')
ORDER BY
    CASE window_unit WHEN 'minute' THEN 1 WHEN 'hour' THEN 2 ELSE 3 END ASC,
    CASE scope_type WHEN 'route' THEN 1 WHEN 'provider' THEN 2 WHEN 'model' THEN 3 ELSE 4 END ASC,
    id ASC
"#;

const BUCKET_UPSERT_SQL: &str = r#"
INSERT INTO llm_rate_limit_buckets
    (policy_id, scope_type, strategy, route, provider, model, window_unit,
     window_started_at, window_ends_at,
     request_count, input_tokens, output_tokens, total_tokens,
     estimated_cost_usd, deny_count,
     last_request_id, last_generation_id, last_seen_at, updated_at)
VALUES ($1, $2, 'fixed_window', $3, $4, $5, $6,
        $7, $8,
        $9, $10, $11, $12, $13, $14,
        $15, $16, NOW(), NOW())
ON CONFLICT (policy_id, window_started_at)
DO UPDATE SET
    request_count       = llm_rate_limit_buckets.request_count + EXCLUDED.request_count,
    input_tokens        = llm_rate_limit_buckets.input_tokens + EXCLUDED.input_tokens,
    output_tokens       = llm_rate_limit_buckets.output_tokens + EXCLUDED.output_tokens,
    total_tokens        = llm_rate_limit_buckets.total_tokens + EXCLUDED.total_tokens,
    estimated_cost_usd  = llm_rate_limit_buckets.estimated_cost_usd + EXCLUDED.estimated_cost_usd,
    deny_count          = llm_rate_limit_buckets.deny_count + EXCLUDED.deny_count,
    last_request_id     = COALESCE(EXCLUDED.last_request_id, llm_rate_limit_buckets.last_request_id),
    last_generation_id  = COALESCE(EXCLUDED.last_generation_id, llm_rate_limit_buckets.last_generation_id),
    last_seen_at        = NOW(),
    updated_at          = NOW()
RETURNING *
"#;

// ─── Repositories ───────────────────────────────────────────────────────────

/// Repository over `llm_rate_limit_policies`.
pub struct RateLimitPoliciesRepo {
    pool: PgPool,
}

impl RateLimitPoliciesRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Upsert a default policy (idempotent via ON CONFLICT).
    pub async fn upsert_default(
        &self,
        route: &str,
        scope_type: &str,
        window_unit: &str,
        max_requests: Option<i64>,
        max_input_tokens: Option<i64>,
        max_output_tokens: Option<i64>,
        max_total_tokens: Option<i64>,
        max_estimated_cost_usd: Option<rust_decimal::Decimal>,
    ) -> Result<RateLimitPolicy, sqlx::Error> {
        let row = sqlx::query_as::<_, RateLimitPolicy>(POLICY_UPSERT_SQL)
            .bind(scope_type)
            .bind(route)
            .bind(DIMENSION_WILDCARD)
            .bind(DIMENSION_WILDCARD)
            .bind(window_unit)
            .bind(max_requests)
            .bind(max_input_tokens)
            .bind(max_output_tokens)
            .bind(max_total_tokens)
            .bind(max_estimated_cost_usd)
            .fetch_one(&self.pool)
            .await?;
        Ok(row)
    }

    /// Fetch applicable enabled policies for the given dimensions.
    pub async fn find_applicable(
        &self,
        route: &str,
        provider: &str,
        model: &str,
    ) -> Result<Vec<RateLimitPolicy>, sqlx::Error> {
        let rows = sqlx::query_as::<_, RateLimitPolicy>(APPLICABLE_POLICIES_SQL)
            .bind(route)
            .bind(provider)
            .bind(model)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }
}

/// Repository over `llm_rate_limit_buckets`.
pub struct RateLimitBucketsRepo {
    pool: PgPool,
}

impl RateLimitBucketsRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Upsert a fixed-window bucket, incrementing counters atomically.
    pub async fn increment(
        &self,
        policy: &RateLimitPolicy,
        mutation: &BucketMutation,
        now: DateTime<Utc>,
    ) -> Result<RateLimitBucket, sqlx::Error> {
        let unit = match policy.window_unit.as_str() {
            "minute" => WindowUnit::Minute,
            "hour" => WindowUnit::Hour,
            _ => WindowUnit::Day,
        };
        let ws = window_start(now, unit);
        let we = window_end(ws, unit);

        let row = sqlx::query_as::<_, RateLimitBucket>(BUCKET_UPSERT_SQL)
            .bind(policy.id)
            .bind(&policy.scope_type)
            .bind(&mutation.route)
            .bind(&mutation.provider)
            .bind(&mutation.model)
            .bind(&mutation.window_unit)
            .bind(ws)
            .bind(we)
            .bind(mutation.request_count)
            .bind(mutation.input_tokens)
            .bind(mutation.output_tokens)
            .bind(mutation.total_tokens)
            .bind(mutation.estimated_cost_usd)
            .bind(mutation.deny_count)
            .bind(&mutation.last_request_id)
            .bind(&mutation.last_generation_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row)
    }

    /// Record usage for a successful request (increment counters).
    pub async fn record_usage(
        &self,
        policy: &RateLimitPolicy,
        input_tokens: i64,
        output_tokens: i64,
        total_tokens: i64,
        estimated_cost_usd: rust_decimal::Decimal,
        request_id: &str,
        generation_id: &str,
        now: DateTime<Utc>,
    ) -> Result<RateLimitBucket, sqlx::Error> {
        self.increment(
            policy,
            &BucketMutation {
                policy_id: policy.id,
                route: policy.route.clone(),
                provider: policy.provider.clone(),
                model: policy.model.clone(),
                window_unit: policy.window_unit.clone(),
                request_count: 1,
                input_tokens,
                output_tokens,
                total_tokens,
                estimated_cost_usd,
                deny_count: 0,
                last_request_id: Some(request_id.to_string()),
                last_generation_id: Some(generation_id.to_string()),
            },
            now,
        )
        .await
    }

    /// Record a denied request (increment deny_count only).
    pub async fn record_denial(
        &self,
        policy: &RateLimitPolicy,
        request_id: &str,
        generation_id: &str,
        now: DateTime<Utc>,
    ) -> Result<RateLimitBucket, sqlx::Error> {
        self.increment(
            policy,
            &BucketMutation {
                policy_id: policy.id,
                route: policy.route.clone(),
                provider: policy.provider.clone(),
                model: policy.model.clone(),
                window_unit: policy.window_unit.clone(),
                request_count: 0,
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
                estimated_cost_usd: rust_decimal::Decimal::ZERO,
                deny_count: 1,
                last_request_id: Some(request_id.to_string()),
                last_generation_id: Some(generation_id.to_string()),
            },
            now,
        )
        .await
    }
}

// ─── Preflight check ────────────────────────────────────────────────────────

/// Check whether a request should be allowed, denied, or degraded.
///
/// - `route`: `"interpret"` or `"respond"`
/// - `provider`: e.g. `"deepseek"`
/// - `model`: e.g. `"deepseek-v4-flash"`
/// - `projected_cost_usd`: estimated cost of this request (for budget checks)
///
/// Returns a `PreflightDecision`:
/// - `Allow` — proceed with the request
/// - `Deny` — reject; caller should return 429
/// - `Degrade` — allow but mark as degraded (e.g. skip caching)
pub async fn preflight_check(
    policies_repo: &RateLimitPoliciesRepo,
    buckets_repo: &RateLimitBucketsRepo,
    route: &str,
    provider: &str,
    model: &str,
    projected_cost_usd: Option<rust_decimal::Decimal>,
    request_id: &str,
    generation_id: &str,
) -> Result<PreflightDecision, sqlx::Error> {
    let now = Utc::now();
    let policies = policies_repo
        .find_applicable(route, provider, model)
        .await?;

    if policies.is_empty() {
        return Ok(PreflightDecision {
            allowed: true,
            action: ExhaustionAction::Allow,
        });
    }

    let estimated_cost = projected_cost_usd.unwrap_or(rust_decimal::Decimal::new(1, 4)); // 0.0001 default

    for policy in &policies {
        // Fetch current bucket for this policy's window
        let unit = match policy.window_unit.as_str() {
            "minute" => WindowUnit::Minute,
            "hour" => WindowUnit::Hour,
            _ => WindowUnit::Day,
        };
        let ws = window_start(now, unit);

        // Build a zero-increment mutation — we just want to check limits
        let check_mutation = BucketMutation {
            policy_id: policy.id,
            route: policy.route.clone(),
            provider: policy.provider.clone(),
            model: policy.model.clone(),
            window_unit: policy.window_unit.clone(),
            request_count: 0,
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            estimated_cost_usd: rust_decimal::Decimal::ZERO,
            deny_count: 0,
            last_request_id: None,
            last_generation_id: None,
        };
        let bucket = buckets_repo.increment(policy, &check_mutation, now).await?;

        // Check request count limit
        if let Some(max_req) = policy.max_requests {
            if bucket.request_count + 1 > max_req {
                buckets_repo
                    .record_denial(policy, request_id, generation_id, now)
                    .await?;
                return Ok(exhausted_decision(route, provider, model));
            }
        }

        // Check cost budget
        if let Some(max_cost) = policy.max_estimated_cost_usd {
            let projected = bucket.estimated_cost_usd + estimated_cost;
            if projected > max_cost {
                buckets_repo
                    .record_denial(policy, request_id, generation_id, now)
                    .await?;
                return Ok(exhausted_decision(route, provider, model));
            }
        }
    }

    Ok(PreflightDecision {
        allowed: true,
        action: ExhaustionAction::Allow,
    })
}

pub fn exhausted_decision(route: &str, _provider: &str, _model: &str) -> PreflightDecision {
    // The action is "deny" for interpret, "degrade" for respond.
    let action = match route {
        ROUTE_INTERPRET => ExhaustionAction::Deny,
        ROUTE_RESPOND => ExhaustionAction::Degrade,
        _ => ExhaustionAction::Deny,
    };
    PreflightDecision {
        allowed: action == ExhaustionAction::Degrade,
        action,
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn test_window_start_minute() {
        let dt = DateTime::from_timestamp(1000, 123_456_789).unwrap(); // with non-zero seconds
        let ws = window_start(dt, WindowUnit::Minute);
        assert_eq!(ws.second(), 0);
        assert_eq!(ws.nanosecond(), 0);
    }

    #[test]
    fn test_window_start_hour() {
        let dt = DateTime::from_timestamp(3600 + 120 + 30, 0).unwrap(); // 1h2m30s
        let ws = window_start(dt, WindowUnit::Hour);
        assert_eq!(ws.minute(), 0);
        assert_eq!(ws.second(), 0);
    }

    #[test]
    fn test_window_start_day() {
        let dt = DateTime::from_timestamp(86400 + 3600 + 120, 0).unwrap(); // 1d1h2m
        let ws = window_start(dt, WindowUnit::Day);
        assert_eq!(ws.hour(), 0);
        assert_eq!(ws.minute(), 0);
        assert_eq!(ws.second(), 0);
    }

    #[test]
    fn test_window_end_minute() {
        let start = DateTime::from_timestamp(1000, 0).unwrap();
        let end = window_end(start, WindowUnit::Minute);
        assert_eq!(end.timestamp(), start.timestamp() + 60);
    }

    #[test]
    fn test_window_end_hour() {
        let start = DateTime::from_timestamp(3600, 0).unwrap();
        let end = window_end(start, WindowUnit::Hour);
        assert_eq!(end.timestamp(), start.timestamp() + 3600);
    }

    #[test]
    fn test_window_end_day() {
        let start = DateTime::from_timestamp(86400, 0).unwrap();
        let end = window_end(start, WindowUnit::Day);
        assert_eq!(end.timestamp(), start.timestamp() + 86400);
    }

    #[test]
    fn test_route_conversion() {
        assert_eq!(RateLimitRoute::from("interpret"), RateLimitRoute::Interpret);
        assert_eq!(RateLimitRoute::from("respond"), RateLimitRoute::Respond);
        assert_eq!(RateLimitRoute::from("all"), RateLimitRoute::All);
        assert_eq!(RateLimitRoute::from("unknown"), RateLimitRoute::All);
    }

    #[test]
    fn test_exhausted_decision_interpret_is_deny() {
        let d = exhausted_decision("interpret", "dp", "m");
        assert!(!d.allowed);
        assert_eq!(d.action, ExhaustionAction::Deny);
    }

    #[test]
    fn test_exhausted_decision_respond_is_degrade() {
        let d = exhausted_decision("respond", "dp", "m");
        assert!(d.allowed);
        assert_eq!(d.action, ExhaustionAction::Degrade);
    }

    #[test]
    fn test_scope_type_as_str() {
        assert_eq!(ScopeType::Global.as_str(), "global");
        assert_eq!(ScopeType::Route.as_str(), "route");
    }

    #[test]
    fn test_window_unit_as_str() {
        assert_eq!(WindowUnit::Minute.as_str(), "minute");
        assert_eq!(WindowUnit::Hour.as_str(), "hour");
        assert_eq!(WindowUnit::Day.as_str(), "day");
    }

    #[test]
    fn test_sql_literals() {
        assert!(POLICY_UPSERT_SQL.contains("INSERT INTO llm_rate_limit_policies"));
        assert!(APPLICABLE_POLICIES_SQL.contains("SELECT"));
        assert!(BUCKET_UPSERT_SQL.contains("ON CONFLICT (policy_id, window_started_at)"));
    }
}

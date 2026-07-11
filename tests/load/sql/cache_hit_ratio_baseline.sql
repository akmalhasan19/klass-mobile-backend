-- =============================================================================
-- Cache Hit Ratio Baseline (LLM Adapter DB)
-- =============================================================================
-- Run these against the LLM Adapter PostgreSQL database.
-- These queries work with the LLM Adapter schema (interpretation_cache_entries,
-- delivery_cache_entries, llm_request_ledger, llm_request_daily_route_aggregates).
-- =============================================================================

-- ---------------------------------------------------------------------------
-- 1. Cache Hit Ratio — Interpret Route
-- ---------------------------------------------------------------------------
SELECT
    'interpret' AS route,
    COUNT(*) AS total_entries,
    COUNT(*) FILTER (WHERE expires_at > NOW()) AS active_entries,
    SUM(hit_count) AS total_hits,
    AVG(hit_count) AS avg_hits_per_entry,
    ROUND(
        SUM(hit_count)::numeric / NULLIF(COUNT(*), 0),
        4
    ) AS avg_hit_ratio
FROM interpretation_cache_entries;

-- ---------------------------------------------------------------------------
-- 2. Cache Hit Ratio — Delivery Route
-- ---------------------------------------------------------------------------
SELECT
    'respond' AS route,
    COUNT(*) AS total_entries,
    COUNT(*) FILTER (WHERE expires_at > NOW()) AS active_entries,
    SUM(hit_count) AS total_hits,
    AVG(hit_count) AS avg_hits_per_entry,
    ROUND(
        SUM(hit_count)::numeric / NULLIF(COUNT(*), 0),
        4
    ) AS avg_hit_ratio
FROM delivery_cache_entries;

-- ---------------------------------------------------------------------------
-- 3. Cache Hit Ratio — From Ledger (most accurate)
-- ---------------------------------------------------------------------------
SELECT
    route,
    COUNT(*) AS total_requests,
    COUNT(*) FILTER (WHERE cache_status = 'hit') AS cache_hits,
    COUNT(*) FILTER (WHERE cache_status = 'miss') AS cache_misses,
    COUNT(*) FILTER (WHERE cache_status = 'bypass') AS cache_bypasses,
    ROUND(
        COUNT(*) FILTER (WHERE cache_status = 'hit')::numeric / NULLIF(COUNT(*), 0),
        6
    ) AS hit_ratio,
    ROUND(
        COUNT(*) FILTER (WHERE cache_status = 'miss')::numeric / NULLIF(COUNT(*), 0),
        6
    ) AS miss_ratio
FROM llm_request_ledger
WHERE created_at >= NOW() - INTERVAL '7 days'
GROUP BY route;

-- ---------------------------------------------------------------------------
-- 4. Cache Hit Ratio — Daily Trend (from VIEW)
-- ---------------------------------------------------------------------------
SELECT
    usage_date,
    route,
    request_count,
    cache_hit_count,
    cache_miss_count,
    cache_hit_ratio,
    estimated_cost_usd
FROM llm_request_daily_route_aggregates
WHERE usage_date >= CURRENT_DATE - INTERVAL '30 days'
ORDER BY usage_date DESC, route;

-- ---------------------------------------------------------------------------
-- 5. Cache Hit Ratio — By Provider/Model
-- ---------------------------------------------------------------------------
SELECT
    route,
    provider,
    model,
    COUNT(*) AS total_requests,
    COUNT(*) FILTER (WHERE cache_status = 'hit') AS cache_hits,
    ROUND(
        COUNT(*) FILTER (WHERE cache_status = 'hit')::numeric / NULLIF(COUNT(*), 0),
        6
    ) AS hit_ratio,
    AVG(latency_ms) AS avg_latency_ms,
    SUM(estimated_cost_usd) AS total_cost_usd
FROM llm_request_ledger
WHERE created_at >= NOW() - INTERVAL '7 days'
GROUP BY route, provider, model
ORDER BY route, total_requests DESC;

-- ---------------------------------------------------------------------------
-- 6. Cost Summary
-- ---------------------------------------------------------------------------
SELECT
    route,
    COUNT(*) AS total_requests,
    SUM(input_tokens) AS total_input_tokens,
    SUM(output_tokens) AS total_output_tokens,
    SUM(estimated_cost_usd) AS total_cost_usd,
    AVG(estimated_cost_usd) AS avg_cost_per_request,
    COUNT(*) FILTER (WHERE cache_status = 'hit') AS cache_saves,
    SUM(CASE WHEN cache_status = 'hit' THEN estimated_cost_usd ELSE 0 END) AS cost_saved_by_cache
FROM llm_request_ledger
WHERE created_at >= NOW() - INTERVAL '7 days'
GROUP BY route;

-- ---------------------------------------------------------------------------
-- 7. Rate Limit Bucket Status
-- ---------------------------------------------------------------------------
SELECT
    rlb.route,
    rlb.provider,
    rlb.model,
    rlb.window_unit,
    rlb.request_count,
    rlb.total_tokens,
    rlb.estimated_cost_usd,
    rlb.deny_count,
    rlb.window_started_at,
    rlb.window_ends_at
FROM rate_limit_buckets rlb
WHERE rlb.window_ends_at > NOW()
ORDER BY rlb.route, rlb.window_unit;

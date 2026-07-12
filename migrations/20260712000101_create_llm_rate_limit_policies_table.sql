CREATE TABLE llm_rate_limit_policies (
    id BIGSERIAL PRIMARY KEY,
    scope_type VARCHAR(32) NOT NULL CHECK (scope_type IN ('global', 'provider', 'model', 'route')),
    strategy VARCHAR(32) NOT NULL DEFAULT 'fixed_window' CHECK (strategy IN ('fixed_window')),
    route VARCHAR(32) NOT NULL DEFAULT 'all' CHECK (route IN ('all', 'interpret', 'respond')),
    provider VARCHAR(100) NOT NULL DEFAULT '*',
    model VARCHAR(200) NOT NULL DEFAULT '*',
    window_unit VARCHAR(16) NOT NULL CHECK (window_unit IN ('minute', 'hour', 'day')),
    max_requests BIGINT NULL CHECK (max_requests IS NULL OR max_requests >= 0),
    max_input_tokens BIGINT NULL CHECK (max_input_tokens IS NULL OR max_input_tokens >= 0),
    max_output_tokens BIGINT NULL CHECK (max_output_tokens IS NULL OR max_output_tokens >= 0),
    max_total_tokens BIGINT NULL CHECK (max_total_tokens IS NULL OR max_total_tokens >= 0),
    max_estimated_cost_usd NUMERIC(20, 8) NULL CHECK (max_estimated_cost_usd IS NULL OR max_estimated_cost_usd >= 0),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (
        max_requests IS NOT NULL
        OR max_input_tokens IS NOT NULL
        OR max_output_tokens IS NOT NULL
        OR max_total_tokens IS NOT NULL
        OR max_estimated_cost_usd IS NOT NULL
    ),
    CHECK ((scope_type <> 'global')   OR (route = 'all' AND provider = '*' AND model = '*')),
    CHECK ((scope_type <> 'route')    OR (route <> 'all' AND provider = '*' AND model = '*')),
    CHECK ((scope_type <> 'provider') OR (provider <> '*' AND model = '*')),
    CHECK ((scope_type <> 'model')    OR model <> '*')
);

CREATE UNIQUE INDEX idx_llm_rate_limit_policies_scope
    ON llm_rate_limit_policies (scope_type, route, provider, model, window_unit);

CREATE INDEX idx_llm_rate_limit_policies_lookup
    ON llm_rate_limit_policies (enabled, route, provider, model, window_unit);

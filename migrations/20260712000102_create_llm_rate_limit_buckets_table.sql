CREATE TABLE llm_rate_limit_buckets (
    id BIGSERIAL PRIMARY KEY,
    policy_id BIGINT NOT NULL REFERENCES llm_rate_limit_policies (id) ON DELETE CASCADE,
    scope_type VARCHAR(32) NOT NULL CHECK (scope_type IN ('global', 'provider', 'model', 'route')),
    strategy VARCHAR(32) NOT NULL DEFAULT 'fixed_window' CHECK (strategy IN ('fixed_window')),
    route VARCHAR(32) NOT NULL CHECK (route IN ('all', 'interpret', 'respond')),
    provider VARCHAR(100) NOT NULL,
    model VARCHAR(200) NOT NULL,
    window_unit VARCHAR(16) NOT NULL CHECK (window_unit IN ('minute', 'hour', 'day')),
    window_started_at TIMESTAMPTZ NOT NULL,
    window_ends_at TIMESTAMPTZ NOT NULL,
    request_count BIGINT NOT NULL DEFAULT 0 CHECK (request_count >= 0),
    input_tokens BIGINT NOT NULL DEFAULT 0 CHECK (input_tokens >= 0),
    output_tokens BIGINT NOT NULL DEFAULT 0 CHECK (output_tokens >= 0),
    total_tokens BIGINT NOT NULL DEFAULT 0 CHECK (total_tokens >= 0),
    estimated_cost_usd NUMERIC(20, 8) NOT NULL DEFAULT 0 CHECK (estimated_cost_usd >= 0),
    deny_count BIGINT NOT NULL DEFAULT 0 CHECK (deny_count >= 0),
    last_request_id VARCHAR(255) NULL,
    last_generation_id VARCHAR(100) NULL,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (window_ends_at > window_started_at)
);

CREATE UNIQUE INDEX idx_llm_rate_limit_buckets_policy_window
    ON llm_rate_limit_buckets (policy_id, window_started_at);

CREATE INDEX idx_llm_rate_limit_buckets_lookup
    ON llm_rate_limit_buckets (route, provider, model, window_unit, window_started_at DESC);

CREATE INDEX idx_llm_rate_limit_buckets_window_ends
    ON llm_rate_limit_buckets (window_ends_at);

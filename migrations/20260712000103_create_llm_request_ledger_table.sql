CREATE TABLE llm_request_ledger (
    id BIGSERIAL PRIMARY KEY,
    request_id VARCHAR(255) NOT NULL,
    generation_id VARCHAR(100) NOT NULL,
    route VARCHAR(32) NOT NULL CHECK (route IN ('interpret', 'respond')),
    request_type VARCHAR(100) NOT NULL,
    provider VARCHAR(100) NOT NULL,
    primary_provider VARCHAR(100) NOT NULL,
    model VARCHAR(200) NOT NULL,
    requested_model VARCHAR(200) NOT NULL,
    latency_ms NUMERIC(12, 2) NULL,
    retry_count INT NOT NULL DEFAULT 0 CHECK (retry_count >= 0),
    cache_status VARCHAR(16) NOT NULL CHECK (cache_status IN ('hit', 'miss', 'bypass')),
    final_status VARCHAR(32) NOT NULL,
    error_class VARCHAR(255) NULL,
    error_code VARCHAR(100) NULL,
    fallback_used BOOLEAN NOT NULL DEFAULT FALSE,
    fallback_reason VARCHAR(100) NULL,
    attempted_providers JSONB NOT NULL DEFAULT '[]'::jsonb,
    upstream_request_id VARCHAR(255) NULL,
    provider_response_id VARCHAR(255) NULL,
    provider_model_version VARCHAR(200) NULL,
    finish_reason VARCHAR(100) NULL,
    candidate_index INT NULL,
    input_tokens BIGINT NULL CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens BIGINT NULL CHECK (output_tokens IS NULL OR output_tokens >= 0),
    total_tokens BIGINT NULL CHECK (total_tokens IS NULL OR total_tokens >= 0),
    estimated_cost_usd NUMERIC(20, 8) NULL CHECK (estimated_cost_usd IS NULL OR estimated_cost_usd >= 0),
    cache_key CHAR(64) NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ NULL,
    CHECK (jsonb_typeof(attempted_providers) = 'array'),
    CHECK (jsonb_typeof(metadata) = 'object')
);

CREATE UNIQUE INDEX idx_llm_request_ledger_request_id ON llm_request_ledger (request_id);
CREATE INDEX idx_llm_request_ledger_created_at ON llm_request_ledger (created_at DESC);
CREATE INDEX idx_llm_request_ledger_route_created ON llm_request_ledger (route, created_at DESC);
CREATE INDEX idx_llm_request_ledger_provider_model ON llm_request_ledger (provider, model, created_at DESC);
CREATE INDEX idx_llm_request_ledger_generation_id ON llm_request_ledger (generation_id);

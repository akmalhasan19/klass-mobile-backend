-- ══════════════════════════════════════════════════════════════════════════════
-- SAFE MIGRATION SCRIPT FOR RENDER POSTGRESQL
-- Run this in: Render Dashboard → PostgreSQL → Query tab
-- Uses IF NOT EXISTS so it's safe to run even if some tables already exist.
-- ══════════════════════════════════════════════════════════════════════════════

-- ─────────────────────────────────────────────────────────────────────────────
-- 1. LLM Cache Entries (for response caching)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS llm_cache_entries (
    id BIGSERIAL PRIMARY KEY,
    cache_key CHAR(64) NOT NULL,
    route VARCHAR(16) NOT NULL CHECK (route IN ('interpret', 'respond')),
    request_payload JSONB NOT NULL,
    response_payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    hit_count BIGINT NOT NULL DEFAULT 0 CHECK (hit_count >= 0),
    last_hit_at TIMESTAMPTZ NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_llm_cache_entries_cache_key ON llm_cache_entries (cache_key);
CREATE INDEX IF NOT EXISTS idx_llm_cache_entries_lookup ON llm_cache_entries (cache_key, expires_at);
CREATE INDEX IF NOT EXISTS idx_llm_cache_entries_expires_interpret ON llm_cache_entries (expires_at) WHERE route = 'interpret';
CREATE INDEX IF NOT EXISTS idx_llm_cache_entries_expires_respond ON llm_cache_entries (expires_at) WHERE route = 'respond';
CREATE INDEX IF NOT EXISTS idx_llm_cache_entries_route_created ON llm_cache_entries (route, created_at);

-- ─────────────────────────────────────────────────────────────────────────────
-- 2. LLM Rate Limit Policies (governance - REQUIRED BY WORKER)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS llm_rate_limit_policies (
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

CREATE UNIQUE INDEX IF NOT EXISTS idx_llm_rate_limit_policies_scope
    ON llm_rate_limit_policies (scope_type, route, provider, model, window_unit);
CREATE INDEX IF NOT EXISTS idx_llm_rate_limit_policies_lookup
    ON llm_rate_limit_policies (enabled, route, provider, model, window_unit);

-- ─────────────────────────────────────────────────────────────────────────────
-- 3. LLM Rate Limit Buckets (tracks usage per policy window)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS llm_rate_limit_buckets (
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

CREATE UNIQUE INDEX IF NOT EXISTS idx_llm_rate_limit_buckets_policy_window
    ON llm_rate_limit_buckets (policy_id, window_started_at);
CREATE INDEX IF NOT EXISTS idx_llm_rate_limit_buckets_lookup
    ON llm_rate_limit_buckets (route, provider, model, window_unit, window_started_at DESC);
CREATE INDEX IF NOT EXISTS idx_llm_rate_limit_buckets_window_ends
    ON llm_rate_limit_buckets (window_ends_at);

-- ─────────────────────────────────────────────────────────────────────────────
-- 4. LLM Request Ledger (audit trail for all LLM calls)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS llm_request_ledger (
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

CREATE UNIQUE INDEX IF NOT EXISTS idx_llm_request_ledger_request_id ON llm_request_ledger (request_id);
CREATE INDEX IF NOT EXISTS idx_llm_request_ledger_created_at ON llm_request_ledger (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_llm_request_ledger_route_created ON llm_request_ledger (route, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_llm_request_ledger_provider_model ON llm_request_ledger (provider, model, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_llm_request_ledger_generation_id ON llm_request_ledger (generation_id);

-- ─────────────────────────────────────────────────────────────────────────────
-- 5. LLM Price Catalog (cost estimation per model)
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS llm_price_catalog (
    id BIGSERIAL PRIMARY KEY,
    provider VARCHAR(100) NOT NULL,
    model VARCHAR(200) NOT NULL,
    input_cost_per_unit_usd NUMERIC(20, 8) NULL CHECK (input_cost_per_unit_usd IS NULL OR input_cost_per_unit_usd >= 0),
    output_cost_per_unit_usd NUMERIC(20, 8) NULL CHECK (output_cost_per_unit_usd IS NULL OR output_cost_per_unit_usd >= 0),
    effective_from TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (input_cost_per_unit_usd IS NOT NULL OR output_cost_per_unit_usd IS NOT NULL)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_llm_price_catalog_effective
    ON llm_price_catalog (provider, model, effective_from);
CREATE INDEX IF NOT EXISTS idx_llm_price_catalog_lookup
    ON llm_price_catalog (provider, model, is_active, effective_from DESC);

-- ─────────────────────────────────────────────────────────────────────────────
-- 6. Ensure media_generations has async job tracking columns
-- ─────────────────────────────────────────────────────────────────────────────
DO $$ BEGIN
    ALTER TABLE media_generations ADD COLUMN generation_job_id UUID NULL;
EXCEPTION WHEN duplicate_column THEN END; $$;

DO $$ BEGIN
    ALTER TABLE media_generations ADD COLUMN generation_status VARCHAR(20) NULL;
EXCEPTION WHEN duplicate_column THEN END; $$;

DO $$ BEGIN
    ALTER TABLE media_generations ADD COLUMN s3_object_key VARCHAR(1024) NULL;
EXCEPTION WHEN duplicate_column THEN END; $$;

DO $$ BEGIN
    ALTER TABLE media_generations ADD COLUMN presigned_download_url TEXT NULL;
EXCEPTION WHEN duplicate_column THEN END; $$;

DO $$ BEGIN
    ALTER TABLE media_generations ADD COLUMN presigned_url_expires_at TIMESTAMPTZ NULL;
EXCEPTION WHEN duplicate_column THEN END; $$;

DO $$ BEGIN
    ALTER TABLE media_generations ADD COLUMN generation_error_code VARCHAR(100) NULL;
EXCEPTION WHEN duplicate_column THEN END; $$;

DO $$ BEGIN
    ALTER TABLE media_generations ADD COLUMN generation_error_message TEXT NULL;
EXCEPTION WHEN duplicate_column THEN END; $$;

DO $$ BEGIN
    ALTER TABLE media_generations ADD COLUMN clarification_state JSONB NULL;
EXCEPTION WHEN duplicate_column THEN END; $$;

DO $$ BEGIN
    ALTER TABLE media_generations ADD COLUMN clarified_at TIMESTAMPTZ NULL;
EXCEPTION WHEN duplicate_column THEN END; $$;

DO $$ BEGIN
    ALTER TABLE media_generations ADD COLUMN clarification_skipped BOOLEAN NOT NULL DEFAULT FALSE;
EXCEPTION WHEN duplicate_column THEN END; $$;

-- Add indexes for new columns (safe to re-run)
CREATE INDEX IF NOT EXISTS idx_media_generations_generation_job_id ON media_generations (generation_job_id);
CREATE INDEX IF NOT EXISTS idx_media_generations_generation_status ON media_generations (generation_status);
CREATE INDEX IF NOT EXISTS idx_media_generations_clarification ON media_generations (clarified_at) WHERE clarified_at IS NOT NULL;

-- ══════════════════════════════════════════════════════════════════════════════
-- DONE! After running this script, restart the gateway service on Render.
-- ══════════════════════════════════════════════════════════════════════════════

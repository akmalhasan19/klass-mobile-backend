CREATE TABLE llm_cache_entries (
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

CREATE UNIQUE INDEX idx_llm_cache_entries_cache_key ON llm_cache_entries (cache_key);
CREATE INDEX idx_llm_cache_entries_lookup ON llm_cache_entries (cache_key, expires_at);

CREATE INDEX idx_llm_cache_entries_expires_interpret
    ON llm_cache_entries (expires_at) WHERE route = 'interpret';

CREATE INDEX idx_llm_cache_entries_expires_respond
    ON llm_cache_entries (expires_at) WHERE route = 'respond';

CREATE INDEX idx_llm_cache_entries_route_created
    ON llm_cache_entries (route, created_at);

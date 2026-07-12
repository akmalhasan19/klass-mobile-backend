CREATE TABLE llm_price_catalog (
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

CREATE UNIQUE INDEX idx_llm_price_catalog_effective
    ON llm_price_catalog (provider, model, effective_from);

CREATE INDEX idx_llm_price_catalog_lookup
    ON llm_price_catalog (provider, model, is_active, effective_from DESC);

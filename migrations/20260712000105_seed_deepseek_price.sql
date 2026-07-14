-- Seed Deepseek V4 Flash pricing into `llm_price_catalog`.
-- These values match the OpenRouter pricing for deepseek/deepseek-v4-flash
-- as of July 2026 (input: $0.10 / 1M tokens, output: $0.40 / 1M tokens).
--
-- This migration is idempotent: ON CONFLICT (provider, model, effective_from)
-- updates the rates if they have changed.

INSERT INTO llm_price_catalog
    (provider, model, input_cost_per_unit_usd, output_cost_per_unit_usd,
     effective_from, is_active)
VALUES
    ('deepseek', 'deepseek-v4-flash', 0.10000000, 0.40000000, NOW(), TRUE)
ON CONFLICT (provider, model, effective_from)
DO UPDATE SET
    input_cost_per_unit_usd  = EXCLUDED.input_cost_per_unit_usd,
    output_cost_per_unit_usd = EXCLUDED.output_cost_per_unit_usd,
    is_active                = EXCLUDED.is_active,
    updated_at               = NOW();

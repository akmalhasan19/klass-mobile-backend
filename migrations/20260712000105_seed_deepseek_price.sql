INSERT INTO llm_price_catalog
    (provider, model, input_cost_per_unit_usd, output_cost_per_unit_usd,
     effective_from, is_active)
VALUES
    ('minimax', 'minimax/minimax-m3', 0.10000000, 0.40000000, NOW(), TRUE)
ON CONFLICT (provider, model, effective_from)
DO UPDATE SET
    input_cost_per_unit_usd  = EXCLUDED.input_cost_per_unit_usd,
    output_cost_per_unit_usd = EXCLUDED.output_cost_per_unit_usd,
    is_active                = EXCLUDED.is_active,
    updated_at               = NOW();

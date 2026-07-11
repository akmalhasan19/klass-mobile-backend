-- =============================================================================
-- Performance Baseline Extraction Queries
-- =============================================================================
-- Run these against Neon PostgreSQL to extract baseline metrics.
-- Requires: media_generations.orchestration_audit_payload populated.
-- =============================================================================

-- ---------------------------------------------------------------------------
-- 1. Per-Endpoint Latency (from StructuredApiLogger logs)
-- ---------------------------------------------------------------------------
-- Parse api.log JSON lines or query if stored in DB.
-- If using log files, use jq or a script to extract duration_ms.
--
-- Expected output: p50, p95, p99 per endpoint path

-- ---------------------------------------------------------------------------
-- 2. Media Generation E2E Latency (from orchestration_audit_payload)
-- ---------------------------------------------------------------------------
SELECT
    CASE
        WHEN (orchestration_audit_payload->'timing'->>'total_duration_ms') IS NOT NULL
        THEN 'has_timing'
        ELSE 'no_timing'
    END AS timing_availability,
    COUNT(*) AS total_generations,

    -- E2E p50/p95/p99
    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY
        (orchestration_audit_payload->'timing'->>'total_duration_ms')::numeric
    ) AS e2e_p50_ms,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY
        (orchestration_audit_payload->'timing'->>'total_duration_ms')::numeric
    ) AS e2e_p95_ms,
    PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY
        (orchestration_audit_payload->'timing'->>'total_duration_ms')::numeric
    ) AS e2e_p99_ms,
    AVG((orchestration_audit_payload->'timing'->>'total_duration_ms')::numeric) AS e2e_avg_ms,
    MAX((orchestration_audit_payload->'timing'->>'total_duration_ms')::numeric) AS e2e_max_ms,
    MIN((orchestration_audit_payload->'timing'->>'total_duration_ms')::numeric) AS e2e_min_ms

FROM media_generations
WHERE orchestration_audit_payload->'timing'->>'total_duration_ms' IS NOT NULL
  AND created_at >= NOW() - INTERVAL '7 days'
GROUP BY timing_availability;

-- ---------------------------------------------------------------------------
-- 3. Per-Status Duration Breakdown
-- ---------------------------------------------------------------------------
WITH status_durations AS (
    SELECT
        id,
        status,
        jsonb_each_text(orchestration_audit_payload->'timing'->'status_durations_ms') AS kv
    FROM media_generations
    WHERE orchestration_audit_payload->'timing'->'status_durations_ms' IS NOT NULL
      AND created_at >= NOW() - INTERVAL '7 days'
)
SELECT
    kv.key AS lifecycle_status,
    COUNT(*) AS sample_count,
    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY kv.value::numeric) AS p50_ms,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY kv.value::numeric) AS p95_ms,
    PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY kv.value::numeric) AS p99_ms,
    AVG(kv.value::numeric) AS avg_ms
FROM status_durations, jsonb_each_text(orchestration_audit_payload->'timing'->'status_durations_ms') kv
GROUP BY kv.key
ORDER BY
    CASE kv.key
        WHEN 'queued' THEN 0
        WHEN 'interpreting' THEN 1
        WHEN 'classified' THEN 2
        WHEN 'generating' THEN 3
        WHEN 'uploading' THEN 4
        WHEN 'publishing' THEN 5
        WHEN 'completed' THEN 6
        ELSE 7
    END;

-- ---------------------------------------------------------------------------
-- 4. Status Distribution (throughput)
-- ---------------------------------------------------------------------------
SELECT
    status,
    COUNT(*) AS count,
    ROUND(COUNT(*) * 100.0 / SUM(COUNT(*)) OVER (), 2) AS percentage,
    COUNT(*) FILTER (WHERE created_at >= NOW() - INTERVAL '24 hours') AS last_24h,
    COUNT(*) FILTER (WHERE created_at >= NOW() - INTERVAL '7 days') AS last_7d
FROM media_generations
GROUP BY status
ORDER BY count DESC;

-- ---------------------------------------------------------------------------
-- 5. Throughput (generations per hour)
-- ---------------------------------------------------------------------------
SELECT
    date_trunc('hour', created_at) AS hour,
    COUNT(*) AS generations_submitted,
    COUNT(*) FILTER (WHERE status = 'completed') AS completed,
    COUNT(*) FILTER (WHERE status = 'failed') AS failed,
    ROUND(
        COUNT(*) FILTER (WHERE status = 'completed') * 100.0 / NULLIF(COUNT(*), 0),
        2
    ) AS completion_rate_pct
FROM media_generations
WHERE created_at >= NOW() - INTERVAL '7 days'
GROUP BY date_trunc('hour', created_at)
ORDER BY hour DESC;

-- ---------------------------------------------------------------------------
-- 6. Error Rate by Error Code
-- ---------------------------------------------------------------------------
SELECT
    error_code,
    COUNT(*) AS error_count,
    ROUND(COUNT(*) * 100.0 / SUM(COUNT(*)) OVER (), 2) AS percentage,
    MAX(created_at) AS last_seen,
    array_agg(DISTINCT status) AS statuses
FROM media_generations
WHERE error_code IS NOT NULL
  AND created_at >= NOW() - INTERVAL '30 days'
GROUP BY error_code
ORDER BY error_count DESC;

-- ---------------------------------------------------------------------------
-- 7. Failure Mode Catalog (from orchestration_audit_payload)
-- ---------------------------------------------------------------------------
SELECT
    (orchestration_audit_payload->'latest_error'->>'error_code') AS error_code,
    (orchestration_audit_payload->'latest_error'->>'error_class') AS error_class,
    (orchestration_audit_payload->'latest_error'->>'message') AS error_message,
    (orchestration_audit_payload->'latest_error'->>'retryable') AS retryable,
    COUNT(*) AS occurrences,
    MAX(created_at) AS last_seen,
    array_agg(DISTINCT status) AS final_statuses
FROM media_generations
WHERE orchestration_audit_payload->'latest_error' IS NOT NULL
  AND created_at >= NOW() - INTERVAL '30 days'
GROUP BY
    orchestration_audit_payload->'latest_error'->>'error_code',
    orchestration_audit_payload->'latest_error'->>'error_class',
    orchestration_audit_payload->'latest_error'->>'message',
    orchestration_audit_payload->'latest_error'->>'retryable'
ORDER BY occurrences DESC;

-- ---------------------------------------------------------------------------
-- 8. Retry Statistics
-- ---------------------------------------------------------------------------
SELECT
    (orchestration_audit_payload->'job'->>'attempt')::int AS attempt_number,
    COUNT(*) AS generation_count,
    COUNT(*) FILTER (WHERE status = 'completed') AS completed,
    COUNT(*) FILTER (WHERE status = 'failed') AS failed
FROM media_generations
WHERE orchestration_audit_payload->'job'->>'attempt' IS NOT NULL
  AND created_at >= NOW() - INTERVAL '30 days'
GROUP BY (orchestration_audit_payload->'job'->>'attempt')::int
ORDER BY attempt_number;

-- ---------------------------------------------------------------------------
-- 9. Output Type Distribution
-- ---------------------------------------------------------------------------
SELECT
    resolved_output_type,
    COUNT(*) AS count,
    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY
        (orchestration_audit_payload->'timing'->>'total_duration_ms')::numeric
    ) AS p50_e2e_ms,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY
        (orchestration_audit_payload->'timing'->>'total_duration_ms')::numeric
    ) AS p95_e2e_ms
FROM media_generations
WHERE resolved_output_type IS NOT NULL
  AND created_at >= NOW() - INTERVAL '7 days'
GROUP BY resolved_output_type
ORDER BY count DESC;

-- ---------------------------------------------------------------------------
-- 10. Queue Depth (pending jobs)
-- ---------------------------------------------------------------------------
SELECT
    queue,
    COUNT(*) AS pending_jobs,
    MIN(created_at) AS oldest_job_at,
    EXTRACT(EPOCH FROM (NOW() - MIN(created_at))) AS oldest_age_seconds
FROM jobs
GROUP BY queue;

-- ---------------------------------------------------------------------------
-- 11. Failed Jobs Summary
-- ---------------------------------------------------------------------------
SELECT
    queue,
    COUNT(*) AS failed_count,
    MAX(failed_at) AS last_failure,
    LEFT(exception, 200) AS exception_preview
FROM failed_jobs
GROUP BY queue
ORDER BY failed_count DESC;

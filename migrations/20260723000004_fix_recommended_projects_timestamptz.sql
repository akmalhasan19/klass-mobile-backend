-- ══════════════════════════════════════════════════════════════════════════════
-- FIX: Alter recommended_projects timestamp columns to TIMESTAMPTZ
-- Run this in: Neon Dashboard → SQL Editor
-- ══════════════════════════════════════════════════════════════════════════════

-- Fix1: Alter created_at to TIMESTAMPTZ
ALTER TABLE recommended_projects
    ALTER COLUMN created_at TYPE TIMESTAMPTZ
    USING created_at AT TIME ZONE 'UTC';

-- Fix 2: Alter updated_at to TIMESTAMPTZ
ALTER TABLE recommended_projects
    ALTER COLUMN updated_at TYPE TIMESTAMPTZ
    USING updated_at AT TIME ZONE 'UTC';

-- Fix 3: Also fix starts_at and ends_at if they exist as TIMESTAMP
ALTER TABLE recommended_projects
    ALTER COLUMN starts_at TYPE TIMESTAMPTZ
    USING starts_at AT TIME ZONE 'UTC';

ALTER TABLE recommended_projects
    ALTER COLUMN ends_at TYPE TIMESTAMPTZ
    USING ends_at AT TIME ZONE 'UTC';

-- Verify column types
SELECT column_name, data_type
FROM information_schema.columns
WHERE table_name = 'recommended_projects'
  AND column_name IN ('created_at', 'updated_at', 'starts_at', 'ends_at');

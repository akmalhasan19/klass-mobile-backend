-- ══════════════════════════════════════════════════════════════════════════════
-- FIX: Update existing recommended_projects with NULL created_at/updated_at
-- Run this in: Neon Dashboard → SQL Editor
-- ══════════════════════════════════════════════════════════════════════════════

-- Fix1: Set NULL created_at to NOW()
UPDATE recommended_projects SET created_at = NOW() WHERE created_at IS NULL;

-- Fix 2: Set NULL updated_at to NOW()
UPDATE recommended_projects SET updated_at = NOW() WHERE updated_at IS NULL;

-- Verify: all rows should now have non-null timestamps
SELECT id, title, created_at, updated_at FROM recommended_projects;

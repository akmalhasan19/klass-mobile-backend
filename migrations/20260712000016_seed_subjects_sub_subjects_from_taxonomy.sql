-- Seed subjects and sub_subjects from embedded taxonomy data.
--
-- This migration is documented as the official entry point for sub-task 4.9.
-- The actual seeding logic is implemented in Rust at runtime:
--   `src/db/seed.rs` — `seed_if_empty()` via `AppState::new()`
--
-- The Rust seeder reads the embedded `subjects.json` taxonomy data,
-- extracts unique subjects (by slug), inserts them into the `subjects` table,
-- then inserts all associated sub_subjects. Uses ON CONFLICT DO NOTHING for
-- idempotency and a transaction for atomicity.
--
-- Advisory lock ID: 20260712000016 (same as this migration number).
--
-- If manual seeding is needed (e.g. offline), run the following from
-- a Rust context that has the embedded JSON and a DB connection:
--   klass_gateway::db::seed::seed_if_empty(&pool).await?;

-- First verify the tables exist (created in migration 000002)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_name = 'subjects'
    ) THEN
        RAISE EXCEPTION 'subjects table does not exist — run migration 000002 first';
    END IF;
END $$;

-- Check if data already exists — if so, skip
DO $$
DECLARE
    subject_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO subject_count FROM subjects;

    IF subject_count > 0 THEN
        RAISE NOTICE 'subjects table already has % rows — skipping seed', subject_count;
    ELSE
        RAISE NOTICE 'subjects table is empty — seed will be performed by Rust startup seeder';
        RAISE NOTICE 'See src/db/seed.rs for the implementation';
    END IF;
END $$;

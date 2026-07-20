-- Add clarification fields to media_generations for the prompt clarification
-- flow (Phase 2: Database & API Integration).
--
-- These columns track the clarification conversation state, when the teacher
-- completed clarification, and whether they skipped the clarification flow.

ALTER TABLE media_generations
    ADD COLUMN clarification_state JSONB NULL,
    ADD COLUMN clarified_at TIMESTAMPTZ NULL,
    ADD COLUMN clarification_skipped BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS idx_media_generations_clarification
    ON media_generations (clarified_at)
    WHERE clarified_at IS NOT NULL;

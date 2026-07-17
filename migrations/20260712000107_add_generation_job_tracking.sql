-- Add job tracking columns to media_generations for the async media generation
-- pipeline (Sync -> Async with direct S3/R2 upload + reliable webhook callback).
--
-- These columns let the Rust Gateway track the async generation lifecycle
-- (pending -> processing -> completed/failed) driven by Python Arq worker
-- webhook callbacks, store the S3 object reference + presigned download URL,
-- and capture any generation error details.

ALTER TABLE media_generations
    ADD COLUMN generation_job_id UUID NULL,
    ADD COLUMN generation_status VARCHAR(20) NULL,
    ADD COLUMN s3_object_key VARCHAR(1024) NULL,
    ADD COLUMN presigned_download_url TEXT NULL,
    ADD COLUMN presigned_url_expires_at TIMESTAMPTZ NULL,
    ADD COLUMN generation_error_code VARCHAR(100) NULL,
    ADD COLUMN generation_error_message TEXT NULL;

ALTER TABLE media_generations
    ADD CONSTRAINT chk_media_generations_generation_status
    CHECK (
        generation_status IS NULL
        OR generation_status IN ('pending', 'processing', 'completed', 'failed')
    );

CREATE INDEX idx_media_generations_generation_job_id ON media_generations (generation_job_id);
CREATE INDEX idx_media_generations_generation_status ON media_generations (generation_status);

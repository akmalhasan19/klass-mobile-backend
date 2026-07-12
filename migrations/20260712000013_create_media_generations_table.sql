CREATE TABLE media_generations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    generated_from_id UUID NULL,
    is_regeneration BOOLEAN NOT NULL DEFAULT FALSE,
    teacher_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    subject_id BIGINT NULL REFERENCES subjects (id) ON DELETE SET NULL,
    sub_subject_id BIGINT NULL REFERENCES sub_subjects (id) ON DELETE SET NULL,
    topic_id UUID NULL,
    content_id UUID NULL,
    recommended_project_id BIGINT NULL,
    raw_prompt TEXT NOT NULL,
    request_fingerprint VARCHAR(64) NOT NULL,
    active_duplicate_key VARCHAR(64) NULL,
    preferred_output_type VARCHAR(10) NOT NULL DEFAULT 'auto',
    resolved_output_type VARCHAR(10) NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'queued',
    llm_provider VARCHAR(100) NULL,
    llm_model VARCHAR(200) NULL,
    generator_provider VARCHAR(100) NULL,
    generator_model VARCHAR(200) NULL,
    interpretation_payload JSONB NULL,
    interpretation_audit_payload JSONB NULL,
    generation_spec_payload JSONB NULL,
    decision_payload JSONB NULL,
    orchestration_audit_payload JSONB NULL,
    delivery_payload JSONB NULL,
    generator_service_response JSONB NULL,
    storage_path VARCHAR(1024) NULL,
    file_url TEXT NULL,
    thumbnail_url TEXT NULL,
    mime_type VARCHAR(255) NULL,
    error_code VARCHAR(100) NULL,
    error_message TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_media_generations_teacher_created ON media_generations (teacher_id, created_at);
CREATE INDEX idx_media_generations_status_created ON media_generations (status, created_at);
CREATE INDEX idx_media_generations_teacher_fingerprint ON media_generations (teacher_id, request_fingerprint);
CREATE UNIQUE INDEX idx_media_generations_duplicate_key ON media_generations (active_duplicate_key) WHERE active_duplicate_key IS NOT NULL;
CREATE INDEX idx_media_generations_generated_from ON media_generations (generated_from_id);

ALTER TABLE media_generations ADD CONSTRAINT fk_media_generations_generated_from FOREIGN KEY (generated_from_id) REFERENCES media_generations (id) ON DELETE SET NULL;
ALTER TABLE media_generations ADD CONSTRAINT fk_media_generations_topic_id FOREIGN KEY (topic_id) REFERENCES topics (id) ON DELETE SET NULL;
ALTER TABLE media_generations ADD CONSTRAINT fk_media_generations_content_id FOREIGN KEY (content_id) REFERENCES contents (id) ON DELETE SET NULL;
ALTER TABLE media_generations ADD CONSTRAINT fk_media_generations_recommended_project_id FOREIGN KEY (recommended_project_id) REFERENCES recommended_projects (id) ON DELETE SET NULL;

ALTER TABLE marketplace_tasks ADD CONSTRAINT fk_marketplace_tasks_media_generation_id FOREIGN KEY (media_generation_id) REFERENCES media_generations (id) ON DELETE SET NULL;

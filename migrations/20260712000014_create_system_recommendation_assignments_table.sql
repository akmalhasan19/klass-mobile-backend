CREATE TABLE system_recommendation_assignments (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    recommendation_key VARCHAR(255) NOT NULL,
    recommendation_item_id VARCHAR(255) NOT NULL,
    source_type VARCHAR(255) NOT NULL,
    source_reference VARCHAR(255) NOT NULL,
    subject_id BIGINT NULL REFERENCES subjects (id) ON DELETE SET NULL,
    sub_subject_id BIGINT NULL REFERENCES sub_subjects (id) ON DELETE SET NULL,
    first_distributed_at TIMESTAMPTZ NOT NULL,
    last_distributed_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_sra_user_recommendation ON system_recommendation_assignments (user_id, recommendation_key);
CREATE INDEX idx_sra_source ON system_recommendation_assignments (source_type, source_reference);
CREATE INDEX idx_sra_sub_subject ON system_recommendation_assignments (sub_subject_id, recommendation_key);
CREATE INDEX idx_sra_subject_id ON system_recommendation_assignments (subject_id);
CREATE INDEX idx_sra_last_distributed_at ON system_recommendation_assignments (last_distributed_at);

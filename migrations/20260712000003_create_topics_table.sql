CREATE TABLE topics (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title VARCHAR(255) NOT NULL,
    teacher_id VARCHAR(255) NOT NULL,
    sub_subject_id BIGINT NULL,
    thumbnail_url TEXT NULL,
    is_published BOOLEAN NOT NULL DEFAULT TRUE,
    "order" INT NOT NULL DEFAULT 0,
    owner_user_id BIGINT NULL,
    ownership_status VARCHAR(50) NOT NULL DEFAULT 'legacy_unresolved',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_topics_order ON topics ("order");
CREATE INDEX idx_topics_ownership_status ON topics (ownership_status);
CREATE INDEX idx_topics_sub_subject_id ON topics (sub_subject_id);

ALTER TABLE topics ADD CONSTRAINT fk_topics_sub_subject_id FOREIGN KEY (sub_subject_id) REFERENCES sub_subjects (id) ON DELETE SET NULL;
ALTER TABLE topics ADD CONSTRAINT fk_topics_owner_user_id FOREIGN KEY (owner_user_id) REFERENCES users (id) ON DELETE SET NULL;

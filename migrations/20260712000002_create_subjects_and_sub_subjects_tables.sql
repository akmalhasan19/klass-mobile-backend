CREATE TABLE subjects (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL,
    description TEXT NULL,
    display_order INT NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_subjects_slug ON subjects (slug);
CREATE INDEX idx_subjects_active_order ON subjects (is_active, display_order);

CREATE TABLE sub_subjects (
    id BIGSERIAL PRIMARY KEY,
    subject_id BIGINT NOT NULL REFERENCES subjects (id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL,
    description TEXT NULL,
    display_order INT NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_sub_subjects_subject_slug ON sub_subjects (subject_id, slug);
CREATE INDEX idx_sub_subjects_active_order ON sub_subjects (subject_id, is_active, display_order);

ALTER TABLE users ADD CONSTRAINT fk_users_primary_subject_id FOREIGN KEY (primary_subject_id) REFERENCES subjects (id) ON DELETE SET NULL;
CREATE INDEX idx_users_primary_subject_id ON users (primary_subject_id);

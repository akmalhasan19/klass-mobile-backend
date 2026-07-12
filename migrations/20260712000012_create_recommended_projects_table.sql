CREATE TABLE recommended_projects (
    id BIGSERIAL PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    description TEXT NULL,
    thumbnail_url TEXT NULL,
    project_file_url TEXT NULL,
    ratio VARCHAR(10) NOT NULL DEFAULT '16:9',
    project_type VARCHAR(100) NULL,
    tags JSONB NULL,
    modules JSONB NULL,
    source_type VARCHAR(100) NOT NULL,
    source_reference VARCHAR(255) NULL,
    source_payload JSONB NULL,
    display_priority INT NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    starts_at TIMESTAMPTZ NULL,
    ends_at TIMESTAMPTZ NULL,
    created_by BIGINT NULL REFERENCES users (id) ON DELETE SET NULL,
    updated_by BIGINT NULL REFERENCES users (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_recommended_projects_source_type ON recommended_projects (source_type);
CREATE INDEX idx_recommended_projects_source_ref ON recommended_projects (source_type, source_reference);
CREATE INDEX idx_recommended_projects_display_priority ON recommended_projects (display_priority);
CREATE INDEX idx_recommended_projects_is_active ON recommended_projects (is_active);
CREATE INDEX idx_recommended_projects_starts_at ON recommended_projects (starts_at);
CREATE INDEX idx_recommended_projects_ends_at ON recommended_projects (ends_at);

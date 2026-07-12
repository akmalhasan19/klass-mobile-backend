CREATE TABLE marketplace_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    content_id UUID NOT NULL REFERENCES contents (id) ON DELETE CASCADE,
    media_generation_id UUID NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'taken', 'done')),
    task_type VARCHAR(20) NOT NULL DEFAULT 'bid',
    description TEXT NULL,
    creator_id VARCHAR(255) NULL,
    suggested_freelancer_id BIGINT NULL REFERENCES users (id) ON DELETE SET NULL,
    attachment_url VARCHAR(2048) NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_marketplace_tasks_task_type ON marketplace_tasks (task_type);
CREATE INDEX idx_marketplace_tasks_media_generation_id ON marketplace_tasks (media_generation_id);
CREATE INDEX idx_marketplace_tasks_suggested_freelancer ON marketplace_tasks (suggested_freelancer_id);

CREATE TABLE contents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    topic_id UUID NOT NULL REFERENCES topics (id) ON DELETE CASCADE,
    type VARCHAR(20) NOT NULL CHECK (type IN ('module', 'quiz', 'brief')),
    title VARCHAR(255) NULL,
    data JSONB NULL,
    media_url TEXT NULL,
    is_published BOOLEAN NOT NULL DEFAULT TRUE,
    "order" INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_contents_order ON contents ("order");
CREATE INDEX idx_contents_topic_id ON contents (topic_id);

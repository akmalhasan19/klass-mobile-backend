CREATE TABLE freelancer_matches (
    id BIGSERIAL PRIMARY KEY,
    media_generation_id UUID NOT NULL REFERENCES media_generations (id) ON DELETE CASCADE,
    freelancer_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    match_score DOUBLE PRECISION NOT NULL DEFAULT 0,
    portfolio_relevance_score DOUBLE PRECISION NOT NULL DEFAULT 0,
    success_rate DOUBLE PRECISION NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_freelancer_matches_gen_freelancer ON freelancer_matches (media_generation_id, freelancer_id);
CREATE INDEX idx_freelancer_matches_media_gen ON freelancer_matches (media_generation_id);
CREATE INDEX idx_freelancer_matches_freelancer ON freelancer_matches (freelancer_id);
CREATE INDEX idx_freelancer_matches_score ON freelancer_matches (match_score);

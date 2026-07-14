use async_trait::async_trait;
use chrono::NaiveDateTime;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FreelancerMatch {
    pub id: i64,
    pub media_generation_id: Uuid,
    pub freelancer_id: i64,
    pub match_score: f64,
    pub portfolio_relevance_score: f64,
    pub success_rate: f64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct FreelancerMatchScores {
    pub match_score: f64,
    pub portfolio_relevance_score: f64,
    pub success_rate: f64,
}

#[async_trait]
pub trait FreelancerMatchesRepo: Send + Sync {
    async fn upsert(
        &self,
        media_generation_id: Uuid,
        freelancer_id: i64,
        scores: &FreelancerMatchScores,
    ) -> anyhow::Result<FreelancerMatch>;

    async fn find_for_generation(
        &self,
        media_generation_id: Uuid,
    ) -> anyhow::Result<Vec<FreelancerMatch>>;
}

pub struct PgFreelancerMatchesRepo {
    pool: PgPool,
}

impl PgFreelancerMatchesRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl FreelancerMatchesRepo for PgFreelancerMatchesRepo {
    async fn upsert(
        &self,
        media_generation_id: Uuid,
        freelancer_id: i64,
        scores: &FreelancerMatchScores,
    ) -> anyhow::Result<FreelancerMatch> {
        let row = sqlx::query_as::<_, FreelancerMatch>(
            r#"
            INSERT INTO freelancer_matches
                (media_generation_id, freelancer_id,
                 match_score, portfolio_relevance_score, success_rate,
                 created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
            ON CONFLICT (media_generation_id, freelancer_id)
            DO UPDATE SET
                match_score = EXCLUDED.match_score,
                portfolio_relevance_score = EXCLUDED.portfolio_relevance_score,
                success_rate = EXCLUDED.success_rate,
                updated_at = NOW()
            RETURNING
                id, media_generation_id, freelancer_id,
                match_score, portfolio_relevance_score, success_rate,
                created_at, updated_at
            "#,
        )
        .bind(media_generation_id)
        .bind(freelancer_id)
        .bind(scores.match_score)
        .bind(scores.portfolio_relevance_score)
        .bind(scores.success_rate)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "failed to upsert freelancer match (gen={media_generation_id}, freelancer={freelancer_id}): {e}"
            )
        })?;

        Ok(row)
    }

    async fn find_for_generation(
        &self,
        media_generation_id: Uuid,
    ) -> anyhow::Result<Vec<FreelancerMatch>> {
        let rows = sqlx::query_as::<_, FreelancerMatch>(
            r#"
            SELECT
                id, media_generation_id, freelancer_id,
                match_score, portfolio_relevance_score, success_rate,
                created_at, updated_at
            FROM freelancer_matches
            WHERE media_generation_id = $1
            ORDER BY match_score DESC
            "#,
        )
        .bind(media_generation_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "failed to find freelancer matches for generation {media_generation_id}: {e}"
            )
        })?;

        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_freelancer_match_struct() {
        let row = FreelancerMatch {
            id: 1,
            media_generation_id: Uuid::new_v4(),
            freelancer_id: 42,
            match_score: 0.85,
            portfolio_relevance_score: 0.75,
            success_rate: 0.92,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };
        assert_eq!(row.freelancer_id, 42);
        assert!((row.match_score - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_freelancer_match_scores_struct() {
        let scores = FreelancerMatchScores {
            match_score: 0.85,
            portfolio_relevance_score: 0.75,
            success_rate: 0.92,
        };
        assert!((scores.match_score - 0.85).abs() < f64::EPSILON);
        assert!((scores.portfolio_relevance_score - 0.75).abs() < f64::EPSILON);
        assert!((scores.success_rate - 0.92).abs() < f64::EPSILON);
    }

    #[test]
    fn test_freelancer_match_scores_roundtrip() {
        let original = FreelancerMatchScores {
            match_score: 0.85,
            portfolio_relevance_score: 0.75,
            success_rate: 0.92,
        };

        let cloned = FreelancerMatchScores {
            match_score: 0.85,
            portfolio_relevance_score: 0.75,
            success_rate: 0.92,
        };

        assert!((original.match_score - cloned.match_score).abs() < f64::EPSILON);
        assert!(
            (original.portfolio_relevance_score - cloned.portfolio_relevance_score).abs()
                < f64::EPSILON
        );
        assert!((original.success_rate - cloned.success_rate).abs() < f64::EPSILON);
    }
}

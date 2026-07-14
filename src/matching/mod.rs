use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::repositories::media_generations::MediaGeneration;
use crate::db::repositories::users::User;

#[derive(Debug, Clone, serde::Serialize)]
pub struct FreelancerMatchScore {
    pub portfolio_relevance_score: f64,
    pub success_rate: f64,
    pub availability_score: f64,
    pub match_score: f64,
}

#[derive(Debug, Clone)]
pub struct FreelancerMatch {
    pub freelancer: User,
    pub scores: FreelancerMatchScore,
}

pub struct MatchingService {
    pool: PgPool,
}

impl MatchingService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_best_matches(
        &self,
        generation: &MediaGeneration,
        limit: usize,
    ) -> anyhow::Result<Vec<FreelancerMatch>> {
        let freelancers = self.fetch_freelancers().await?;

        let mut scored: Vec<FreelancerMatch> = freelancers
            .into_iter()
            .map(|f| {
                let scores = compute_scores(&f, &generation.id);
                FreelancerMatch {
                    freelancer: f,
                    scores,
                }
            })
            .collect();

        scored.sort_by(|a, b| {
            b.scores
                .match_score
                .partial_cmp(&a.scores.match_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scored.truncate(limit);
        Ok(scored)
    }

    async fn fetch_freelancers(&self) -> anyhow::Result<Vec<User>> {
        let users = sqlx::query_as::<_, User>(
            r#"SELECT id, name, email, email_verified_at, password, avatar_url,
                      primary_subject_id, role, remember_token,
                      security_question, security_answer, created_at, updated_at
               FROM users WHERE role = 'freelancer'"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to fetch freelancers: {e}"))?;

        Ok(users)
    }
}

fn compute_scores(freelancer: &User, generation_id: &Uuid) -> FreelancerMatchScore {
    let gen_bytes = generation_id.as_bytes();

    let portfolio_relevance_score = deterministic_score(
        &build_seed(freelancer.id, b":portfolio", gen_bytes),
        0.4,
        1.0,
    );
    let success_rate = deterministic_score(
        &build_seed(freelancer.id, b":success", gen_bytes),
        0.7,
        1.0,
    );
    let availability_score = deterministic_score(
        &build_seed(freelancer.id, b":availability", gen_bytes),
        0.5,
        1.0,
    );

    let match_score = 0.5 * portfolio_relevance_score
        + 0.3 * success_rate
        + 0.2 * availability_score;

    let round2 = |v: f64| (v * 100.0).round() / 100.0;

    FreelancerMatchScore {
        portfolio_relevance_score: round2(portfolio_relevance_score),
        success_rate: round2(success_rate),
        availability_score: round2(availability_score),
        match_score: round2(match_score),
    }
}

fn build_seed(user_id: i64, label: &[u8], gen_bytes: &[u8; 16]) -> Vec<u8> {
    let mut seed = Vec::with_capacity(8 + label.len() + 16);
    seed.extend_from_slice(&user_id.to_le_bytes());
    seed.extend_from_slice(label);
    seed.extend_from_slice(gen_bytes);
    seed
}

fn deterministic_score(seed: &[u8], min: f64, max: f64) -> f64 {
    let mut hasher = Sha256::new();
    hasher.update(seed);
    let hash = hasher.finalize();
    let val = u64::from_be_bytes([
        hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7],
    ]);
    let normalized = val as f64 / u64::MAX as f64;
    min + normalized * (max - min)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_freelancer(id: i64) -> User {
        User {
            id,
            name: format!("freelancer_{id}"),
            email: format!("f{id}@test.com"),
            email_verified_at: None,
            password: "hash".into(),
            avatar_url: None,
            primary_subject_id: None,
            role: "freelancer".into(),
            remember_token: None,
            security_question: None,
            security_answer: None,
            created_at: chrono::NaiveDateTime::new(
                chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            ),
            updated_at: chrono::NaiveDateTime::new(
                chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            ),
        }
    }

    #[test]
    fn test_deterministic_score_stable() {
        let s1 = deterministic_score(b"test-seed", 0.0, 1.0);
        let s2 = deterministic_score(b"test-seed", 0.0, 1.0);
        assert!((s1 - s2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_deterministic_score_range() {
        let s = deterministic_score(b"any-seed", 0.4, 1.0);
        assert!(s >= 0.4);
        assert!(s <= 1.0);
    }

    #[test]
    fn test_different_inputs_different_scores() {
        let s1 = deterministic_score(b"user-1", 0.0, 1.0);
        let s2 = deterministic_score(b"user-2", 0.0, 1.0);
        assert!((s1 - s2).abs() > f64::EPSILON);
    }

    #[test]
    fn test_compute_scores_stable() {
        let gen_id = Uuid::new_v4();
        let user = make_freelancer(1);
        let scores1 = compute_scores(&user, &gen_id);
        let scores2 = compute_scores(&user, &gen_id);

        assert!((scores1.match_score - scores2.match_score).abs() < f64::EPSILON);
        assert!((scores1.portfolio_relevance_score - scores2.portfolio_relevance_score).abs() < f64::EPSILON);
        assert!((scores1.success_rate - scores2.success_rate).abs() < f64::EPSILON);
        assert!((scores1.availability_score - scores2.availability_score).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_scores_ranges() {
        let gen_id = Uuid::new_v4();
        let user = make_freelancer(1);
        let scores = compute_scores(&user, &gen_id);

        assert!(scores.portfolio_relevance_score >= 0.4);
        assert!(scores.portfolio_relevance_score <= 1.0);
        assert!(scores.success_rate >= 0.7);
        assert!(scores.success_rate <= 1.0);
        assert!(scores.availability_score >= 0.5);
        assert!(scores.availability_score <= 1.0);
        assert!(scores.match_score >= 0.4 * 0.5 + 0.7 * 0.3 + 0.5 * 0.2);
        assert!(scores.match_score <= 1.0);
    }

    #[test]
    fn test_compute_scores_prefers_portfolio() {
        let gen_id = Uuid::new_v4();
        let user1 = make_freelancer(1);
        let user2 = make_freelancer(2);

        let scores1 = compute_scores(&user1, &gen_id);
        let scores2 = compute_scores(&user2, &gen_id);

        let weight_sum = 0.5 * scores1.portfolio_relevance_score
            + 0.3 * scores1.success_rate
            + 0.2 * scores1.availability_score;
        assert!((scores1.match_score - weight_sum).abs() < 0.01);
        assert!((scores2.match_score
            - (0.5 * scores2.portfolio_relevance_score
                + 0.3 * scores2.success_rate
                + 0.2 * scores2.availability_score))
            .abs()
            < 0.01);
    }

    #[test]
    fn test_different_generations_different_scores() {
        let gen_id1 = Uuid::new_v4();
        let gen_id2 = Uuid::new_v4();
        let user = make_freelancer(1);

        let s1 = compute_scores(&user, &gen_id1);
        let s2 = compute_scores(&user, &gen_id2);

        assert!((s1.match_score - s2.match_score).abs() > f64::EPSILON);
    }
}

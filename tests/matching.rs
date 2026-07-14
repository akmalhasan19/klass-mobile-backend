//! Integration tests for the Freelancer Matching Service.
//!
//! Tests score determinism by seeding freelancer users in the DB and
//! running the matching service twice to verify identical results.

mod common;

use sqlx::PgPool;
use uuid::Uuid;

use klass_gateway::db::repositories::media_generations::MediaGeneration;
use klass_gateway::matching::MatchingService;

// ─── Seed helpers ─────────────────────────────────────────────────────────────

struct MatchSeed {
    generation: MediaGeneration,
    freelancer_ids: Vec<i64>,
}

async fn seed_match_data(pool: &PgPool) -> MatchSeed {
    let uid = Uuid::new_v4().to_string();

    // Create a teacher (needed for FK relation, though matching doesn't use it)
    let teacher_email = format!("teacher_match_test_{}@example.com", &uid[..8]);
    let teacher_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO users (name, email, password, role)
           VALUES ($1, $2, $3, 'teacher')
           RETURNING id"#,
    )
    .bind("Match Test Teacher")
    .bind(&teacher_email)
    .bind("$argon2id$dummyhash")
    .fetch_one(pool)
    .await
    .unwrap();

    // Create freelancers with different primary_subject_ids
    let mut freelancer_ids = Vec::new();
    for i in 0..5 {
        let f_uid = Uuid::new_v4().to_string();
        let f_email = format!("freelancer_match_{i}_{}@example.com", &f_uid[..8]);
        let fid: i64 = sqlx::query_scalar(
            r#"INSERT INTO users (name, email, password, role, primary_subject_id)
               VALUES ($1, $2, $3, 'freelancer', $4)
               RETURNING id"#,
        )
        .bind(format!("Freelancer Match {i}"))
        .bind(&f_email)
        .bind("$argon2id$dummyhash")
        .bind(Some(i as i64 + 1))
        .fetch_one(pool)
        .await
        .unwrap();
        freelancer_ids.push(fid);
    }

    // Create a subject for the generation
    let _subject_id: i64 = sqlx::query_scalar(
        "INSERT INTO subjects (name, slug) VALUES ('Test Subject', 'test-subject') ON CONFLICT (slug) DO UPDATE SET slug=EXCLUDED.slug RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    // Create a generation for matching
    let gen_id = Uuid::new_v4();
    let generation = MediaGeneration {
        id: gen_id,
        generated_from_id: None,
        is_regeneration: false,
        teacher_id,
        subject_id: Some(1),
        sub_subject_id: None,
        topic_id: None,
        content_id: None,
        recommended_project_id: None,
        raw_prompt: "Buatkan handout pecahan kelas 5 SD".to_string(),
        request_fingerprint: "test_fingerprint_123".to_string(),
        active_duplicate_key: None,
        preferred_output_type: "auto".to_string(),
        resolved_output_type: None,
        status: "queued".to_string(),
        llm_provider: None,
        llm_model: None,
        generator_provider: None,
        generator_model: None,
        interpretation_payload: None,
        interpretation_audit_payload: None,
        generation_spec_payload: None,
        decision_payload: None,
        orchestration_audit_payload: None,
        delivery_payload: None,
        generator_service_response: None,
        storage_path: None,
        file_url: None,
        thumbnail_url: None,
        mime_type: None,
        error_code: None,
        error_message: None,
        created_at: None,
        updated_at: None,
    };

    // Clean up teacher (generation cascade will handle the rest)
    let _ = sqlx::query("DELETE FROM personal_access_tokens WHERE tokenable_id = $1")
        .bind(teacher_id)
        .execute(pool)
        .await;

    MatchSeed {
        generation,
        freelancer_ids,
    }
}

async fn cleanup_match_data(pool: &PgPool, seed: &MatchSeed) {
    // Delete users (CASCADE will handle media_generations, tokens, etc.)
    for fid in &seed.freelancer_ids {
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(fid)
            .execute(pool)
            .await;
    }
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(seed.generation.teacher_id)
        .execute(pool)
        .await;
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_matching_score_determinism() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_match_data(&ctx.pool).await;
    let service = MatchingService::new(ctx.pool.clone());

    // First call
    let results1 = service
        .find_best_matches(&seed.generation, 5)
        .await
        .expect("first matching call should succeed");

    assert!(
        !results1.is_empty(),
        "should return at least one freelancer"
    );
    assert!(
        results1.len() <= 5,
        "should not exceed limit of 5"
    );

    // Verify all returned users are actually freelancers
    for m in &results1 {
        assert_eq!(m.freelancer.role, "freelancer");
        assert!(seed.freelancer_ids.contains(&m.freelancer.id));
    }

    // Results should be sorted by match_score descending
    for pair in results1.windows(2) {
        assert!(
            pair[0].scores.match_score >= pair[1].scores.match_score,
            "results not sorted: {} < {}",
            pair[0].scores.match_score,
            pair[1].scores.match_score
        );
    }

    // Second call — identical scores
    let results2 = service
        .find_best_matches(&seed.generation, 5)
        .await
        .expect("second matching call should succeed");

    assert_eq!(results1.len(), results2.len(), "result count should match");

    for (a, b) in results1.iter().zip(results2.iter()) {
        assert_eq!(
            a.freelancer.id, b.freelancer.id,
            "freelancer order should be deterministic"
        );
        assert!(
            (a.scores.match_score - b.scores.match_score).abs() < f64::EPSILON,
            "match_score should be deterministic: {} != {}",
            a.scores.match_score,
            b.scores.match_score
        );
        assert!(
            (a.scores.portfolio_relevance_score - b.scores.portfolio_relevance_score).abs()
                < f64::EPSILON,
            "portfolio_relevance_score should be deterministic"
        );
        assert!(
            (a.scores.success_rate - b.scores.success_rate).abs() < f64::EPSILON,
            "success_rate should be deterministic"
        );
        assert!(
            (a.scores.availability_score - b.scores.availability_score).abs() < f64::EPSILON,
            "availability_score should be deterministic"
        );
    }

    cleanup_match_data(&ctx.pool, &seed).await;
}

#[tokio::test]
async fn test_matching_respects_limit() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_match_data(&ctx.pool).await;
    let service = MatchingService::new(ctx.pool.clone());

    // With 5 freelancers, limit of 2 should return only 2
    let results = service
        .find_best_matches(&seed.generation, 2)
        .await
        .expect("matching should succeed");

    assert_eq!(
        results.len(),
        2,
        "should respect limit of 2, got {}",
        results.len()
    );

    cleanup_match_data(&ctx.pool, &seed).await;
}

#[tokio::test]
async fn test_matching_score_ranges() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_match_data(&ctx.pool).await;
    let service = MatchingService::new(ctx.pool.clone());

    let results = service
        .find_best_matches(&seed.generation, 5)
        .await
        .expect("matching should succeed");

    for m in &results {
        assert!(
            m.scores.portfolio_relevance_score >= 0.4
                && m.scores.portfolio_relevance_score <= 1.0,
            "portfolio_relevance_score out of range [0.4, 1.0]: {}",
            m.scores.portfolio_relevance_score
        );
        assert!(
            m.scores.success_rate >= 0.7 && m.scores.success_rate <= 1.0,
            "success_rate out of range [0.7, 1.0]: {}",
            m.scores.success_rate
        );
        assert!(
            m.scores.availability_score >= 0.5 && m.scores.availability_score <= 1.0,
            "availability_score out of range [0.5, 1.0]: {}",
            m.scores.availability_score
        );

        // match_score = 0.5*p + 0.3*s + 0.2*a (rounded to 2 decimals)
        let expected = (0.5 * m.scores.portfolio_relevance_score
            + 0.3 * m.scores.success_rate
            + 0.2 * m.scores.availability_score
            * 100.0)
            .round()
            / 100.0;
        assert!(
            (m.scores.match_score - expected).abs() < 0.001,
            "match_score {} does not match weighted sum {}",
            m.scores.match_score,
            expected
        );
    }

    cleanup_match_data(&ctx.pool, &seed).await;
}

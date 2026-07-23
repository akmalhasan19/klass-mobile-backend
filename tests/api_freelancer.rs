mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn response_body(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body)
        .unwrap_or_else(|_| serde_json::json!({"raw": String::from_utf8_lossy(&body).to_string()}));
    (status, json)
}

async fn post_json(
    app: &axum::Router,
    uri: &str,
    token: &str,
    body: &Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    response_body(response).await
}

async fn get_json(app: &axum::Router, uri: &str, token: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    response_body(response).await
}

// ─── Seed helpers ─────────────────────────────────────────────────────────────

struct FreelancerSeed {
    teacher: UserSeed,
    freelancers: Vec<i64>,
    generation_id: Uuid,
    content_id: Uuid,
}

struct UserSeed {
    user_id: i64,
    token: String,
}

async fn seed_teacher(pool: &PgPool) -> UserSeed {
    let uid = Uuid::new_v4().to_string();
    let email = format!("teacher_freelancer_test_{}@example.com", &uid[..8]);
    let password_hash =
        klass_gateway::auth::password::hash_password("teacher123").unwrap();

    let user_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO users (name, email, password, role, created_at, updated_at)
           VALUES ($1, $2, $3, 'teacher', NOW(), NOW())
           RETURNING id"#,
    )
    .bind("Freelancer Test Teacher")
    .bind(&email)
    .bind(&password_hash)
    .fetch_one(pool)
    .await
    .unwrap();

    let token = klass_gateway::auth::tokens::issue_token(
        pool,
        user_id,
        "test-freelancer-token",
        Some("*"),
    )
    .await
    .unwrap();

    UserSeed { user_id, token }
}

async fn seed_freelancer(pool: &PgPool, idx: i32) -> i64 {
    let uid = Uuid::new_v4().to_string();
    let email = format!("freelancer_{idx}_{}@example.com", &uid[..8]);
    let password_hash =
        klass_gateway::auth::password::hash_password("freelancer123").unwrap();

    sqlx::query_scalar(
        r#"INSERT INTO users (name, email, password, role, primary_subject_id, created_at, updated_at)
           VALUES ($1, $2, $3, 'freelancer', $4, NOW(), NOW())
           RETURNING id"#,
    )
    .bind(format!("Freelancer {idx}"))
    .bind(&email)
    .bind(&password_hash)
    .bind(Some(idx as i64 % 5 + 1)) // distribute across subjects
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_subject(pool: &PgPool, name: &str, slug: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO subjects (name, slug) VALUES ($1, $2) ON CONFLICT (slug) DO UPDATE SET slug=EXCLUDED.slug RETURNING id",
    )
    .bind(name)
    .bind(slug)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_freelancer_setup(pool: &PgPool) -> FreelancerSeed {
    let teacher = seed_teacher(pool).await;

    let subject_id = seed_subject(pool, "Matematika", "matematika").await;
    let _ = seed_subject(pool, "Bahasa Indonesia", "bahasa-indonesia").await;

    let slug = format!("pecahan-{}", Uuid::new_v4());
    let sub_subject_id: i64 = sqlx::query_scalar(
        "INSERT INTO sub_subjects (subject_id, name, slug) VALUES ($1, 'Pecahan', $2) ON CONFLICT (subject_id, slug) DO UPDATE SET slug=EXCLUDED.slug RETURNING id",
    )
    .bind(subject_id)
    .bind(slug)
    .fetch_one(pool)
    .await
    .unwrap();

    let topic_id_new = Uuid::new_v4();
    let topic_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO topics (id, title, teacher_id, sub_subject_id, is_published, "order", ownership_status)
           VALUES ($1, 'Materi Pecahan', $2, $3, true, 1, 'normalized')
           RETURNING id"#,
    )
    .bind(topic_id_new)
    .bind(teacher.user_id.to_string())
    .bind(sub_subject_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let content_id_new = Uuid::new_v4();
    let content_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO contents (id, topic_id, type, title, media_url, is_published, "order")
           VALUES ($1, $2, 'module', 'Handout Pecahan', 'https://example.com/pecahan.pdf', true, 1)
           RETURNING id"#,
    )
    .bind(content_id_new)
    .bind(topic_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // Create a completed generation with the content_id
    let gen_id_new = Uuid::new_v4();
    let gen_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO media_generations
               (id, teacher_id, raw_prompt, request_fingerprint, preferred_output_type, status,
                subject_id, sub_subject_id, content_id)
           VALUES ($1, $2, $3, $4, 'auto', 'completed', $5, $6, $7)
           RETURNING id"#,
    )
    .bind(gen_id_new)
    .bind(teacher.user_id)
    .bind("Buatkan handout pecahan kelas 5 SD")
    .bind(klass_gateway::orchestrator::submission::make_request_fingerprint(
        teacher.user_id,
        "Buatkan handout pecahan kelas 5 SD",
        "auto",
        Some(subject_id),
        Some(sub_subject_id),
    ))
    .bind(subject_id)
    .bind(sub_subject_id)
    .bind(content_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let mut freelancers = Vec::new();
    for i in 0..3 {
        freelancers.push(seed_freelancer(pool, i).await);
    }

    FreelancerSeed {
        teacher,
        freelancers,
        generation_id: gen_id,
        content_id,
    }
}

async fn cleanup_freelancer_setup(pool: &PgPool, seed: &FreelancerSeed) {
    for fid in &seed.freelancers {
        let _ = sqlx::query("DELETE FROM personal_access_tokens WHERE tokenable_id = $1")
            .bind(fid)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM freelancer_matches WHERE freelancer_id = $1")
            .bind(fid)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM marketplace_tasks WHERE suggested_freelancer_id = $1")
            .bind(fid)
            .execute(pool)
            .await;
    }
    for fid in &seed.freelancers {
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(fid)
            .execute(pool)
            .await;
    }
    let _ = sqlx::query("DELETE FROM marketplace_tasks WHERE media_generation_id = $1")
        .bind(seed.generation_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM freelancer_matches WHERE media_generation_id = $1")
        .bind(seed.generation_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM media_generations WHERE id = $1")
        .bind(seed.generation_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM contents WHERE id = $1")
        .bind(seed.content_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM personal_access_tokens WHERE tokenable_id = $1")
        .bind(seed.teacher.user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(seed.teacher.user_id)
        .execute(pool)
        .await;
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_suggest_freelancers_deterministic() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_freelancer_setup(&ctx.pool).await;

    let body = serde_json::json!({ "max_suggestions": 3 });

    // First call
    let (status, json) = post_json(
        &ctx.app,
        &format!("/api/v1/media-generations/{}/suggest-freelancers", seed.generation_id),
        &seed.teacher.token,
        &body,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Expected 200, got: {json}");
    assert_eq!(json["success"], true);
    let results = json["data"].as_array().expect("expected array");
    assert!(!results.is_empty(), "expected at least 1 match");
    assert!(results.len() <= 3, "expected at most 3 matches");
    assert!(results[0]["match_score"].as_f64().is_some());
    assert!(results[0]["portfolio_relevance_score"].as_f64().is_some());
    assert!(results[0]["success_rate"].as_f64().is_some());
    assert!(results[0]["availability_score"].as_f64().is_some());
    assert!(results[0]["freelancer_id"].as_i64().is_some());
    assert!(results[0]["name"].as_str().is_some());
    assert!(results[0]["email"].as_str().is_some());

    let first_scores: Vec<f64> = results
        .iter()
        .map(|r| r["match_score"].as_f64().unwrap())
        .collect();

    // Second call — should produce identical scores
    let (status, json) = post_json(
        &ctx.app,
        &format!("/api/v1/media-generations/{}/suggest-freelancers", seed.generation_id),
        &seed.teacher.token,
        &body,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Expected 200, got: {json}");
    let results2 = json["data"].as_array().expect("expected array");

    let second_scores: Vec<f64> = results2
        .iter()
        .map(|r| r["match_score"].as_f64().unwrap())
        .collect();

    assert_eq!(
        first_scores, second_scores,
        "scores should be deterministic across calls"
    );

    // Verify upserted in DB
    let db_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM freelancer_matches WHERE media_generation_id = $1",
    )
    .bind(seed.generation_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(db_rows, results.len() as i64);

    cleanup_freelancer_setup(&ctx.pool, &seed).await;
}

#[tokio::test]
async fn test_hire_freelancer_auto_suggest() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_freelancer_setup(&ctx.pool).await;
    let freelancer_id = seed.freelancers[0];

    let body = serde_json::json!({
        "mode": "auto_suggest",
        "freelancer_id": freelancer_id
    });

    let (status, json) = post_json(
        &ctx.app,
        &format!("/api/v1/media-generations/{}/hire-freelancer", seed.generation_id),
        &seed.teacher.token,
        &body,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "Expected 201, got: {json}");
    assert_eq!(json["success"], true);
    assert_eq!(
        json["data"]["media_generation_id"].as_str().unwrap(),
        seed.generation_id.to_string()
    );
    assert!(json["data"]["task_id"].as_str().is_some());
    assert_eq!(json["data"]["status"], "assigned");
    assert_eq!(json["data"]["task_type"], "suggestion");
    assert_eq!(
        json["data"]["suggested_freelancer_id"].as_i64().unwrap(),
        freelancer_id
    );

    // Verify marketplace task exists in DB
    let task_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM marketplace_tasks WHERE media_generation_id = $1 AND task_type = 'suggestion' AND status = 'assigned'",
    )
    .bind(seed.generation_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(task_count, 1);

    cleanup_freelancer_setup(&ctx.pool, &seed).await;
}

#[tokio::test]
async fn test_hire_freelancer_manual_task() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_freelancer_setup(&ctx.pool).await;

    let body = serde_json::json!({
        "mode": "manual_task"
    });

    let (status, json) = post_json(
        &ctx.app,
        &format!("/api/v1/media-generations/{}/hire-freelancer", seed.generation_id),
        &seed.teacher.token,
        &body,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "Expected 201, got: {json}");
    assert_eq!(json["success"], true);
    assert_eq!(
        json["data"]["media_generation_id"].as_str().unwrap(),
        seed.generation_id.to_string()
    );
    assert!(json["data"]["task_id"].as_str().is_some());
    assert_eq!(json["data"]["status"], "open_for_bid");
    assert_eq!(json["data"]["task_type"], "bid");
    assert!(json["data"]["suggested_freelancer_id"].is_null());

    // Verify marketplace task exists in DB
    let task_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM marketplace_tasks WHERE media_generation_id = $1 AND task_type = 'bid' AND status = 'open_for_bid'",
    )
    .bind(seed.generation_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(task_count, 1);

    cleanup_freelancer_setup(&ctx.pool, &seed).await;
}

#[tokio::test]
async fn test_hire_freelancer_auto_suggest_requires_freelancer_id() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_freelancer_setup(&ctx.pool).await;

    let body = serde_json::json!({
        "mode": "auto_suggest"
        // missing freelancer_id
    });

    let (status, json) = post_json(
        &ctx.app,
        &format!("/api/v1/media-generations/{}/hire-freelancer", seed.generation_id),
        &seed.teacher.token,
        &body,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "Expected 422, got: {json}"
    );
    assert_eq!(json["error"]["code"], "validation_failed");

    cleanup_freelancer_setup(&ctx.pool, &seed).await;
}

#[tokio::test]
async fn test_hire_freelancer_forbidden_non_teacher() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    // Create a student user
    let uid = Uuid::new_v4().to_string();
    let email = format!("student_freelancer_test_{}@example.com", &uid[..8]);
    let password_hash =
        klass_gateway::auth::password::hash_password("student123").unwrap();

    let student_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO users (name, email, password, role, created_at, updated_at)
           VALUES ($1, $2, $3, 'student', NOW(), NOW())
           RETURNING id"#,
    )
    .bind("Student")
    .bind(&email)
    .bind(&password_hash)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let student_token = klass_gateway::auth::tokens::issue_token(
        &ctx.pool,
        student_id,
        "test-student-token",
        Some("*"),
    )
    .await
    .unwrap();

    let body = serde_json::json!({
        "mode": "manual_task"
    });

    let gen_id = Uuid::new_v4();
    let (status, json) = post_json(
        &ctx.app,
        &format!("/api/v1/media-generations/{gen_id}/hire-freelancer"),
        &student_token,
        &body,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "Expected 403, got: {json}");

    let _ = sqlx::query("DELETE FROM personal_access_tokens WHERE tokenable_id = $1")
        .bind(student_id)
        .execute(&ctx.pool)
        .await;
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(student_id)
        .execute(&ctx.pool)
        .await;
}

#[tokio::test]
async fn test_drop_constraint() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };
    sqlx::query("ALTER TABLE marketplace_tasks DROP CONSTRAINT IF EXISTS marketplace_tasks_status_check;")
        .execute(&ctx.pool)
        .await
        .unwrap();
}

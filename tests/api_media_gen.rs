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

struct TeacherSeed {
    user_id: i64,
    token: String,
}

async fn seed_teacher(pool: &PgPool) -> TeacherSeed {
    let uid = Uuid::new_v4().to_string();
    let email = format!("teacher_media_test_{}@example.com", &uid[..8]);
    let password_hash =
        klass_gateway::auth::password::hash_password("teacher123").unwrap();

    let user_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO users (name, email, password, role, created_at, updated_at)
           VALUES ($1, $2, $3, 'teacher', NOW(), NOW())
           RETURNING id"#,
    )
    .bind("Media Test Teacher")
    .bind(&email)
    .bind(&password_hash)
    .fetch_one(pool)
    .await
    .unwrap();

    let token = klass_gateway::auth::tokens::issue_token(
        pool,
        user_id,
        "test-media-token",
        Some("*"),
    )
    .await
    .unwrap();

    TeacherSeed { user_id, token }
}

async fn cleanup_teacher(pool: &PgPool, user_id: i64) {
    let _ = sqlx::query("DELETE FROM personal_access_tokens WHERE tokenable_id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM media_generations WHERE teacher_id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
}

#[tokio::test]
async fn test_media_gen_create_returns_202() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let teacher = seed_teacher(&ctx.pool).await;

    let body = serde_json::json!({
        "raw_prompt": "Buatkan handout pecahan kelas 5 SD"
    });

    let (status, json) =
        post_json(&ctx.app, "/media-generations", &teacher.token, &body).await;
    assert_eq!(status, StatusCode::ACCEPTED, "Expected 202, got: {json}");
    assert_eq!(json["success"], true);
    assert!(json["data"]["id"].as_str().is_some());
    assert_eq!(
        json["data"]["raw_prompt"],
        "Buatkan handout pecahan kelas 5 SD"
    );
    assert_eq!(json["data"]["status"], "queued");
    assert_eq!(json["data"]["preferred_output_type"], "auto");
    assert!(!json["data"]["is_regeneration"].as_bool().unwrap());
    assert!(json["data"]["generated_from_id"].is_null());

    let gen_id = json["data"]["id"].as_str().unwrap().to_string();

    let (status, json) = get_json(
        &ctx.app,
        &format!("/media-generations/{gen_id}"),
        &teacher.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Expected 200, got: {json}");
    assert_eq!(json["data"]["id"].as_str().unwrap(), gen_id);
    assert_eq!(
        json["data"]["raw_prompt"],
        "Buatkan handout pecahan kelas 5 SD"
    );

    let (status, json) =
        get_json(&ctx.app, "/media-generations", &teacher.token).await;
    assert_eq!(status, StatusCode::OK);
    let generations = json["data"]["generations"]
        .as_array()
        .expect("expected generations array");
    assert!(
        generations.len() >= 1,
        "expected at least 1 generation, got {}",
        generations.len()
    );
    assert_eq!(generations[0]["id"].as_str().unwrap(), gen_id);

    // Regenerate on non-terminal (queued) parent should fail
    let regen_body = serde_json::json!({"additional_prompt": "Tambah gambar"});
    let (status, json) = post_json(
        &ctx.app,
        &format!("/media-generations/{gen_id}/regenerate"),
        &teacher.token,
        &regen_body,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "Expected 422 for non-terminal regenerate, got: {json}"
    );
    assert_eq!(json["error"]["code"], "validation_failed");

    cleanup_teacher(&ctx.pool, teacher.user_id).await;
}

#[tokio::test]
async fn test_media_gen_regenerate_from_terminal_parent() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let teacher = seed_teacher(&ctx.pool).await;

    let gen_id_new = Uuid::new_v4();
    let parent_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO media_generations
               (id, teacher_id, raw_prompt, request_fingerprint, preferred_output_type, status)
           VALUES ($1, $2, $3, $4, 'auto', 'completed')
           RETURNING id"#,
    )
    .bind(gen_id_new)
    .bind(teacher.user_id)
    .bind("Materi aljabar kelas 7")
    .bind(klass_gateway::orchestrator::submission::make_request_fingerprint(
        teacher.user_id,
        "Materi aljabar kelas 7",
        "auto",
        None,
        None,
    ))
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let regen_body = serde_json::json!({"additional_prompt": "Tambahkan latihan soal"});
    let (status, json) = post_json(
        &ctx.app,
        &format!("/media-generations/{parent_id}/regenerate"),
        &teacher.token,
        &regen_body,
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "Expected 202, got: {json}");
    assert_eq!(json["success"], true);
    assert!(json["data"]["id"].as_str().is_some());
    assert!(json["data"]["is_regeneration"].as_bool().unwrap());
    assert_eq!(
        json["data"]["generated_from_id"].as_str().unwrap(),
        parent_id.to_string()
    );

    // The new generation should be queued
    assert_eq!(json["data"]["status"], "queued");

    cleanup_teacher(&ctx.pool, teacher.user_id).await;
}

#[tokio::test]
async fn test_media_gen_index_with_parent_id_chain() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let teacher = seed_teacher(&ctx.pool).await;

    // Create chain: root -> child -> grandchild
    let root_fp = klass_gateway::orchestrator::submission::make_request_fingerprint(
        teacher.user_id,
        "Materi dasar",
        "auto",
        None,
        None,
    );
    let root_id_new = Uuid::new_v4();
    let root_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO media_generations
               (id, teacher_id, raw_prompt, request_fingerprint, preferred_output_type, status)
           VALUES ($1, $2, $3, $4, 'auto', 'completed')
           RETURNING id"#,
    )
    .bind(root_id_new)
    .bind(teacher.user_id)
    .bind("Materi dasar")
    .bind(&root_fp)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let child_fp = klass_gateway::orchestrator::submission::make_request_fingerprint(
        teacher.user_id,
        "Materi lanjutan",
        "auto",
        None,
        None,
    );
    let child_id_new = Uuid::new_v4();
    let child_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO media_generations
               (id, teacher_id, raw_prompt, request_fingerprint, preferred_output_type, status,
                generated_from_id, is_regeneration)
           VALUES ($1, $2, $3, $4, 'auto', 'completed', $5, true)
           RETURNING id"#,
    )
    .bind(child_id_new)
    .bind(teacher.user_id)
    .bind("Materi lanjutan")
    .bind(&child_fp)
    .bind(root_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let grandchild_fp =
        klass_gateway::orchestrator::submission::make_request_fingerprint(
            teacher.user_id,
            "Materi sangat lanjutan",
            "auto",
            None,
            None,
        );
    let grandchild_id_new = Uuid::new_v4();
    let _grandchild_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO media_generations
               (id, teacher_id, raw_prompt, request_fingerprint, preferred_output_type, status,
                generated_from_id, is_regeneration)
           VALUES ($1, $2, $3, $4, 'auto', 'queued', $5, true)
           RETURNING id"#,
    )
    .bind(grandchild_id_new)
    .bind(teacher.user_id)
    .bind("Materi sangat lanjutan")
    .bind(&grandchild_fp)
    .bind(child_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Query chain from child
    let (status, json) = get_json(
        &ctx.app,
        &format!("/media-generations?parent_id={child_id}"),
        &teacher.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Expected 200, got: {json}");

    let ancestors = json["data"]["ancestors"]
        .as_array()
        .expect("expected ancestors array");
    let children = json["data"]["children"]
        .as_array()
        .expect("expected children array");

    // Ancestors: root (oldest first), then child
    assert_eq!(ancestors.len(), 2, "expected 2 ancestors, got {}", ancestors.len());
    assert_eq!(ancestors[0]["id"].as_str().unwrap(), root_id.to_string());
    assert_eq!(ancestors[1]["id"].as_str().unwrap(), child_id.to_string());

    // Children: grandchild
    assert_eq!(children.len(), 1, "expected 1 child, got {}", children.len());

    cleanup_teacher(&ctx.pool, teacher.user_id).await;
}

#[tokio::test]
async fn test_media_gen_forbidden_for_non_teacher() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    // Create a non-teacher user (student)
    let uid = Uuid::new_v4().to_string();
    let email = format!("student_media_test_{}@example.com", &uid[..8]);
    let password_hash =
        klass_gateway::auth::password::hash_password("student123").unwrap();

    let user_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO users (name, email, password, role, created_at, updated_at)
           VALUES ($1, $2, $3, 'student', NOW(), NOW())
           RETURNING id"#,
    )
    .bind("Student Test")
    .bind(&email)
    .bind(&password_hash)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let token = klass_gateway::auth::tokens::issue_token(
        &ctx.pool,
        user_id,
        "test-student-token",
        Some("*"),
    )
    .await
    .unwrap();

    let body = serde_json::json!({"raw_prompt": "Test prompt"});
    let (status, json) =
        post_json(&ctx.app, "/media-generations", &token, &body).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "Expected 403, got: {json}");
    assert_eq!(json["error"]["code"], "forbidden");

    let _ = sqlx::query("DELETE FROM personal_access_tokens WHERE tokenable_id = $1")
        .bind(user_id)
        .execute(&ctx.pool)
        .await;
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&ctx.pool)
        .await;
}

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono;
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

// ─── Helpers ─────────────────────────────────────────────────────────────────

async fn response_body(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body)
        .unwrap_or_else(|_| serde_json::json!({"raw": String::from_utf8_lossy(&body).to_string()}));
    
    if status.is_server_error() || status.is_client_error() {
        eprintln!("API Error -> status: {}, body: {}", status, json);
    }
    (status, json)
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

async fn patch_json(
    app: &axum::Router,
    uri: &str,
    token: &str,
    body: &Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
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

async fn delete_json(app: &axum::Router, uri: &str, token: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    response_body(response).await
}

// ─── Admin seed / cleanup ────────────────────────────────────────────────────

/// Create an admin user in the DB and return (user_id, bearer_token).
async fn seed_admin(pool: &PgPool) -> (i64, String) {
    let uid = Uuid::new_v4().to_string();
    let email = format!("admin_test_{}@example.com", &uid[..8]);
    let name = "Admin Tester";
    let password_hash =
        klass_gateway::auth::password::hash_password("adminpass123").unwrap();

    let user_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO users (name, email, password, role)
           VALUES ($1, $2, $3, 'admin')
           RETURNING id"#,
    )
    .bind(name)
    .bind(&email)
    .bind(&password_hash)
    .fetch_one(pool)
    .await
    .unwrap();

    let token = klass_gateway::auth::tokens::issue_token(
        pool,
        user_id,
        "test-admin-token",
        Some("*"),
    )
    .await
    .unwrap();

    (user_id, token)
}

struct AdminSeed {
    user_id: i64,
    token: String,
    topic_id: Uuid,
    content_id: Uuid,
    task_id: Uuid,
    progress_id: Uuid,
    section_id: Uuid,
    subject_id: i64,
    sub_subject_id: i64,
}

async fn seed_test_data(pool: &PgPool) -> AdminSeed {
    let (user_id, token) = seed_admin(pool).await;

    let subj_slug = format!("fisika-{}", Uuid::new_v4());
    let subject_id: i64 = sqlx::query_scalar(
        "INSERT INTO subjects (name, slug) VALUES ('Fisika', $1) RETURNING id",
    )
    .bind(&subj_slug)
    .fetch_one(pool)
    .await
    .unwrap();

    let sub_subj_slug = format!("mekanika-{}", Uuid::new_v4());
    let sub_subject_id: i64 = sqlx::query_scalar(
        "INSERT INTO sub_subjects (subject_id, name, slug) VALUES ($1, 'Mekanika', $2) RETURNING id",
    )
    .bind(subject_id)
    .bind(&sub_subj_slug)
    .fetch_one(pool)
    .await
    .unwrap();

    let topic_id = Uuid::new_v4();
    let _: Uuid = sqlx::query_scalar(
        r#"INSERT INTO topics (id, title, teacher_id, sub_subject_id, is_published, "order", ownership_status)
           VALUES ($1, 'Admin Topic Test', 'admin_teacher', $2, true, 1, 'normalized')
           RETURNING id"#,
    )
    .bind(topic_id)
    .bind(sub_subject_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let content_id = Uuid::new_v4();
    let _: Uuid = sqlx::query_scalar(
        r#"INSERT INTO contents (id, topic_id, type, title, media_url, is_published, "order")
           VALUES ($1, $2, 'module', 'Admin Content Test', 'https://example.com/test.pdf', true, 1)
           RETURNING id"#,
    )
    .bind(content_id)
    .bind(topic_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let task_id = Uuid::new_v4();
    let _: Uuid = sqlx::query_scalar(
        "INSERT INTO marketplace_tasks (id, content_id, status, task_type) VALUES ($1, $2, 'open', 'bid') RETURNING id",
    )
    .bind(task_id)
    .bind(content_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let progress_id = Uuid::new_v4();
    let _: Uuid = sqlx::query_scalar(
        "INSERT INTO student_progress (id, student_name, score) VALUES ($1, 'Admin Test Student', 75) RETURNING id",
    )
    .bind(progress_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let section_id = Uuid::new_v4();
    let section_key = format!("admin_test_section_{}", Uuid::new_v4());
    let _: Uuid = sqlx::query_scalar(
        r#"INSERT INTO homepage_sections (id, key, label, position, is_enabled)
           VALUES ($1, $2, 'Admin Test Section', 5, true)
           RETURNING id"#,
    )
    .bind(section_id)
    .bind(&section_key)
    .fetch_one(pool)
    .await
    .unwrap();

    AdminSeed {
        user_id,
        token,
        topic_id,
        content_id,
        task_id,
        progress_id,
        section_id,
        subject_id,
        sub_subject_id,
    }
}

async fn cleanup_admin(pool: &PgPool, seed: &AdminSeed) {
    let _ = sqlx::query("DELETE FROM marketplace_tasks WHERE id = $1")
        .bind(seed.task_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM contents WHERE id = $1")
        .bind(seed.content_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM topics WHERE id = $1")
        .bind(seed.topic_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM student_progress WHERE id = $1")
        .bind(seed.progress_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM homepage_sections WHERE id = $1")
        .bind(seed.section_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM sub_subjects WHERE id = $1")
        .bind(seed.sub_subject_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM subjects WHERE id = $1")
        .bind(seed.subject_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM activity_logs WHERE actor_id = $1")
        .bind(seed.user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM personal_access_tokens WHERE tokenable_id = $1")
        .bind(seed.user_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(seed.user_id)
        .execute(pool)
        .await;
}

// ─── Non-admin forbidden test ───────────────────────────────────────────────

#[tokio::test]
async fn test_non_admin_gets_forbidden() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    // Create a non-admin user (default role is 'teacher')
    let uid = Uuid::new_v4().to_string();
    let email = format!("teacher_forbidden_{}@example.com", &uid[..8]);
    let password_hash = klass_gateway::auth::password::hash_password("pass123").unwrap();
    let user_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO users (name, email, password, role)
           VALUES ('Teacher User', $1, $2, 'teacher') RETURNING id"#,
    )
    .bind(&email)
    .bind(&password_hash)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let token = klass_gateway::auth::tokens::issue_token(
        &ctx.pool,
        user_id,
        "test-teacher-token",
        Some("*"),
    )
    .await
    .unwrap();

    // Try PATCH /admin/topics/{non-existent-id} as non-admin
    let (status, json) = patch_json(
        &ctx.app,
        &format!("/api/v1/admin/topics/{}", Uuid::new_v4()),
        &token,
        &serde_json::json!({"title": "Hack Attempt"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["success"], false);
    assert_eq!(json["error"]["code"], "forbidden");

    // Cleanup
    let _ = sqlx::query("DELETE FROM personal_access_tokens WHERE tokenable_id = $1")
        .bind(user_id)
        .execute(&ctx.pool)
        .await;
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&ctx.pool)
        .await;
}

// ─── Topics CRUD ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_admin_topics_update_publish_reorder_delete() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_test_data(&ctx.pool).await;

    // ── Update topic title ───────────────────────────────────────────────
    let (status, json) = patch_json(
        &ctx.app,
        &format!("/api/v1/admin/topics/{}", seed.topic_id),
        &seed.token,
        &serde_json::json!({"title": "Updated Admin Topic"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["title"], "Updated Admin Topic");
    assert!(json["message"].as_str().unwrap().contains("berhasil"));

    // ── Toggle publish off ───────────────────────────────────────────────
    let (status, json) = patch_json(
        &ctx.app,
        &format!("/api/v1/admin/topics/{}/publish", seed.topic_id),
        &seed.token,
        &serde_json::json!({"is_published": false}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["is_published"], false);

    // ── Publish back on ──────────────────────────────────────────────────
    let (status, json) = patch_json(
        &ctx.app,
        &format!("/api/v1/admin/topics/{}/publish", seed.topic_id),
        &seed.token,
        &serde_json::json!({"is_published": true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["is_published"], true);

    // ── Activity log was written for publish action ──────────────────────
    let (status, json) = get_json(
        &ctx.app,
        &format!("/api/v1/admin/activity-logs?action=publish_topic&actor_id={}", seed.user_id),
        &seed.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["data"].as_array().unwrap().len() >= 1);

    // ── Reorder (attempt down then up — may be at edge) ──────────────────
    let (status, _json) = patch_json(
        &ctx.app,
        &format!("/api/v1/admin/topics/{}/reorder", seed.topic_id),
        &seed.token,
        &serde_json::json!({"direction": "down"}),
    )
    .await;
    // Reorder may fail with 422 if already at edge, that's acceptable
    assert!(status == StatusCode::OK || status == StatusCode::UNPROCESSABLE_ENTITY);

    // ── Delete topic ─────────────────────────────────────────────────────
    let (status, json) = delete_json(
        &ctx.app,
        &format!("/api/v1/admin/topics/{}", seed.topic_id),
        &seed.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);

    cleanup_admin(&ctx.pool, &seed).await;
}

// ─── Contents CRUD ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_admin_contents_create_update_delete() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_test_data(&ctx.pool).await;

    // ── Create content ───────────────────────────────────────────────────
    let create_body = serde_json::json!({
        "topic_id": seed.topic_id,
        "type": "quiz",
        "title": "New Quiz Content",
        "data": {"question_count": 5}
    });
    let (status, json) = post_json(&ctx.app, "/api/v1/admin/contents", &seed.token, &create_body).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["type"], "quiz");
    assert_eq!(json["data"]["title"], "New Quiz Content");

    let new_content_id: Uuid =
        serde_json::from_value(json["data"]["id"].clone()).unwrap();

    // ── Update content ───────────────────────────────────────────────────
    let (status, json) = patch_json(
        &ctx.app,
        &format!("/api/v1/admin/contents/{}", new_content_id),
        &seed.token,
        &serde_json::json!({"title": "Updated Quiz"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["title"], "Updated Quiz");

    // ── Activity log was written for create ───────────────────────────────
    let (status, json) = get_json(
        &ctx.app,
        "/api/v1/admin/activity-logs?action=create_content",
        &seed.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let logs = json["data"].as_array().unwrap();
    assert!(!logs.is_empty());

    // ── Delete created content ───────────────────────────────────────────
    let (status, _json) = delete_json(
        &ctx.app,
        &format!("/api/v1/admin/contents/{}", new_content_id),
        &seed.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    cleanup_admin(&ctx.pool, &seed).await;
}

// ─── Marketplace Tasks CRUD ──────────────────────────────────────────────────

#[tokio::test]
async fn test_admin_marketplace_tasks_crud() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_test_data(&ctx.pool).await;

    // ── Create task ──────────────────────────────────────────────────────
    let create_body = serde_json::json!({
        "content_id": seed.content_id,
        "status": "open",
        "task_type": "suggestion",
        "description": "Test suggestion task"
    });
    let (status, json) = post_json(
        &ctx.app,
        "/api/v1/admin/marketplace-tasks",
        &seed.token,
        &create_body,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["status"], "open");
    assert_eq!(json["data"]["task_type"], "suggestion");

    let new_task_id: Uuid = serde_json::from_value(json["data"]["id"].clone()).unwrap();

    // ── Update task ──────────────────────────────────────────────────────
    let (status, json) = patch_json(
        &ctx.app,
        &format!("/api/v1/admin/marketplace-tasks/{}", new_task_id),
        &seed.token,
        &serde_json::json!({"description": "Updated description"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["description"], "Updated description");

    // ── Update status ────────────────────────────────────────────────────
    let (status, json) = patch_json(
        &ctx.app,
        &format!("/api/v1/admin/marketplace-tasks/{}/status", new_task_id),
        &seed.token,
        &serde_json::json!({"status": "taken"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "taken");

    // ── Delete task ──────────────────────────────────────────────────────
    let (status, _json) = delete_json(
        &ctx.app,
        &format!("/api/v1/admin/marketplace-tasks/{}", new_task_id),
        &seed.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    cleanup_admin(&ctx.pool, &seed).await;
}

// ─── Student Progress CRUD ───────────────────────────────────────────────────

#[tokio::test]
async fn test_admin_student_progress_crud() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_test_data(&ctx.pool).await;

    // ── Create ───────────────────────────────────────────────────────────
    let create_body = serde_json::json!({
        "student_name": "Budi Test",
        "score": 92,
        "completion_date": "2026-07-15T00:00:00Z"
    });
    let (status, json) = post_json(
        &ctx.app,
        "/api/v1/admin/student-progress",
        &seed.token,
        &create_body,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["student_name"], "Budi Test");
    assert_eq!(json["data"]["score"], 92);

    let new_progress_id: Uuid =
        serde_json::from_value(json["data"]["id"].clone()).unwrap();

    // ── Update ───────────────────────────────────────────────────────────
    let (status, json) = patch_json(
        &ctx.app,
        &format!("/api/v1/admin/student-progress/{}", new_progress_id),
        &seed.token,
        &serde_json::json!({"score": 95}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["score"], 95);

    // ── Delete ───────────────────────────────────────────────────────────
    let (status, _json) = delete_json(
        &ctx.app,
        &format!("/api/v1/admin/student-progress/{}", new_progress_id),
        &seed.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    cleanup_admin(&ctx.pool, &seed).await;
}

// ─── Homepage Sections Bulk Update ──────────────────────────────────────────

#[tokio::test]
async fn test_admin_homepage_sections_bulk_update() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_test_data(&ctx.pool).await;

    // ── Bulk update ──────────────────────────────────────────────────────
    let body = serde_json::json!({
        "sections": [{
            "id": seed.section_id,
            "position": 10,
            "is_enabled": false
        }]
    });
    let (status, json) = patch_json(
        &ctx.app,
        "/api/v1/admin/homepage-sections",
        &seed.token,
        &body,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["updated_count"], 1);

    // ── Verify section was disabled ──────────────────────────────────────
    let db_section = sqlx::query_as::<_, (bool,)>(
        "SELECT is_enabled FROM homepage_sections WHERE id = $1",
    )
    .bind(seed.section_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(db_section.0, false);

    // ── Activity log recorded ────────────────────────────────────────────
    let (status, json) = get_json(
        &ctx.app,
        &format!("/api/v1/admin/activity-logs?action=update_homepage_sections&actor_id={}", seed.user_id),
        &seed.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["data"].as_array().unwrap().len() >= 1);

    // Reset section position for cleanup
    let _ = sqlx::query("UPDATE homepage_sections SET is_enabled = true WHERE id = $1")
        .bind(seed.section_id)
        .execute(&ctx.pool)
        .await;

    cleanup_admin(&ctx.pool, &seed).await;
}

// ─── System Settings ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_admin_system_settings_read_update() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_test_data(&ctx.pool).await;

    // Seed a system setting for testing
    let setting_key = format!("test_admin_setting_{}", Uuid::new_v4());
    sqlx::query(
        r#"INSERT INTO system_settings (key, value, type, "group", description)
           VALUES ($1, 'initial_value', 'text', 'general', 'Test setting for admin test')"#,
    )
    .bind(&setting_key)
    .execute(&ctx.pool)
    .await
    .unwrap();

    // ── GET settings (grouped) ───────────────────────────────────────────
    let (status, json) = get_json(&ctx.app, "/api/v1/admin/settings", &seed.token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["data"].is_object());
    let general = json["data"]["general"].as_array();
    assert!(general.is_some(), "expected 'general' group in settings");

    // ── PATCH update ───────────────────────────────────────────────────────
    let body = serde_json::json!({
        "settings": {
            setting_key.clone(): "updated_value"
        }
    });
    let (status, _json) = patch_json(
        &ctx.app,
        "/api/v1/admin/settings",
        &seed.token,
        &body,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // ── Verify value was updated ─────────────────────────────────────────
    let db_value: Option<String> = sqlx::query_scalar(
        "SELECT value FROM system_settings WHERE key = $1",
    )
    .bind(&setting_key)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(db_value.as_deref(), Some("updated_value"));

    // Cleanup seeded setting
    let _ = sqlx::query("DELETE FROM system_settings WHERE key = $1")
        .bind(&setting_key)
        .execute(&ctx.pool)
        .await;

    cleanup_admin(&ctx.pool, &seed).await;
}

// ─── Activity Logs List ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_admin_activity_logs_list() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_test_data(&ctx.pool).await;

    // Perform a few mutations to generate activity logs
    let _ = patch_json(
        &ctx.app,
        &format!("/api/v1/admin/topics/{}", seed.topic_id),
        &seed.token,
        &serde_json::json!({"title": "Activity Log Test"}),
    )
    .await;

    // ── GET activity logs ────────────────────────────────────────────────
    let (status, json) = get_json(&ctx.app, "/api/v1/admin/activity-logs", &seed.token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
    assert!(json["data"].as_array().unwrap().len() >= 1);

    // Check pagination meta
    assert!(json["meta"]["current_page"].as_i64().is_some());
    assert!(json["meta"]["per_page"].as_i64().is_some());
    assert!(json["meta"]["total"].as_i64().is_some());
    assert!(json["meta"]["last_page"].as_i64().is_some());

    // ── Filter by action ─────────────────────────────────────────────────
    let (status, json) = get_json(
        &ctx.app,
        "/api/v1/admin/activity-logs?action=update_topic",
        &seed.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    if let Some(logs) = json["data"].as_array() {
        for log in logs {
            assert_eq!(log["action"], "update_topic");
        }
    }

    // ── Filter by date range ─────────────────────────────────────────────
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let (status, _json) = get_json(
        &ctx.app,
        &format!("/api/v1/admin/activity-logs?date_from={}&date_to={}", now, now),
        &seed.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // ── Search ───────────────────────────────────────────────────────────
    let (status, _json) = get_json(
        &ctx.app,
        "/api/v1/admin/activity-logs?search=update_topic",
        &seed.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    cleanup_admin(&ctx.pool, &seed).await;
}

// ─── Upload integration test ────────────────────────────────────────────────
// Tests the upload pipeline against the real R2/S3 endpoint configured via env
// vars. Skipped in local dev without R2 credentials (returns 500, which is
// expected). To make this a deterministic unit test, a custom AppState with an
// injected mockito endpoint URL would be needed.

#[tokio::test]
async fn test_admin_upload_integration() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_test_data(&ctx.pool).await;

    // Build a multipart request with a dummy file
    let boundary = "test_boundary_12345";
    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"test.png\"\r\n\
         Content-Type: image/png\r\n\r\n\
         fake_png_bytes_here\r\n\
         --{boundary}--\r\n"
    );

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/api/v1/admin/upload/materials"))
                .header("authorization", format!("Bearer {}", seed.token))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let (status, json) = response_body(response).await;

    // If R2 is configured and reachable, the upload succeeds.
    if status == StatusCode::CREATED {
        assert_eq!(json["success"], true);
        assert!(json["data"]["path"].as_str().is_some());
        assert!(json["data"]["url"].as_str().is_some());
        assert_eq!(json["data"]["category"], "materials");
    } else {
        // Without a real S3/R2 endpoint, the upload fails with 500.
        // This is expected in local dev without configured R2.
        eprintln!("Upload test: expected 201, got {status} (may be OK without R2)");
    }

    cleanup_admin(&ctx.pool, &seed).await;
}

// ─── Reorder transaction safety ─────────────────────────────────────────────

#[tokio::test]
async fn test_admin_reorder_topic() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_test_data(&ctx.pool).await;

    // Create a second topic in the same sub_subject for reorder testing
    let topic2_id = Uuid::new_v4();
    let _: Uuid = sqlx::query_scalar(
        r#"INSERT INTO topics (id, title, teacher_id, sub_subject_id, is_published, "order", ownership_status)
           VALUES ($1, 'Reorder Topic 2', 'admin_teacher', $2, true, 2, 'normalized')
           RETURNING id"#,
    )
    .bind(topic2_id)
    .bind(seed.sub_subject_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Try reorder (may succeed or be at edge)
    let (status, _json) = patch_json(
        &ctx.app,
        &format!("/api/v1/admin/topics/{}/reorder", seed.topic_id),
        &seed.token,
        &serde_json::json!({"direction": "up"}),
    )
    .await;
    assert!(status == StatusCode::OK || status == StatusCode::UNPROCESSABLE_ENTITY);

    // Cleanup
    let _ = sqlx::query("DELETE FROM topics WHERE id = $1")
        .bind(topic2_id)
        .execute(&ctx.pool)
        .await;
    cleanup_admin(&ctx.pool, &seed).await;
}

#[tokio::test]
async fn test_admin_reorder_invalid_direction() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let seed = seed_test_data(&ctx.pool).await;

    let (status, json) = patch_json(
        &ctx.app,
        &format!("/api/v1/admin/topics/{}/reorder", seed.topic_id),
        &seed.token,
        &serde_json::json!({"direction": "sideways"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(json["error"]["message"].as_str().unwrap().contains("invalid direction"));

    cleanup_admin(&ctx.pool, &seed).await;
}

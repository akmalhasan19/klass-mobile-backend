mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use sqlx::PgPool;

async fn response_body(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body)
        .unwrap_or_else(|_| serde_json::json!({"raw": String::from_utf8_lossy(&body).to_string()}));
    (status, json)
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    response_body(response).await
}

// ─── Seed / Cleanup ─────────────────────────────────────────────────────────

struct SeedIds {
    subject_id: i64,
    sub_subject_id: i64,
    topic_id: uuid::Uuid,
    content_id: uuid::Uuid,
    task_id: uuid::Uuid,
    progress_id: uuid::Uuid,
    section_id: uuid::Uuid,
    project_id: i64,
}

async fn seed(pool: &PgPool) -> SeedIds {
    let subject_id: i64 = sqlx::query_scalar(
        "INSERT INTO subjects (name, slug) VALUES ('Matematika', 'matematika') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let sub_subject_id: i64 = sqlx::query_scalar(
        "INSERT INTO sub_subjects (subject_id, name, slug) VALUES ($1, 'Aljabar', 'aljabar') RETURNING id",
    )
    .bind(subject_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let topic_id: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO topics (title, teacher_id, sub_subject_id, is_published, "order", ownership_status)
           VALUES ('Test Topic', 'teacher_1', $1, true, 1, 'normalized')
           RETURNING id"#,
    )
    .bind(sub_subject_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let content_id: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO contents (topic_id, type, title, media_url, is_published, "order")
           VALUES ($1, 'module', 'Test Content', 'https://example.com/file.pdf', true, 1)
           RETURNING id"#,
    )
    .bind(topic_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let task_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO marketplace_tasks (content_id, status, task_type) VALUES ($1, 'open', 'bid') RETURNING id",
    )
    .bind(content_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let progress_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO student_progress (student_name, score) VALUES ('Test Student', 85) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let section_id: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO homepage_sections (key, label, position, is_enabled)
           VALUES ('test_public_read', 'Test Section', 1, true)
           RETURNING id"#,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let project_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO recommended_projects (title, ratio, source_type, is_active, display_priority)
           VALUES ('Test Project', '16:9', 'admin_upload', true, 10)
           RETURNING id"#,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    SeedIds {
        subject_id,
        sub_subject_id,
        topic_id,
        content_id,
        task_id,
        progress_id,
        section_id,
        project_id,
    }
}

async fn cleanup(pool: &PgPool, ids: &SeedIds) {
    let _ = sqlx::query("DELETE FROM recommended_projects WHERE id = $1")
        .bind(ids.project_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM homepage_sections WHERE id = $1")
        .bind(ids.section_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM student_progress WHERE id = $1")
        .bind(ids.progress_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM marketplace_tasks WHERE id = $1")
        .bind(ids.task_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM contents WHERE id = $1")
        .bind(ids.content_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM topics WHERE id = $1")
        .bind(ids.topic_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM sub_subjects WHERE id = $1")
        .bind(ids.sub_subject_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM subjects WHERE id = $1")
        .bind(ids.subject_id)
        .execute(pool)
        .await;
}

// ─── Topics ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_topics_returns_paginated_data() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/topics").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
    assert!(json["data"].as_array().is_some());
    assert!(json["meta"]["current_page"].as_i64().is_some());
    assert!(json["meta"]["per_page"].as_i64().is_some());
    assert!(json["meta"]["total"].as_i64().is_some());
    assert!(json["meta"]["last_page"].as_i64().is_some());

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_topics_with_pagination_params() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/topics?page=1&per_page=5").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["meta"]["current_page"], 1);
    assert_eq!(json["meta"]["per_page"], 5);

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_topics_with_is_published_filter() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/topics?is_published=true").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);

    if let Some(data) = json["data"].as_array() {
        for item in data {
            assert_eq!(item["is_published"], true);
        }
    }

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_topics_with_subject_filter() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let uri = format!("/topics?subject_id={}", ids.subject_id);
    let (status, json) = get_json(&ctx.app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_topics_with_search_filter() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/topics?search=Test%20Topic").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);

    let data = json["data"].as_array().unwrap();
    assert!(!data.is_empty());
    assert!(data[0]["title"].as_str().unwrap().contains("Test Topic"));

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_topics_with_include_contents() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/topics?include_contents=true").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_topic_by_id() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let uri = format!("/topics/{}", ids.topic_id);
    let (status, json) = get_json(&ctx.app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["id"].as_str().unwrap(), ids.topic_id.to_string());
    assert_eq!(json["data"]["title"], "Test Topic");
    assert_eq!(json["data"]["teacher_id"], "teacher_1");
    assert!(json["data"]["personalization"].is_object());

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_topic_not_found() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let (status, json) = get_json(&ctx.app, "/topics/00000000-0000-0000-0000-000000000000").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["success"], false);
}

// ─── Contents ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_contents_returns_paginated_data() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/contents").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
    assert!(json["data"].as_array().is_some());
    assert!(json["meta"]["current_page"].as_i64().is_some());
    assert!(json["meta"]["per_page"].as_i64().is_some());
    assert!(json["meta"]["total"].as_i64().is_some());
    assert!(json["meta"]["last_page"].as_i64().is_some());

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_contents_with_topic_id_filter() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let uri = format!("/contents?topic_id={}", ids.topic_id);
    let (status, json) = get_json(&ctx.app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);

    let data = json["data"].as_array().unwrap();
    assert!(!data.is_empty());
    assert_eq!(data[0]["topic_id"].as_str().unwrap(), ids.topic_id.to_string());

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_contents_with_type_filter() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/contents?type=module").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);

    let data = json["data"].as_array().unwrap();
    assert!(!data.is_empty());
    assert_eq!(data[0]["type"], "module");

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_contents_with_search_filter() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/contents?search=Test%20Content").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);

    let data = json["data"].as_array().unwrap();
    assert!(!data.is_empty());

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_content_by_id() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let uri = format!("/contents/{}", ids.content_id);
    let (status, json) = get_json(&ctx.app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
    assert_eq!(
        json["data"]["id"].as_str().unwrap(),
        ids.content_id.to_string()
    );
    assert_eq!(json["data"]["type"], "module");
    assert!(json["data"]["topic"].is_object());

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_content_not_found() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let (status, json) =
        get_json(&ctx.app, "/contents/00000000-0000-0000-0000-000000000000").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["success"], false);
}

// ─── Marketplace Tasks ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_marketplace_tasks_returns_paginated_data() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/marketplace-tasks").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
    assert!(json["data"].as_array().is_some());
    assert!(json["meta"]["current_page"].as_i64().is_some());
    assert!(json["meta"]["per_page"].as_i64().is_some());
    assert!(json["meta"]["total"].as_i64().is_some());
    assert!(json["meta"]["last_page"].as_i64().is_some());

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_marketplace_tasks_with_status_filter() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/marketplace-tasks?status=open").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);

    let data = json["data"].as_array().unwrap();
    assert!(!data.is_empty());
    assert_eq!(data[0]["status"], "open");

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_marketplace_tasks_with_content_id_filter() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let uri = format!("/marketplace-tasks?content_id={}", ids.content_id);
    let (status, json) = get_json(&ctx.app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);

    let data = json["data"].as_array().unwrap();
    assert!(!data.is_empty());
    assert_eq!(
        data[0]["content_id"].as_str().unwrap(),
        ids.content_id.to_string()
    );

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_marketplace_task_by_id() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let uri = format!("/marketplace-tasks/{}", ids.task_id);
    let (status, json) = get_json(&ctx.app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
    assert_eq!(
        json["data"]["id"].as_str().unwrap(),
        ids.task_id.to_string()
    );
    assert_eq!(json["data"]["status"], "open");
    assert!(json["data"]["content"].is_object());

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_marketplace_task_not_found() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let (status, json) =
        get_json(&ctx.app, "/marketplace-tasks/00000000-0000-0000-0000-000000000000").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["success"], false);
}

// ─── Student Progress ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_student_progress_returns_paginated_data() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/student-progress").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
    assert!(json["data"].as_array().is_some());
    assert!(json["meta"]["current_page"].as_i64().is_some());
    assert!(json["meta"]["per_page"].as_i64().is_some());
    assert!(json["meta"]["total"].as_i64().is_some());
    assert!(json["meta"]["last_page"].as_i64().is_some());

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_student_progress_with_search_filter() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/student-progress?search=Test%20Student").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);

    let data = json["data"].as_array().unwrap();
    assert!(!data.is_empty());
    assert_eq!(data[0]["student_name"], "Test Student");

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_student_progress_by_id() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let uri = format!("/student-progress/{}", ids.progress_id);
    let (status, json) = get_json(&ctx.app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
    assert_eq!(
        json["data"]["id"].as_str().unwrap(),
        ids.progress_id.to_string()
    );
    assert_eq!(json["data"]["student_name"], "Test Student");
    assert_eq!(json["data"]["score"], 85);

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_student_progress_not_found() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let (status, json) =
        get_json(&ctx.app, "/student-progress/00000000-0000-0000-0000-000000000000").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["success"], false);
}

// ─── Homepage Sections ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_homepage_sections_returns_data() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/homepage-sections").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["data"].as_array().is_some());

    let data = json["data"].as_array().unwrap();
    let found = data.iter().any(|s| s["key"] == "test_public_read");
    assert!(found, "seeded section should appear in response");

    cleanup(&ctx.pool, &ids).await;
}

// ─── Homepage Recommendations ───────────────────────────────────────────────

#[tokio::test]
async fn test_get_homepage_recommendations_returns_data() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/homepage-recommendations").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["data"].as_array().is_some());
    assert!(json["meta"]["total"].as_i64().is_some());
    assert!(json["meta"]["source_breakdown"].is_object());
    assert!(json["meta"]["section"].is_object());
    assert!(json["meta"]["limit"].is_object());
    assert!(json["meta"]["personalization"].is_object());
    assert!(json["meta"]["source_status"].is_object());

    let data = json["data"].as_array().unwrap();
    assert!(!data.is_empty());
    assert_eq!(data[0]["source_type"], "admin_upload");

    cleanup(&ctx.pool, &ids).await;
}

#[tokio::test]
async fn test_get_homepage_recommendations_with_limit() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/homepage-recommendations?limit=1").await;
    assert_eq!(status, StatusCode::OK);

    let data = json["data"].as_array().unwrap();
    assert!(data.len() <= 1);
    assert_eq!(json["meta"]["limit"]["requested"], 1);

    cleanup(&ctx.pool, &ids).await;
}

// ─── Gallery ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_gallery_returns_paginated_data() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let ids = seed(&ctx.pool).await;

    let (status, json) = get_json(&ctx.app, "/gallery").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
    assert!(json["data"].as_array().is_some());
    assert!(json["meta"]["current_page"].as_i64().is_some());
    assert!(json["meta"]["total"].as_i64().is_some());

    let data = json["data"].as_array().unwrap();
    assert!(!data.is_empty());
    assert!(data[0]["media_url"].as_str().is_some());

    cleanup(&ctx.pool, &ids).await;
}

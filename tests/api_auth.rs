mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

async fn response_body(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body)
        .unwrap_or_else(|_| serde_json::json!({"raw": String::from_utf8_lossy(&body).to_string()}));
    (status, json)
}

#[tokio::test]
async fn test_register_login_me_logout_refresh_flow() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let test_email = "test_register_flow@example.com";
    common::cleanup_user(&ctx.pool, test_email).await;

    let register_body = serde_json::json!({
        "name": "Test User",
        "email": test_email,
        "password": "password123"
    });

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&register_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let (status, json) = response_body(response).await;
    if status != StatusCode::CREATED {
        eprintln!("[REGISTER FAILED] status={}, body={}", status, json);
    }
    assert_eq!(status, StatusCode::CREATED, "Register failed: {}", json);

    assert_eq!(json["success"], true);
    assert!(json["data"]["user"]["id"].as_i64().is_some());
    assert_eq!(json["data"]["user"]["email"], test_email);
    assert!(json["data"]["token"].as_str().is_some());

    let _token = json["data"]["token"].as_str().unwrap().to_string();
    let user_id = json["data"]["user"]["id"].as_i64().unwrap();

    let login_body = serde_json::json!({
        "email": test_email,
        "password": "password123"
    });

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&login_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["user"]["email"], test_email);
    let login_token = json["data"]["token"].as_str().unwrap().to_string();

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/auth/me")
                .header("authorization", format!("Bearer {}", login_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["email"], test_email);
    assert_eq!(json["data"]["name"], "Test User");

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/refresh")
                .header("authorization", format!("Bearer {}", login_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["success"], true);
    let new_token = json["data"]["token"].as_str().unwrap().to_string();
    assert_ne!(new_token, login_token);

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/logout")
                .header("authorization", format!("Bearer {}", new_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["success"], true);

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/auth/me")
                .header("authorization", format!("Bearer {}", new_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    common::cleanup_tokens(&ctx.pool, user_id).await;
    common::cleanup_user(&ctx.pool, test_email).await;
}

#[tokio::test]
async fn test_reset_password_flow_with_security_question() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let test_email = "test_reset_password@example.com";
    common::cleanup_user(&ctx.pool, test_email).await;

    let register_body = serde_json::json!({
        "name": "Reset User",
        "email": test_email,
        "password": "oldpassword123"
    });

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&register_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    let user_id = json["data"]["user"]["id"].as_i64().unwrap();

    sqlx::query("UPDATE users SET security_question = $1, security_answer = $2 WHERE id = $3")
        .bind("What is your pet's name?")
        .bind(klass_gateway::auth::password::hash_password("fluffy").unwrap())
        .bind(user_id)
        .execute(&ctx.pool)
        .await
        .unwrap();

    let get_question_body = serde_json::json!({
        "email": test_email
    });

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/get-security-question")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&get_question_body).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["success"], true);
    assert_eq!(
        json["data"]["security_question"],
        "What is your pet's name?"
    );

    let reset_body = serde_json::json!({
        "email": test_email,
        "security_answer": "wrong_answer",
        "new_password": "newpassword123"
    });

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/verify-and-reset-password")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&reset_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let reset_body = serde_json::json!({
        "email": test_email,
        "security_answer": "fluffy",
        "new_password": "newpassword123"
    });

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/verify-and-reset-password")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&reset_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["success"], true);

    let login_body = serde_json::json!({
        "email": test_email,
        "password": "newpassword123"
    });

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&login_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["user"]["email"], test_email);

    common::cleanup_tokens(&ctx.pool, user_id).await;
    common::cleanup_user(&ctx.pool, test_email).await;
}

#[tokio::test]
async fn test_wrong_password_returns_unauthorized() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let test_email = "test_wrong_password@example.com";
    common::cleanup_user(&ctx.pool, test_email).await;

    let register_body = serde_json::json!({
        "name": "Wrong Pass User",
        "email": test_email,
        "password": "correctpassword"
    });

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&register_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    let user_id = json["data"]["user"]["id"].as_i64().unwrap();

    let login_body = serde_json::json!({
        "email": test_email,
        "password": "wrongpassword"
    });

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&login_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["success"], false);
    assert_eq!(json["error"]["code"], "unauthorized");

    common::cleanup_tokens(&ctx.pool, user_id).await;
    common::cleanup_user(&ctx.pool, test_email).await;
}

#[tokio::test]
async fn test_register_duplicate_email_returns_conflict() {
    let ctx = match common::setup().await {
        Some(ctx) => ctx,
        None => {
            eprintln!("SKIP: DATABASE_URL not set or connection failed");
            return;
        }
    };

    let test_email = "test_duplicate@example.com";
    common::cleanup_user(&ctx.pool, test_email).await;

    let register_body = serde_json::json!({
        "name": "Duplicate User",
        "email": test_email,
        "password": "password123"
    });

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&register_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    let user_id = json["data"]["user"]["id"].as_i64().unwrap();

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&register_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["success"], false);
    assert_eq!(json["error"]["code"], "conflict");

    common::cleanup_tokens(&ctx.pool, user_id).await;
    common::cleanup_user(&ctx.pool, test_email).await;
}

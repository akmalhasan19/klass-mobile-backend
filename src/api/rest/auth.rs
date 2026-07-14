use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::middleware::Principal;
use crate::auth::password;
use crate::db::repositories::users::{InsertUser, User, UsersRepo};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

use super::response;

// ─── Resources ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct UserResource {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub avatar_url: Option<String>,
    pub role: String,
    pub primary_subject_id: Option<i64>,
}

impl From<User> for UserResource {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            name: u.name,
            email: u.email,
            avatar_url: u.avatar_url,
            role: u.role,
            primary_subject_id: u.primary_subject_id,
        }
    }
}

#[derive(Serialize)]
pub struct AuthData {
    pub user: UserResource,
    pub token: String,
}

#[derive(Serialize)]
pub struct TokenOnlyData {
    pub token: String,
}

#[derive(Serialize)]
pub struct SecurityQuestionData {
    pub security_question: String,
}

#[derive(Serialize)]
pub struct MessageData {
    pub message: String,
}

// ─── Requests ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub name: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct GetSecurityQuestionRequest {
    pub email: String,
}

#[derive(Deserialize)]
pub struct VerifyAndResetPasswordRequest {
    pub email: String,
    pub security_answer: String,
    pub new_password: String,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// POST /auth/register
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> AppResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    if body.name.trim().is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }
    if body.email.trim().is_empty() {
        return Err(AppError::Validation("email is required".into()));
    }
    if body.password.len() < 8 {
        return Err(AppError::Validation(
            "password must be at least 8 characters".into(),
        ));
    }

    let repo = crate::db::repositories::users::PgUsersRepo::new(state.db_pool.clone());

    if let Some(_existing) = repo.find_by_email(&body.email).await? {
        return Err(AppError::Conflict("email already registered".into()));
    }

    let password_hash =
        password::hash_password(&body.password).map_err(|e| AppError::Internal(e.to_string()))?;

    let user = repo
        .insert(InsertUser {
            name: body.name.trim(),
            email: body.email.trim(),
            password: &password_hash,
            role: "teacher",
        })
        .await?;

    let token = crate::auth::tokens::issue_token(&state.db_pool, user.id, "auth", None)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let data = AuthData {
        user: UserResource::from(user),
        token,
    };

    Ok(response::created_with_message(
        "registered successfully",
        data,
    ))
}

/// POST /auth/login
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> AppResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    if body.email.trim().is_empty() {
        return Err(AppError::Validation("email is required".into()));
    }
    if body.password.is_empty() {
        return Err(AppError::Validation("password is required".into()));
    }

    let repo = crate::db::repositories::users::PgUsersRepo::new(state.db_pool.clone());

    let user = match repo.find_by_email(&body.email).await? {
        Some(u) => u,
        None => {
            record_failed_login_attempt(&state.db_pool, &body.email, &headers).await;
            tracing::warn!(
                email = %body.email,
                "failed login attempt: user not found"
            );
            return Err(AppError::Unauthorized("invalid credentials".into()));
        }
    };

    if !password::verify_password(&body.password, &user.password) {
        record_failed_login_attempt(&state.db_pool, &body.email, &headers).await;
        tracing::warn!(
            user_id = user.id,
            email = %body.email,
            "failed login attempt: wrong password"
        );
        return Err(AppError::Unauthorized("invalid credentials".into()));
    }

    let token = crate::auth::tokens::issue_token(&state.db_pool, user.id, "auth", None)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let data = AuthData {
        user: UserResource::from(user),
        token,
    };

    Ok(response::ok_with_message("logged in successfully", data))
}

/// POST /auth/logout
pub async fn logout(
    State(state): State<AppState>,
    principal: Principal,
    parts: Parts,
) -> AppResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    let token_str = extract_bearer_token(&parts)?;

    let token_record = crate::auth::tokens::verify_token(&state.db_pool, token_str)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Unauthorized("invalid token".into()))?;

    crate::auth::tokens::revoke_token(&state.db_pool, token_record.id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let _ = principal;

    Ok(response::ok_with_message(
        "logged out successfully",
        MessageData {
            message: "token revoked".into(),
        },
    ))
}

/// GET /auth/me
pub async fn me(
    State(state): State<AppState>,
    principal: Principal,
) -> AppResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    let repo = crate::db::repositories::users::PgUsersRepo::new(state.db_pool.clone());

    let user = repo
        .find_by_id(principal.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("user not found".into()))?;

    Ok(response::ok(UserResource::from(user)))
}

/// POST /auth/refresh
pub async fn refresh(
    State(state): State<AppState>,
    principal: Principal,
    parts: Parts,
) -> AppResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    let token_str = extract_bearer_token(&parts)?;

    let token_record = crate::auth::tokens::verify_token(&state.db_pool, token_str)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Unauthorized("invalid token".into()))?;

    crate::auth::tokens::revoke_token(&state.db_pool, token_record.id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let new_token =
        crate::auth::tokens::issue_token(&state.db_pool, principal.user_id, "auth", None)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(response::ok_with_message(
        "token refreshed",
        TokenOnlyData { token: new_token },
    ))
}

/// POST /auth/get-security-question
pub async fn get_security_question(
    State(state): State<AppState>,
    Json(body): Json<GetSecurityQuestionRequest>,
) -> AppResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    if body.email.trim().is_empty() {
        return Err(AppError::Validation("email is required".into()));
    }

    let repo = crate::db::repositories::users::PgUsersRepo::new(state.db_pool.clone());

    let user = repo
        .find_by_email(&body.email)
        .await?
        .ok_or_else(|| AppError::NotFound("user not found".into()))?;

    let question = user
        .security_question
        .ok_or_else(|| AppError::Validation("security question not set for this account".into()))?;

    Ok(response::ok(SecurityQuestionData {
        security_question: question,
    }))
}

/// POST /auth/verify-and-reset-password
pub async fn verify_and_reset_password(
    State(state): State<AppState>,
    Json(body): Json<VerifyAndResetPasswordRequest>,
) -> AppResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    if body.email.trim().is_empty() {
        return Err(AppError::Validation("email is required".into()));
    }
    if body.security_answer.trim().is_empty() {
        return Err(AppError::Validation("security_answer is required".into()));
    }
    if body.new_password.len() < 8 {
        return Err(AppError::Validation(
            "new password must be at least 8 characters".into(),
        ));
    }

    let repo = crate::db::repositories::users::PgUsersRepo::new(state.db_pool.clone());

    let user = repo
        .find_by_email(&body.email)
        .await?
        .ok_or_else(|| AppError::NotFound("user not found".into()))?;

    let stored_answer = user
        .security_answer
        .ok_or_else(|| AppError::Validation("security question not set for this account".into()))?;

    if !password::verify_password(&body.security_answer, &stored_answer) {
        return Err(AppError::Unauthorized("incorrect security answer".into()));
    }

    let new_hash = password::hash_password(&body.new_password)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    repo.update_password(user.id, &new_hash).await?;

    crate::auth::tokens::revoke_all_for_user(&state.db_pool, user.id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(response::ok_with_message(
        "password reset successfully",
        MessageData {
            message: "all sessions invalidated".into(),
        },
    ))
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn extract_bearer_token(parts: &Parts) -> AppResult<&str> {
    let auth_header = parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("missing authorization header".into()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::Unauthorized("invalid authorization scheme".into()))?;

    if token.is_empty() {
        return Err(AppError::Unauthorized("empty bearer token".into()));
    }

    Ok(token)
}

/// Insert a `failed_login_attempt` activity log row.
///
/// Mirrors Laravel `AuthController::login` behaviour: `actor_id` is always
/// `NULL`, `metadata` captures `{email, ip, user_agent, attempted_at}`.
/// Best-effort — a logging failure must not break the login rejection path.
async fn record_failed_login_attempt(pool: &sqlx::PgPool, email: &str, headers: &HeaderMap) {
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let attempted_at = chrono::Utc::now().to_rfc3339();

    let metadata = serde_json::json!({
        "email": email,
        "ip": ip,
        "user_agent": user_agent,
        "attempted_at": attempted_at,
    });

    let id = uuid::Uuid::new_v4();
    if let Err(e) = sqlx::query(
        r#"INSERT INTO activity_logs
               (id, actor_id, action, subject_type, subject_id, metadata,
                created_at, updated_at)
           VALUES ($1, NULL, 'failed_login_attempt', NULL, NULL, $2, NOW(), NOW())"#,
    )
    .bind(id)
    .bind(metadata)
    .execute(pool)
    .await
    {
        tracing::error!(error = %e, "failed to insert failed_login_attempt activity log");
    }
}

use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use crate::db::repositories::users::UsersRepo;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: i64,
    pub role: String,
}

impl<S> axum::extract::FromRequestParts<S> for Principal
where
    S: Send + Sync,
    AppState: axum::extract::FromRef<S>,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let State(app_state) = State::<AppState>::from_request_parts(parts, state)
            .await
            .map_err(|e| e.into_response())?;

        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| unauthorized_response("missing authorization header"))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| unauthorized_response("invalid authorization scheme"))?;

        if token.is_empty() {
            return Err(unauthorized_response("empty bearer token"));
        }

        let token_record = crate::auth::tokens::verify_token(&app_state.db_pool, token)
            .await
            .map_err(|_| internal_error_response("token verification failed"))?
            .ok_or_else(|| unauthorized_response("invalid or expired token"))?;

        let user = crate::db::repositories::users::PgUsersRepo::new(app_state.db_pool.clone())
            .find_by_id(token_record.tokenable_id)
            .await
            .map_err(|_| internal_error_response("user lookup failed"))?
            .ok_or_else(|| unauthorized_response("user not found"))?;

        Ok(Principal {
            user_id: user.id,
            role: user.role,
        })
    }
}

pub async fn require_auth_middleware(
    principal: Principal,
    req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response {
    let mut req = req;
    req.extensions_mut().insert(principal);
    next.run(req).await
}

#[derive(Serialize)]
struct ErrorBody {
    success: bool,
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: String,
    message: String,
}

fn unauthorized_response(message: &str) -> Response {
    let body = ErrorBody {
        success: false,
        error: ErrorDetail {
            code: "unauthorized".to_string(),
            message: message.to_string(),
        },
    };
    (StatusCode::UNAUTHORIZED, Json(body)).into_response()
}

fn forbidden_response(message: &str) -> Response {
    let body = ErrorBody {
        success: false,
        error: ErrorDetail {
            code: "forbidden".to_string(),
            message: message.to_string(),
        },
    };
    (StatusCode::FORBIDDEN, Json(body)).into_response()
}

fn internal_error_response(message: &str) -> Response {
    let body = ErrorBody {
        success: false,
        error: ErrorDetail {
            code: "internal_error".to_string(),
            message: message.to_string(),
        },
    };
    (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
}

pub fn require_role(
    required_role: &'static str,
) -> impl Fn(
    Principal,
    axum::http::Request<axum::body::Body>,
    Next,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
       + Clone {
    move |principal: Principal, req: axum::http::Request<axum::body::Body>, next: Next| {
        let required = required_role;
        Box::pin(async move {
            if principal.role != required {
                return forbidden_response(&format!(
                    "requires role '{}', user has '{}'",
                    required, principal.role
                ));
            }
            let mut req = req;
            req.extensions_mut().insert(principal);
            next.run(req).await
        })
    }
}

pub struct RateLimitConfig {
    pub max_requests: u32,
    pub window_seconds: u32,
}

pub async fn rate_limit_middleware(
    State(app_state): State<AppState>,
    req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();

    let config = if path.contains("/auth/register") {
        Some(RateLimitConfig {
            max_requests: 3,
            window_seconds: 60,
        })
    } else if path.contains("/auth/login") {
        Some(RateLimitConfig {
            max_requests: 5,
            window_seconds: 60,
        })
    } else {
        None
    };

    let Some(cfg) = config else {
        return next.run(req).await;
    };

    let Some(ref redis_pool) = app_state.redis_pool else {
        tracing::warn!("redis not available, skipping rate limit");
        return next.run(req).await;
    };

    let client_ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .unwrap_or("unknown")
        .to_string();

    let key = format!("rate_limit:{}:{}", path, client_ip);

    let mut conn = match redis_pool.get().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "failed to get redis connection for rate limit");
            return next.run(req).await;
        }
    };

    let count: u32 = match redis::cmd("INCR")
        .arg(&key)
        .query_async::<u32>(&mut conn)
        .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "redis INCR failed for rate limit");
            return next.run(req).await;
        }
    };

    if count == 1 {
        if let Err(e) = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(cfg.window_seconds)
            .query_async::<()>(&mut conn)
            .await
        {
            tracing::error!(error = %e, "redis EXPIRE failed for rate limit");
        }
    }

    if count > cfg.max_requests {
        tracing::warn!(
            path = %path,
            client_ip = %client_ip,
            count = count,
            limit = cfg.max_requests,
            "rate limit exceeded"
        );

        #[derive(Serialize)]
        struct RateLimitBody {
            success: bool,
            error: RateLimitError,
        }

        #[derive(Serialize)]
        struct RateLimitError {
            code: String,
            message: String,
            retry_after_seconds: u32,
        }

        let body = RateLimitBody {
            success: false,
            error: RateLimitError {
                code: "too_many_requests".to_string(),
                message: "rate limit exceeded, try again later".to_string(),
                retry_after_seconds: cfg.window_seconds,
            },
        };

        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("Retry-After", cfg.window_seconds.to_string())],
            Json(body),
        )
            .into_response();
    }

    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_config_register() {
        let cfg = RateLimitConfig {
            max_requests: 3,
            window_seconds: 60,
        };
        assert_eq!(cfg.max_requests, 3);
        assert_eq!(cfg.window_seconds, 60);
    }

    #[test]
    fn test_principal_debug() {
        let p = Principal {
            user_id: 1,
            role: "teacher".to_string(),
        };
        let debug = format!("{:?}", p);
        assert!(debug.contains("teacher"));
        assert!(debug.contains("1"));
    }

    #[test]
    fn test_unauthorized_response_status() {
        let resp = unauthorized_response("test");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_forbidden_response_status() {
        let resp = forbidden_response("test");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}

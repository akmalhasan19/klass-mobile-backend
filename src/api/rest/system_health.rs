//! GET /api/v1/system-health — Lightweight dependency health check.
//!
//! Returns the status of each downstream service so the frontend can display
//! a "system unstable" banner before the user attempts a generation.
//!
//! Each check runs concurrently with a 3-second timeout so the endpoint
//! always responds quickly even if a service is completely down.

use axum::Json;
use serde::Serialize;
use std::time::Duration;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct SystemHealthResponse {
    /// Overall status: "healthy" if all services are up, "degraded" if some are
    /// down, "unhealthy" if critical services (DB) are unreachable.
    pub status: &'static str,
    pub services: Vec<ServiceHealth>,
}

#[derive(Debug, Serialize)]
pub struct ServiceHealth {
    pub name: String,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// GET /api/v1/system-health
///
/// Checks the availability of downstream services concurrently:
/// - PostgreSQL database
/// - Redis cache
/// - Python media renderer
/// - OpenRouter LLM provider
pub async fn system_health(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<serde_json::Value> {
    let (db_result, redis_result, python_result, llm_result) = tokio::join!(
        check_database(&state),
        check_redis(&state),
        check_python_renderer(&state),
        check_openrouter(&state),
    );

    let services = vec![db_result, redis_result, python_result, llm_result];

    // Determine overall status
    let has_unhealthy = services.iter().any(|s| s.status == "unhealthy");
    let has_degraded = services.iter().any(|s| s.status == "degraded");

    let overall = if has_unhealthy {
        "unhealthy"
    } else if has_degraded {
        "degraded"
    } else {
        "healthy"
    };

    let response = SystemHealthResponse {
        status: overall,
        services,
    };

    Json(serde_json::to_value(response).unwrap_or_else(|_| {
        serde_json::json!({ "status": "error", "message": "failed to serialize health response" })
    }))
}

// ─── Individual health checks ───────────────────────────────────────────────

const CHECK_TIMEOUT: Duration = Duration::from_secs(3);

async fn check_database(state: &AppState) -> ServiceHealth {
    let start = std::time::Instant::now();

    let result = tokio::time::timeout(CHECK_TIMEOUT, async {
        sqlx::query("SELECT 1")
            .execute(&state.db_pool)
            .await
    })
    .await;

    let latency_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(_)) => ServiceHealth {
            name: "database".into(),
            status: "healthy",
            latency_ms: Some(latency_ms),
            error: None,
        },
        Ok(Err(e)) => ServiceHealth {
            name: "database".into(),
            status: "unhealthy",
            latency_ms: Some(latency_ms),
            error: Some(e.to_string()),
        },
        Err(_) => ServiceHealth {
            name: "database".into(),
            status: "unhealthy",
            latency_ms: Some(latency_ms),
            error: Some("timeout after 3s".into()),
        },
    }
}

async fn check_redis(state: &AppState) -> ServiceHealth {
    let start = std::time::Instant::now();

    let Some(ref pool) = state.redis_pool else {
        return ServiceHealth {
            name: "redis".into(),
            status: "degraded",
            latency_ms: None,
            error: Some("REDIS_URL not configured".into()),
        };
    };

    let result = tokio::time::timeout(CHECK_TIMEOUT, async {
        let mut conn = pool.get().await.map_err(|e| e.to_string())?;
        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| e.to_string())
    })
    .await;

    let latency_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(_)) => ServiceHealth {
            name: "redis".into(),
            status: "healthy",
            latency_ms: Some(latency_ms),
            error: None,
        },
        Ok(Err(e)) => ServiceHealth {
            name: "redis".into(),
            status: "degraded",
            latency_ms: Some(latency_ms),
            error: Some(e),
        },
        Err(_) => ServiceHealth {
            name: "redis".into(),
            status: "degraded",
            latency_ms: Some(latency_ms),
            error: Some("timeout after 3s".into()),
        },
    }
}

async fn check_python_renderer(state: &AppState) -> ServiceHealth {
    let start = std::time::Instant::now();

    let media_gen_url = &state.config.media_gen_url;
    if media_gen_url.is_empty() {
        return ServiceHealth {
            name: "python_renderer".into(),
            status: "degraded",
            latency_ms: None,
            error: Some("MEDIA_GEN_URL not configured".into()),
        };
    }

    // Try hitting the root or a lightweight endpoint on the Python renderer
    let base_url = media_gen_url.trim_end_matches('/');
    // Strip trailing /v1/jobs or /v1/generate if present — we just want the base
    let base_url = if base_url.ends_with("/v1/jobs") {
        &base_url[..base_url.len() - 8]
    } else if base_url.ends_with("/v1/generate") {
        &base_url[..base_url.len() - 11]
    } else {
        base_url
    };

    let result = tokio::time::timeout(CHECK_TIMEOUT, async {
        state
            .http
            .get(base_url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
    })
    .await;

    let latency_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(resp)) => {
            let status_code = resp.status().as_u16();
            // Any response (even 404/500) means the service is reachable
            if status_code < 500 {
                ServiceHealth {
                    name: "python_renderer".into(),
                    status: "healthy",
                    latency_ms: Some(latency_ms),
                    error: None,
                }
            } else {
                ServiceHealth {
                    name: "python_renderer".into(),
                    status: "unhealthy",
                    latency_ms: Some(latency_ms),
                    error: Some(format!("HTTP {status_code}")),
                }
            }
        }
        Ok(Err(e)) => ServiceHealth {
            name: "python_renderer".into(),
            status: "unhealthy",
            latency_ms: Some(latency_ms),
            error: Some(e.to_string()),
        },
        Err(_) => ServiceHealth {
            name: "python_renderer".into(),
            status: "unhealthy",
            latency_ms: Some(latency_ms),
            error: Some("timeout after 3s".into()),
        },
    }
}

async fn check_openrouter(state: &AppState) -> ServiceHealth {
    let start = std::time::Instant::now();

    let base_url = &state.config.openrouter_base_url;
    if base_url.is_empty() {
        return ServiceHealth {
            name: "openrouter".into(),
            status: "degraded",
            latency_ms: None,
            error: Some("OPENROUTER_BASE_URL not configured".into()),
        };
    }

    // Hit the models endpoint with no auth — if we get 401, the service is up
    let models_url = format!("{}/models", base_url.trim_end_matches('/'));

    let result = tokio::time::timeout(CHECK_TIMEOUT, async {
        state
            .http
            .get(&models_url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
    })
    .await;

    let latency_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(resp)) => {
            let status_code = resp.status().as_u16();
            // 200 = healthy; 401/403 = service is up but needs auth = still healthy
            if status_code < 500 {
                ServiceHealth {
                    name: "openrouter".into(),
                    status: "healthy",
                    latency_ms: Some(latency_ms),
                    error: None,
                }
            } else {
                ServiceHealth {
                    name: "openrouter".into(),
                    status: "unhealthy",
                    latency_ms: Some(latency_ms),
                    error: Some(format!("HTTP {status_code}")),
                }
            }
        }
        Ok(Err(e)) => ServiceHealth {
            name: "openrouter".into(),
            status: "unhealthy",
            latency_ms: Some(latency_ms),
            error: Some(e.to_string()),
        },
        Err(_) => ServiceHealth {
            name: "openrouter".into(),
            status: "unhealthy",
            latency_ms: Some(latency_ms),
            error: Some("timeout after 3s".into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_health_serialization() {
        let h = ServiceHealth {
            name: "database".into(),
            status: "healthy",
            latency_ms: Some(12),
            error: None,
        };
        let v = serde_json::to_value(&h).unwrap();
        assert_eq!(v["name"], "database");
        assert_eq!(v["status"], "healthy");
        assert_eq!(v["latency_ms"], 12);
        assert!(v.get("error").is_none()); // skipped
    }

    #[test]
    fn test_service_health_with_error() {
        let h = ServiceHealth {
            name: "redis".into(),
            status: "degraded",
            latency_ms: None,
            error: Some("not configured".into()),
        };
        let v = serde_json::to_value(&h).unwrap();
        assert_eq!(v["status"], "degraded");
        assert!(v.get("latency_ms").is_none()); // skipped
        assert_eq!(v["error"], "not configured");
    }
}

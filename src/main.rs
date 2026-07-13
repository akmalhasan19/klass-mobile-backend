use axum::{routing::get, Router};
use std::net::SocketAddr;
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

mod config;
mod error;
mod state;

mod auth;
mod cache;
mod db;
mod governance;
mod media_gen;
mod orchestrator;
mod providers;
mod recommendation;

mod api;

use config::AppConfig;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,klass_gateway=debug".into()),
        )
        .json()
        .init();

    let config = AppConfig::from_env()?;
    info!(
        host = %config.host,
        port = %config.port,
        grpc_port = %config.grpc_port,
        "Starting Klass Gateway"
    );

    let state = AppState::new(config.clone()).await?;

    let cors = build_cors_layer(&config.cors_allowed_origins);

    let app = Router::new()
        .route("/health", get(health_check))
        .with_state(state)
        .layer(
            tower::ServiceBuilder::new()
                .layer(SetRequestIdLayer::new(
                    "x-request-id".parse().unwrap(),
                    MakeRequestUuid,
                ))
                .layer(TraceLayer::new_for_http())
                .layer(PropagateRequestIdLayer::new(
                    "x-request-id".parse().unwrap(),
                ))
                .layer(CompressionLayer::new())
                .layer(cors)
                .layer(tower_http::timeout::TimeoutLayer::with_status_code(
                    axum::http::StatusCode::REQUEST_TIMEOUT,
                    Duration::from_secs(120),
                ))
                .into_inner(),
        );

    let addr = SocketAddr::new(config.host.parse()?, config.port);
    info!("REST server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn build_cors_layer(origins: &str) -> CorsLayer {
    if origins.is_empty() || origins == "*" {
        return CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
    }

    let allowed_origins: Vec<_> = origins
        .split(',')
        .filter_map(|o| {
            let o = o.trim();
            if o.is_empty() {
                None
            } else {
                o.parse().ok()
            }
        })
        .collect();

    if allowed_origins.is_empty() {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(allowed_origins))
            .allow_methods(Any)
            .allow_headers(Any)
    }
}

async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

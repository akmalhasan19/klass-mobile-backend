use axum::{routing::get, Router};
use std::net::SocketAddr;
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

    let app = Router::new()
        .route("/health", get(health_check))
        .with_state(state);

    let addr = SocketAddr::new(config.host.parse()?, config.port);
    info!("REST server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

use axum::Router;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::Once;

use klass_gateway::config::AppConfig;
use klass_gateway::state::AppState;

static INIT: Once = Once::new();

pub struct TestContext {
    pub app: Router,
    pub pool: PgPool,
}

fn gateway_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub async fn setup() -> Option<TestContext> {
    INIT.call_once(|| {
        let dir = gateway_dir();
        let env_path = dir.join(".env");
        let env_local_path = dir.join(".env.local");

        if env_path.exists() {
            let _ = dotenvy::from_path(&env_path);
        }
        if env_local_path.exists() {
            let _ = dotenvy::from_path(&env_local_path);
        }
    });

    let database_url = std::env::var("DATABASE_URL").ok()?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .ok()?;

    let config = AppConfig::from_env().ok()?;
    let state = AppState::new(config).await.ok()?;
    let app = axum::Router::new()
        .nest("/api/v1", klass_gateway::api::rest::api_router())
        .with_state(state);

    Some(TestContext { app, pool })
}

#[allow(dead_code)]
pub async fn cleanup_user(pool: &PgPool, email: &str) {
    let _ = sqlx::query("DELETE FROM users WHERE email = $1")
        .bind(email)
        .execute(pool)
        .await;
}

#[allow(dead_code)]
pub async fn cleanup_tokens(pool: &PgPool, user_id: i64) {
    let _ = sqlx::query("DELETE FROM personal_access_tokens WHERE tokenable_id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
}

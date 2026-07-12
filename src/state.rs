use crate::config::AppConfig;
use deadpool_redis::Pool as RedisPool;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db_pool: PgPool,
    pub redis_pool: Option<RedisPool>,
}

impl AppState {
    pub async fn new(config: AppConfig) -> anyhow::Result<Self> {
        let db_pool = PgPool::connect(&config.database_url).await?;

        let redis_pool = if !config.redis_url.is_empty() {
            let cfg = deadpool_redis::Config::from_url(&config.redis_url);
            Some(cfg.create_pool(Some(deadpool_redis::Runtime::Tokio1))?)
        } else {
            tracing::warn!("REDIS_URL not configured — Redis features disabled");
            None
        };

        Ok(Self {
            config,
            db_pool,
            redis_pool,
        })
    }
}

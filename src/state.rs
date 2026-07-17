use std::sync::Arc;

use crate::config::AppConfig;
use crate::recommendation::TaxonomyCatalog;
use aws_sdk_s3::Client as S3Client;
use deadpool_redis::Pool as RedisPool;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db_pool: PgPool,
    pub redis_pool: Option<RedisPool>,
    pub s3_client: S3Client,
    pub http: reqwest::Client,
    pub taxonomy: Arc<TaxonomyCatalog>,
}

impl AppState {
    pub async fn new(config: AppConfig) -> anyhow::Result<Self> {
        let db_pool = PgPoolOptions::new()
            .max_connections(config.database_max_connections)
            .connect(&config.database_url)
            .await?;

        let redis_pool = if !config.redis_url.is_empty() {
            let cfg = deadpool_redis::Config::from_url(&config.redis_url);
            Some(cfg.create_pool(Some(deadpool_redis::Runtime::Tokio1))?)
        } else {
            tracing::warn!("REDIS_URL not configured — Redis features disabled");
            None
        };

        let s3_client = build_s3_client(&config).await;

        let http = reqwest::Client::builder()
            .use_rustls_tls()
            .gzip(true)
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(90))
            .build()?;

        let taxonomy = Arc::new(load_taxonomy());

        // Seed subjects and sub_subjects if empty
        if let Err(e) = crate::db::seed::seed_if_empty(&db_pool).await {
            tracing::warn!(error = %e, "seed: failed to seed subjects from taxonomy — continuing anyway");
        }

        Ok(Self {
            config,
            db_pool,
            redis_pool,
            s3_client,
            http,
            taxonomy,
        })
    }
}

async fn build_s3_client(config: &AppConfig) -> S3Client {
    let credentials = aws_sdk_s3::config::Credentials::new(
        &config.r2_access_key_id,
        &config.r2_secret_access_key,
        None,
        None,
        "r2",
    );

    let s3_config = aws_sdk_s3::Config::builder()
        .endpoint_url(&config.r2_endpoint)
        .credentials_provider(credentials)
        .region(aws_sdk_s3::config::Region::new("auto"))
        .behavior_version_latest()
        .force_path_style(true)
        .build();

    S3Client::from_conf(s3_config)
}

fn load_taxonomy() -> TaxonomyCatalog {
    let catalog = TaxonomyCatalog::load_default();
    tracing::info!("taxonomy catalog loaded from embedded JSON");
    catalog
}

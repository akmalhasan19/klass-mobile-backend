use config::{Config, Environment};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub grpc_port: u16,

    pub database_url: String,
    pub database_max_connections: u32,

    pub redis_url: String,

    pub r2_endpoint: String,
    pub r2_access_key_id: String,
    pub r2_secret_access_key: String,
    pub r2_bucket_name: String,
    pub r2_transit_bucket_name: String,
    pub r2_public_url: String,

    pub media_gen_url: String,
    pub media_gen_hmac_secret: String,
    pub media_gen_webhook_secret: String,
    pub webhook_base_url: String,

    pub openrouter_api_key: String,
    pub openrouter_model: String,
    pub openrouter_base_url: String,

    pub llm_adapter_fallback_url: String,

    pub media_generation: MediaGenerationConfig,

    pub hmac_secret: String,
    pub hmac_max_age_seconds: u64,

    pub rust_log: String,
    pub log_format: String,

    pub cors_allowed_origins: String,


    #[serde(default)]
    pub recommendations: RecommendationsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MediaGenerationConfig {
    pub interpreter: ServiceTimeoutsConfig,
    pub drafting: ServiceTimeoutsConfig,
    pub delivery: ServiceTimeoutsConfig,
    pub python: ServiceTimeoutsConfig,
    pub queue: QueueConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceTimeoutsConfig {
    pub timeout_seconds: f64,
    pub connect_timeout_seconds: f64,
    pub retry_attempts: u32,
    pub retry_sleep_milliseconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QueueConfig {
    pub tries: u32,
    pub timeout_seconds: u64,
    pub backoff_seconds: u64,
    pub concurrency: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecommendationsConfig {
    pub homepage: HomepageConfig,
    pub distribution_summary: DistributionSummaryConfig,
}

impl Default for RecommendationsConfig {
    fn default() -> Self {
        Self {
            homepage: HomepageConfig {
                section_key: "project_recommendations".to_string(),
                feed_endpoint: "/api/v1/homepage-recommendations".to_string(),
            },
            distribution_summary: DistributionSummaryConfig {
                minimum_distinct_user_count: 2,
                maximum_items_per_sub_subject: 1,
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct HomepageConfig {
    pub section_key: String,
    pub feed_endpoint: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DistributionSummaryConfig {
    pub minimum_distinct_user_count: u32,
    pub maximum_items_per_sub_subject: u32,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let _ = dotenvy::from_filename(".env.local");

        let config = Config::builder()
            .add_source(Environment::default().separator("__"))
            .set_default("host", "0.0.0.0")?
            .set_default("port", 8080)?
            .set_default("grpc_port", 50051)?
            .set_default("database_max_connections", 5)?
            .set_default("hmac_max_age_seconds", 300)?
            .set_default("rust_log", "info")?
            .set_default("log_format", "json")?
            .set_default("cors_allowed_origins", "")?
            .set_default("openrouter_model", "xiaomi/mimo-v2.5-pro")?
            .set_default("openrouter_base_url", "https://openrouter.ai/api/v1")?
            .set_default("llm_adapter_fallback_url", "")?
            .set_default("media_gen_webhook_secret", "")?
            .set_default("media_gen_hmac_secret", "")?
            .set_default("webhook_base_url", "")?
            .set_default("media_generation.interpreter.timeout_seconds", 30.0)?
            .set_default("media_generation.interpreter.connect_timeout_seconds", 10.0)?
            .set_default("media_generation.interpreter.retry_attempts", 2)?
            .set_default("media_generation.interpreter.retry_sleep_milliseconds", 250)?
            .set_default("media_generation.drafting.timeout_seconds", 30.0)?
            .set_default("media_generation.drafting.connect_timeout_seconds", 10.0)?
            .set_default("media_generation.drafting.retry_attempts", 2)?
            .set_default("media_generation.drafting.retry_sleep_milliseconds", 250)?
            .set_default("media_generation.delivery.timeout_seconds", 30.0)?
            .set_default("media_generation.delivery.connect_timeout_seconds", 10.0)?
            .set_default("media_generation.delivery.retry_attempts", 2)?
            .set_default("media_generation.delivery.retry_sleep_milliseconds", 250)?
            .set_default("media_generation.python.timeout_seconds", 60.0)?
            .set_default("media_generation.python.connect_timeout_seconds", 10.0)?
            .set_default("media_generation.python.retry_attempts", 2)?
            .set_default("media_generation.python.retry_sleep_milliseconds", 500)?
            .set_default("media_generation.queue.tries", 3)?
            .set_default("media_generation.queue.timeout_seconds", 300)?
            .set_default("media_generation.queue.backoff_seconds", 30)?
            .set_default("media_generation.queue.concurrency", 1)?
            .set_default("r2_transit_bucket_name", "media-generation-service-bucket")?
            .build()?;

        let cfg: Self = config.try_deserialize()?;
        Ok(cfg)
    }
}

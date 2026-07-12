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
    pub r2_public_url: String,

    pub media_gen_url: String,
    pub media_gen_hmac_secret: String,

    pub gemini_api_key: String,
    pub openai_api_key: String,

    pub hmac_secret: String,
    pub hmac_max_age_seconds: u64,

    pub rust_log: String,
    pub log_format: String,

    pub cors_allowed_origins: String,

    pub sanctum_hash_algo: String,
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
            .set_default("sanctum_hash_algo", "sha256")?
            .set_default("cors_allowed_origins", "")?
            .build()?;

        let cfg: Self = config.try_deserialize()?;
        Ok(cfg)
    }
}

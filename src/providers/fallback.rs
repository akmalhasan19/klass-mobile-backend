//! Fallback Python-adapter provider client.
//!
//! Implements the [`Provider`](super::Provider) trait by sending HMAC-signed
//! requests to `LLM_ADAPTER_FALLBACK_URL` (the Python adapter) instead of
//! calling OpenRouter directly.
//!
//! **Toggle mechanism** — purely env-var based:
//! - `LLM_ADAPTER_FALLBACK_URL` set → use this provider as the primary
//! - `LLM_ADAPTER_FALLBACK_URL` empty → use `OpenRouterProviderClient`
//!
//! No code changes required to toggle between the two modes.
//!
//! The Python adapter receives the standard OpenAI-compatible
//! [`CompletionRequest`] JSON body at `{base_url}/chat/completions` with
//! HMAC-SHA256 authentication headers.

use async_trait::async_trait;
use std::time::Duration;

use crate::auth::signing::InterServiceRequestSigner;
use crate::providers::{
    CompletionRequest, CompletionResponse, Provider, ProviderError,
};

// ─── Configuration ──────────────────────────────────────────────────────────

/// Configuration for the fallback Python-adapter provider.
#[derive(Debug, Clone)]
pub struct FallbackProviderConfig {
    /// Base URL of the Python adapter (e.g. `http://localhost:8000`).
    pub base_url: String,
    /// HMAC secret for inter-service authentication.
    pub hmac_secret: String,
    /// HTTP request timeout in seconds.
    pub timeout_seconds: u64,
    /// Number of retry attempts on failure.
    pub retry_attempts: u32,
    /// Base backoff in milliseconds between retries.
    pub retry_backoff_ms: u64,
}

impl FallbackProviderConfig {
    /// Build config from `AppConfig` fields.
    pub fn from_app_config(config: &crate::config::AppConfig) -> Option<Self> {
        let url = config.llm_adapter_fallback_url.trim();
        if url.is_empty() {
            return None;
        }
        Some(Self {
            base_url: url.to_string(),
            hmac_secret: config.hmac_secret.clone(),
            timeout_seconds: 90,
            retry_attempts: 2,
            retry_backoff_ms: 500,
        })
    }
}

// ─── Provider client ────────────────────────────────────────────────────────

/// Provider client that routes requests to the Python adapter via HMAC.
///
/// Sends POST requests to `{base_url}/chat/completions` with:
/// - `Content-Type: application/json`
/// - `X-Request-Id`: UUID v4
/// - `X-Klass-Generation-Id`: UUID (unique per request)
/// - `X-Klass-Request-Timestamp`: Unix epoch seconds
/// - `X-Klass-Signature-Algorithm`: `hmac-sha256`
/// - `X-Klass-Signature`: HMAC-SHA256 hex digest over `timestamp.body`
pub struct FallbackProviderClient {
    http: reqwest::Client,
    config: FallbackProviderConfig,
    signer: InterServiceRequestSigner,
}

impl FallbackProviderClient {
    /// Create a new fallback provider client.
    pub fn new(http: reqwest::Client, config: FallbackProviderConfig) -> Self {
        let signer = InterServiceRequestSigner::new(config.hmac_secret.clone());
        Self {
            http,
            config,
            signer,
        }
    }

    /// Check whether the fallback URL is configured.
    pub fn is_configured(config: &crate::config::AppConfig) -> bool {
        !config.llm_adapter_fallback_url.trim().is_empty()
    }

    /// Build the full URL for the chat completions endpoint.
    fn completions_url(&self) -> String {
        let base = self.config.base_url.trim_end_matches('/');
        format!("{base}/chat/completions")
    }

    /// Send a completion request with HMAC signing and retry logic.
    async fn complete_with_retry(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse, ProviderError> {
        let url = self.completions_url();
        let mut last_error = None;

        for attempt in 1..=self.config.retry_attempts.max(1) {
            // Serialize the request body to JSON bytes for HMAC signing
            let body_bytes = serde_json::to_vec(&request)
                .map_err(|e| ProviderError::Deserialization(e.to_string()))?;

            // Generate a unique generation_id for this request
            let generation_id = uuid::Uuid::new_v4().to_string();

            // Build HMAC-signed headers
            let signed = self.signer.build(&generation_id, &body_bytes);

            // Send the request with HMAC headers
            let req_builder = self
                .http
                .post(&url)
                .header("Content-Type", "application/json")
                .header("X-Request-Id", &signed.request_id)
                .header("X-Klass-Generation-Id", &signed.generation_id)
                .header("X-Klass-Request-Timestamp", &signed.timestamp)
                .header("X-Klass-Signature-Algorithm", &signed.signature_algorithm)
                .header("X-Klass-Signature", &signed.signature)
                .body(body_bytes)
                .timeout(Duration::from_secs(self.config.timeout_seconds));

            match req_builder.send().await {
                Ok(response) => {
                    let status = response.status();
                    if !status.is_success() {
                        let body = response.text().await.unwrap_or_default();
                        last_error = Some(ProviderError::Api {
                            status: status.as_u16(),
                            body,
                        });
                        // Don't retry client errors (4xx except 429)
                        if status.as_u16() < 500 && status.as_u16() != 429 {
                            return Err(last_error.take().unwrap());
                        }
                    } else {
                        // Deserialize into CompletionResponse
                        let completion: CompletionResponse = response
                            .json()
                            .await
                            .map_err(|e| ProviderError::Deserialization(e.to_string()))?;
                        return Ok(completion);
                    }
                }
                Err(e) => {
                    last_error = Some(ProviderError::Http(e));
                }
            }

            // Backoff before retry
            if attempt < self.config.retry_attempts.max(1) {
                let delay = self.config.retry_backoff_ms * (1u64 << (attempt - 1));
                tracing::debug!(
                    "fallback-adapter attempt {}/{} failed — retrying in {}ms",
                    attempt,
                    self.config.retry_attempts,
                    delay
                );
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ProviderError::AllExhausted {
                attempts: self.config.retry_attempts,
                reason: "fallback-adapter: all retry attempts exhausted".to_string(),
            }
        }))
    }
}

#[async_trait]
impl Provider for FallbackProviderClient {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, ProviderError> {
        self.complete_with_retry(request).await
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_config_from_app_config_when_empty() {
        let mut config = crate::config::AppConfig {
            llm_adapter_fallback_url: "".to_string(),
            hmac_secret: "test-secret".to_string(),
            host: "0.0.0.0".to_string(),
            port: 8080,
            grpc_port: 50051,
            database_url: String::new(),
            database_max_connections: 5,
            redis_url: String::new(),
            r2_endpoint: String::new(),
            r2_access_key_id: String::new(),
            r2_secret_access_key: String::new(),
            r2_bucket_name: String::new(),
            r2_public_url: String::new(),
            media_gen_url: String::new(),
            media_gen_hmac_secret: String::new(),
            media_gen_webhook_secret: String::new(),
            openrouter_api_key: String::new(),
            openrouter_model: String::new(),
            openrouter_base_url: String::new(),
            media_generation: crate::config::MediaGenerationConfig {
                interpreter: crate::config::ServiceTimeoutsConfig {
                    timeout_seconds: 30.0, connect_timeout_seconds: 10.0,
                    retry_attempts: 2, retry_sleep_milliseconds: 250,
                },
                drafting: crate::config::ServiceTimeoutsConfig {
                    timeout_seconds: 30.0, connect_timeout_seconds: 10.0,
                    retry_attempts: 2, retry_sleep_milliseconds: 250,
                },
                delivery: crate::config::ServiceTimeoutsConfig {
                    timeout_seconds: 30.0, connect_timeout_seconds: 10.0,
                    retry_attempts: 2, retry_sleep_milliseconds: 250,
                },
                python: crate::config::ServiceTimeoutsConfig {
                    timeout_seconds: 60.0, connect_timeout_seconds: 10.0,
                    retry_attempts: 2, retry_sleep_milliseconds: 500,
                },
                queue: crate::config::QueueConfig {
                    tries: 3, timeout_seconds: 300, backoff_seconds: 30, concurrency: 1,
                },
            },
            hmac_max_age_seconds: 300,
            rust_log: "info".to_string(),
            log_format: "json".to_string(),
            cors_allowed_origins: String::new(),
            recommendations: crate::config::RecommendationsConfig::default(),
        };
        assert!(FallbackProviderConfig::from_app_config(&config).is_none());
        assert!(!FallbackProviderClient::is_configured(&config));

        config.llm_adapter_fallback_url = "http://adapter:8000".to_string();
        let fb_config = FallbackProviderConfig::from_app_config(&config);
        assert!(fb_config.is_some());
        assert_eq!(fb_config.as_ref().unwrap().base_url, "http://adapter:8000");
        assert!(FallbackProviderClient::is_configured(&config));
    }

    #[test]
    fn test_completions_url_no_trailing_slash() {
        let config = FallbackProviderConfig {
            base_url: "http://adapter:8000".to_string(),
            hmac_secret: "secret".to_string(),
            timeout_seconds: 90,
            retry_attempts: 2,
            retry_backoff_ms: 500,
        };
        let client = FallbackProviderClient::new(reqwest::Client::new(), config);
        assert_eq!(
            client.completions_url(),
            "http://adapter:8000/chat/completions"
        );
    }

    #[test]
    fn test_completions_url_with_trailing_slash() {
        let config = FallbackProviderConfig {
            base_url: "http://adapter:8000/".to_string(),
            hmac_secret: "secret".to_string(),
            timeout_seconds: 90,
            retry_attempts: 2,
            retry_backoff_ms: 500,
        };
        let client = FallbackProviderClient::new(reqwest::Client::new(), config);
        assert_eq!(
            client.completions_url(),
            "http://adapter:8000/chat/completions"
        );
    }

    #[test]
    fn test_fallback_config_default_timeout() {
        let config = FallbackProviderConfig {
            base_url: "http://adapter:8000".to_string(),
            hmac_secret: "s3cr3t".to_string(),
            timeout_seconds: 90,
            retry_attempts: 2,
            retry_backoff_ms: 500,
        };
        assert_eq!(config.timeout_seconds, 90);
        assert_eq!(config.retry_attempts, 2);
        assert_eq!(config.retry_backoff_ms, 500);
    }

    #[test]
    fn test_fallback_client_new() {
        let config = FallbackProviderConfig {
            base_url: "http://adapter:8000".to_string(),
            hmac_secret: "s3cr3t".to_string(),
            timeout_seconds: 60,
            retry_attempts: 1,
            retry_backoff_ms: 200,
        };
        let client = FallbackProviderClient::new(reqwest::Client::new(), config);
        assert_eq!(client.config.timeout_seconds, 60);
        assert_eq!(client.config.retry_attempts, 1);
    }

    #[test]
    fn test_is_configured() {
        let mut config = crate::config::AppConfig {
            llm_adapter_fallback_url: "".to_string(),
            host: "0.0.0.0".to_string(),
            port: 8080,
            grpc_port: 50051,
            database_url: String::new(),
            database_max_connections: 5,
            redis_url: String::new(),
            r2_endpoint: String::new(),
            r2_access_key_id: String::new(),
            r2_secret_access_key: String::new(),
            r2_bucket_name: String::new(),
            r2_public_url: String::new(),
            media_gen_url: String::new(),
            media_gen_hmac_secret: String::new(),
            media_gen_webhook_secret: String::new(),
            openrouter_api_key: String::new(),
            openrouter_model: String::new(),
            openrouter_base_url: String::new(),
            media_generation: crate::config::MediaGenerationConfig {
                interpreter: crate::config::ServiceTimeoutsConfig {
                    timeout_seconds: 30.0, connect_timeout_seconds: 10.0,
                    retry_attempts: 2, retry_sleep_milliseconds: 250,
                },
                drafting: crate::config::ServiceTimeoutsConfig {
                    timeout_seconds: 30.0, connect_timeout_seconds: 10.0,
                    retry_attempts: 2, retry_sleep_milliseconds: 250,
                },
                delivery: crate::config::ServiceTimeoutsConfig {
                    timeout_seconds: 30.0, connect_timeout_seconds: 10.0,
                    retry_attempts: 2, retry_sleep_milliseconds: 250,
                },
                python: crate::config::ServiceTimeoutsConfig {
                    timeout_seconds: 60.0, connect_timeout_seconds: 10.0,
                    retry_attempts: 2, retry_sleep_milliseconds: 500,
                },
                queue: crate::config::QueueConfig {
                    tries: 3, timeout_seconds: 300, backoff_seconds: 30, concurrency: 1,
                },
            },
            hmac_secret: "test".to_string(),
            hmac_max_age_seconds: 300,
            rust_log: "info".to_string(),
            log_format: "json".to_string(),
            cors_allowed_origins: String::new(),
            recommendations: crate::config::RecommendationsConfig::default(),
        };
        assert!(!FallbackProviderClient::is_configured(&config));

        config.llm_adapter_fallback_url = "http://adapter:8000".to_string();
        assert!(FallbackProviderClient::is_configured(&config));
    }
}

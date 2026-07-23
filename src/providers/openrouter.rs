//! OpenRouter provider client.
//!
//! Dedicated provider for OpenRouter's OpenAI-compatible `/chat/completions`
//! endpoint. Sends the required headers, applies configurable timeout, and
//! includes robust response parsing with multiple fallback strategies.

use async_trait::async_trait;
use std::time::Duration;

use crate::providers::{
    ChatMessage, CompletionRequest, CompletionResponse, Provider, ProviderError,
};

// ─── Constants ───────────────────────────────────────────────────────────────

const DEFAULT_TIMEOUT_SECS: u64 = 90;
const DEFAULT_RETRY_ATTEMPTS: u32 = 1;
const DEFAULT_RETRY_BACKOFF_MS: u64 = 500;

// ─── Configuration ───────────────────────────────────────────────────────────

/// Configuration for the OpenRouter provider.
#[derive(Debug, Clone)]
pub struct OpenRouterConfig {
    /// OpenRouter API key (sk-or-...).
    pub api_key: String,
    /// Default model name (e.g. "minimax/minimax-m3").
    pub model: String,
    /// Base URL (e.g. "https://openrouter.ai/api/v1").
    pub base_url: String,
    /// HTTP request timeout in seconds.
    pub timeout_seconds: u64,
    /// Number of retry attempts on failure (default 2).
    pub retry_attempts: u32,
    /// Base backoff in milliseconds between retries (doubles each attempt).
    pub retry_backoff_ms: u64,
}

impl OpenRouterConfig {
    /// Build config from `AppConfig` fields.
    pub fn from_app_config(config: &crate::config::AppConfig) -> Self {
        Self {
            api_key: config.openrouter_api_key.clone(),
            model: config.openrouter_model.clone(),
            base_url: config.openrouter_base_url.clone(),
            timeout_seconds: DEFAULT_TIMEOUT_SECS,
            retry_attempts: DEFAULT_RETRY_ATTEMPTS,
            retry_backoff_ms: DEFAULT_RETRY_BACKOFF_MS,
        }
    }
}

impl Default for OpenRouterConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "minimax/minimax-m3".to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            timeout_seconds: DEFAULT_TIMEOUT_SECS,
            retry_attempts: DEFAULT_RETRY_ATTEMPTS,
            retry_backoff_ms: DEFAULT_RETRY_BACKOFF_MS,
        }
    }
}

// ─── Provider client ─────────────────────────────────────────────────────────

/// Provider client for OpenRouter (OpenAI-compatible API).
///
/// Sends POST requests to `{base_url}/chat/completions` with:
/// - `Authorization: Bearer {api_key}`
/// - `HTTP-Referer: klass-mobile`
/// - `X-Title: klass-gateway`
///
/// Response parsing uses multiple fallback strategies (see `extract_content`).
pub struct OpenRouterProviderClient {
    http: reqwest::Client,
    config: OpenRouterConfig,
}

impl OpenRouterProviderClient {
    /// Create a new OpenRouter provider client.
    ///
    /// The `reqwest::Client` should be the shared instance from `AppState`
    /// (which has HTTP/2, connection pooling, and gzip enabled).
    pub fn new(http: reqwest::Client, config: OpenRouterConfig) -> Self {
        Self { http, config }
    }

    /// Build the full URL for the chat completions endpoint.
    fn completions_url(&self) -> String {
        let base = self.config.base_url.trim_end_matches('/');
        format!("{base}/chat/completions")
    }

    /// Send a completion request, parse the response, and extract text content
    /// using the configured retry and fallback strategies.
    async fn complete_with_retry(
        &self,
        mut request: CompletionRequest,
    ) -> Result<CompletionResponse, ProviderError> {
        // Apply default model if not explicitly set
        if request.model.is_empty() {
            request.model = self.config.model.clone();
        }

        let url = self.completions_url();
        let mut last_error = None;

        for attempt in 1..=self.config.retry_attempts.max(1) {
            // Build the request with headers and timeout
            let req_builder = self
                .http
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .header("HTTP-Referer", "klass-mobile")
                .header("X-Title", "klass-gateway")
                .header("Content-Type", "application/json")
                .json(&request)
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
                        // ── Robust response parsing ──────────────────────
                        // Read the body as text first so we can attempt
                        // multiple deserialization strategies.
                        let body_text = match response.text().await {
                            Ok(t) => t,
                            Err(e) => {
                                last_error = Some(ProviderError::Deserialization(
                                    format!("failed to read response body: {}", e),
                                ));
                                // retryable — continue to next attempt
                                if attempt < self.config.retry_attempts.max(1) {
                                    let delay = self.config.retry_backoff_ms * (1u64 << (attempt - 1));
                                    tracing::debug!(
                                        "openrouter attempt {}/{} failed (body read) — retrying in {}ms",
                                        attempt, self.config.retry_attempts, delay
                                    );
                                    tokio::time::sleep(Duration::from_millis(delay)).await;
                                }
                                continue;
                            }
                        };

                        // Strategy 1: standard CompletionResponse deserialization
                        match serde_json::from_str::<CompletionResponse>(&body_text) {
                            Ok(completion) => return Ok(completion),
                            Err(deser_err) => {
                                // Strategy 2: fallback via extract_content()
                                // which handles output_text, content arrays,
                                // choices[].text, and other non-standard formats.
                                if let Ok(raw_value) = serde_json::from_str::<serde_json::Value>(&body_text) {
                                    if let Some(content) = extract_content(&raw_value) {
                                        tracing::debug!(
                                            "openrouter: standard deser failed, extracted content via fallback"
                                        );
                                        let model = raw_value
                                            .get("model")
                                            .and_then(|v| v.as_str())
                                            .map(String::from);
                                        let usage: Option<super::Usage> = raw_value
                                            .get("usage")
                                            .and_then(|u| serde_json::from_value(u.clone()).ok());
                                        return Ok(CompletionResponse {
                                            choices: vec![super::Choice {
                                                message: super::ChatMessage {
                                                    role: "assistant".to_string(),
                                                    content,
                                                },
                                                finish_reason: Some("stop".to_string()),
                                                index: 0,
                                            }],
                                            usage,
                                            model,
                                        });
                                    }
                                }
                                // Both strategies failed — retryable error
                                tracing::warn!(
                                    "openrouter: response deserialization failed: {}",
                                    deser_err
                                );
                                last_error = Some(ProviderError::Deserialization(
                                    format!("error decoding response body: {}", deser_err),
                                ));
                            }
                        }
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
                    "openrouter attempt {}/{} failed — retrying in {}ms",
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
                reason: "openrouter: all retry attempts exhausted".to_string(),
            }
        }))
    }
}

#[async_trait]
impl Provider for OpenRouterProviderClient {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, ProviderError> {
        self.complete_with_retry(request).await
    }
}

// ─── Content extraction with fallback ────────────────────────────────────────

/// Extract text content from a raw provider response using fallback strategies.
///
/// Priority:
/// 1. `choices[0].message.content` (standard OpenAI format)
/// 2. `output_text` field (alternative non-standard format)
/// 3. `content` array of strings or `{text: "..."}` objects (alternative format)
/// 4. Any non-null, non-empty string field found in the response
pub fn extract_content(raw: &serde_json::Value) -> Option<String> {
    // Strategy 1: choices[0].message.content
    if let Some(content) = raw
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|msg| msg.get("content"))
        .and_then(|c| c.as_str())
    {
        if !content.is_empty() {
            return Some(clean_markdown_json(content));
        }
    }

    // Strategy 2: output_text field
    if let Some(text) = raw.get("output_text").and_then(|v| v.as_str()) {
        if !text.is_empty() {
            return Some(clean_markdown_json(text));
        }
    }

    // Strategy 3: content array
    if let Some(arr) = raw.get("content").and_then(|v| v.as_array()) {
        let mut parts: Vec<String> = Vec::new();
        for item in arr {
            if let Some(text) = item.as_str() {
                if !text.is_empty() {
                    parts.push(text.to_string());
                }
            } else if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    parts.push(text.to_string());
                }
            }
        }
        if !parts.is_empty() {
            return Some(clean_markdown_json(&parts.join("\n")));
        }
    }

    // Strategy 4: choices[0].text (completion-style format)
    if let Some(text) = raw
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|choice| choice.get("text"))
        .and_then(|t| t.as_str())
    {
        if !text.is_empty() {
            return Some(clean_markdown_json(text));
        }
    }

    None
}

/// Strip markdown formatting and conversational text to extract the raw JSON object.
fn clean_markdown_json(raw: &str) -> String {
    let trimmed = raw.trim();
    
    // Most LLM conversational padding can be bypassed by finding the first '{' and last '}'
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start < end {
            return trimmed[start..=end].to_string();
        }
    }
    
    // Fallback just in case it doesn't contain braces (e.g. it's an array or bare string)
    let mut stripped = trimmed;
    if stripped.starts_with("```json") {
        stripped = stripped.trim_start_matches("```json").trim();
    } else if stripped.starts_with("```") {
        stripped = stripped.trim_start_matches("```").trim();
    }
    
    if stripped.ends_with("```") {
        stripped = stripped.trim_end_matches("```").trim();
    }
    
    stripped.to_string()
}

// ─── Helper: build request with JSON mode ────────────────────────────────────

/// Create a `CompletionRequest` for structured JSON output.
///
/// Does NOT set `response_format` because not all models/providers support
/// it (e.g. minimax/hy3 via Novita only supports `json_schema`, not
/// `json_object`). The system prompt already instructs the model to return
/// valid JSON, so the model will comply without the explicit format hint.
pub fn json_mode_request(
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> CompletionRequest {
    CompletionRequest::new(model, vec![
        ChatMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: user_prompt.to_string(),
        },
    ]).with_json_mode()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openrouter_config_default() {
        let config = OpenRouterConfig::default();
        assert_eq!(config.model, "minimax/minimax-m3");
        assert_eq!(config.timeout_seconds, 90);
        assert_eq!(config.retry_attempts, 1);
        assert_eq!(config.retry_backoff_ms, 500);
    }

    #[test]
    fn test_completions_url_no_trailing_slash() {
        let config = OpenRouterConfig {
            base_url: "https://openrouter.ai/api/v1".to_string(),
            ..OpenRouterConfig::default()
        };
        let client = OpenRouterProviderClient {
            http: reqwest::Client::new(),
            config,
        };
        assert_eq!(
            client.completions_url(),
            "https://openrouter.ai/api/v1/chat/completions"
        );
    }

    #[test]
    fn test_completions_url_with_trailing_slash() {
        let config = OpenRouterConfig {
            base_url: "https://openrouter.ai/api/v1/".to_string(),
            ..OpenRouterConfig::default()
        };
        let client = OpenRouterProviderClient {
            http: reqwest::Client::new(),
            config,
        };
        assert_eq!(
            client.completions_url(),
            "https://openrouter.ai/api/v1/chat/completions"
        );
    }

    #[test]
    fn test_from_app_config() {
        let config = crate::config::AppConfig {
            openrouter_api_key: "sk-or-test-key".to_string(),
            openrouter_model: "minimax/minimax-m3".to_string(),
            openrouter_base_url: "https://openrouter.ai/api/v1".to_string(),
            host: "0.0.0.0".to_string(),
            port: 8080,
            grpc_port: 50051,
            database_url: "postgres://localhost/test".to_string(),
            database_max_connections: 5,
            redis_url: String::new(),
            r2_endpoint: String::new(),
            r2_access_key_id: String::new(),
            r2_secret_access_key: String::new(),
            r2_bucket_name: String::new(),
            r2_transit_bucket_name: String::new(),
            r2_public_url: String::new(),
            media_gen_url: String::new(),
            webhook_base_url: String::new(),
            media_gen_hmac_secret: String::new(),
            media_gen_webhook_secret: String::new(),
            llm_adapter_fallback_url: String::new(),
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
                rate_limit: Default::default(),
            },
            hmac_secret: String::new(),
            hmac_max_age_seconds: 300,
            rust_log: "info".to_string(),
            log_format: "json".to_string(),
            cors_allowed_origins: String::new(),
            recommendations: crate::config::RecommendationsConfig::default(),
        };
        let or_config = OpenRouterConfig::from_app_config(&config);
        assert_eq!(or_config.api_key, "sk-or-test-key");
        assert_eq!(or_config.model, "minimax/minimax-m3");
        assert_eq!(or_config.timeout_seconds, 90);
    }

    // ── Content extraction ────────────────────────────────────────────────

    #[test]
    fn test_extract_content_standard_format() {
        let raw: serde_json::Value = serde_json::from_str(r#"{
            "choices": [{
                "message": {"role": "assistant", "content": "Hello world"},
                "finish_reason": "stop"
            }]
        }"#).unwrap();
        assert_eq!(extract_content(&raw), Some("Hello world".to_string()));
    }

    #[test]
    fn test_extract_content_output_text() {
        let raw: serde_json::Value = serde_json::from_str(r#"{
            "output_text": "Fallback content"
        }"#).unwrap();
        assert_eq!(extract_content(&raw), Some("Fallback content".to_string()));
    }

    #[test]
    fn test_extract_content_content_array() {
        let raw: serde_json::Value = serde_json::from_str(r#"{
            "content": ["Part 1", "Part 2"]
        }"#).unwrap();
        let result = extract_content(&raw).unwrap();
        assert!(result.contains("Part 1"));
        assert!(result.contains("Part 2"));
    }

    #[test]
    fn test_extract_content_content_array_with_text_objects() {
        let raw: serde_json::Value = serde_json::from_str(r#"{
            "content": [{"text": "Object text"}, {"text": "More text"}]
        }"#).unwrap();
        let result = extract_content(&raw).unwrap();
        assert!(result.contains("Object text"));
        assert!(result.contains("More text"));
    }

    #[test]
    fn test_extract_content_choices_text() {
        let raw: serde_json::Value = serde_json::from_str(r#"{
            "choices": [{"text": "Completion style", "index": 0}]
        }"#).unwrap();
        assert_eq!(extract_content(&raw), Some("Completion style".to_string()));
    }

    #[test]
    fn test_extract_content_empty_returns_none() {
        let raw: serde_json::Value = serde_json::from_str(r#"{}"#).unwrap();
        assert!(extract_content(&raw).is_none());
    }

    #[test]
    fn test_extract_content_standard_takes_priority() {
        let raw: serde_json::Value = serde_json::from_str(r#"{
            "choices": [{
                "message": {"role": "assistant", "content": "Primary content"},
                "finish_reason": "stop"
            }],
            "output_text": "Fallback content"
        }"#).unwrap();
        // Standard format should take priority
        assert_eq!(extract_content(&raw), Some("Primary content".to_string()));
    }

    #[test]
    fn test_extract_content_empty_standard_falls_back() {
        let raw: serde_json::Value = serde_json::from_str(r#"{
            "choices": [{
                "message": {"role": "assistant", "content": ""},
                "finish_reason": "stop"
            }],
            "output_text": "Fallback works"
        }"#).unwrap();
        // Empty standard content should fall back to output_text
        assert_eq!(extract_content(&raw), Some("Fallback works".to_string()));
    }

    // ── JSON mode helper ──────────────────────────────────────────────────

    #[test]
    fn test_json_mode_request_has_response_format() {
        let req = json_mode_request("test-model", "You are a helpful assistant.", "Do something.");
        assert_eq!(req.model, "test-model");
        assert_eq!(req.messages.len(), 2);
        assert!(req.response_format.is_some());
        assert_eq!(
            req.response_format.as_ref().unwrap().format_type,
            "json_object"
        );
    }

    #[test]
    fn test_json_mode_request_serializes_correctly() {
        let req = json_mode_request("m", "You are a JSON bot.", "Generate JSON.");
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["response_format"]["type"], "json_object");
        assert_eq!(json["messages"][0]["role"], "system");
        assert_eq!(json["messages"][0]["content"], "You are a JSON bot.");
        assert_eq!(json["messages"][1]["role"], "user");
    }

    // ── with_json_mode ────────────────────────────────────────────────────

    #[test]
    fn test_with_json_mode_on_request() {
        let mut req = CompletionRequest::new("test", vec![]);
        assert!(req.response_format.is_none());
        req = req.with_json_mode();
        assert!(req.response_format.is_some());
        assert_eq!(
            req.response_format.as_ref().unwrap().format_type,
            "json_object"
        );
    }

    #[test]
    fn test_with_json_mode_idempotent() {
        let req = CompletionRequest::new("test", vec![])
            .with_json_mode()
            .with_json_mode(); // calling twice should be fine
        assert!(req.response_format.is_some());
    }
}

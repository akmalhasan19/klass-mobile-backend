//! LLM Provider module.
//!
//! Declares the `Provider` trait, request/response types, and submodules for
//! specific provider implementations (`openrouter`) and the routing layer
//! (`router`) with primary + fallback selection and circuit breaker.
//!
//! HTTP/2 connection pooling is provided by the shared `reqwest::Client` stored
//! in `AppState`.

pub mod fallback;
pub mod openrouter;
pub mod router;
pub use fallback::{FallbackProviderClient, FallbackProviderConfig};
pub use openrouter::{extract_content, json_mode_request, OpenRouterConfig, OpenRouterProviderClient};
pub use router::{ProviderRouter, RetryConfig};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ─── Error types ─────────────────────────────────────────────────────────────

/// Errors that can occur during LLM provider calls.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// HTTP transport error (connection refused, timeout, DNS, etc.)
    #[error("HTTP transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// API returned a non-success HTTP status.
    #[error("API error (status={status}): {body}")]
    Api { status: u16, body: String },

    /// Failed to deserialize the provider response.
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// All provider attempts (primary + fallback) exhausted.
    #[error("All providers exhausted after {attempts} attempt(s): {reason}")]
    AllExhausted { attempts: u32, reason: String },

    /// Provider configuration error.
    #[error("Provider configuration error: {0}")]
    Config(String),
}

// ─── Request types ───────────────────────────────────────────────────────────

/// A single chat message in the OpenAI-compatible format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role: system, user, or assistant.
    pub role: String,
    /// Content of the message.
    pub content: String,
}

/// Response format specification (JSON mode, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    /// Format type, e.g. "json_object".
    #[serde(rename = "type")]
    pub format_type: String,
}

/// Request payload for the provider's `/chat/completions` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// Model identifier (if empty, provider uses its default model).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub model: String,
    /// Conversation messages.
    pub messages: Vec<ChatMessage>,
    /// Optional response format (e.g. JSON mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Sampling temperature (0.0 – 2.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
}

impl CompletionRequest {
    /// Create a new completion request with the given messages.
    pub fn new(model: &str, messages: Vec<ChatMessage>) -> Self {
        let model = model.to_string();
        Self {
            model,
            messages,
            response_format: None,
            max_tokens: None,
            temperature: None,
        }
    }

    /// Enable JSON mode by setting `response_format: {type: "json_object"}`.
    pub fn with_json_mode(mut self) -> Self {
        self.response_format = Some(ResponseFormat {
            format_type: "json_object".to_string(),
        });
        self
    }
}

// ─── Response types ──────────────────────────────────────────────────────────

/// Usage statistics returned by the provider.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Usage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

/// A single choice (completion) returned by the provider.
#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub index: u32,
}

/// Response from the provider's `/chat/completions` endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct CompletionResponse {
    #[serde(default)]
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
    pub model: Option<String>,
}

impl CompletionResponse {
    /// Extract the content from the first choice, if available.
    pub fn first_choice_content(&self) -> Option<&str> {
        self.choices.first().map(|c| c.message.content.as_str())
    }

    /// Extract the finish_reason from the first choice.
    pub fn first_finish_reason(&self) -> Option<&str> {
        self.choices.first().and_then(|c| c.finish_reason.as_deref())
    }
}

// ─── Provider trait ──────────────────────────────────────────────────────────

/// Abstract LLM provider that can complete chat requests.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Send a completion request and return the response.
    ///
    /// Implementations should handle:
    /// - Transport errors (connection refused, timeouts)
    /// - API errors (non-success HTTP status codes)
    /// - Response deserialization
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, ProviderError>;
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── ChatMessage ──────────────────────────────────────────────────────────

    #[test]
    fn test_chat_message() {
        let msg = ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        };
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_chat_message_serialization() {
        let msg = ChatMessage {
            role: "system".to_string(),
            content: "You are helpful.".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"system\""));
        assert!(json.contains("\"You are helpful.\""));
    }

    // ── CompletionRequest ─────────────────────────────────────────────────

    #[test]
    fn test_completion_request_new() {
        let req = CompletionRequest::new(
            "tencent/hy3:free",
            vec![ChatMessage {
                role: "user".to_string(),
                content: "Hi".to_string(),
            }],
        );
        assert_eq!(req.model, "tencent/hy3:free");
        assert_eq!(req.messages.len(), 1);
    }

    #[test]
    fn test_completion_request_optional_fields() {
        let req = CompletionRequest {
            model: "test".to_string(),
            messages: vec![],
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
            }),
            max_tokens: Some(2048),
            temperature: Some(0.7),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["response_format"]["type"], "json_object");
        assert_eq!(json["max_tokens"], 2048);
        assert_eq!(json["temperature"], 0.7);
    }

    #[test]
    fn test_completion_request_with_json_mode() {
        let req = CompletionRequest::new("test", vec![]).with_json_mode();
        assert!(req.response_format.is_some());
        assert_eq!(req.response_format.as_ref().unwrap().format_type, "json_object");
    }

    #[test]
    fn test_completion_request_skips_empty_model() {
        let req = CompletionRequest {
            model: "".to_string(),
            messages: vec![],
            response_format: None,
            max_tokens: None,
            temperature: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("model").is_none(), "empty model should be skipped");
    }

    // ── CompletionResponse ────────────────────────────────────────────────

    #[test]
    fn test_completion_response_first_choice_content() {
        let resp = CompletionResponse {
            choices: vec![Choice {
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: "Hello there!".to_string(),
                },
                finish_reason: Some("stop".to_string()),
                index: 0,
            }],
            usage: Some(Usage {
                prompt_tokens: Some(10),
                completion_tokens: Some(5),
                total_tokens: Some(15),
            }),
            model: Some("test-model".to_string()),
        };
        assert_eq!(resp.first_choice_content(), Some("Hello there!"));
        assert_eq!(resp.first_finish_reason(), Some("stop"));
    }

    #[test]
    fn test_completion_response_empty_choices() {
        let resp = CompletionResponse {
            choices: vec![],
            usage: None,
            model: None,
        };
        assert!(resp.first_choice_content().is_none());
        assert!(resp.first_finish_reason().is_none());
    }

    #[test]
    fn test_completion_response_deserialization() {
        let json = r#"{
            "choices": [{
                "message": {"role": "assistant", "content": "Hi"},
                "finish_reason": "stop",
                "index": 0
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15},
            "model": "tencent/hy3:free"
        }"#;
        let resp: CompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.first_choice_content(), Some("Hi"));
        assert_eq!(resp.model.as_deref(), Some("tencent/hy3:free"));
    }

    // ── Provider trait ────────────────────────────────────────────────────

    struct MockProvider {
        should_fail: bool,
        response_model: String,
    }

    impl MockProvider {
        fn new(should_fail: bool) -> Self {
            Self {
                should_fail,
                response_model: "mock-model".to_string(),
            }
        }
    }

    #[async_trait]
    impl Provider for MockProvider {
        async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse, ProviderError> {
            if self.should_fail {
                Err(ProviderError::Api {
                    status: 500,
                    body: "mock failure".to_string(),
                })
            } else {
                Ok(CompletionResponse {
                    choices: vec![Choice {
                        message: ChatMessage {
                            role: "assistant".to_string(),
                            content: "Hello!".to_string(),
                        },
                        finish_reason: Some("stop".to_string()),
                        index: 0,
                    }],
                    usage: Some(Usage {
                        prompt_tokens: Some(10),
                        completion_tokens: Some(5),
                        total_tokens: Some(15),
                    }),
                    model: Some(self.response_model.clone()),
                })
            }
        }
    }

    #[tokio::test]
    async fn test_mock_provider_success() {
        let provider = MockProvider::new(false);
        let response = provider
            .complete(CompletionRequest::new("test", vec![]))
            .await
            .unwrap();
        assert_eq!(response.first_choice_content(), Some("Hello!"));
        assert_eq!(response.usage.as_ref().and_then(|u| u.total_tokens), Some(15));
    }

    #[tokio::test]
    async fn test_mock_provider_failure() {
        let provider = MockProvider::new(true);
        let err = provider
            .complete(CompletionRequest::new("test", vec![]))
            .await
            .unwrap_err();
        match err {
            ProviderError::Api { status, .. } => assert_eq!(status, 500),
            _ => panic!("Expected Api error"),
        }
    }

    // ── ProviderError ─────────────────────────────────────────────────────

    #[test]
    fn test_provider_error_display() {
        let err = ProviderError::Api {
            status: 429,
            body: "Rate limited".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("429"));
        assert!(msg.contains("Rate limited"));
    }

    #[test]
    fn test_provider_error_all_exhausted() {
        let err = ProviderError::AllExhausted {
            attempts: 3,
            reason: "all providers returned errors".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("3"));
        assert!(msg.contains("all providers"));
    }
}

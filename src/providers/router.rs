//! Provider router — primary + fallback selection with retry and circuit breaker.
//!
//! Routes completion requests to the primary provider, retries on failure
//! with exponential backoff, then falls back to the optional secondary provider.
//! A simple circuit breaker opens after `circuit_breaker_threshold` consecutive
//! failures.

use std::time::Duration;

use crate::providers::{
    CompletionRequest, CompletionResponse, Provider, ProviderError,
};

// ─── Retry configuration ─────────────────────────────────────────────────────

/// Retry configuration for provider calls.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of attempts (including the initial call).
    pub max_attempts: u32,
    /// Base backoff duration in milliseconds (doubles each retry).
    pub base_backoff_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_backoff_ms: 500,
        }
    }
}

// ─── Provider router ─────────────────────────────────────────────────────────

/// Router that selects between primary and fallback providers.
///
/// - Primary provider is always tried first.
/// - On failure, retries with exponential backoff (up to `retry_config.max_attempts`).
/// - If primary still fails, tries the optional fallback provider.
/// - Circuit breaker state can be checked via `is_circuit_open()`.
pub struct ProviderRouter {
    primary: Box<dyn Provider>,
    fallback: Option<Box<dyn Provider>>,
    retry_config: RetryConfig,
    /// Simple failure counter (reset on success, open circuit at threshold).
    consecutive_failures: std::sync::atomic::AtomicU32,
    /// Maximum consecutive failures before circuit opens.
    circuit_breaker_threshold: u32,
}

impl ProviderRouter {
    /// Create a new provider router with the given primary provider.
    pub fn new(primary: Box<dyn Provider>) -> Self {
        Self {
            primary,
            fallback: None,
            retry_config: RetryConfig::default(),
            consecutive_failures: std::sync::atomic::AtomicU32::new(0),
            circuit_breaker_threshold: 5,
        }
    }

    /// Set the fallback provider.
    pub fn with_fallback(mut self, fallback: Box<dyn Provider>) -> Self {
        self.fallback = Some(fallback);
        self
    }

    /// Set a custom retry configuration.
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// Set the circuit breaker threshold.
    pub fn with_circuit_breaker_threshold(mut self, threshold: u32) -> Self {
        self.circuit_breaker_threshold = threshold;
        self
    }

    /// Check whether the circuit is open (too many consecutive failures).
    pub fn is_circuit_open(&self) -> bool {
        self.consecutive_failures
            .load(std::sync::atomic::Ordering::Relaxed)
            >= self.circuit_breaker_threshold
    }

    /// Send a completion request through the router.
    ///
    /// Returns the response from the primary provider if successful.
    /// On repeated failure, tries the fallback provider (if configured).
    /// Returns `ProviderError::AllExhausted` if all attempts fail.
    pub async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, ProviderError> {
        // Circuit breaker: fast-fail if open
        if self.is_circuit_open() {
            return Err(ProviderError::AllExhausted {
                attempts: 0,
                reason: "circuit breaker is open — too many consecutive failures".to_string(),
            });
        }

        // Try primary with retries
        match self.try_provider_with_retry(&*self.primary, &request).await {
            Ok(response) => {
                // Success: reset failure counter
                self.consecutive_failures.store(0, std::sync::atomic::Ordering::Relaxed);
                return Ok(response);
            }
            Err(primary_err) => {
                // Primary failed: increment failure counter
                self.consecutive_failures
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                // Try fallback if available
                if let Some(ref fallback) = self.fallback {
                    tracing::warn!(
                        "provider router: primary failed ({}), trying fallback",
                        primary_err
                    );
                    match self.try_provider_with_retry(&**fallback, &request).await {
                        Ok(response) => {
                            self.consecutive_failures.store(0, std::sync::atomic::Ordering::Relaxed);
                            return Ok(response);
                        }
                        Err(fallback_err) => {
                            return Err(ProviderError::AllExhausted {
                                attempts: self.retry_config.max_attempts * 2,
                                reason: format!("primary: {}, fallback: {}", primary_err, fallback_err),
                            });
                        }
                    }
                }

                return Err(ProviderError::AllExhausted {
                    attempts: self.retry_config.max_attempts,
                    reason: primary_err.to_string(),
                });
            }
        }
    }

    /// Try a single provider with retry + exponential backoff.
    async fn try_provider_with_retry(
        &self,
        provider: &dyn Provider,
        request: &CompletionRequest,
    ) -> Result<CompletionResponse, ProviderError> {
        let mut last_error = None;

        for attempt in 1..=self.retry_config.max_attempts {
            match provider.complete(request.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    // Don't retry configuration errors
                    if matches!(&e, ProviderError::Config(_)) {
                        return Err(e);
                    }
                    last_error = Some(e);
                    if attempt < self.retry_config.max_attempts {
                        let delay = self.retry_config.base_backoff_ms * (1u64 << (attempt - 1));
                        tracing::debug!(
                            "provider attempt {}/{} failed — retrying in {}ms",
                            attempt,
                            self.retry_config.max_attempts,
                            delay
                        );
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ProviderError::AllExhausted {
                attempts: self.retry_config.max_attempts,
                reason: "unknown error".to_string(),
            }
        }))
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use super::*;
    use crate::providers::{ChatMessage, Choice, Usage};

    // ── Mock provider for testing ─────────────────────────────────────────

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

    // ── RetryConfig ───────────────────────────────────────────────────────

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.base_backoff_ms, 500);
    }

    #[test]
    fn test_retry_config_custom() {
        let config = RetryConfig {
            max_attempts: 5,
            base_backoff_ms: 1000,
        };
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.base_backoff_ms, 1000);
    }

    // ── ProviderRouter ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_router_primary_success() {
        let primary = Box::new(MockProvider::new(false));
        let router = ProviderRouter::new(primary);
        let response = router
            .complete(CompletionRequest::new("test", vec![]))
            .await
            .unwrap();
        assert_eq!(response.first_choice_content(), Some("Hello!"));
    }

    #[tokio::test]
    async fn test_router_fallback_used_on_primary_failure() {
        let primary = Box::new(MockProvider::new(true));
        let fallback = Box::new(MockProvider::new(false));
        let router = ProviderRouter::new(primary).with_fallback(fallback);
        let response = router
            .complete(CompletionRequest::new("test", vec![]))
            .await
            .unwrap();
        assert_eq!(response.first_choice_content(), Some("Hello!"));
    }

    #[tokio::test]
    async fn test_router_both_fail() {
        let primary = Box::new(MockProvider::new(true));
        let fallback = Box::new(MockProvider::new(true));
        let router = ProviderRouter::new(primary)
            .with_fallback(fallback)
            .with_retry_config(RetryConfig {
                max_attempts: 1,
                base_backoff_ms: 10,
            });
        let err = router
            .complete(CompletionRequest::new("test", vec![]))
            .await
            .unwrap_err();
        match err {
            ProviderError::AllExhausted { .. } => {} // expected
            _ => panic!("Expected AllExhausted"),
        }
    }

    #[tokio::test]
    async fn test_router_circuit_breaker() {
        let failing = Box::new(MockProvider::new(true));
        let router = ProviderRouter::new(failing)
            .with_retry_config(RetryConfig {
                max_attempts: 1,
                base_backoff_ms: 10,
            })
            .with_circuit_breaker_threshold(2);

        // First call fails
        let _ = router.complete(CompletionRequest::new("test", vec![])).await;

        // Second call fails → circuit opens
        let _ = router.complete(CompletionRequest::new("test", vec![])).await;

        // Third call → circuit is open
        let err = router
            .complete(CompletionRequest::new("test", vec![]))
            .await
            .unwrap_err();
        match err {
            ProviderError::AllExhausted { reason, .. } => {
                assert!(reason.contains("circuit breaker"), "Expected circuit breaker message, got: {}", reason);
            }
            _ => panic!("Expected circuit breaker error"),
        }
    }

    #[tokio::test]
    async fn test_router_resets_failure_count_on_success() {
        let failing = Box::new(MockProvider::new(true));
        let router = ProviderRouter::new(failing)
            .with_retry_config(RetryConfig {
                max_attempts: 1,
                base_backoff_ms: 10,
            });

        // First call fails
        let _ = router.complete(CompletionRequest::new("test", vec![])).await;

        // Create a new router with a successful provider
        let success = Box::new(MockProvider::new(false));
        let router = ProviderRouter::new(success);
        // This should succeed immediately
        let response = router
            .complete(CompletionRequest::new("test", vec![]))
            .await
            .unwrap();
        assert_eq!(response.first_choice_content(), Some("Hello!"));
    }

    #[tokio::test]
    async fn test_router_no_fallback_no_retry_exhausted() {
        let failing = Box::new(MockProvider::new(true));
        let router = ProviderRouter::new(failing)
            .with_retry_config(RetryConfig {
                max_attempts: 1,
                base_backoff_ms: 10,
            });
        let err = router
            .complete(CompletionRequest::new("test", vec![]))
            .await
            .unwrap_err();
        match err {
            ProviderError::AllExhausted { attempts, .. } => {
                assert_eq!(attempts, 1, "Expected 1 attempt");
            }
            _ => panic!("Expected AllExhausted"),
        }
    }
}

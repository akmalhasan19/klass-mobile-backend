//! Integration tests for the OpenRouter provider client.
//!
//! Uses `mockito` to mock the `/chat/completions` endpoint and verify:
//! - Request headers (Authorization, HTTP-Referer, X-Title)
//! - Response parsing and content extraction
//! - Retry logic (transient 5xx, no retry on 4xx)
//! - JSON mode request serialization
//! - Fallback content extraction strategies

use klass_gateway::providers::{
    extract_content, json_mode_request, ChatMessage, CompletionRequest,
    OpenRouterConfig, OpenRouterProviderClient, Provider,
};
use mockito::Server;

// ─── Helper ─────────────────────────────────────────────────────────────────

fn make_client(server_url: &str) -> OpenRouterProviderClient {
    let config = OpenRouterConfig {
        api_key: "sk-or-test-key".to_string(),
        model: "minimax/minimax-m3".to_string(),
        base_url: server_url.trim_end_matches('/').to_string(),
        timeout_seconds: 10,
        retry_attempts: 3, // Set to 3 to support retry tests
        retry_backoff_ms: 10,
        fallback_models: Vec::new(),
    };
    OpenRouterProviderClient::new(reqwest::Client::new(), config)
}

fn make_client_with_timeout(server_url: &str, timeout_seconds: u64) -> OpenRouterProviderClient {
    let config = OpenRouterConfig {
        api_key: "sk-or-test-key".to_string(),
        model: "minimax/minimax-m3".to_string(),
        base_url: server_url.trim_end_matches('/').to_string(),
        timeout_seconds,
        retry_attempts: 3,
        retry_backoff_ms: 10,
        fallback_models: Vec::new(),
    };
    OpenRouterProviderClient::new(reqwest::Client::new(), config)
}

fn sample_request() -> CompletionRequest {
    CompletionRequest::new(
        "minimax/minimax-m3",
        vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You are a helpful assistant.".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "Say hello".to_string(),
            },
        ],
    )
}

// ─── Happy path ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_completion_success() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "choices": [{
                    "message": {"role": "assistant", "content": "Hello there!"},
                    "finish_reason": "stop",
                    "index": 0
                }],
                "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15},
                "model": "minimax/minimax-m3"
            }"#,
        )
        .create();

    let client = make_client(&server.url());
    let result = client.complete(sample_request()).await;
    assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());

    let resp = result.unwrap();
    assert_eq!(resp.first_choice_content(), Some("Hello there!"));
    assert_eq!(resp.model.as_deref(), Some("minimax/minimax-m3"));
    assert_eq!(resp.usage.as_ref().and_then(|u| u.prompt_tokens), Some(10));
    assert_eq!(resp.usage.as_ref().and_then(|u| u.completion_tokens), Some(5));

    mock.assert();
}

#[tokio::test]
async fn test_completion_includes_required_headers() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"choices": [{"message": {"role": "assistant", "content": "OK"}, "index": 0}]}"#)
        .match_header("authorization", "Bearer sk-or-test-key")
        .match_header("http-referer", "klass-mobile")
        .match_header("x-title", "klass-gateway")
        .match_header("content-type", "application/json")
        .create();

    let client = make_client(&server.url());
    let result = client.complete(sample_request()).await;
    assert!(result.is_ok());

    mock.assert();
}

#[tokio::test]
async fn test_completion_applies_default_model_when_empty() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"choices": [{"message": {"role": "assistant", "content": "OK"}, "index": 0}], "model": "minimax/minimax-m3"}"#)
        .match_body(mockito::Matcher::Regex(r#".*"model":"minimax/minimax-m3".*"#.to_string()))
        .create();

    let client = make_client(&server.url());
    let mut req = sample_request();
    req.model = "".to_string(); // empty model → should use default from config

    let result = client.complete(req).await;
    assert!(result.is_ok(), "error: {:?}", result.err());

    mock.assert();
}

// ─── Error handling ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_completion_retries_on_5xx() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(503)
        .with_body("Service Unavailable")
        .expect_at_least(2)
        .create();

    let client = make_client(&server.url());
    let result = client.complete(sample_request()).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        klass_gateway::providers::ProviderError::Api { status, .. } => {
            assert_eq!(status, 503);
        }
        err => panic!("expected Api error, got: {}", err),
    }

    mock.assert();
}

#[tokio::test]
async fn test_completion_does_not_retry_4xx() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(400)
        .with_body(r#"{"error": "Bad request"}"#)
        .expect_at_most(1)
        .create();

    let client = make_client(&server.url());
    let result = client.complete(sample_request()).await;
    assert!(result.is_err());

    mock.assert();
}

#[tokio::test]
async fn test_completion_retries_on_429() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(429)
        .with_body(r#"{"error": "Rate limited"}"#)
        .expect_at_least(2)
        .create();

    let client = make_client(&server.url());
    let result = client.complete(sample_request()).await;
    assert!(result.is_err());

    mock.assert();
}

#[tokio::test]
async fn test_completion_timeout_returns_error() {
    // Use a server that never responds
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_chunked_body(|w| {
            std::thread::sleep(std::time::Duration::from_secs(5));
            let _ = w.write_all(b"OK");
            Ok(())
        })
        .create();

    let client = make_client_with_timeout(&server.url(), 2); // 2s timeout

    let result = client.complete(sample_request()).await;
    assert!(result.is_err());
}

// ─── Response parsing ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_completion_parses_usage() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "choices": [{"message": {"role": "assistant", "content": "Hi"}, "index": 0}],
                "usage": {"prompt_tokens": 50, "completion_tokens": 100, "total_tokens": 150}
            }"#,
        )
        .create();

    let client = make_client(&server.url());
    let result = client.complete(sample_request()).await.unwrap();
    let usage = result.usage.unwrap();
    assert_eq!(usage.prompt_tokens, Some(50));
    assert_eq!(usage.completion_tokens, Some(100));
    assert_eq!(usage.total_tokens, Some(150));

    mock.assert();
}

#[tokio::test]
async fn test_completion_handles_no_usage() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"choices": [{"message": {"role": "assistant", "content": "Hi"}, "index": 0}]}"#)
        .create();

    let client = make_client(&server.url());
    let result = client.complete(sample_request()).await.unwrap();
    assert!(result.usage.is_none());

    mock.assert();
}

#[tokio::test]
async fn test_completion_empty_choices() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"choices": []}"#)
        .create();

    let client = make_client(&server.url());
    let result = client.complete(sample_request()).await.unwrap();
    assert!(result.first_choice_content().is_none());

    mock.assert();
}

// ─── Content extraction ─────────────────────────────────────────────────────

#[test]
fn test_extract_content_standard() {
    let raw: serde_json::Value = serde_json::from_str(
        r#"{"choices": [{"message": {"role": "assistant", "content": "Hello"}, "index": 0}]}"#,
    )
    .unwrap();
    assert_eq!(extract_content(&raw), Some("Hello".to_string()));
}

#[test]
fn test_extract_content_fallback_output_text() {
    let raw: serde_json::Value =
        serde_json::from_str(r#"{"output_text": "Fallback"}"#).unwrap();
    assert_eq!(extract_content(&raw), Some("Fallback".to_string()));
}

#[test]
fn test_extract_content_content_array() {
    let raw: serde_json::Value =
        serde_json::from_str(r#"{"content": ["Part A", "Part B"]}"#).unwrap();
    let result = extract_content(&raw).unwrap();
    assert!(result.contains("Part A"));
    assert!(result.contains("Part B"));
}

#[test]
fn test_extract_content_choices_text() {
    let raw: serde_json::Value = serde_json::from_str(
        r#"{"choices": [{"text": "Completion text", "index": 0}]}"#,
    )
    .unwrap();
    assert_eq!(
        extract_content(&raw),
        Some("Completion text".to_string())
    );
}

#[test]
fn test_extract_content_empty_returns_none() {
    let raw: serde_json::Value = serde_json::from_str(r#"{}"#).unwrap();
    assert!(extract_content(&raw).is_none());
}

#[test]
fn test_extract_content_empty_string_in_standard_falls_back() {
    let raw: serde_json::Value = serde_json::from_str(
        r#"{"choices": [{"message": {"role": "assistant", "content": ""}, "index": 0}], "output_text": "backup"}"#,
    )
    .unwrap();
    assert_eq!(extract_content(&raw), Some("backup".to_string()));
}

// ─── JSON mode request ──────────────────────────────────────────────────────

#[test]
fn test_json_mode_request_structure() {
    let req = json_mode_request(
        "minimax/minimax-m3",
        "System prompt",
        "User prompt",
    );
    assert_eq!(req.messages.len(), 2);
    assert_eq!(req.messages[0].role, "system");
    assert_eq!(req.messages[1].role, "user");
    assert!(req.response_format.is_some());
    assert_eq!(
        req.response_format.as_ref().unwrap().format_type,
        "json_object"
    );
}

#[test]
fn test_json_mode_request_serialization() {
    let req = json_mode_request("m", "s", "u");
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["response_format"]["type"], "json_object");
    assert_eq!(json["messages"][0]["content"], "s");
    assert_eq!(json["messages"][1]["content"], "u");
}
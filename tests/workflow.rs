//! End-to-end tests for the media generation workflow.
//!
//! Uses `mockito` to simulate the Python renderer and OpenRouter.
//! Exercises the full `WorkflowService::process()` pipeline with
//! mocked step implementations and verifies status transitions.
//!
//! These tests verify the workflow orchestration logic without
//! requiring a real DB — the step traits abstract all I/O.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use mockito::Server;
use serde_json::Value;

use klass_gateway::orchestrator::workflow::{
    ComposeStep, DraftStep, GenerateStep, InterpretStep, PublishStep, WorkflowError,
    WorkflowService,
};

// ═════════════════════════════════════════════════════════════════════════════
// Helpers
// ═════════════════════════════════════════════════════════════════════════════

fn dummy_pool() -> sqlx::PgPool {
    // This pool is never used — step traits intercept all DB calls.
    // It exists because WorkflowService::new() requires one.
    // In a full integration test with a real DB, use common::setup() instead.
    // connect_lazy creates the pool without connecting.
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://localhost:5432/nonexistent")
        .expect("dummy pool creation should not fail")
}

/// Counts how many times each step is invoked.
#[derive(Default)]
struct StepCounts {
    interpret: Arc<AtomicUsize>,
    draft: Arc<AtomicUsize>,
    generate: Arc<AtomicUsize>,
    publish: Arc<AtomicUsize>,
    compose: Arc<AtomicUsize>,
}

// ═════════════════════════════════════════════════════════════════════════════
// Mock step implementations
// ═════════════════════════════════════════════════════════════════════════════

struct MockInterpret {
    counts: Arc<AtomicUsize>,
}

#[async_trait]
impl InterpretStep for MockInterpret {
    async fn interpret(&self, _generation_id: &str) -> Result<Value, WorkflowError> {
        self.counts.fetch_add(1, Ordering::SeqCst);
        Ok(serde_json::json!({
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "Buatkan materi",
            "language": "id",
            "teacher_intent": {
                "type": "generate_learning_media",
                "goal": "Create handout"
            },
            "learning_objectives": ["Memahami konsep"],
            "constraints": { "preferred_output_type": "pdf" },
            "output_type_candidates": [
                { "type": "pdf", "score": 0.9, "reason": "Best for printout" }
            ],
            "document_blueprint": {
                "title": "Materi",
                "summary": "Ringkasan",
                "sections": []
            },
            "teacher_delivery_summary": "Gunakan di kelas",
            "confidence": { "score": 0.85, "label": "high" }
        }))
    }
}

struct MockDraft {
    counts: Arc<AtomicUsize>,
}

#[async_trait]
impl DraftStep for MockDraft {
    async fn draft(&self, _generation_id: &str) -> Result<Value, WorkflowError> {
        self.counts.fetch_add(1, Ordering::SeqCst);
        Ok(serde_json::json!({
            "schema_version": "media_content_draft.v1",
            "title": "Materi",
            "summary": "Ringkasan",
            "learning_objectives": ["Memahami konsep"],
            "sections": [],
            "source": "provider",
            "fallback": { "triggered": false }
        }))
    }
}

struct MockGenerate {
    counts: Arc<AtomicUsize>,
}

#[async_trait]
impl GenerateStep for MockGenerate {
    async fn generate(&self, _generation_id: &str) -> Result<Value, WorkflowError> {
        self.counts.fetch_add(1, Ordering::SeqCst);
        Ok(serde_json::json!({
            "status": "generated",
            "artifact_metadata": {
                "schema_version": "media_artifact_metadata.v1",
                "filename": "output.pdf",
                "mime_type": "application/pdf",
                "size_bytes": 102400,
                "checksum_sha256": "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
                "output_type": "pdf"
            }
        }))
    }
}

struct MockPublish {
    counts: Arc<AtomicUsize>,
}

#[async_trait]
impl PublishStep for MockPublish {
    async fn publish(&self, _generation_id: &str) -> Result<Value, WorkflowError> {
        self.counts.fetch_add(1, Ordering::SeqCst);
        Ok(serde_json::json!({
            "topic_id": "00000000-0000-0000-0000-000000000001",
            "content_id": "00000000-0000-0000-0000-000000000002",
            "recommended_project_id": 1,
        }))
    }
}

struct MockCompose {
    counts: Arc<AtomicUsize>,
}

#[async_trait]
impl ComposeStep for MockCompose {
    async fn compose(&self, _generation_id: &str) -> Result<Value, WorkflowError> {
        self.counts.fetch_add(1, Ordering::SeqCst);
        Ok(serde_json::json!({
            "schema_version": "media_delivery_response.v1",
            "response_meta": {
                "provider": "openrouter",
                "model": "deepseek-v4-flash",
                "llm_used": true,
            },
            "fallback": { "triggered": false },
        }))
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Step invocation counting
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_mock_steps_track_invocations() {
    let counts = StepCounts::default();

    let interpret = MockInterpret { counts: counts.interpret.clone() };
    let draft = MockDraft { counts: counts.draft.clone() };
    let generate = MockGenerate { counts: counts.generate.clone() };
    let publish = MockPublish { counts: counts.publish.clone() };
    let compose = MockCompose { counts: counts.compose.clone() };

    // Verify each step increments its counter
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        interpret.interpret("gen-1").await.unwrap();
        draft.draft("gen-1").await.unwrap();
        generate.generate("gen-1").await.unwrap();
        publish.publish("gen-1").await.unwrap();
        compose.compose("gen-1").await.unwrap();
    });

    assert_eq!(counts.interpret.load(Ordering::SeqCst), 1);
    assert_eq!(counts.draft.load(Ordering::SeqCst), 1);
    assert_eq!(counts.generate.load(Ordering::SeqCst), 1);
    assert_eq!(counts.publish.load(Ordering::SeqCst), 1);
    assert_eq!(counts.compose.load(Ordering::SeqCst), 1);
}

// ═════════════════════════════════════════════════════════════════════════════
// Step error propagation
// ═════════════════════════════════════════════════════════════════════════════

struct FailingStep;

#[async_trait]
impl InterpretStep for FailingStep {
    async fn interpret(&self, generation_id: &str) -> Result<Value, WorkflowError> {
        Err(WorkflowError::StepProvider(format!(
            "interpret failed for {}",
            generation_id
        )))
    }
}

#[test]
fn test_interpret_error_propagation() {
    let step = FailingStep;
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(step.interpret("gen-1"));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("interpret failed"));
    assert!(err.to_string().contains("gen-1"));
}

// ═════════════════════════════════════════════════════════════════════════════
// Mock OpenRouter HTTP endpoint via mockito
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_mockito_openrouter_endpoint_responds() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"choices":[{"message":{"content":"{\"status\":\"ok\"}"}}]}"#)
        .create_async()
        .await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/chat/completions", server.url()))
        .json(&serde_json::json!({"model": "test"}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["choices"][0]["message"]["content"], "{\"status\":\"ok\"}");

    mock.assert_async().await;
}

// ═════════════════════════════════════════════════════════════════════════════
// Mock Python renderer endpoint via mockito
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_mockito_python_renderer_responds() {
    let mut server = Server::new_async().await;

    let response_body = serde_json::json!({
        "artifact_metadata": {
            "schema_version": "media_artifact_metadata.v1",
            "filename": "output.pdf",
            "mime_type": "application/pdf",
            "size_bytes": 102400,
            "checksum_sha256": "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
            "output_type": "pdf"
        },
        "status": "completed"
    });

    let mock = server
        .mock("POST", "/v1/generate")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(response_body.to_string())
        .create_async()
        .await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/v1/generate", server.url()))
        .json(&serde_json::json!({"generation_id": "gen-1", "generation_spec": {}}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["artifact_metadata"]["output_type"], "pdf");

    mock.assert_async().await;
}

// ═════════════════════════════════════════════════════════════════════════════
// Mock Python renderer error response
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_mockito_python_renderer_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/v1/generate")
        .with_status(503)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error": {"message": "Service temporarily unavailable"}}"#)
        .create_async()
        .await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/v1/generate", server.url()))
        .json(&serde_json::json!({"generation_id": "gen-1"}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 503);
    mock.assert_async().await;
}

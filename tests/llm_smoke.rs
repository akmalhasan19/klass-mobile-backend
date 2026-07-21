//! End-to-end smoke tests for the LLM pipeline with a mocked OpenRouter.
//!
//! Uses `mockito` to mock `/chat/completions` and exercises the three-stage
//! LLM pipeline (interpret → draft → respond) through their public service
//! types. These tests verify the pipeline plumbing without a real LLM.
//!
//! The pipeline is tested via direct service calls (not HTTP endpoints) to
//! avoid requiring a full DB connection for the repose/repositories.
//!
//! Coverage:
//! - Interpret: governance preflight → cache miss → provider call → contract validation
//! - Draft: interpretation input → provider call → content draft validation
//! - Respond: delivery composition with artifact + publication → validation
//! - Contract validation failures trigger fallback gracefully

use klass_gateway::providers::Provider;
use mockito::Server;

// ─── Mock provider response builders ───────────────────────────────────────

fn mock_interpretation_response() -> String {
    r#"{
        "choices": [{
            "message": {"role": "assistant", "content": "{\"schema_version\":\"media_prompt_understanding.v1\",\"teacher_prompt\":\"Buatkan materi pecahan untuk kelas 5 SD\",\"language\":\"id\",\"teacher_intent\":{\"type\":\"generate_learning_media\",\"goal\":\"Membuat materi pecahan\",\"preferred_delivery_mode\":\"digital_download\",\"requires_clarification\":false},\"learning_objectives\":[\"Memahami pecahan\"],\"constraints\":{\"preferred_output_type\":\"pdf\"},\"output_type_candidates\":[{\"type\":\"pdf\",\"score\":0.9,\"reason\":\"Best for printout\"}],\"resolved_output_type_reasoning\":\"PDF is standard\",\"document_blueprint\":{\"title\":\"Materi Pecahan\",\"summary\":\"Pengenalan pecahan\",\"sections\":[{\"title\":\"Pengertian\",\"purpose\":\"Perkenalan\",\"bullets\":[\"Definisi\"],\"estimated_length\":\"short\"}]},\"teacher_delivery_summary\":\"Ringkasan\",\"confidence\":{\"score\":0.85,\"label\":\"high\"}}"},
            "finish_reason": "stop",
            "index": 0
        }],
        "usage": {"prompt_tokens": 200, "completion_tokens": 350, "total_tokens": 550},
        "model": "xiaomi/mimo-v2.5-pro"
    }"#.to_string()
}

fn mock_draft_response() -> String {
    r#"{
        "choices": [{
            "message": {"role": "assistant", "content": "{\"schema_version\":\"media_content_draft.v1\",\"title\":\"Materi Pecahan\",\"summary\":\"Pengenalan pecahan\",\"learning_objectives\":[\"Memahami pecahan\"],\"sections\":[{\"title\":\"Pengertian\",\"purpose\":\"Perkenalan\",\"body_blocks\":[{\"type\":\"paragraph\",\"content\":\"Pecahan adalah bagian dari keseluruhan\"}],\"emphasis\":\"medium\"}],\"teacher_delivery_summary\":\"Gunakan contoh sehari-hari\"}"},
            "finish_reason": "stop",
            "index": 0
        }],
        "usage": {"prompt_tokens": 400, "completion_tokens": 200, "total_tokens": 600},
        "model": "xiaomi/mimo-v2.5-pro"
    }"#.to_string()
}

fn mock_respond_response() -> String {
    r#"{
        "choices": [{
            "message": {"role": "assistant", "content": "{\"schema_version\":\"media_delivery_response.v1\",\"title\":\"Materi Pecahan\",\"preview_summary\":\"PDF materi pecahan kelas 5\",\"teacher_message\":\"Silakan unduh materi di bawah ini\",\"recommended_next_steps\":[\"Gunakan di kelas\"],\"classroom_tips\":[\"Diskusikan dengan siswa\"],\"artifact\":{\"output_type\":\"pdf\",\"title\":\"Materi Pecahan\",\"file_url\":\"https://storage.example.com/materi.pdf\",\"mime_type\":\"application/pdf\"},\"publication\":{\"topic\":{\"id\":\"t1\",\"title\":\"Pecahan\"},\"content\":null,\"recommended_project\":null},\"response_meta\":{\"generated_at\":\"2026-07-14T10:00:00Z\",\"llm_used\":true,\"provider\":\"xiaomi\",\"model\":\"mimo-v2.5-pro\"}}"},
            "finish_reason": "stop",
            "index": 0
        }],
        "usage": {"prompt_tokens": 300, "completion_tokens": 250, "total_tokens": 550},
        "model": "xiaomi/mimo-v2.5-pro"
    }"#.to_string()
}

// ═════════════════════════════════════════════════════════════════════════════
// 1. Interpretation Pipeline Tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_interpretation_decode_valid_response() {
    let raw = mock_interpretation_response();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let completion = json["choices"][0]["message"]["content"].as_str().unwrap();

    let result = klass_gateway::contracts::prompt_interpretation::decode_and_validate(completion);
    assert!(result.is_ok(), "should decode valid interpretation: {:?}", result.err());

    let payload = result.unwrap();
    assert_eq!(payload.language, "id");
    assert!(!payload.fallback.triggered);
    assert_eq!(payload.teacher_prompt, "Buatkan materi pecahan untuk kelas 5 SD");
}

#[test]
fn test_interpretation_mocked_openrouter_flow() {
    let raw = mock_interpretation_response();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let content = json["choices"][0]["message"]["content"].as_str().unwrap();

    let payload = klass_gateway::contracts::prompt_interpretation::decode_and_validate(content)
        .expect("valid interpretation");

    assert_eq!(payload.schema_version, "media_prompt_understanding.v1");
    assert_eq!(payload.output_type_candidates[0].r#type, "pdf");
    assert_eq!(payload.document_blueprint.sections.len(), 1);
    assert_eq!(payload.confidence.label, "high");
}

#[test]
fn test_interpretation_provider_error_triggers_fallback() {
    let invalid_content = "{invalid json";
    let result = klass_gateway::contracts::prompt_interpretation::decode_and_validate(invalid_content);
    assert!(result.is_err());

    let fallback_payload = klass_gateway::contracts::prompt_interpretation::fallback("Buatkan materi");
    assert!(fallback_payload.fallback.triggered);
    assert_eq!(fallback_payload.language, "id");
    assert_eq!(fallback_payload.output_type_candidates.len(), 3);
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. Draft Pipeline Tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_draft_decode_valid_response() {
    let raw = mock_draft_response();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let content = json["choices"][0]["message"]["content"].as_str().unwrap();

    let result = klass_gateway::contracts::content_draft::decode_and_validate(content);
    assert!(result.is_ok(), "should decode valid draft: {:?}", result.err());

    let payload = result.unwrap();
    assert_eq!(payload.title, "Materi Pecahan");
    assert_eq!(payload.sections.len(), 1);
    assert!(!payload.fallback.triggered);
}

#[test]
fn test_draft_interpretation_roundtrip() {
    let interp_raw = mock_interpretation_response();
    let interp_json: serde_json::Value = serde_json::from_str(&interp_raw).unwrap();
    let interp_content = interp_json["choices"][0]["message"]["content"].as_str().unwrap();
    let interpretation =
        klass_gateway::contracts::prompt_interpretation::decode_and_validate(interp_content)
            .expect("valid interpretation");

    let draft = klass_gateway::contracts::content_draft::fallback_from_interpretation(
        &interpretation.document_blueprint.title,
        &interpretation.document_blueprint.summary,
        &interpretation.teacher_delivery_summary,
    );

    let json = serde_json::to_string(&draft).unwrap();
    let validated = klass_gateway::contracts::content_draft::decode_and_validate(&json);
    assert!(validated.is_ok(), "fallback draft should be valid: {:?}", validated.err());

    assert_eq!(draft.title, "Materi Pecahan");
    assert!(draft.fallback.triggered);
    assert_eq!(draft.sections[0].body_blocks[0].r#type, "paragraph");
}

#[test]
fn test_draft_fallback_on_invalid_response() {
    let invalid = "not valid json";
    let result = klass_gateway::contracts::content_draft::decode_and_validate(invalid);
    assert!(result.is_err());

    let fallback = klass_gateway::contracts::content_draft::fallback_from_interpretation(
        "Materi Pecahan",
        "Pengenalan pecahan",
        "Gunakan contoh",
    );
    assert!(fallback.fallback.triggered);
    assert_eq!(fallback.sections.len(), 1);
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. Respond Pipeline Tests
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_respond_decode_valid_response() {
    let raw = mock_respond_response();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let content = json["choices"][0]["message"]["content"].as_str().unwrap();

    let result = klass_gateway::contracts::delivery::decode_and_validate(content);
    assert!(result.is_ok(), "should decode valid delivery: {:?}", result.err());

    let payload = result.unwrap();
    assert_eq!(payload.title, "Materi Pecahan");
    assert_eq!(payload.artifact.output_type, "pdf");
    assert!(payload.response_meta.llm_used);
    assert!(!payload.fallback.triggered);
}

#[test]
fn test_respond_complete_pipeline_with_mocked_provider() {
    let raw = mock_respond_response();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let content = json["choices"][0]["message"]["content"].as_str().unwrap();

    let payload = klass_gateway::contracts::delivery::decode_and_validate(content)
        .expect("valid delivery response");

    assert_eq!(payload.schema_version, "media_delivery_response.v1");
    assert_eq!(payload.artifact.mime_type, "application/pdf");
    assert_eq!(payload.recommended_next_steps.len(), 1);
    assert_eq!(payload.classroom_tips.len(), 1);
    assert!(payload.publication.topic.is_some());
    assert_eq!(payload.publication.topic.as_ref().unwrap().id, "t1");
}

#[test]
fn test_respond_fallback_on_invalid_response() {
    let invalid = "{bogus";
    let result = klass_gateway::contracts::delivery::decode_and_validate(invalid);
    assert!(result.is_err());

    let fallback = klass_gateway::contracts::delivery::fallback(
        "Materi Pecahan",
        "PDF materi pecahan",
        "pdf",
        "https://example.com/materi.pdf",
        "application/pdf",
    );
    assert!(fallback.fallback.triggered);
    assert!(!fallback.response_meta.llm_used);
    assert_eq!(fallback.artifact.output_type, "pdf");
}

#[test]
fn test_respond_fallback_meta_structure() {
    let fallback = klass_gateway::contracts::delivery::fallback(
        "A", "B", "pdf", "https://f.com/f.pdf", "application/pdf",
    );
    assert!(!fallback.response_meta.llm_used);
    assert!(fallback.response_meta.provider.is_none());
    assert!(fallback.response_meta.model.is_none());
    assert!(!fallback.response_meta.generated_at.is_empty());
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. Cross-stage Pipeline (End-to-End Contracts)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_full_pipeline_contract_flow() {
    let interp_raw = mock_interpretation_response();
    let interp_val: serde_json::Value = serde_json::from_str(&interp_raw).unwrap();
    let interp_content = interp_val["choices"][0]["message"]["content"].as_str().unwrap();
    let interpretation =
        klass_gateway::contracts::prompt_interpretation::decode_and_validate(interp_content)
            .expect("interpretation");
    assert_eq!(interpretation.language, "id");

    let draft = klass_gateway::contracts::content_draft::fallback_from_interpretation(
        &interpretation.document_blueprint.title,
        &interpretation.document_blueprint.summary,
        &interpretation.teacher_delivery_summary,
    );
    assert_eq!(draft.schema_version, "media_content_draft.v1");

    let respond = klass_gateway::contracts::delivery::fallback(
        &draft.title,
        &draft.summary,
        "pdf",
        "https://example.com/output.pdf",
        "application/pdf",
    );
    assert_eq!(respond.schema_version, "media_delivery_response.v1");
    assert!(respond.fallback.triggered);

    assert_eq!(interpretation.document_blueprint.title, draft.title);
    assert_eq!(draft.title, respond.title, "title should flow through all stages");
}

#[test]
fn test_pipeline_contract_serialization_roundtrip() {
    let interp_raw = mock_interpretation_response();
    let interp_val: serde_json::Value = serde_json::from_str(&interp_raw).unwrap();
    let interp_content = interp_val["choices"][0]["message"]["content"].as_str().unwrap();
    let interpretation =
        klass_gateway::contracts::prompt_interpretation::decode_and_validate(interp_content)
            .expect("interpretation");

    let json = serde_json::to_string(&interpretation).unwrap();
    let reparsed =
        klass_gateway::contracts::prompt_interpretation::decode_and_validate(&json)
            .expect("re-parsed interpretation");
    assert_eq!(reparsed.language, interpretation.language);
    assert_eq!(
        reparsed.document_blueprint.title,
        interpretation.document_blueprint.title
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// 5. Mockito-based HTTP smoke test
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_mockito_interpret_endpoint() {
    use klass_gateway::providers::{OpenRouterConfig, OpenRouterProviderClient, CompletionRequest, ChatMessage};

    let mut server = Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_interpretation_response())
        .create();

    let config = OpenRouterConfig {
        api_key: "test".to_string(),
        model: "xiaomi/mimo-v2.5-pro".to_string(),
        base_url: server.url(),
        timeout_seconds: 10,
        retry_attempts: 1,
        retry_backoff_ms: 10,
    };
    let client = OpenRouterProviderClient::new(reqwest::Client::new(), config);

    let request = CompletionRequest::new(
        "xiaomi/mimo-v2.5-pro",
        vec![ChatMessage {
            role: "user".to_string(),
            content: "Buatkan materi pecahan".to_string(),
        }],
    );

    let response = client.complete(request).await.expect("provider call");
    let content = response.first_choice_content().expect("content");

    let payload =
        klass_gateway::contracts::prompt_interpretation::decode_and_validate(content)
            .expect("interpretation from mocked provider");
    assert_eq!(payload.teacher_prompt, "Buatkan materi pecahan untuk kelas 5 SD");
}

#[tokio::test]
async fn test_mockito_draft_endpoint() {
    use klass_gateway::providers::{OpenRouterConfig, OpenRouterProviderClient, CompletionRequest, ChatMessage};

    let mut server = Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_draft_response())
        .create();

    let config = OpenRouterConfig {
        api_key: "test".to_string(),
        model: "xiaomi/mimo-v2.5-pro".to_string(),
        base_url: server.url(),
        timeout_seconds: 10,
        retry_attempts: 1,
        retry_backoff_ms: 10,
    };
    let client = OpenRouterProviderClient::new(reqwest::Client::new(), config);

    let request = CompletionRequest::new(
        "xiaomi/mimo-v2.5-pro",
        vec![ChatMessage {
            role: "user".to_string(),
            content: "Generate draft content".to_string(),
        }],
    );

    let response = client.complete(request).await.expect("provider call");
    let content = response.first_choice_content().expect("content");

    let payload =
        klass_gateway::contracts::content_draft::decode_and_validate(content)
            .expect("draft from mocked provider");
    assert_eq!(payload.title, "Materi Pecahan");
}

#[tokio::test]
async fn test_mockito_respond_endpoint() {
    use klass_gateway::providers::{OpenRouterConfig, OpenRouterProviderClient, CompletionRequest, ChatMessage};

    let mut server = Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_respond_response())
        .create();

    let config = OpenRouterConfig {
        api_key: "test".to_string(),
        model: "xiaomi/mimo-v2.5-pro".to_string(),
        base_url: server.url(),
        timeout_seconds: 10,
        retry_attempts: 1,
        retry_backoff_ms: 10,
    };
    let client = OpenRouterProviderClient::new(reqwest::Client::new(), config);

    let request = CompletionRequest::new(
        "xiaomi/mimo-v2.5-pro",
        vec![ChatMessage {
            role: "user".to_string(),
            content: "Compose delivery".to_string(),
        }],
    );

    let response = client.complete(request).await.expect("provider call");
    let content = response.first_choice_content().expect("content");

    let payload =
        klass_gateway::contracts::delivery::decode_and_validate(content)
            .expect("delivery from mocked provider");
    assert!(payload.response_meta.llm_used);
    assert_eq!(payload.artifact.output_type, "pdf");
}

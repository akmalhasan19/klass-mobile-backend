//! `MediaContentDraftSchema` — schema version `media_content_draft.v1`.
//!
//! Port of Python `ContentDraftPayload` / `ContentDraftContractModel`.

use garde::Validate;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "media_content_draft.v1";

// ─── Sub-types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct BodyBlock {
    #[garde(skip)]
    pub r#type: String,
    #[garde(length(min = 1, max = 1000))]
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ContentSection {
    #[garde(length(min = 1, max = 200))]
    pub title: String,
    #[garde(length(min = 1, max = 500))]
    pub purpose: String,
    #[garde(dive)]
    #[garde(length(min = 1))]
    pub body_blocks: Vec<BodyBlock>,
    #[garde(skip)]
    pub emphasis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DraftFallback {
    #[garde(skip)]
    #[serde(default)]
    pub triggered: bool,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

impl Default for DraftFallback {
    fn default() -> Self {
        Self {
            triggered: false,
            reason_code: None,
            action: None,
        }
    }
}

// ─── Main payload ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ContentDraftPayload {
    #[garde(length(min = 1))]
    pub schema_version: String,
    #[garde(length(min = 1, max = 200))]
    pub title: String,
    #[garde(length(min = 1, max = 1000))]
    pub summary: String,
    #[garde(skip)]
    #[serde(default)]
    pub learning_objectives: Vec<String>,
    #[garde(dive)]
    #[garde(length(min = 1))]
    pub sections: Vec<ContentSection>,
    #[garde(length(min = 1, max = 1000))]
    pub teacher_delivery_summary: String,
    #[garde(skip)]
    #[serde(default)]
    pub fallback: DraftFallback,
}

// ─── Public API ─────────────────────────────────────────────────────────────

use crate::contracts::prompt_interpretation::ContractValidationError;

/// Decode a raw JSON string into a validated `ContentDraftPayload`.
///
/// Applies a repair step before parsing to fix common LLM output issues:
/// - Missing `schema_version`
/// - Missing or null required fields
/// - Empty sections
pub fn decode_and_validate(raw_json: &str) -> Result<ContentDraftPayload, ContractValidationError> {
    let trimmed = raw_json.trim();
    if trimmed.is_empty() {
        return Err(ContractValidationError {
            code: "empty_completion",
            message: "Provider completion was empty.".to_string(),
            details: serde_json::json!({"reason": "empty_completion"}),
            raw_completion: raw_json.to_string(),
        });
    }

    // ── Repair step: fix common LLM output issues ─────────────────────
    let repaired = repair_draft_json(trimmed);

    let payload: ContentDraftPayload = serde_json::from_str(&repaired).map_err(|e| {
        ContractValidationError {
            code: "provider_response_contract_invalid",
            message: format!("Failed to decode content draft completion as JSON: {}", e),
            details: serde_json::json!({"json_error": e.to_string()}),
            raw_completion: raw_json.to_string(),
        }
    })?;

    if let Err(errors) = payload.validate() {
        return Err(ContractValidationError {
            code: "provider_response_contract_invalid",
            message: "Provider completion failed MediaContentDraftSchema validation.".to_string(),
            details: serde_json::json!({"errors": errors.to_string()}),
            raw_completion: raw_json.to_string(),
        });
    }

    Ok(payload)
}

/// Repair common LLM output issues in content draft JSON.
///
/// Fixes:
/// 1. Missing `schema_version` → inject correct version
/// 2. Missing or null `title` / `summary` → inject defaults
/// 3. Empty or missing `sections` → inject a minimal section
/// 4. Empty `teacher_delivery_summary` → use title as fallback
/// 5. Section body_blocks with empty content → fill from purpose
fn repair_draft_json(raw: &str) -> String {
    let parsed: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return raw.to_string(),
    };

    let mut obj = match parsed {
        serde_json::Value::Object(m) => m,
        _ => return raw.to_string(),
    };

    // ── 1. Fix schema_version ──────────────────────────────────────────
    if !obj.contains_key("schema_version") || obj["schema_version"].is_null()
        || obj["schema_version"].as_str().map_or(false, |s| s.is_empty())
    {
        obj.insert(
            "schema_version".to_string(),
            serde_json::json!(SCHEMA_VERSION),
        );
    }

    // ── 2. Fix title ──────────────────────────────────────────────────
    if !obj.contains_key("title") || obj["title"].is_null()
        || obj["title"].as_str().map_or(false, |s| s.is_empty())
    {
        obj.insert(
            "title".to_string(),
            serde_json::json!("Untitled Draft"),
        );
    }

    // ── 3. Fix summary ────────────────────────────────────────────────
    if !obj.contains_key("summary") || obj["summary"].is_null()
        || obj["summary"].as_str().map_or(false, |s| s.is_empty())
    {
        let title = obj.get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("No summary available.");
        obj.insert(
            "summary".to_string(),
            serde_json::json!(truncate_str(title, 1000)),
        );
    }

    // ── 4. Fix teacher_delivery_summary ────────────────────────────────
    if !obj.contains_key("teacher_delivery_summary") || obj["teacher_delivery_summary"].is_null()
        || obj["teacher_delivery_summary"].as_str().map_or(false, |s| s.is_empty())
    {
        let title = obj.get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Deliver the requested learning material.");
        obj.insert(
            "teacher_delivery_summary".to_string(),
            serde_json::json!(truncate_str(title, 1000)),
        );
    }

    // ── 5. Fix sections ───────────────────────────────────────────────
    let has_valid_sections = obj.get("sections")
        .and_then(|s| s.as_array())
        .map(|arr| !arr.is_empty())
        .unwrap_or(false);

    if !has_valid_sections {
        let title = obj.get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Content");
        obj.insert("sections".to_string(), serde_json::json!([{
            "title": "Requested Content",
            "purpose": "Deliver the requested learning material.",
            "body_blocks": [{
                "type": "paragraph",
                "content": title,
            }],
            "emphasis": "medium"
        }]));
    } else {
        // Repair each section
        let sections: Vec<serde_json::Value> = obj["sections"]
            .as_array()
            .unwrap()
            .iter()
            .map(repair_draft_section)
            .collect();
        obj.insert("sections".to_string(), serde_json::json!(sections));
    }

    // ── 6. Fix learning_objectives ─────────────────────────────────────
    if !obj.contains_key("learning_objectives") || obj["learning_objectives"].is_null() {
        obj.insert("learning_objectives".to_string(), serde_json::json!([]));
    }

    serde_json::to_string(&serde_json::Value::Object(obj)).unwrap_or_else(|_| raw.to_string())
}

/// Repair a single draft section.
fn repair_draft_section(s: &serde_json::Value) -> serde_json::Value {
    let m = match s.as_object() {
        Some(m) => m,
        None => return s.clone(),
    };

    let title = m.get("title")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Untitled Section");
    let purpose = m.get("purpose")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Section content");
    let emphasis = m.get("emphasis")
        .and_then(|v| v.as_str())
        .unwrap_or("medium");

    // Repair body_blocks — ensure at least one non-empty block
    let body_blocks: Vec<serde_json::Value> = m.get("body_blocks")
        .and_then(|b| b.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|block| {
                    let btype = block.get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("paragraph");
                    let content = block.get("content")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())?;
                    Some(serde_json::json!({
                        "type": normalize_body_block_type(btype),
                        "content": content,
                    }))
                })
                .collect()
        })
        .unwrap_or_default();

    let body_blocks = if body_blocks.is_empty() {
        vec![serde_json::json!({
            "type": "paragraph",
            "content": purpose,
        })]
    } else {
        body_blocks
    };

    serde_json::json!({
        "title": truncate_str(title, 200),
        "purpose": truncate_str(purpose, 500),
        "body_blocks": body_blocks,
        "emphasis": emphasis,
    })
}

/// Normalize body block type to allowed values.
fn normalize_body_block_type(raw: &str) -> &'static str {
    match raw.to_lowercase().as_str() {
        "paragraph" | "text" | "p" => "paragraph",
        "bullet" | "bullets" | "list" | "ul" => "bullet",
        "checklist" | "check" | "checkbox" => "checklist",
        "note" | "callout" | "info" => "note",
        _ => "paragraph",
    }
}

/// Truncate a string safely (char-aware).
fn truncate_str(value: &str, max: usize) -> String {
    let trimmed = value.trim();
    let chars: String = trimmed.chars().take(max).collect();
    if chars.len() < trimmed.chars().count() {
        format!("{}...", chars)
    } else {
        chars
    }
}

/// Build a fallback content draft payload from an interpretation.
pub fn fallback_from_interpretation(
    title: &str,
    summary: &str,
    teacher_delivery_summary: &str,
) -> ContentDraftPayload {
    ContentDraftPayload {
        schema_version: SCHEMA_VERSION.to_string(),
        title: truncate_str(title, 200),
        summary: truncate_str(summary, 1000),
        learning_objectives: vec![],
        sections: vec![ContentSection {
            title: "Requested Content".to_string(),
            purpose: "Deliver the requested learning material.".to_string(),
            body_blocks: vec![BodyBlock {
                r#type: "paragraph".to_string(),
                content: truncate_str(summary, 1000),
            }],
            emphasis: "medium".to_string(),
        }],
        teacher_delivery_summary: truncate_str(teacher_delivery_summary, 1000),
        fallback: DraftFallback {
            triggered: true,
            reason_code: Some("provider_response_contract_invalid".to_string()),
            action: Some("fallback_from_interpretation".to_string()),
        },
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_version() {
        assert_eq!(SCHEMA_VERSION, "media_content_draft.v1");
    }

    #[test]
    fn test_fallback_creates_valid_payload() {
        let p = fallback_from_interpretation(
            "Materi Pecahan",
            "Pengenalan pecahan untuk kelas 5",
            "Ringkasan pengajaran",
        );
        assert_eq!(p.schema_version, SCHEMA_VERSION);
        assert_eq!(p.sections.len(), 1);
        assert!(p.fallback.triggered);
    }

    #[test]
    fn test_decode_valid_payload() {
        let json = r#"{
            "schema_version": "media_content_draft.v1",
            "title": "Materi Pecahan",
            "summary": "Pengenalan pecahan",
            "sections": [
                {
                    "title": "Pengertian",
                    "purpose": "Perkenalan",
                    "body_blocks": [
                        {"type": "paragraph", "content": "Pecahan adalah bagian dari keseluruhan"}
                    ],
                    "emphasis": "medium"
                }
            ],
            "teacher_delivery_summary": "Ringkasan pengajaran"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "decode failed: {:?}", result.err());
        let payload = result.unwrap();
        assert_eq!(payload.title, "Materi Pecahan");
    }

    #[test]
    fn test_decode_empty_returns_error() {
        let result = decode_and_validate("");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_invalid_json_returns_error() {
        let result = decode_and_validate("{bad}");
        assert!(result.is_err());
    }

    // ── Repair function tests ────────────────────────────────────────────

    #[test]
    fn test_draft_repair_missing_schema_version() {
        let json = r#"{
            "title": "Test",
            "summary": "Test summary",
            "sections": [{"title": "s", "purpose": "p", "body_blocks": [{"type": "paragraph", "content": "c"}], "emphasis": "medium"}],
            "teacher_delivery_summary": "test"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "repair should inject schema_version: {:?}", result.err());
        assert_eq!(result.unwrap().schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn test_draft_repair_empty_sections() {
        let json = r#"{
            "schema_version": "media_content_draft.v1",
            "title": "Test",
            "summary": "Test summary",
            "sections": [],
            "teacher_delivery_summary": "test"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "repair should inject minimal section: {:?}", result.err());
        let payload = result.unwrap();
        assert_eq!(payload.sections.len(), 1);
        assert_eq!(payload.sections[0].title, "Requested Content");
    }

    #[test]
    fn test_draft_repair_missing_title() {
        let json = r#"{
            "schema_version": "media_content_draft.v1",
            "summary": "Test summary",
            "sections": [{"title": "s", "purpose": "p", "body_blocks": [{"type": "paragraph", "content": "c"}], "emphasis": "medium"}],
            "teacher_delivery_summary": "test"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "repair should inject default title: {:?}", result.err());
    }

    #[test]
    fn test_draft_repair_empty_body_blocks() {
        let json = r#"{
            "schema_version": "media_content_draft.v1",
            "title": "Test",
            "summary": "Test summary",
            "sections": [{"title": "s", "purpose": "My purpose", "body_blocks": [], "emphasis": "medium"}],
            "teacher_delivery_summary": "test"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "repair should inject body_block from purpose: {:?}", result.err());
        let payload = result.unwrap();
        assert_eq!(payload.sections[0].body_blocks[0].content, "My purpose");
    }
}

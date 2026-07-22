//! `MediaContentDraftSchema` — schema version `media_content_draft.v1`.
//!
//! Port of Python `ContentDraftPayload` / `ContentDraftContractModel`.

use garde::Validate;
use serde::{Deserialize, Serialize};

use serde::Deserializer;

pub const SCHEMA_VERSION: &str = "media_content_draft.v1";

/// Deserialize a string that may arrive as a String, JSON Object/Map, Array, Number, or Bool.
pub fn deserialize_string_lenient<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    match val {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Bool(b) => Ok(b.to_string()),
        serde_json::Value::Null => Ok(String::new()),
        serde_json::Value::Object(m) => {
            if let Some(s) = m.get("text").or_else(|| m.get("content")).or_else(|| m.get("label")).or_else(|| m.get("name")).or_else(|| m.get("value")).and_then(|v| v.as_str()) {
                Ok(s.to_string())
            } else if let Some(items) = m.get("items").and_then(|v| v.as_array()) {
                let strings: Vec<String> = items.iter().filter_map(|it| match it {
                    serde_json::Value::String(s) => Some(s.clone()),
                    serde_json::Value::Number(n) => Some(n.to_string()),
                    _ => None,
                }).collect();
                Ok(strings.join("\n- "))
            } else {
                Ok(serde_json::to_string(&serde_json::Value::Object(m)).unwrap_or_default())
            }
        }
        serde_json::Value::Array(arr) => {
            let strings: Vec<String> = arr.iter().filter_map(|v| match v {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            }).collect();
            Ok(strings.join("\n- "))
        }
    }
}

// ─── Sub-types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct BodyBlock {
    #[garde(skip)]
    pub r#type: String,
    #[garde(length(min = 1, max = 1000))]
    #[serde(deserialize_with = "deserialize_string_lenient")]
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ContentSection {
    #[garde(length(min = 1, max = 200))]
    #[serde(deserialize_with = "deserialize_string_lenient")]
    pub title: String,
    #[garde(length(min = 1, max = 500))]
    #[serde(deserialize_with = "deserialize_string_lenient")]
    pub purpose: String,
    #[garde(dive)]
    #[garde(length(min = 1))]
    pub body_blocks: Vec<BodyBlock>,
    #[garde(skip)]
    #[serde(deserialize_with = "deserialize_string_lenient")]
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

// ─── PPTX slide types ───────────────────────────────────────────────────────

/// A single content item inside a PPTX slide (heading + body pair).
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PptxSlideContentItem {
    #[garde(skip)]
    #[serde(default)]
    pub heading: String,
    #[garde(skip)]
    #[serde(default)]
    pub body: String,
}

/// A single slide in the PPTX presentation structure.
///
/// Produced by the LLM when using `DEFAULT_PPTX_DRAFT_INSTRUCTION`.
/// Each slide has an explicit `layout_type` from the 8-layout catalog.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PptxSlide {
    #[garde(skip)]
    pub slide_number: u32,
    #[garde(length(min = 1, max = 50))]
    pub layout_type: String,
    #[garde(length(min = 1, max = 200))]
    #[serde(deserialize_with = "deserialize_string_lenient")]
    pub title: String,
    #[garde(skip)]
    #[serde(default)]
    pub subtitle: Option<String>,
    #[garde(skip)]
    #[serde(default)]
    pub content: Vec<PptxSlideContentItem>,
}

// ─── Main payload ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ContentDraftPayload {
    #[garde(length(min = 1))]
    #[serde(deserialize_with = "deserialize_string_lenient")]
    pub schema_version: String,
    #[garde(length(min = 1, max = 200))]
    #[serde(deserialize_with = "deserialize_string_lenient")]
    pub title: String,
    #[garde(length(min = 1, max = 1000))]
    #[serde(deserialize_with = "deserialize_string_lenient")]
    pub summary: String,
    #[garde(skip)]
    #[serde(default)]
    pub learning_objectives: Vec<String>,
    #[garde(dive)]
    #[serde(default)]
    pub sections: Vec<ContentSection>,
    #[garde(length(min = 1, max = 1000))]
    #[serde(deserialize_with = "deserialize_string_lenient")]
    pub teacher_delivery_summary: String,
    #[garde(skip)]
    #[serde(default)]
    pub fallback: DraftFallback,

    // ── PPTX-specific fields (optional, only present for slide output) ────

    /// Structured slide definitions produced by the PPTX draft instruction.
    /// When present, these take precedence over `sections` for building
    /// the `SlideBlueprint` in the media generator.
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slides: Option<Vec<PptxSlide>>,

    /// Presentation title suggested by the LLM (may differ from `title`).
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_title: Option<String>,

    /// Theme suggestion from the LLM: "dark_executive", "clean_light", or "modern_blue".
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_suggestion: Option<String>,
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
    // Aggressively extract the JSON object to bypass LLM conversational padding
    let trimmed = raw.trim();
    let mut cleaned = trimmed.to_string();
    
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start < end {
            cleaned = trimmed[start..=end].to_string();
        }
    } else {
        // Fallback markdown stripping
        if cleaned.starts_with("```json") {
            cleaned = cleaned.trim_start_matches("```json").trim().to_string();
        } else if cleaned.starts_with("```") {
            cleaned = cleaned.trim_start_matches("```").trim().to_string();
        }
        if cleaned.ends_with("```") {
            cleaned = cleaned.trim_end_matches("```").trim().to_string();
        }
    }

    let parsed: serde_json::Value = match serde_json::from_str(&cleaned) {
        Ok(v) => v,
        Err(_) => return cleaned,
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
    let title_str = obj.get("title")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Untitled Draft")
        .to_string();

    let summary_str = obj.get("summary")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&title_str)
        .to_string();

    let delivery_str = obj.get("teacher_delivery_summary")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&title_str)
        .to_string();

    obj.insert("title".to_string(), serde_json::json!(truncate_str(&title_str, 200)));
    obj.insert("summary".to_string(), serde_json::json!(truncate_str(&summary_str, 1000)));
    obj.insert(
        "teacher_delivery_summary".to_string(),
        serde_json::json!(truncate_str(&delivery_str, 1000)),
    );

    // ── 5. Fix sections ───────────────────────────────────────────────
    let has_valid_sections = obj.get("sections")
        .and_then(|s| s.as_array())
        .map(|arr| !arr.is_empty())
        .unwrap_or(false);

    // PPTX drafts produce slides[] instead of sections[]; allow empty sections
    // when structured slide data is present.
    let has_valid_slides = obj.get("slides")
        .and_then(|s| s.as_array())
        .map(|arr| !arr.is_empty())
        .unwrap_or(false);

    if !has_valid_sections && !has_valid_slides {
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
    } else if !has_valid_sections && has_valid_slides {
        // PPTX mode: ensure sections is a valid empty array
        obj.insert("sections".to_string(), serde_json::json!([]));
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
    } else if let Some(arr) = obj.get("learning_objectives").and_then(|v| v.as_array()).cloned() {
        // LLM may return [{"objective": "..."}, ...] instead of ["...", "..."]
        let fixed: Vec<serde_json::Value> = arr.iter().map(|item| match item {
            serde_json::Value::String(_) => item.clone(),
            serde_json::Value::Object(m) => {
                if let Some(s) = m.get("objective").or_else(|| m.get("text")).or_else(|| m.get("content")).or_else(|| m.get("description")).or_else(|| m.get("title")).and_then(|v| v.as_str()) {
                    serde_json::Value::String(s.to_string())
                } else {
                    serde_json::Value::String(serde_json::to_string(item).unwrap_or_default())
                }
            }
            serde_json::Value::Number(n) => serde_json::Value::String(n.to_string()),
            _ => serde_json::Value::String(item.to_string()),
        }).collect();
        obj.insert("learning_objectives".to_string(), serde_json::json!(fixed));
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
                    // Extract content — handle string, map, array, or other types
                    let content = match block.get("content") {
                        Some(serde_json::Value::String(s)) => {
                            if s.is_empty() { None } else { Some(s.clone()) }
                        }
                        Some(serde_json::Value::Object(m)) => {
                            // LLM may return {"text": "...", "items": [...]} or similar
                            if let Some(s) = m.get("text").or_else(|| m.get("content")).or_else(|| m.get("value")).or_else(|| m.get("description")).and_then(|v| v.as_str()) {
                                Some(s.to_string())
                            } else if let Some(items) = m.get("items").and_then(|v| v.as_array()) {
                                let joined: String = items.iter().filter_map(|it| it.as_str()).collect::<Vec<_>>().join("\n- ");
                                if joined.is_empty() { None } else { Some(format!("- {}", joined)) }
                            } else {
                                // Serialize the whole map as a string fallback
                                let s = serde_json::to_string(&serde_json::Value::Object(m.clone())).unwrap_or_default();
                                if s.is_empty() || s == "{}" { None } else { Some(s) }
                            }
                        }
                        Some(serde_json::Value::Array(arr)) => {
                            let joined: String = arr.iter().filter_map(|v| match v {
                                serde_json::Value::String(s) => Some(s.clone()),
                                _ => v.as_str().map(|s| s.to_string()),
                            }).collect::<Vec<_>>().join("\n- ");
                            if joined.is_empty() { None } else { Some(format!("- {}", joined)) }
                        }
                        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
                        _ => None,
                    };
                    let content = content?;
                    Some(serde_json::json!({
                        "type": normalize_body_block_type(btype),
                        "content": truncate_str(&content, 1000),
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
        slides: None,
        presentation_title: None,
        theme_suggestion: None,
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

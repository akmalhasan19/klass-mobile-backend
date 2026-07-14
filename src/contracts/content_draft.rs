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

    let payload: ContentDraftPayload = serde_json::from_str(trimmed).map_err(|e| {
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

/// Build a fallback content draft payload from an interpretation.
pub fn fallback_from_interpretation(
    title: &str,
    summary: &str,
    teacher_delivery_summary: &str,
) -> ContentDraftPayload {
    ContentDraftPayload {
        schema_version: SCHEMA_VERSION.to_string(),
        title: truncate(title, 200),
        summary: truncate(summary, 1000),
        learning_objectives: vec![],
        sections: vec![ContentSection {
            title: "Requested Content".to_string(),
            purpose: "Deliver the requested learning material.".to_string(),
            body_blocks: vec![BodyBlock {
                r#type: "paragraph".to_string(),
                content: truncate(summary, 1000),
            }],
            emphasis: "medium".to_string(),
        }],
        teacher_delivery_summary: truncate(teacher_delivery_summary, 1000),
        fallback: DraftFallback {
            triggered: true,
            reason_code: Some("provider_response_contract_invalid".to_string()),
            action: Some("fallback_from_interpretation".to_string()),
        },
    }
}

fn truncate(value: &str, max: usize) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= max {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..max.saturating_sub(3)])
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
}

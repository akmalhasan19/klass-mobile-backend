//! `MediaGenerationSpecContract` — the spec sent to the Python renderer.
//!
//! Contains the resolved output type, interpretation data, taxonomy hints,
//! and all parameters needed for document generation.

use garde::Validate;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "media_generation_spec.v1";

// ─── Sub-types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SpecTaxonomyInfo {
    #[garde(skip)]
    pub subject_name: Option<String>,
    #[garde(skip)]
    pub subject_slug: Option<String>,
    #[garde(skip)]
    pub sub_subject_name: Option<String>,
    #[garde(skip)]
    pub sub_subject_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SpecSection {
    #[garde(length(min = 1, max = 200))]
    pub title: String,
    #[garde(length(min = 1, max = 500))]
    pub purpose: String,
    #[garde(skip)]
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SpecContentDraft {
    #[garde(length(min = 1, max = 200))]
    pub title: String,
    #[garde(length(min = 1, max = 1000))]
    pub summary: String,
    #[garde(skip)]
    pub learning_objectives: Vec<String>,
    #[garde(dive)]
    #[garde(length(min = 1))]
    pub sections: Vec<SpecSection>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Validate)]
pub struct GenerationParameters {
    /// Visual density (low, medium, high).

    /// Language code.
    #[garde(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual_density: Option<String>,
    #[garde(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tone: Option<String>,
    #[garde(skip)]
    #[serde(default = "default_language")]
    pub language: String,
    /// Preferred format features.
    #[garde(skip)]
    #[serde(default)]
    pub features: Vec<String>,
}

fn default_language() -> String {
    "id".to_string()
}

// ─── Main payload ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct GenerationSpec {
    #[garde(length(min = 1))]
    pub schema_version: String,
    /// Resolved output type (pdf, docx, pptx).
    #[garde(length(min = 1, max = 20))]
    pub output_type: String,
    #[garde(dive)]
    pub content_draft: SpecContentDraft,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taxonomy: Option<SpecTaxonomyInfo>,
    #[garde(skip)]
    #[serde(default)]
    pub parameters: GenerationParameters,
}

// ─── Public API ─────────────────────────────────────────────────────────────

use crate::contracts::prompt_interpretation::ContractValidationError;

/// Decode a raw JSON string into validated `GenerationSpec`.
pub fn decode_and_validate(raw_json: &str) -> Result<GenerationSpec, ContractValidationError> {
    let trimmed = raw_json.trim();
    if trimmed.is_empty() {
        return Err(ContractValidationError {
            code: "empty_completion",
            message: "Generation spec was empty.".to_string(),
            details: serde_json::json!({"reason": "empty_completion"}),
            raw_completion: raw_json.to_string(),
        });
    }

    let spec: GenerationSpec = serde_json::from_str(trimmed).map_err(|e| {
        ContractValidationError {
            code: "generation_spec_invalid",
            message: format!("Failed to decode generation spec as JSON: {}", e),
            details: serde_json::json!({"json_error": e.to_string()}),
            raw_completion: raw_json.to_string(),
        }
    })?;

    if let Err(errors) = spec.validate() {
        return Err(ContractValidationError {
            code: "generation_spec_invalid",
            message: "Generation spec failed validation.".to_string(),
            details: serde_json::json!({"errors": errors.to_string()}),
            raw_completion: raw_json.to_string(),
        });
    }

    Ok(spec)
}

/// Build a fallback generation spec from content draft data.
pub fn fallback_from_draft(
    output_type: &str,
    title: &str,
    summary: &str,
) -> GenerationSpec {
    GenerationSpec {
        schema_version: SCHEMA_VERSION.to_string(),
        output_type: output_type.to_string(),
        content_draft: SpecContentDraft {
            title: truncate(title, 200),
            summary: truncate(summary, 1000),
            learning_objectives: vec![],
            sections: vec![SpecSection {
                title: "Requested Content".to_string(),
                purpose: "Deliver the requested learning material.".to_string(),
                body: truncate(summary, 1000),
            }],
        },
        taxonomy: None,
        parameters: GenerationParameters {
            visual_density: Some("medium".to_string()),
            tone: None,
            language: "id".to_string(),
            features: vec![],
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
        assert_eq!(SCHEMA_VERSION, "media_generation_spec.v1");
    }

    #[test]
    fn test_fallback_creates_valid_spec() {
        let spec = fallback_from_draft("pdf", "Materi Pecahan", "Pengenalan pecahan");
        assert_eq!(spec.output_type, "pdf");
        assert_eq!(spec.content_draft.sections.len(), 1);
    }

    #[test]
    fn test_decode_valid_spec() {
        let json = r#"{
            "schema_version": "media_generation_spec.v1",
            "output_type": "pdf",
            "content_draft": {
                "title": "Materi Pecahan",
                "summary": "Pengenalan pecahan",
                "sections": [
                    {
                        "title": "Pengertian",
                        "purpose": "Perkenalan",
                        "body": "Pecahan adalah bagian dari keseluruhan"
                    }
                ]
            },
            "parameters": {
                "language": "id"
            }
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "decode failed: {:?}", result.err());
        let spec = result.unwrap();
        assert_eq!(spec.output_type, "pdf");
    }

    #[test]
    fn test_decode_empty_returns_error() {
        let result = decode_and_validate("");
        assert!(result.is_err());
    }
}

//! `MediaDeliveryResponseSchema` — schema version `media_delivery_response.v1`.
//!
//! Port of Python `DeliveryResponsePayload` / `DeliveryContractModel`.

use garde::Validate;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "media_delivery_response.v1";

// ─── Sub-types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DeliveryArtifact {
    #[garde(length(min = 1, max = 100))]
    pub output_type: String,
    #[garde(length(min = 1, max = 200))]
    pub title: String,
    #[garde(length(min = 1, max = 2048))]
    pub file_url: String,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    #[garde(length(min = 1, max = 255))]
    pub mime_type: String,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TopicNode {
    #[garde(length(min = 1, max = 100))]
    pub id: String,
    #[garde(length(min = 1, max = 200))]
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ContentNode {
    #[garde(length(min = 1, max = 100))]
    pub id: String,
    #[garde(length(min = 1, max = 200))]
    pub title: String,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RecommendedProjectNode {
    #[garde(length(min = 1, max = 100))]
    pub id: String,
    #[garde(length(min = 1, max = 200))]
    pub title: String,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_file_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Publication {
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<TopicNode>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<ContentNode>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_project: Option<RecommendedProjectNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ResponseMeta {
    #[garde(length(min = 1, max = 100))]
    pub generated_at: String,
    #[garde(skip)]
    pub llm_used: bool,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DeliveryFallback {
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

impl Default for DeliveryFallback {
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
pub struct DeliveryResponsePayload {
    #[garde(length(min = 1))]
    pub schema_version: String,
    #[garde(length(min = 1, max = 200))]
    pub title: String,
    #[garde(length(min = 1, max = 1000))]
    pub preview_summary: String,
    #[garde(length(min = 1, max = 2000))]
    pub teacher_message: String,
    #[garde(skip)]
    #[serde(default)]
    pub recommended_next_steps: Vec<String>,
    #[garde(skip)]
    #[serde(default)]
    pub classroom_tips: Vec<String>,
    #[garde(dive)]
    pub artifact: DeliveryArtifact,
    #[garde(dive)]
    pub publication: Publication,
    #[garde(dive)]
    pub response_meta: ResponseMeta,
    #[garde(skip)]
    #[serde(default)]
    pub fallback: DeliveryFallback,
}

// ─── Public API ─────────────────────────────────────────────────────────────

use crate::contracts::prompt_interpretation::ContractValidationError;

/// Decode a raw JSON string into a validated `DeliveryResponsePayload`.
pub fn decode_and_validate(raw_json: &str) -> Result<DeliveryResponsePayload, ContractValidationError> {
    let trimmed = raw_json.trim();
    if trimmed.is_empty() {
        return Err(ContractValidationError {
            code: "empty_completion",
            message: "Provider completion was empty.".to_string(),
            details: serde_json::json!({"reason": "empty_completion"}),
            raw_completion: raw_json.to_string(),
        });
    }

    let payload: DeliveryResponsePayload = serde_json::from_str(trimmed).map_err(|e| {
        ContractValidationError {
            code: "provider_response_contract_invalid",
            message: format!("Failed to decode delivery completion as JSON: {}", e),
            details: serde_json::json!({"json_error": e.to_string()}),
            raw_completion: raw_json.to_string(),
        }
    })?;

    if let Err(errors) = payload.validate() {
        return Err(ContractValidationError {
            code: "provider_response_contract_invalid",
            message: "Provider completion failed MediaDeliveryResponseSchema validation.".to_string(),
            details: serde_json::json!({"errors": errors.to_string()}),
            raw_completion: raw_json.to_string(),
        });
    }

    Ok(payload)
}

/// Build a fallback delivery response.
pub fn fallback(
    title: &str,
    preview_summary: &str,
    output_type: &str,
    file_url: &str,
    mime_type: &str,
) -> DeliveryResponsePayload {
    let now = chrono::Utc::now().to_rfc3339();
    DeliveryResponsePayload {
        schema_version: SCHEMA_VERSION.to_string(),
        title: truncate(title, 200),
        preview_summary: truncate(preview_summary, 1000),
        teacher_message: format!(
            "Your {} has been generated. You can download it from the provided link.",
            output_type
        ),
        recommended_next_steps: vec![],
        classroom_tips: vec![],
        artifact: DeliveryArtifact {
            output_type: output_type.to_string(),
            title: truncate(title, 200),
            file_url: file_url.to_string(),
            thumbnail_url: None,
            mime_type: truncate(mime_type, 255),
            filename: None,
        },
        publication: Publication {
            topic: None,
            content: None,
            recommended_project: None,
        },
        response_meta: ResponseMeta {
            generated_at: now,
            llm_used: false,
            provider: None,
            model: None,
        },
        fallback: DeliveryFallback {
            triggered: true,
            reason_code: Some("provider_response_contract_invalid".to_string()),
            action: Some("fallback_from_delivery".to_string()),
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
        assert_eq!(SCHEMA_VERSION, "media_delivery_response.v1");
    }

    #[test]
    fn test_fallback_creates_valid_payload() {
        let p = fallback(
            "Materi Pecahan",
            "PDF materi pecahan kelas 5",
            "pdf",
            "https://storage.example.com/materi-pecahan.pdf",
            "application/pdf",
        );
        assert_eq!(p.schema_version, SCHEMA_VERSION);
        assert!(p.fallback.triggered);
        assert_eq!(p.response_meta.llm_used, false);
    }

    #[test]
    fn test_decode_valid_payload() {
        let json = r#"{
            "schema_version": "media_delivery_response.v1",
            "title": "Materi Pecahan",
            "preview_summary": "Preview summary",
            "teacher_message": "Pesan untuk guru",
            "artifact": {
                "output_type": "pdf",
                "title": "Materi Pecahan",
                "file_url": "https://example.com/file.pdf",
                "mime_type": "application/pdf"
            },
            "publication": {},
            "response_meta": {
                "generated_at": "2026-04-03T10:00:00Z",
                "llm_used": true,
                "provider": "minimax",
                "model": "minimax-m3"
            }
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "decode failed: {:?}", result.err());
        let payload = result.unwrap();
        assert_eq!(payload.artifact.output_type, "pdf");
        assert!(payload.response_meta.llm_used);
    }

    #[test]
    fn test_decode_empty_returns_error() {
        let result = decode_and_validate("");
        assert!(result.is_err());
    }
}

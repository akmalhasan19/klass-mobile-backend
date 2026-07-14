//! `MediaPromptTaxonomyInferenceService` output schema.
//!
//! Mirrors the output of the recommendation taxonomy inference module
//! as a contract schema for the LLM adapter response.

use garde::Validate;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "media_prompt_taxonomy_inference.v1";

// ─── Sub-types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct MatchedSubject {
    #[garde(skip)]
    pub id: i64,
    #[garde(length(min = 1, max = 100))]
    pub name: String,
    #[garde(length(min = 1, max = 100))]
    pub slug: String,
    #[garde(skip)]
    pub confidence_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct MatchedSubSubject {
    #[garde(skip)]
    pub id: i64,
    #[garde(length(min = 1, max = 100))]
    pub name: String,
    #[garde(length(min = 1, max = 100))]
    pub slug: String,
    #[garde(skip)]
    pub confidence_score: f64,
    /// The parent subject for this sub-subject.
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<MatchedSubject>,
}

/// Education level detected from the prompt.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DetectedJenjang {
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jenjang: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<i32>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semester: Option<i32>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bab: Option<i32>,
}

/// A single candidate match from the taxonomy classifier.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CandidateMatch {
    #[garde(dive)]
    pub subject: MatchedSubject,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_subject: Option<MatchedSubSubject>,
    #[garde(skip)]
    pub score: f64,
}

// ─── Main payload ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TaxonomyInferencePayload {
    #[garde(length(min = 1))]
    pub schema_version: String,
    /// Original prompt text.
    #[garde(length(min = 1, max = 5000))]
    pub prompt: String,
    /// Confidence label: low, medium, high.
    #[garde(skip)]
    pub confidence_label: String,
    /// The best match subject.
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_match_subject: Option<MatchedSubject>,
    /// The best match sub_subject (if found).
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_match_sub_subject: Option<MatchedSubSubject>,
    /// Candidate matches sorted by score descending.
    #[garde(skip)]
    #[serde(default)]
    pub candidate_matches: Vec<CandidateMatch>,
    /// Education level detection.
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_jenjang: Option<DetectedJenjang>,
    /// Whether this inference used a direct phrase match.
    #[garde(skip)]
    #[serde(default)]
    pub phrase_match: bool,
}

// ─── Public API ─────────────────────────────────────────────────────────────

use crate::contracts::prompt_interpretation::ContractValidationError;

/// Decode a raw JSON string into validated `TaxonomyInferencePayload`.
pub fn decode_and_validate(raw_json: &str) -> Result<TaxonomyInferencePayload, ContractValidationError> {
    let trimmed = raw_json.trim();
    if trimmed.is_empty() {
        return Err(ContractValidationError {
            code: "empty_completion",
            message: "Taxonomy inference completion was empty.".to_string(),
            details: serde_json::json!({"reason": "empty_completion"}),
            raw_completion: raw_json.to_string(),
        });
    }

    let payload: TaxonomyInferencePayload = serde_json::from_str(trimmed).map_err(|e| {
        ContractValidationError {
            code: "provider_response_contract_invalid",
            message: format!("Failed to decode taxonomy inference completion as JSON: {}", e),
            details: serde_json::json!({"json_error": e.to_string()}),
            raw_completion: raw_json.to_string(),
        }
    })?;

    if let Err(errors) = payload.validate() {
        return Err(ContractValidationError {
            code: "provider_response_contract_invalid",
            message: "Provider completion failed MediaPromptTaxonomyInference validation.".to_string(),
            details: serde_json::json!({"errors": errors.to_string()}),
            raw_completion: raw_json.to_string(),
        });
    }

    Ok(payload)
}

/// Build a fallback taxonomy inference payload.
pub fn fallback(prompt: &str) -> TaxonomyInferencePayload {
    TaxonomyInferencePayload {
        schema_version: SCHEMA_VERSION.to_string(),
        prompt: prompt.to_string(),
        confidence_label: "low".to_string(),
        best_match_subject: None,
        best_match_sub_subject: None,
        candidate_matches: vec![],
        detected_jenjang: None,
        phrase_match: false,
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_version() {
        assert_eq!(SCHEMA_VERSION, "media_prompt_taxonomy_inference.v1");
    }

    #[test]
    fn test_fallback_creates_empty_result() {
        let p = fallback("handout pecahan kelas 5 SD");
        assert!(!p.confidence_label.is_empty());
        assert!(p.best_match_subject.is_none());
        assert!(!p.phrase_match);
    }

    #[test]
    fn test_decode_valid_payload() {
        let json = r#"{
            "schema_version": "media_prompt_taxonomy_inference.v1",
            "prompt": "handout pecahan kelas 5 SD",
            "confidence_label": "high",
            "best_match_subject": {
                "id": 1,
                "name": "Matematika",
                "slug": "matematika",
                "confidence_score": 0.95
            },
            "best_match_sub_subject": {
                "id": 101,
                "name": "Pecahan",
                "slug": "pecahan",
                "confidence_score": 0.88
            },
            "candidate_matches": [
                {
                    "subject": {"id": 1, "name": "Matematika", "slug": "matematika", "confidence_score": 0.95},
                    "sub_subject": {"id": 101, "name": "Pecahan", "slug": "pecahan", "confidence_score": 0.88},
                    "score": 0.88
                }
            ],
            "detected_jenjang": {
                "jenjang": "SD",
                "class": 5
            },
            "phrase_match": true
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "decode failed: {:?}", result.err());
        let payload = result.unwrap();
        assert_eq!(payload.confidence_label, "high");
        assert!(payload.phrase_match);
        assert_eq!(payload.best_match_subject.as_ref().unwrap().name, "Matematika");
    }

    #[test]
    fn test_decode_empty_returns_error() {
        let result = decode_and_validate("");
        assert!(result.is_err());
    }
}

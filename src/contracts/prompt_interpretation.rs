//! `MediaPromptInterpretationSchema` — schema version `media_prompt_understanding.v1`.
//!
//! Port of Python `InterpretationPayload` / `InterpretationContractModel`.
//! Uses `serde` for JSON deserialization and `garde` for field validation.

use garde::Validate;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "media_prompt_understanding.v1";

// ─── Sub-types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TeacherIntent {
    #[garde(length(min = 1, max = 100))]
    pub r#type: String,
    #[garde(length(min = 1, max = 500))]
    pub goal: String,
    #[garde(length(min = 1, max = 100))]
    pub preferred_delivery_mode: String,
    #[garde(skip)]
    pub requires_clarification: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct InterpretationConstraints {
    #[garde(skip)]
    #[serde(default = "default_preferred_output_type")]
    pub preferred_output_type: String,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_duration_minutes: Option<i32>,
    #[garde(skip)]
    #[serde(default)]
    pub must_include: Vec<String>,
    #[garde(skip)]
    #[serde(default)]
    pub avoid: Vec<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<String>,
}

fn default_preferred_output_type() -> String {
    "auto".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct OutputCandidate {
    #[garde(length(min = 1, max = 100))]
    pub r#type: String,
    #[garde(skip)]
    pub score: f64,
    #[garde(length(min = 1, max = 500))]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct BlueprintSection {
    #[garde(length(min = 1, max = 200))]
    pub title: String,
    #[garde(length(min = 1, max = 500))]
    pub purpose: String,
    #[garde(skip)]
    pub bullets: Vec<String>,
    #[garde(skip)]
    pub estimated_length: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DocumentBlueprint {
    #[garde(length(min = 1, max = 200))]
    pub title: String,
    #[garde(length(min = 1, max = 1000))]
    pub summary: String,
    #[garde(length(min = 1))]
    pub sections: Vec<BlueprintSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SubjectContext {
    #[garde(length(min = 1, max = 100))]
    pub subject_name: String,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SubSubjectContext {
    #[garde(length(min = 1, max = 100))]
    pub sub_subject_name: String,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_subject_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TargetAudience {
    #[garde(length(min = 1, max = 100))]
    pub label: String,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_range: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RequestedMediaCharacteristics {
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<String>,
    #[garde(skip)]
    #[serde(default)]
    pub format_preferences: Vec<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_density: Option<String>,
}

impl Default for RequestedMediaCharacteristics {
    fn default() -> Self {
        Self {
            tone: None,
            format_preferences: vec![],
            visual_density: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Asset {
    #[garde(length(min = 1, max = 100))]
    pub r#type: String,
    #[garde(length(min = 1, max = 500))]
    pub description: String,
    #[garde(skip)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AssessmentBlock {
    #[garde(length(min = 1, max = 200))]
    pub title: String,
    #[garde(skip)]
    pub r#type: String,
    #[garde(length(min = 1, max = 1000))]
    pub instructions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Confidence {
    #[garde(skip)]
    pub score: f64,
    #[garde(length(min = 1, max = 100))]
    pub label: String,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ContentIntegrity {
    #[garde(skip)]
    pub integrity_score: f64,
    #[garde(skip)]
    #[serde(default)]
    pub violations: Vec<serde_json::Value>,
    #[garde(length(min = 1, max = 50))]
    pub classification_source: String,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

// ─── Main payload ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct InterpretationPayload {
    #[garde(length(min = 1))]
    pub schema_version: String,
    #[garde(length(min = 1, max = 5000))]
    pub teacher_prompt: String,
    #[garde(length(min = 1, max = 32))]
    pub language: String,
    #[garde(dive)]
    pub teacher_intent: TeacherIntent,
    #[garde(skip)]
    pub learning_objectives: Vec<String>,
    #[garde(dive)]
    pub constraints: InterpretationConstraints,
    #[garde(dive)]
    #[garde(length(min = 1))]
    pub output_type_candidates: Vec<OutputCandidate>,
    #[garde(length(min = 1, max = 1000))]
    pub resolved_output_type_reasoning: String,
    #[garde(dive)]
    pub document_blueprint: DocumentBlueprint,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_context: Option<SubjectContext>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_subject_context: Option<SubSubjectContext>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_audience: Option<TargetAudience>,
    #[garde(skip)]
    #[serde(default)]
    pub requested_media_characteristics: RequestedMediaCharacteristics,
    #[garde(skip)]
    #[serde(default)]
    pub assets: Vec<Asset>,
    #[garde(skip)]
    #[serde(default)]
    pub assessment_or_activity_blocks: Vec<AssessmentBlock>,
    #[garde(length(min = 1, max = 1000))]
    pub teacher_delivery_summary: String,
    #[garde(dive)]
    pub confidence: Confidence,
    #[garde(skip)]
    #[serde(default)]
    pub fallback: InterpretationFallback,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_integrity: Option<ContentIntegrity>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "_meta_repairs")]
    pub meta_repairs: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct InterpretationFallback {
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

impl Default for InterpretationFallback {
    fn default() -> Self {
        Self {
            triggered: false,
            reason_code: None,
            action: None,
        }
    }
}

// ─── Validation error ───────────────────────────────────────────────────────

use std::fmt;

#[derive(Debug)]
pub struct ContractValidationError {
    pub code: &'static str,
    pub message: String,
    pub details: serde_json::Value,
    pub raw_completion: String,
}

impl fmt::Display for ContractValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.code, self.message, self.details)
    }
}

impl std::error::Error for ContractValidationError {}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Decode a raw JSON string into a validated `InterpretationPayload`.
pub fn decode_and_validate(raw_json: &str) -> Result<InterpretationPayload, ContractValidationError> {
    let trimmed = raw_json.trim();
    if trimmed.is_empty() {
        return Err(ContractValidationError {
            code: "empty_completion",
            message: "Provider completion was empty.".to_string(),
            details: serde_json::json!({"reason": "empty_completion"}),
            raw_completion: raw_json.to_string(),
        });
    }

    let payload: InterpretationPayload = serde_json::from_str(trimmed).map_err(|e| {
        ContractValidationError {
            code: "provider_response_contract_invalid",
            message: format!("Failed to decode interpretation completion as JSON: {}", e),
            details: serde_json::json!({"json_error": e.to_string()}),
            raw_completion: raw_json.to_string(),
        }
    })?;

    if let Err(errors) = payload.validate() {
        return Err(ContractValidationError {
            code: "provider_response_contract_invalid",
            message: "Provider completion failed MediaPromptInterpretationSchema validation.".to_string(),
            details: serde_json::json!({"errors": errors.to_string()}),
            raw_completion: raw_json.to_string(),
        });
    }

    Ok(payload)
}

/// Build a fallback interpretation payload when the provider response is invalid.
pub fn fallback(teacher_prompt: &str) -> InterpretationPayload {
    InterpretationPayload {
        schema_version: SCHEMA_VERSION.to_string(),
        teacher_prompt: teacher_prompt.to_string(),
        language: detect_language(teacher_prompt),
        teacher_intent: TeacherIntent {
            r#type: "generate_learning_media".to_string(),
            goal: truncate(teacher_prompt, 500),
            preferred_delivery_mode: "digital_download".to_string(),
            requires_clarification: false,
        },
        learning_objectives: vec![],
        constraints: InterpretationConstraints {
            preferred_output_type: "auto".to_string(),
            max_duration_minutes: None,
            must_include: vec![],
            avoid: vec![],
            tone: None,
        },
        output_type_candidates: vec![
            OutputCandidate {
                r#type: "pdf".to_string(),
                score: 0.82,
                reason: "Default PDF fallback.".to_string(),
            },
            OutputCandidate {
                r#type: "docx".to_string(),
                score: 0.64,
                reason: "Default DOCX fallback.".to_string(),
            },
            OutputCandidate {
                r#type: "pptx".to_string(),
                score: 0.46,
                reason: "Default PPTX fallback.".to_string(),
            },
        ],
        resolved_output_type_reasoning: "Default PDF fallback from interpretation contract.".to_string(),
        document_blueprint: DocumentBlueprint {
            title: truncate(teacher_prompt, 200),
            summary: truncate(teacher_prompt, 1000),
            sections: vec![BlueprintSection {
                title: "Requested Content".to_string(),
                purpose: "Deliver the requested learning material clearly.".to_string(),
                bullets: vec![truncate(teacher_prompt, 300)],
                estimated_length: "medium".to_string(),
            }],
        },
        subject_context: None,
        sub_subject_context: None,
        target_audience: None,
        requested_media_characteristics: RequestedMediaCharacteristics::default(),
        assets: vec![],
        assessment_or_activity_blocks: vec![],
        teacher_delivery_summary: truncate(teacher_prompt, 1000),
        confidence: Confidence {
            score: 0.6,
            label: "medium".to_string(),
            rationale: None,
        },
        fallback: InterpretationFallback {
            triggered: true,
            reason_code: Some("provider_response_contract_invalid".to_string()),
            action: Some("fallback_from_interpretation".to_string()),
        },
        content_integrity: None,
        meta_repairs: None,
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

fn detect_language(prompt: &str) -> String {
    let lowered = prompt.to_lowercase();
    let id_markers = ["buatkan", "kelas", "siswa", "materi", "untuk", "dan"];
    let en_markers = ["create", "grade", "students", "lesson", "for", "and"];
    let id_score = id_markers.iter().filter(|m| lowered.contains(*m)).count();
    let en_score = en_markers.iter().filter(|m| lowered.contains(*m)).count();
    if id_score > en_score {
        "id".to_string()
    } else if en_score > id_score {
        "en".to_string()
    } else {
        "und".to_string()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_version() {
        assert_eq!(SCHEMA_VERSION, "media_prompt_understanding.v1");
    }

    #[test]
    fn test_fallback_creates_valid_payload() {
        let p = fallback("Create a worksheet about fractions for grade 5");
        assert_eq!(p.schema_version, SCHEMA_VERSION);
        assert!(p.fallback.triggered);
        assert_eq!(p.output_type_candidates.len(), 3);
        assert!(p.teacher_prompt.contains("fractions"));
    }

    #[test]
    fn test_decode_valid_payload() {
        let json = r#"{
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "Buatkan materi tentang pecahan",
            "language": "id",
            "teacher_intent": {
                "type": "generate_learning_media",
                "goal": "Membuat materi pecahan",
                "preferred_delivery_mode": "digital_download",
                "requires_clarification": false
            },
            "learning_objectives": ["Memahami konsep pecahan"],
            "constraints": {
                "preferred_output_type": "pdf"
            },
            "output_type_candidates": [
                {"type": "pdf", "score": 0.9, "reason": "Best for printout"}
            ],
            "resolved_output_type_reasoning": "PDF is best",
            "document_blueprint": {
                "title": "Materi Pecahan",
                "summary": "Pengenalan pecahan",
                "sections": [
                    {
                        "title": "Pengertian",
                        "purpose": "Perkenalan",
                        "bullets": ["Definisi"],
                        "estimated_length": "short"
                    }
                ]
            },
            "teacher_delivery_summary": "Ringkasan pengajaran",
            "confidence": {
                "score": 0.85,
                "label": "high"
            }
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "decode failed: {:?}", result.err());
        let payload = result.unwrap();
        assert_eq!(payload.language, "id");
        assert_eq!(payload.output_type_candidates.len(), 1);
    }

    #[test]
    fn test_decode_empty_returns_error() {
        let result = decode_and_validate("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "empty_completion");
    }

    #[test]
    fn test_decode_invalid_json_returns_error() {
        let result = decode_and_validate("{invalid json}");
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_language_indonesian() {
        let lang = detect_language("Buatkan materi pelajaran untuk siswa kelas 5 SD");
        assert_eq!(lang, "id");
    }

    #[test]
    fn test_detect_language_english() {
        let lang = detect_language("Create a lesson for grade 5 students");
        assert_eq!(lang, "en");
    }

    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate("hello", 100), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        let long = "a".repeat(1000);
        let result = truncate(&long, 10);
        assert_eq!(result.len(), 10);
        assert!(result.ends_with("..."));
    }
}

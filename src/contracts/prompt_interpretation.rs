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
///
/// Applies a repair step before parsing to fix common LLM output issues:
/// - `output_type_candidates` as a string instead of an array of objects
/// - Missing `schema_version`
/// - Missing or null required fields
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

    // ── Repair step: fix common LLM output issues ─────────────────────
    let repaired = repair_interpretation_json(trimmed);

    let payload: InterpretationPayload = serde_json::from_str(&repaired).map_err(|e| {
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

/// Repair common LLM output issues before JSON parsing.
///
/// Many LLMs (especially smaller or free models) return JSON that is
/// structurally close but doesn't match the strict schema exactly.
/// This function normalizes the most common variations:
///
/// 1. `output_type_candidates` as a string (e.g. `"pdf"`) instead of
///    an array of objects → wrap in `[{type: ..., score: ..., reason: ...}]`
/// 2. Missing `schema_version` → inject the correct version
/// 3. Missing required string fields → inject sensible defaults
fn repair_interpretation_json(raw: &str) -> String {
    let parsed: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return raw.to_string(), // not valid JSON, let parser handle the error
    };

    let mut obj = match parsed {
        serde_json::Value::Object(m) => m,
        _ => return raw.to_string(), // not an object, nothing to repair
    };

    // ── Extract teacher_prompt early to avoid borrow conflicts ─────────
    let teacher_prompt = obj.get("teacher_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("Learning material")
        .to_string();

    // ── 1. Fix schema_version ──────────────────────────────────────────
    if !obj.contains_key("schema_version") || obj["schema_version"].is_null() {
        obj.insert(
            "schema_version".to_string(),
            serde_json::json!(SCHEMA_VERSION),
        );
    }

    // ── 2. Fix output_type_candidates ─────────────────────────────────
    {
        let candidates = obj.get("output_type_candidates").cloned();
        if let Some(candidates) = candidates {
            match candidates {
                // LLM returned a plain string like "pdf" or "auto"
                serde_json::Value::String(s) => {
                    let fixed = normalize_output_type_candidates_string(&s);
                    obj.insert("output_type_candidates".to_string(), fixed);
                }
                // LLM returned an object instead of array
                serde_json::Value::Object(_) => {
                    let fixed = normalize_output_type_candidates_object(&candidates);
                    obj.insert("output_type_candidates".to_string(), fixed);
                }
                // Already an array — check each element
                serde_json::Value::Array(arr) => {
                    let fixed: Vec<serde_json::Value> = arr
                        .iter()
                        .map(|item| match item {
                            // Element is a string like "pdf" → wrap in object
                            serde_json::Value::String(s) => {
                                serde_json::json!({
                                    "type": normalize_output_type(s),
                                    "score": 0.7,
                                    "reason": format!("LLM suggested {}.", s),
                                })
                            }
                            serde_json::Value::Object(m) => {
                                // Ensure required fields exist
                                let type_val = m.get("type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("pdf");
                                let score = m.get("score")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.7);
                                let reason = m.get("reason")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("LLM candidate.");
                                serde_json::json!({
                                    "type": normalize_output_type(type_val),
                                    "score": score,
                                    "reason": reason,
                                })
                            }
                            other => other.clone(),
                        })
                        .collect();
                    obj.insert("output_type_candidates".to_string(), serde_json::json!(fixed));
                }
                _ => {}
            }
        } else {
            // Missing entirely → inject default candidates
            obj.insert(
                "output_type_candidates".to_string(),
                serde_json::json!([
                    {"type": "pdf", "score": 0.82, "reason": "Default PDF candidate."},
                    {"type": "docx", "score": 0.64, "reason": "Default DOCX candidate."},
                    {"type": "pptx", "score": 0.46, "reason": "Default PPTX candidate."}
                ]),
            );
        }
    }

    // ── 3. Fix document_blueprint ──────────────────────────────────────
    if let Some(blueprint) = obj.get_mut("document_blueprint") {
        if let Some(bp) = blueprint.as_object_mut() {
            // Ensure title exists and is non-empty
            if !bp.contains_key("title") || bp["title"].is_null()
                || bp["title"].as_str().map_or(false, |s| s.is_empty())
            {
                bp.insert("title".to_string(), serde_json::json!(truncate_str(&teacher_prompt, 200)));
            }
            // Ensure summary exists and is non-empty
            if !bp.contains_key("summary") || bp["summary"].is_null()
                || bp["summary"].as_str().map_or(false, |s| s.is_empty())
            {
                bp.insert("summary".to_string(), serde_json::json!(truncate_str(&teacher_prompt, 1000)));
            }
            // Ensure sections exists and has at least one entry
            if !bp.contains_key("sections") || bp["sections"].is_null()
                || bp["sections"].as_array().map_or(true, |a| a.is_empty())
            {
                bp.insert("sections".to_string(), serde_json::json!([{
                    "title": "Requested Content",
                    "purpose": "Deliver the requested learning material.",
                    "bullets": [truncate_str(&teacher_prompt, 300)],
                    "estimated_length": "medium"
                }]));
            } else {
                // Repair each section
                let sections: Vec<serde_json::Value> = bp["sections"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| repair_section(s))
                    .collect();
                bp.insert("sections".to_string(), serde_json::json!(sections));
            }
        }
    }

    // ── 4. Fix teacher_intent ──────────────────────────────────────────
    if let Some(intent) = obj.get_mut("teacher_intent") {
        if let Some(m) = intent.as_object_mut() {
            if !m.contains_key("type") || m["type"].is_null()
                || m["type"].as_str().map_or(false, |s| s.is_empty())
            {
                m.insert("type".to_string(), serde_json::json!("generate_learning_media"));
            }
            if !m.contains_key("goal") || m["goal"].is_null()
                || m["goal"].as_str().map_or(false, |s| s.is_empty())
            {
                m.insert("goal".to_string(), serde_json::json!(truncate_str(&teacher_prompt, 500)));
            }
            if !m.contains_key("preferred_delivery_mode") || m["preferred_delivery_mode"].is_null()
                || m["preferred_delivery_mode"].as_str().map_or(false, |s| s.is_empty())
            {
                m.insert("preferred_delivery_mode".to_string(), serde_json::json!("digital_download"));
            }
            // Ensure requires_clarification is a bool
            if !m.contains_key("requires_clarification") || m["requires_clarification"].is_null() {
                m.insert("requires_clarification".to_string(), serde_json::json!(false));
            }
        }
    }

    // ── 5. Fix constraints ─────────────────────────────────────────────
    if let Some(constraints) = obj.get_mut("constraints") {
        if let Some(m) = constraints.as_object_mut() {
            if !m.contains_key("preferred_output_type") || m["preferred_output_type"].is_null()
                || m["preferred_output_type"].as_str().map_or(false, |s| s.is_empty())
            {
                m.insert("preferred_output_type".to_string(), serde_json::json!("auto"));
            }
        }
    }

    // ── 6. Fix confidence ──────────────────────────────────────────────
    if let Some(confidence) = obj.get_mut("confidence") {
        if let Some(m) = confidence.as_object_mut() {
            if !m.contains_key("label") || m["label"].is_null()
                || m["label"].as_str().map_or(false, |s| s.is_empty())
            {
                m.insert("label".to_string(), serde_json::json!("medium"));
            }
            if !m.contains_key("score") || m["score"].is_null() {
                m.insert("score".to_string(), serde_json::json!(0.6));
            }
        }
    }

    // ── 7. Fix teacher_delivery_summary ────────────────────────────────
    if !obj.contains_key("teacher_delivery_summary") || obj["teacher_delivery_summary"].is_null()
        || obj["teacher_delivery_summary"].as_str().map_or(false, |s| s.is_empty())
    {
        obj.insert(
            "teacher_delivery_summary".to_string(),
            serde_json::json!(truncate_str(&teacher_prompt, 1000)),
        );
    }

    // ── 8. Fix language ────────────────────────────────────────────────
    if !obj.contains_key("language") || obj["language"].is_null()
        || obj["language"].as_str().map_or(false, |s| s.is_empty())
    {
        obj.insert("language".to_string(), serde_json::json!(detect_language(&teacher_prompt)));
    }

    // ── 9. Fix resolved_output_type_reasoning ──────────────────────────
    if !obj.contains_key("resolved_output_type_reasoning") || obj["resolved_output_type_reasoning"].is_null()
        || obj["resolved_output_type_reasoning"].as_str().map_or(false, |s| s.is_empty())
    {
        obj.insert(
            "resolved_output_type_reasoning".to_string(),
            serde_json::json!("Auto-selected based on content analysis."),
        );
    }

    serde_json::to_string(&serde_json::Value::Object(obj)).unwrap_or_else(|_| raw.to_string())
}

/// Repair a single section object from the interpretation blueprint.
fn repair_section(s: &serde_json::Value) -> serde_json::Value {
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

    // Repair bullets — ensure at least one non-empty bullet
    let bullets: Vec<String> = m.get("bullets")
        .and_then(|b| b.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|b| b.as_str().filter(|s| !s.is_empty()).map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let bullets = if bullets.is_empty() {
        vec![purpose.to_string()]
    } else {
        bullets
    };

    let estimated_length = m.get("estimated_length")
        .and_then(|v| v.as_str())
        .unwrap_or("medium");

    serde_json::json!({
        "title": truncate_str(title, 200),
        "purpose": truncate_str(purpose, 500),
        "bullets": bullets,
        "estimated_length": estimated_length,
    })
}

/// Normalize output type string to a valid format.
fn normalize_output_type(raw: &str) -> &'static str {
    match raw.to_lowercase().as_str() {
        "pdf" => "pdf",
        "docx" | "doc" | "word" => "docx",
        "pptx" | "ppt" | "powerpoint" | "slide" | "slides" => "pptx",
        _ => "pdf",
    }
}

/// Convert a plain string like "pdf" into an array of candidate objects.
fn normalize_output_type_candidates_string(s: &str) -> serde_json::Value {
    let primary = normalize_output_type(s);
    let (secondary, tertiary) = match primary {
        "pdf" => ("docx", "pptx"),
        "docx" => ("pdf", "pptx"),
        _ => ("pdf", "docx"),
    };
    serde_json::json!([
        {"type": primary, "score": 0.82, "reason": format!("LLM suggested {}.", primary)},
        {"type": secondary, "score": 0.64, "reason": format!("Alternative {}.", secondary)},
        {"type": tertiary, "score": 0.46, "reason": format!("Alternative {}.", tertiary)},
    ])
}

/// Convert an object like {"preferred": "pdf"} into an array of candidates.
fn normalize_output_type_candidates_object(obj: &serde_json::Value) -> serde_json::Value {
    // Try to extract a type from the object
    let primary = obj.get("preferred")
        .or_else(|| obj.get("type"))
        .or_else(|| obj.get("format"))
        .and_then(|v| v.as_str())
        .map(normalize_output_type)
        .unwrap_or("pdf");
    normalize_output_type_candidates_string(primary)
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

/// Build a fallback interpretation payload when the provider response is invalid.
pub fn fallback(teacher_prompt: &str) -> InterpretationPayload {
    InterpretationPayload {
        schema_version: SCHEMA_VERSION.to_string(),
        teacher_prompt: teacher_prompt.to_string(),
        language: detect_language(teacher_prompt),
        teacher_intent: TeacherIntent {
            r#type: "generate_learning_media".to_string(),
            goal: truncate_str(teacher_prompt, 500),
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
            title: truncate_str(teacher_prompt, 200),
            summary: truncate_str(teacher_prompt, 1000),
            sections: vec![BlueprintSection {
                title: "Requested Content".to_string(),
                purpose: "Deliver the requested learning material clearly.".to_string(),
                bullets: vec![truncate_str(teacher_prompt, 300)],
                estimated_length: "medium".to_string(),
            }],
        },
        subject_context: None,
        sub_subject_context: None,
        target_audience: None,
        requested_media_characteristics: RequestedMediaCharacteristics::default(),
        assets: vec![],
        assessment_or_activity_blocks: vec![],
        teacher_delivery_summary: truncate_str(teacher_prompt, 1000),
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
    fn test_truncate_str_short_string() {
        assert_eq!(truncate_str("hello", 100), "hello");
    }

    #[test]
    fn test_truncate_str_long_string() {
        let long = "a".repeat(1000);
        let result = truncate_str(&long, 10);
        assert!(result.len() <= 13); // 10 chars + "..."
        assert!(result.ends_with("..."));
    }

    // ── Repair function tests ────────────────────────────────────────────

    #[test]
    fn test_repair_missing_schema_version() {
        let json = r#"{"teacher_prompt": "test"}"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "repair should inject schema_version: {:?}", result.err());
    }

    #[test]
    fn test_repair_output_type_candidates_as_string() {
        let json = r#"{
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "test",
            "language": "id",
            "teacher_intent": {"type": "generate_learning_media", "goal": "test", "preferred_delivery_mode": "digital_download", "requires_clarification": false},
            "output_type_candidates": "pdf",
            "resolved_output_type_reasoning": "test",
            "document_blueprint": {"title": "test", "summary": "test", "sections": [{"title": "s", "purpose": "p", "bullets": ["b"], "estimated_length": "medium"}]},
            "constraints": {"preferred_output_type": "auto"},
            "confidence": {"score": 0.8, "label": "high"},
            "teacher_delivery_summary": "test"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "repair should convert string to array: {:?}", result.err());
        let payload = result.unwrap();
        assert_eq!(payload.output_type_candidates.len(), 3);
        assert_eq!(payload.output_type_candidates[0].r#type, "pdf");
    }

    #[test]
    fn test_repair_output_type_candidates_missing() {
        let json = r#"{
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "test",
            "language": "id",
            "teacher_intent": {"type": "generate_learning_media", "goal": "test", "preferred_delivery_mode": "digital_download", "requires_clarification": false},
            "resolved_output_type_reasoning": "test",
            "document_blueprint": {"title": "test", "summary": "test", "sections": [{"title": "s", "purpose": "p", "bullets": ["b"], "estimated_length": "medium"}]},
            "constraints": {"preferred_output_type": "auto"},
            "confidence": {"score": 0.8, "label": "high"},
            "teacher_delivery_summary": "test"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "repair should inject default candidates: {:?}", result.err());
        let payload = result.unwrap();
        assert_eq!(payload.output_type_candidates.len(), 3);
    }

    #[test]
    fn test_repair_missing_teacher_intent_goal() {
        let json = r#"{
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "Buatkan materi pecahan",
            "language": "id",
            "teacher_intent": {"type": "generate_learning_media", "preferred_delivery_mode": "digital_download", "requires_clarification": false},
            "output_type_candidates": [{"type": "pdf", "score": 0.8, "reason": "test"}],
            "resolved_output_type_reasoning": "test",
            "document_blueprint": {"title": "test", "summary": "test", "sections": [{"title": "s", "purpose": "p", "bullets": ["b"], "estimated_length": "medium"}]},
            "constraints": {"preferred_output_type": "auto"},
            "confidence": {"score": 0.8, "label": "high"},
            "teacher_delivery_summary": "test"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "repair should inject goal from prompt: {:?}", result.err());
    }
}

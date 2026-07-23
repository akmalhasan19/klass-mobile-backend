//! `MediaPromptInterpretationSchema` — schema version `media_prompt_understanding.v1`.
//!
//! Port of Python `InterpretationPayload` / `InterpretationContractModel`.
//! Uses `serde` for JSON deserialization and `garde` for field validation.

use garde::Validate;
use serde::{Deserialize, Deserializer, Serialize};

pub const SCHEMA_VERSION: &str = "media_prompt_understanding.v1";

/// Deserialize an optional i32 that may arrive as a string (e.g. "30" or "Menggunakan...").
/// Returns None for null/missing, parses numeric strings, and strips non-numeric strings.
fn deserialize_optional_i32<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    if val.is_null() {
        return Ok(None);
    }
    match &val {
        serde_json::Value::Number(n) => Ok(n.as_i64().map(|v| v as i32)),
        serde_json::Value::String(s) => {
            if let Ok(n) = s.trim().parse::<i32>() {
                Ok(Some(n))
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

/// Deserialize an f64 that may arrive as a string (e.g. "0.8" or a sentence).
fn deserialize_f64_lenient<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    match &val {
        serde_json::Value::Number(n) => Ok(n.as_f64().unwrap_or(0.6)),
        serde_json::Value::String(s) => {
            if let Ok(n) = s.trim().parse::<f64>() {
                Ok(n)
            } else {
                Ok(0.6)
            }
        }
        _ => Ok(0.6),
    }
}

/// Deserialize a bool that may arrive as a string.
fn deserialize_bool_lenient<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    match &val {
        serde_json::Value::Bool(b) => Ok(*b),
        serde_json::Value::String(s) => Ok(s.to_lowercase() == "true"),
        serde_json::Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0) != 0.0),
        _ => Ok(false),
    }
}

/// Deserialize an optional string that may arrive as a string, map/object, array, number, or bool.
pub fn deserialize_optional_string_lenient<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    if val.is_null() {
        return Ok(None);
    }
    match val {
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
        serde_json::Value::Number(n) => Ok(Some(n.to_string())),
        serde_json::Value::Bool(b) => Ok(Some(b.to_string())),
        serde_json::Value::Object(m) => {
            if let Some(s) = m.get("text").or_else(|| m.get("content")).or_else(|| m.get("label")).or_else(|| m.get("name")).or_else(|| m.get("subject_name")).or_else(|| m.get("title")).or_else(|| m.get("value")).and_then(|v| v.as_str()) {
                Ok(Some(s.to_string()))
            } else {
                Ok(Some(serde_json::to_string(&serde_json::Value::Object(m)).unwrap_or_default()))
            }
        }
        serde_json::Value::Array(arr) => {
            let strings: Vec<String> = arr.iter().filter_map(|v| match v {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            }).collect();
            if !strings.is_empty() {
                Ok(Some(strings.join(", ")))
            } else {
                Ok(Some(serde_json::to_string(&serde_json::Value::Array(arr)).unwrap_or_default()))
            }
        }
        serde_json::Value::Null => Ok(None),
    }
}

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
    #[serde(deserialize_with = "deserialize_bool_lenient")]
    pub requires_clarification: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct InterpretationConstraints {
    #[garde(skip)]
    #[serde(default = "default_preferred_output_type")]
    pub preferred_output_type: String,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_i32")]
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
    #[serde(deserialize_with = "deserialize_f64_lenient")]
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
    #[serde(deserialize_with = "deserialize_bool_lenient")]
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
    #[serde(deserialize_with = "deserialize_f64_lenient")]
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
    #[serde(deserialize_with = "deserialize_f64_lenient")]
    pub integrity_score: f64,
    #[garde(skip)]
    #[serde(default)]
    pub violations: Vec<serde_json::Value>,
    #[garde(length(min = 1, max = 50))]
    pub classification_source: String,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta_repairs: Option<serde_json::Value>,
}

// ─── PLAN MODE types ─────────────────────────────────────────────────────

/// PLAN MODE status — indicates whether the LLM detected missing fields
/// that require teacher clarification before generation can proceed.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PlanMode {
    /// Whether PLAN MODE is active (true = clarification needed).
    #[garde(skip)]
    #[serde(deserialize_with = "deserialize_bool_lenient")]
    pub active: bool,
    /// Reason why PLAN MODE was triggered (if active=true).
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The content type detected by the LLM.
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_content_type: Option<String>,
    /// Confidence of content type detection (0.0 - 1.0).
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_f64")]
    pub content_type_confidence: Option<f64>,
}

impl Default for PlanMode {
    fn default() -> Self {
        Self {
            active: false,
            reason: None,
            detected_content_type: None,
            content_type_confidence: None,
        }
    }
}

/// Deserialize an optional f64 that may arrive as a string.
fn deserialize_optional_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    if val.is_null() {
        return Ok(None);
    }
    match &val {
        serde_json::Value::Number(n) => Ok(n.as_f64()),
        serde_json::Value::String(s) => {
            if let Ok(n) = s.trim().parse::<f64>() {
                Ok(Some(n))
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

/// Fields that the LLM was able to interpret from the teacher's prompt.
///
/// These are the "best effort" extraction results. Null values indicate
/// fields the LLM could not determine from the prompt.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct InterpretedFields {
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_string_lenient")]
    pub target_audience: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_string_lenient")]
    pub output_type: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_string_lenient")]
    pub subject: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_string_lenient")]
    pub topic: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learning_objectives: Option<Vec<String>>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_string_lenient")]
    pub page_count: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_string_lenient")]
    pub difficulty_level: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_bool_lenient_option")]
    pub include_activities: Option<bool>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_string_lenient")]
    pub slide_count: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_count: Option<i32>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_string_lenient")]
    pub meeting_duration: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_string_lenient")]
    pub teaching_method: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_string_lenient")]
    pub assessment_method: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_string_lenient")]
    pub visual_density: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_string_lenient")]
    pub speaker_notes: Option<String>,
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_string_lenient")]
    pub question_type: Option<String>,
}

/// Deserialize an optional bool that may arrive as a string.
fn deserialize_bool_lenient_option<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let val = serde_json::Value::deserialize(deserializer)?;
    if val.is_null() {
        return Ok(None);
    }
    match &val {
        serde_json::Value::Bool(b) => Ok(Some(*b)),
        serde_json::Value::String(s) => Ok(Some(s.to_lowercase() == "true")),
        serde_json::Value::Number(n) => Ok(Some(n.as_f64().unwrap_or(0.0) != 0.0)),
        _ => Ok(None),
    }
}

/// A single missing field that needs clarification.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct MissingField {
    #[garde(length(min = 1, max = 50))]
    pub field_id: String,
    #[garde(length(min = 1, max = 100))]
    pub field_label: String,
    #[garde(length(min = 1, max = 20))]
    pub priority: String,
    #[garde(length(min = 1, max = 500))]
    pub question: String,
    #[garde(skip)]
    #[serde(default)]
    pub suggestions: Vec<serde_json::Value>,
    #[garde(length(min = 1, max = 20))]
    pub input_type: String,
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
    #[serde(default)]
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

    // ── PLAN MODE fields ──────────────────────────────────────────────────

    /// PLAN MODE status — whether clarification is needed.
    #[garde(skip)]
    #[serde(default)]
    pub plan_mode: PlanMode,

    /// Fields that the LLM was able to interpret from the prompt.
    /// Null values indicate fields the LLM could not determine.
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interpreted_fields: Option<InterpretedFields>,

    /// Missing fields that need teacher clarification.
    /// Only populated when plan_mode.active = true.
    #[garde(skip)]
    #[serde(default)]
    pub missing_fields: Vec<MissingField>,
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
        Err(_) => return cleaned, // not valid JSON, let parser handle the error
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

    // ── 1b. Fix teacher_prompt (required field) ──────────────────────
    if !obj.contains_key("teacher_prompt") || obj["teacher_prompt"].is_null()
        || obj["teacher_prompt"].as_str().map_or(false, |s| s.is_empty())
    {
        obj.insert(
            "teacher_prompt".to_string(),
            serde_json::json!(teacher_prompt),
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

    // ── 3. Fix max_duration_minutes if it's a string ───────────────────
    if let Some(constraints) = obj.get_mut("constraints").and_then(|v| v.as_object_mut()) {
        if let Some(duration) = constraints.get("max_duration_minutes") {
            if let Some(s) = duration.as_str() {
                // If it's a string, try to parse it, otherwise remove it
                if let Ok(num) = s.parse::<i32>() {
                    constraints.insert("max_duration_minutes".to_string(), serde_json::json!(num));
                } else {
                    constraints.remove("max_duration_minutes");
                }
            }
        }
    }

    // ── 4. Fix document_blueprint ──────────────────────────────────────
    // Handle document_blueprint being a string (LLM returned a plain string
    // like "Presentasi PPT 6 slide..." instead of an object)
    let bp_is_string = obj.get("document_blueprint")
        .map_or(false, |v| v.is_string());
    if bp_is_string {
        let bp_str = obj["document_blueprint"].as_str().unwrap_or("").to_string();
        obj.insert("document_blueprint".to_string(), serde_json::json!({
            "title": truncate_str(&bp_str, 200),
            "summary": truncate_str(&bp_str, 1000),
            "sections": [{
                "title": "Requested Content",
                "purpose": truncate_str(&bp_str, 500),
                "bullets": [truncate_str(&bp_str, 300)],
                "estimated_length": "medium"
            }]
        }));
    }
    if !obj.contains_key("document_blueprint") || obj["document_blueprint"].is_null() {
        // Check if LLM returned key aliases: "blueprint" or "document"
        if let Some(bp) = obj.remove("blueprint").or_else(|| obj.remove("document")) {
            obj.insert("document_blueprint".to_string(), bp);
        } else {
            // Missing entirely — inject default blueprint from teacher_prompt
            obj.insert("document_blueprint".to_string(), serde_json::json!({
                "title": truncate_str(&teacher_prompt, 200),
                "summary": truncate_str(&teacher_prompt, 1000),
                "sections": [{
                    "title": "Requested Content",
                    "purpose": "Deliver the requested learning material.",
                    "bullets": [truncate_str(&teacher_prompt, 300)],
                    "estimated_length": "medium"
                }]
            }));
        }
    }

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
    } else {
        obj.insert("teacher_intent".to_string(), serde_json::json!({
            "type": "generate_learning_media",
            "goal": truncate_str(&teacher_prompt, 500),
            "preferred_delivery_mode": "digital_download",
            "requires_clarification": false,
        }));
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
    } else {
        obj.insert("constraints".to_string(), serde_json::json!({
            "preferred_output_type": "auto"
        }));
    }

    // ── 6. Fix confidence ──────────────────────────────────────────────
    // LLMs may return confidence as:
    //   - An object: {"score": 0.95, "label": "high"}  (correct)
    //   - A float:   0.95
    //   - A string:  "high" or "0.95"
    //   - null or missing entirely
    if let Some(confidence) = obj.get_mut("confidence") {
        if let Some(m) = confidence.as_object_mut() {
            if !m.contains_key("label") || m["label"].is_null()
                || m["label"].as_str().map_or(false, |s| s.is_empty())
            {
                m.insert("label".to_string(), serde_json::json!("medium"));
            }
            if !m.contains_key("score") || m["score"].is_null() {
                m.insert("score".to_string(), serde_json::json!(0.6));
            } else if let Some(s) = m.get("score").and_then(|v| v.as_str()) {
                let parsed = s.parse::<f64>().unwrap_or(0.6);
                m.insert("score".to_string(), serde_json::json!(parsed));
            }
        } else {
            // Confidence is a scalar (float, string, null) — wrap into struct
            let (score, label) = match confidence.clone() {
                serde_json::Value::Number(n) => {
                    let s = n.as_f64().unwrap_or(0.6);
                    let l = if s >= 0.8 { "high" } else if s >= 0.5 { "medium" } else { "low" };
                    (s, l.to_string())
                }
                serde_json::Value::String(s) => {
                    if let Ok(n) = s.trim().parse::<f64>() {
                        let l = if n >= 0.8 { "high" } else if n >= 0.5 { "medium" } else { "low" };
                        (n, l.to_string())
                    } else {
                        // String label like "high", "medium", "low"
                        let label = s.trim().to_lowercase();
                        let score = match label.as_str() {
                            "high" | "very high" => 0.9,
                            "medium" | "moderate" => 0.6,
                            "low" | "very low" => 0.3,
                            _ => 0.6,
                        };
                        (score, if label.is_empty() { "medium".to_string() } else { label })
                    }
                }
                _ => (0.6, "medium".to_string()),
            };
            *confidence = serde_json::json!({"score": score, "label": label});
        }
    } else {
        // Confidence field missing entirely — inject default
        obj.insert("confidence".to_string(), serde_json::json!({"score": 0.6, "label": "medium"}));
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

    // ── 10. Fix content_integrity.integrity_score (f64, required) ─────
    if let Some(ci) = obj.get_mut("content_integrity") {
        if let Some(m) = ci.as_object_mut() {
            if !m.contains_key("integrity_score") || m["integrity_score"].is_null() {
                m.insert("integrity_score".to_string(), serde_json::json!(0.8));
            } else if let Some(s) = m.get("integrity_score").and_then(|v| v.as_str()) {
                let parsed = s.parse::<f64>().unwrap_or(0.8);
                m.insert("integrity_score".to_string(), serde_json::json!(parsed));
            }
            if !m.contains_key("classification_source") || m["classification_source"].is_null()
                || m["classification_source"].as_str().map_or(false, |s| s.is_empty())
            {
                m.insert("classification_source".to_string(), serde_json::json!("llm_interpret"));
            }
        }
    }

    // ── 11. Fix assets[].required (bool) ──────────────────────────────
    if let Some(assets) = obj.get_mut("assets").and_then(|v| v.as_array_mut()) {
        for asset in assets.iter_mut() {
            if let Some(m) = asset.as_object_mut() {
                if !m.contains_key("required") || m["required"].is_null() {
                    m.insert("required".to_string(), serde_json::json!(true));
                } else if let Some(s) = m.get("required").and_then(|v| v.as_str()) {
                    let parsed = s.to_lowercase() == "true";
                    m.insert("required".to_string(), serde_json::json!(parsed));
                }
            }
        }
    }

    // ── 12. Fix learning_objectives ───────────────────────────────────
    if !obj.contains_key("learning_objectives") || obj["learning_objectives"].is_null() {
        obj.insert("learning_objectives".to_string(), serde_json::json!([]));
    } else if let Some(arr) = obj.get("learning_objectives").and_then(|v| v.as_array()) {
        let fixed: Vec<serde_json::Value> = arr
            .iter()
            .map(|item| match item {
                serde_json::Value::String(s) => serde_json::json!(s),
                serde_json::Value::Object(m) => {
                    let text = m.values().next().and_then(|v| v.as_str()).unwrap_or("Objective");
                    serde_json::json!(text)
                }
                other => serde_json::json!(other.to_string()),
            })
            .collect();
        obj.insert("learning_objectives".to_string(), serde_json::json!(fixed));
    }

    // ── 13. Fix plan_mode ────────────────────────────────────────────
    if !obj.contains_key("plan_mode") || obj["plan_mode"].is_null() {
        // Infer plan_mode from teacher_intent.requires_clarification
        let requires_clarification = obj
            .get("teacher_intent")
            .and_then(|ti| ti.get("requires_clarification"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let confidence_score = obj
            .get("confidence")
            .and_then(|c| c.get("score"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.6);

        // Low confidence or explicit clarification flag → activate plan mode
        let plan_active = requires_clarification || confidence_score < 0.5;

        let reason = if plan_active {
            Some("Prompt lacks required information for media generation. Teacher clarification needed.".to_string())
        } else {
            None
        };

        obj.insert(
            "plan_mode".to_string(),
            serde_json::json!({
                "active": plan_active,
                "reason": reason,
                "detected_content_type": null,
                "content_type_confidence": confidence_score,
            }),
        );
    } else if let Some(pm) = obj.get_mut("plan_mode") {
        if let Some(m) = pm.as_object_mut() {
            // Ensure 'active' field exists
            if !m.contains_key("active") || m["active"].is_null() {
                m.insert("active".to_string(), serde_json::json!(false));
            }
        }
    }

    // ── 14. Fix interpreted_fields ───────────────────────────────────
    // Auto-detect slide_count and page_count from the teacher prompt.
    let detected_slide_count = detect_slide_count_from_prompt(&teacher_prompt);
    let detected_page_count = detect_page_count_from_prompt(&teacher_prompt);

    if !obj.contains_key("interpreted_fields") || obj["interpreted_fields"].is_null() {
        // Build interpreted_fields from existing extracted data
        let target_audience = obj
            .get("target_audience")
            .and_then(|ta| ta.get("label"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                obj.get("target_audience")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });

        let output_type = obj
            .get("constraints")
            .and_then(|c| c.get("preferred_output_type"))
            .and_then(|v| v.as_str())
            .filter(|s| *s != "auto")
            .map(|s| s.to_string());

        let subject = obj
            .get("subject_context")
            .and_then(|sc| sc.get("subject_name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let topic = obj
            .get("document_blueprint")
            .and_then(|db| db.get("title"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let learning_objectives = obj
            .get("learning_objectives")
            .and_then(|lo| lo.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty());

        obj.insert(
            "interpreted_fields".to_string(),
            serde_json::json!({
                "target_audience": target_audience,
                "output_type": output_type,
                "subject": subject,
                "topic": topic,
                "learning_objectives": learning_objectives,
                "page_count": detected_page_count,
                "difficulty_level": null,
                "include_activities": null,
                "slide_count": detected_slide_count,
                "question_count": null,
                "meeting_duration": null,
                "teaching_method": null,
                "assessment_method": null,
                "visual_density": null,
                "speaker_notes": null,
                "question_type": null,
            }),
        );
    } else {
        // ── 14b. Repair existing interpreted_fields with null slide/page counts ──
        if let Some(ifields) = obj.get_mut("interpreted_fields").and_then(|v| v.as_object_mut()) {
            // Fill in slide_count from prompt detection if LLM returned null
            let slide_null = ifields.get("slide_count").map_or(true, |v| v.is_null());
            if slide_null {
                if let Some(sc) = &detected_slide_count {
                    ifields.insert("slide_count".to_string(), serde_json::json!(sc));
                }
            }
            // Fill in page_count from prompt detection if LLM returned null
            let page_null = ifields.get("page_count").map_or(true, |v| v.is_null());
            if page_null {
                if let Some(pc) = &detected_page_count {
                    ifields.insert("page_count".to_string(), serde_json::json!(pc));
                }
            }
        }
    }

    // ── 15. Fix missing_fields ───────────────────────────────────────
    if !obj.contains_key("missing_fields") || obj["missing_fields"].is_null() {
        obj.insert("missing_fields".to_string(), serde_json::json!([]));
    } else if let Some(mf) = obj.get_mut("missing_fields") {
        if let Some(arr) = mf.as_array_mut() {
            // Ensure each missing field has required properties
            let repaired: Vec<serde_json::Value> = arr
                .iter()
                .filter_map(|item| {
                    if let Some(m) = item.as_object() {
                        let field_id = m.get("field_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let field_label = m.get("field_label")
                            .and_then(|v| v.as_str())
                            .unwrap_or(field_id);
                        let priority = m.get("priority")
                            .and_then(|v| v.as_str())
                            .unwrap_or("required");
                        let question = m.get("question")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let input_type = m.get("input_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("select");

                        if field_id.is_empty() || question.is_empty() {
                            return None; // Skip invalid entries
                        }

                        Some(serde_json::json!({
                            "field_id": field_id,
                            "field_label": field_label,
                            "priority": priority,
                            "question": question,
                            "suggestions": m.get("suggestions").cloned().unwrap_or(serde_json::json!([])),
                            "input_type": input_type,
                        }))
                    } else {
                        None
                    }
                })
                .collect();
            *arr = repaired;
        }
    }

    serde_json::to_string(&serde_json::Value::Object(obj)).unwrap_or_else(|_| raw.to_string())
}

/// Repair a single section object from the interpretation blueprint.
fn repair_section(s: &serde_json::Value) -> serde_json::Value {
    // If the LLM returned a string instead of a section object, convert it.
    if let Some(title_str) = s.as_str() {
        return serde_json::json!({
            "title": truncate_str(title_str, 200),
            "purpose": truncate_str(title_str, 500),
            "bullets": [truncate_str(title_str, 300)],
            "estimated_length": "medium",
        });
    }

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

    // Repair bullets — handle string, array-of-strings, or missing
    let bullets: Vec<String> = match m.get("bullets") {
        // LLM returned bullets as a plain string instead of an array
        Some(serde_json::Value::String(s)) if !s.is_empty() => {
            vec![s.clone()]
        }
        Some(serde_json::Value::Array(arr)) => {
            arr.iter()
                .filter_map(|b| match b {
                    serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
                    serde_json::Value::Object(m) => m.values().next().and_then(|v| v.as_str()).map(|s| s.to_string()),
                    _ => None,
                })
                .collect()
        }
        _ => Vec::new(),
    };

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

/// Auto-detect slide count from the teacher prompt.
///
/// Uses `content_standards::detect_slide_count` but only returns numeric
/// values (e.g. "5") that can be parsed as i64 by `build_generation_spec`.
/// Returns `None` for non-numeric results like "specified".
fn detect_slide_count_from_prompt(prompt: &str) -> Option<String> {
    use crate::standards::content_standards::detect_slide_count;
    detect_slide_count(prompt).filter(|s| s.parse::<i64>().is_ok())
}

/// Auto-detect page count from the teacher prompt.
///
/// Uses `content_standards::detect_page_count` but only returns numeric
/// values (e.g. "5") that can be parsed as i64 by `build_generation_spec`.
/// Returns `None` for non-numeric results like "specified".
fn detect_page_count_from_prompt(prompt: &str) -> Option<String> {
    use crate::standards::content_standards::detect_page_count;
    detect_page_count(prompt).filter(|s| s.parse::<i64>().is_ok())
}

/// Re-derive `plan_mode`, `interpreted_fields`, and `missing_fields` from the
/// interpretation payload's content, producing fresh and contextual questions.
///
/// This is called after a **cache hit** so that PLAN MODE questions are always
/// regenerated deterministically from the actual interpretation content, rather
/// than returning stale frozen questions from a previous cache entry.
///
/// The function inspects which required fields are present/missing for the
/// detected content type and builds contextual missing_field questions in
/// Indonesian (Bahasa).
pub fn regenerate_plan_mode_from_interpretation(mut payload: InterpretationPayload) -> InterpretationPayload {
    // ── 1. Build interpreted_fields from the payload ────────────────────
    let interpreted_fields = build_interpreted_fields_from_payload(&payload);

    // ── 2. Detect content type ──────────────────────────────────────────
    let detected_content_type = detect_content_type_from_payload(&payload);

    // ── 3. Determine required fields per content type ───────────────────
    let required_field_ids = content_type_required_fields(&detected_content_type);

    // ── 4. Find missing required fields ─────────────────────────────────
    let missing_field_ids = find_missing_required_fields(&payload, &required_field_ids);

    // ── 5. Generate contextual missing_field questions ──────────────────
    let missing_fields: Vec<MissingField> = missing_field_ids
        .iter()
        .map(|field_id| generate_contextual_missing_field(field_id, &payload, &detected_content_type))
        .collect();

    // ── 6. Derive plan_mode ─────────────────────────────────────────────
    let plan_active = !missing_fields.is_empty();
    let reason = if plan_active {
        let missing_labels: Vec<&str> = missing_fields.iter().map(|f| f.field_label.as_str()).collect();
        Some(format!(
            "Berdasarkan analisis prompt, terdapat informasi yang belum lengkap untuk {}: {}. Silakan lengkapi informasi berikut agar media dapat dibuat dengan tepat.",
            detected_content_type_label(&detected_content_type),
            missing_labels.join(", ")
        ))
    } else {
        None
    };

    // ── 7. Update payload ───────────────────────────────────────────────
    payload.plan_mode = PlanMode {
        active: plan_active,
        reason,
        detected_content_type: Some(detected_content_type.clone()),
        content_type_confidence: Some(payload.confidence.score),
    };
    payload.interpreted_fields = Some(interpreted_fields);
    payload.missing_fields = missing_fields;

    // ── 8. Update requires_clarification to match ───────────────────────
    payload.teacher_intent.requires_clarification = plan_active;

    payload
}

/// Detect the content type from the interpretation payload.
///
/// Uses keyword analysis on the teacher prompt and blueprint title/summary
/// to determine which content type best matches.
fn detect_content_type_from_payload(payload: &InterpretationPayload) -> String {
    let text = format!(
        "{} {} {}",
        payload.teacher_prompt.to_lowercase(),
        payload.document_blueprint.title.to_lowercase(),
        payload.document_blueprint.summary.to_lowercase(),
    );

    // Score each content type by keyword matches
    let mut scores: Vec<(&str, i32)> = vec![
        ("slide_presentasi", 0),
        ("rpp", 0),
        ("lembar_kerja", 0),
        ("penilaian", 0),
        ("silabus", 0),
        ("materi_pembelajaran", 0),
    ];

    for (content_type, score) in scores.iter_mut() {
        *score = content_type_keywords(content_type)
            .iter()
            .filter(|kw| text.contains(*kw))
            .count() as i32;
    }

    scores.sort_by(|a, b| b.1.cmp(&a.1));

    // If top score is 0, default to materi_pembelajaran
    if scores[0].1 == 0 {
        "materi_pembelajaran".to_string()
    } else {
        scores[0].0.to_string()
    }
}

/// Keywords used to detect each content type from the prompt text.
fn content_type_keywords(content_type: &str) -> Vec<&'static str> {
    match content_type {
        "slide_presentasi" => vec!["slide", "presentasi", "powerpoint", "pptx", "tayangan"],
        "rpp" => vec!["rpp", "rencana pelaksanaan", "lesson plan", "perencanaan pembelajaran"],
        "lembar_kerja" => vec!["lembar kerja", "worksheet", "latihan", "praktik"],
        "penilaian" => vec!["penilaian", "asesmen", "assessment", "soal", "ujian", "tes"],
        "silabus" => vec!["silabus", "syllabus", "kurikulum"],
        "materi_pembelajaran" => vec!["materi", "modul", "handout", "bahan ajar", "pelajaran", "belajar"],
        _ => vec![],
    }
}

/// Required fields per content type.
fn content_type_required_fields(content_type: &str) -> Vec<&'static str> {
    match content_type {
        "slide_presentasi" => vec!["target_audience", "output_type"],
        "rpp" => vec!["target_audience", "learning_objectives"],
        "lembar_kerja" => vec!["target_audience", "difficulty_level"],
        "penilaian" => vec!["target_audience", "difficulty_level", "question_count"],
        "silabus" => vec!["target_audience"],
        "materi_pembelajaran" => vec!["target_audience", "output_type"],
        _ => vec!["target_audience"],
    }
}

/// User-friendly label for each content type.
fn detected_content_type_label(content_type: &str) -> String {
    match content_type {
        "slide_presentasi" => "slide presentasi".to_string(),
        "rpp" => "Rencana Pelaksanaan Pembelajaran (RPP)".to_string(),
        "lembar_kerja" => "lembar kerja".to_string(),
        "penilaian" => "penilaian/asesmen".to_string(),
        "silabus" => "silabus".to_string(),
        "materi_pembelajaran" => "materi pembelajaran".to_string(),
        _ => "media pembelajaran".to_string(),
    }
}

/// Check which required fields are missing from the interpretation payload.
fn find_missing_required_fields(payload: &InterpretationPayload, required: &[&str]) -> Vec<String> {
    let mut missing = Vec::new();

    for field_id in required {
        let is_present = match *field_id {
            "target_audience" => payload.target_audience.is_some(),
            "output_type" => {
                payload.constraints.preferred_output_type != "auto"
            }
            "learning_objectives" => !payload.learning_objectives.is_empty(),
            "difficulty_level" => payload
                .interpreted_fields
                .as_ref()
                .and_then(|f| f.difficulty_level.as_ref())
                .is_some(),
            "question_count" => payload
                .interpreted_fields
                .as_ref()
                .and_then(|f| f.question_count)
                .is_some(),
            _ => false,
        };

        if !is_present {
            missing.push(field_id.to_string());
        }
    }

    missing
}

/// Generate a contextual missing field question based on what is actually
/// missing, the detected content type, and the teacher's prompt context.
fn generate_contextual_missing_field(
    field_id: &str,
    payload: &InterpretationPayload,
    content_type: &str,
) -> MissingField {
    let topic_hint = &payload.document_blueprint.title;
    let subject_hint = payload
        .subject_context
        .as_ref()
        .map(|sc| sc.subject_name.as_str())
        .unwrap_or("pelajaran ini");

    match field_id {
        "target_audience" => {
            let (label, question, input_type) = match content_type {
                "slide_presentasi" => (
                    "Jenjang/Kelas",
                    format!("Slide presentasi '{}' ini ditujukan untuk siswa jenjang/kelas mana?", topic_hint),
                    "select",
                ),
                "rpp" => (
                    "Jenjang/Kelas",
                    format!("RPP '{}' ini disusun untuk jenjang/kelas berapa?", topic_hint),
                    "select",
                ),
                _ => (
                    "Jenjang/Kelas",
                    format!("Materi '{}' untuk {} ini ditujukan untuk siswa jenjang/kelas mana?", topic_hint, subject_hint),
                    "select",
                ),
            };
            MissingField {
                field_id: "target_audience".to_string(),
                field_label: label.to_string(),
                priority: "required".to_string(),
                question,
                suggestions: vec_jenjang_suggestions(),
                input_type: input_type.to_string(),
            }
        }
        "output_type" => {
            let (question, suggestions) = match content_type {
                "slide_presentasi" => (
                    format!("Format file apa yang Anda inginkan untuk slide '{}' ini?", topic_hint),
                    vec![
                        serde_json::json!({"value": "pptx", "label": "PowerPoint (.pptx)"}),
                        serde_json::json!({"value": "pdf", "label": "PDF (cetak)"}),
                    ],
                ),
                _ => (
                    format!("Format file apa yang Anda inginkan untuk materi '{}' ini?", topic_hint),
                    vec![
                        serde_json::json!({"value": "pdf", "label": "PDF (cetak)"}),
                        serde_json::json!({"value": "docx", "label": "Word (.docx)"}),
                        serde_json::json!({"value": "pptx", "label": "PowerPoint (.pptx)"}),
                    ],
                ),
            };
            MissingField {
                field_id: "output_type".to_string(),
                field_label: "Format Output".to_string(),
                priority: "required".to_string(),
                question,
                suggestions,
                input_type: "select".to_string(),
            }
        }
        "learning_objectives" => MissingField {
            field_id: "learning_objectives".to_string(),
            field_label: "Tujuan Pembelajaran".to_string(),
            priority: "required".to_string(),
            question: format!(
                "Apa saja tujuan pembelajaran yang ingin dicapai dari '{}'? {}",
                topic_hint,
                "Contoh: Memahami konsep X, Menganalisis Y, Menerapkan Z."
            ),
            suggestions: vec_string_suggestions(&["Memahami konsep...", "Menganalisis...", "Menerapkan...", "Menjelaskan..."]),
            input_type: "textarea".to_string(),
        },
        "difficulty_level" => MissingField {
            field_id: "difficulty_level".to_string(),
            field_label: "Tingkat Kesulitan".to_string(),
            priority: "required".to_string(),
            question: format!(
                "Tingkat kesulitan materi '{}' ini sebaiknya seperti apa?",
                topic_hint
            ),
            suggestions: vec_string_suggestions(&["Mudah (pengenalan dasar)", "Sedang (pemahaman konsep)", "Sulit (analisis dan evaluasi)"]),
            input_type: "select".to_string(),
        },
        "question_count" => MissingField {
            field_id: "question_count".to_string(),
            field_label: "Jumlah Soal".to_string(),
            priority: "required".to_string(),
            question: format!(
                "Berapa jumlah soal yang Anda butuhkan untuk asesmen '{}' ini?",
                topic_hint
            ),
            suggestions: vec_number_suggestions(&[10, 15, 20, 25, 30]),
            input_type: "number".to_string(),
        },
        _ => MissingField {
            field_id: field_id.to_string(),
            field_label: field_id.replace('_', " ").to_string(),
            priority: "recommended".to_string(),
            question: format!("Informasi '{}' belum terdeteksi. Mohon lengkapi jika diperlukan.", field_id),
            suggestions: vec![],
            input_type: "text".to_string(),
        },
    }
}

/// Build a Vec<serde_json::Value> of jenjang/kelas suggestion strings.
fn vec_jenjang_suggestions() -> Vec<serde_json::Value> {
    vec_string_suggestions(&[
        "SD Kelas 1", "SD Kelas 2", "SD Kelas 3",
        "SD Kelas 4", "SD Kelas 5", "SD Kelas 6",
        "SMP Kelas 7", "SMP Kelas 8", "SMP Kelas 9",
        "SMA Kelas 10", "SMA Kelas 11", "SMA Kelas 12",
    ])
}

/// Build a Vec<serde_json::Value> from a list of string slices.
fn vec_string_suggestions(items: &[&str]) -> Vec<serde_json::Value> {
    items.iter().map(|s| serde_json::json!(s)).collect()
}

/// Build a Vec<serde_json::Value> from a list of i32 values.
fn vec_number_suggestions(items: &[i32]) -> Vec<serde_json::Value> {
    items.iter().map(|n| serde_json::json!(n)).collect()
}

/// Build interpreted_fields from the interpretation payload content.
/// Preserves existing values from the payload's interpreted_fields when available,
/// and only fills in what can be derived from the main payload.
fn build_interpreted_fields_from_payload(payload: &InterpretationPayload) -> InterpretedFields {
    // Start from the existing interpreted_fields if present, to preserve
    // values the LLM already extracted (difficulty_level, question_count, etc.)
    let existing = payload.interpreted_fields.as_ref();

    let target_audience = payload.target_audience.as_ref().map(|ta| {
        match &ta.level {
            Some(level) => format!("{} {}", ta.label, level),
            None => ta.label.clone(),
        }
    });

    let output_type = if payload.constraints.preferred_output_type != "auto" {
        Some(payload.constraints.preferred_output_type.clone())
    } else {
        // Infer from top output_type_candidate
        payload.output_type_candidates.first().map(|c| c.r#type.clone())
    };

    let subject = payload.subject_context.as_ref().map(|sc| sc.subject_name.clone());
    let topic = Some(payload.document_blueprint.title.clone());

    let learning_objectives = if payload.learning_objectives.is_empty() {
        None
    } else {
        Some(payload.learning_objectives.clone())
    };

    InterpretedFields {
        // Prefer freshly derived values, fall back to existing
        target_audience: target_audience.or_else(|| existing.and_then(|e| e.target_audience.clone())),
        output_type: output_type.or_else(|| existing.and_then(|e| e.output_type.clone())),
        subject: subject.or_else(|| existing.and_then(|e| e.subject.clone())),
        topic: topic.or_else(|| existing.and_then(|e| e.topic.clone())),
        learning_objectives: learning_objectives.or_else(|| existing.and_then(|e| e.learning_objectives.clone())),
        // Preserve all other fields from the original payload
        page_count: existing.and_then(|e| e.page_count.clone()),
        difficulty_level: existing.and_then(|e| e.difficulty_level.clone()),
        include_activities: existing.and_then(|e| e.include_activities),
        slide_count: existing.and_then(|e| e.slide_count.clone()),
        question_count: existing.and_then(|e| e.question_count),
        meeting_duration: existing.and_then(|e| e.meeting_duration.clone()),
        teaching_method: existing.and_then(|e| e.teaching_method.clone()),
        assessment_method: existing.and_then(|e| e.assessment_method.clone()),
        visual_density: payload.requested_media_characteristics.visual_density.clone()
            .or_else(|| existing.and_then(|e| e.visual_density.clone())),
        speaker_notes: existing.and_then(|e| e.speaker_notes.clone()),
        question_type: existing.and_then(|e| e.question_type.clone()),
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
        plan_mode: PlanMode::default(),
        interpreted_fields: None,
        missing_fields: vec![],
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
        }\"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "repair should inject goal from prompt: {:?}", result.err());
    }

    // ── Confidence repair edge cases ─────────────────────────────────────

    /// Helper: build a full interpretation JSON with a custom confidence value.
    fn full_json_with_confidence(confidence_fragment: &str) -> String {
        format!(r#"{{
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "Buatkan materi matematika",
            "language": "id",
            "teacher_intent": {{"type": "generate_learning_media", "goal": "test", "preferred_delivery_mode": "digital_download", "requires_clarification": false}},
            "output_type_candidates": [{{"type": "pdf", "score": 0.8, "reason": "test"}}],
            "resolved_output_type_reasoning": "test",
            "document_blueprint": {{"title": "test", "summary": "test", "sections": [{{"title": "s", "purpose": "p", "bullets": ["b"], "estimated_length": "medium"}}]}},
            "constraints": {{"preferred_output_type": "auto"}},
            "confidence": {},
            "teacher_delivery_summary": "test"
        }}"#, confidence_fragment)
    }

    #[test]
    fn test_repair_confidence_as_float() {
        // Exact scenario from production logs: LLM returns "confidence": 0.95
        let json = full_json_with_confidence("0.95");
        let result = decode_and_validate(&json);
        assert!(result.is_ok(), "repair should wrap float confidence: {:?}", result.err());
        let payload = result.unwrap();
        assert!((payload.confidence.score - 0.95).abs() < f64::EPSILON);
        assert_eq!(payload.confidence.label, "high");
    }

    #[test]
    fn test_repair_confidence_as_low_float() {
        let json = full_json_with_confidence("0.3");
        let result = decode_and_validate(&json);
        assert!(result.is_ok(), "repair should wrap low float confidence: {:?}", result.err());
        let payload = result.unwrap();
        assert!((payload.confidence.score - 0.3).abs() < f64::EPSILON);
        assert_eq!(payload.confidence.label, "low");
    }

    #[test]
    fn test_repair_confidence_as_string_number() {
        let json = full_json_with_confidence(r#""0.85""#);
        let result = decode_and_validate(&json);
        assert!(result.is_ok(), "repair should parse string number confidence: {:?}", result.err());
        let payload = result.unwrap();
        assert!((payload.confidence.score - 0.85).abs() < f64::EPSILON);
        assert_eq!(payload.confidence.label, "high");
    }

    #[test]
    fn test_repair_confidence_as_string_label() {
        let json = full_json_with_confidence(r#""high""#);
        let result = decode_and_validate(&json);
        assert!(result.is_ok(), "repair should convert string label confidence: {:?}", result.err());
        let payload = result.unwrap();
        assert!((payload.confidence.score - 0.9).abs() < f64::EPSILON);
        assert_eq!(payload.confidence.label, "high");
    }

    #[test]
    fn test_repair_confidence_as_null() {
        let json = full_json_with_confidence("null");
        let result = decode_and_validate(&json);
        assert!(result.is_ok(), "repair should replace null confidence: {:?}", result.err());
        let payload = result.unwrap();
        assert!((payload.confidence.score - 0.6).abs() < f64::EPSILON);
        assert_eq!(payload.confidence.label, "medium");
    }

    #[test]
    fn test_repair_confidence_missing_entirely() {
        // JSON without any confidence field
        let json = r#"{
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "Buatkan materi",
            "language": "id",
            "teacher_intent": {"type": "generate_learning_media", "goal": "test", "preferred_delivery_mode": "digital_download", "requires_clarification": false},
            "output_type_candidates": [{"type": "pdf", "score": 0.8, "reason": "test"}],
            "resolved_output_type_reasoning": "test",
            "document_blueprint": {"title": "test", "summary": "test", "sections": [{"title": "s", "purpose": "p", "bullets": ["b"], "estimated_length": "medium"}]},
            "constraints": {"preferred_output_type": "auto"},
            "teacher_delivery_summary": "test"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "repair should inject default confidence: {:?}", result.err());
        let payload = result.unwrap();
        assert!((payload.confidence.score - 0.6).abs() < f64::EPSILON);
        assert_eq!(payload.confidence.label, "medium");
    }

    #[test]
    fn test_repair_missing_document_blueprint() {
        let json = r#"{
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "Buatkan materi pythagoras",
            "language": "id"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "repair should inject default document_blueprint: {:?}", result.err());
        let payload = result.unwrap();
        assert_eq!(payload.document_blueprint.title, "Buatkan materi pythagoras");
        assert_eq!(payload.document_blueprint.sections.len(), 1);
    }

    #[test]
    fn test_repair_document_blueprint_alias() {
        let json = r#"{
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "Buatkan materi",
            "blueprint": {"title": "Alias Title", "summary": "Alias Summary", "sections": [{"title": "s", "purpose": "p", "bullets": ["b"], "estimated_length": "medium"}]}
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "repair should convert blueprint alias: {:?}", result.err());
        let payload = result.unwrap();
        assert_eq!(payload.document_blueprint.title, "Alias Title");
    }

    // ── PLAN MODE tests ──────────────────────────────────────────────────

    #[test]
    fn test_plan_mode_active_when_low_confidence() {
        let json = r#"{
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "Buatkan materi",
            "language": "id",
            "teacher_intent": {"type": "generate_learning_media", "goal": "test", "preferred_delivery_mode": "digital_download", "requires_clarification": false},
            "output_type_candidates": [{"type": "pdf", "score": 0.8, "reason": "test"}],
            "resolved_output_type_reasoning": "test",
            "document_blueprint": {"title": "test", "summary": "test", "sections": [{"title": "s", "purpose": "p", "bullets": ["b"], "estimated_length": "medium"}]},
            "constraints": {"preferred_output_type": "auto"},
            "confidence": {"score": 0.3, "label": "low"},
            "teacher_delivery_summary": "test"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "decode failed: {:?}", result.err());
        let payload = result.unwrap();
        assert!(payload.plan_mode.active, "low confidence should trigger plan mode");
        assert!(payload.plan_mode.reason.is_some());
    }

    #[test]
    fn test_plan_mode_inactive_when_high_confidence() {
        let json = r#"{
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "Buatkan materi pecahan untuk kelas 5 SD format PDF",
            "language": "id",
            "teacher_intent": {"type": "generate_learning_media", "goal": "test", "preferred_delivery_mode": "digital_download", "requires_clarification": false},
            "output_type_candidates": [{"type": "pdf", "score": 0.9, "reason": "test"}],
            "resolved_output_type_reasoning": "test",
            "document_blueprint": {"title": "test", "summary": "test", "sections": [{"title": "s", "purpose": "p", "bullets": ["b"], "estimated_length": "medium"}]},
            "constraints": {"preferred_output_type": "pdf"},
            "confidence": {"score": 0.85, "label": "high"},
            "teacher_delivery_summary": "test"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "decode failed: {:?}", result.err());
        let payload = result.unwrap();
        assert!(!payload.plan_mode.active, "high confidence should not trigger plan mode");
    }

    #[test]
    fn test_plan_mode_active_when_requires_clarification() {
        let json = r#"{
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "Buatkan materi",
            "language": "id",
            "teacher_intent": {"type": "generate_learning_media", "goal": "test", "preferred_delivery_mode": "digital_download", "requires_clarification": true},
            "output_type_candidates": [{"type": "pdf", "score": 0.8, "reason": "test"}],
            "resolved_output_type_reasoning": "test",
            "document_blueprint": {"title": "test", "summary": "test", "sections": [{"title": "s", "purpose": "p", "bullets": ["b"], "estimated_length": "medium"}]},
            "constraints": {"preferred_output_type": "auto"},
            "confidence": {"score": 0.7, "label": "medium"},
            "teacher_delivery_summary": "test"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "decode failed: {:?}", result.err());
        let payload = result.unwrap();
        assert!(payload.plan_mode.active, "requires_clarification=true should trigger plan mode");
    }

    #[test]
    fn test_plan_mode_default_inactive() {
        let json = r#"{
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "Buatkan materi",
            "language": "id",
            "teacher_intent": {"type": "generate_learning_media", "goal": "test", "preferred_delivery_mode": "digital_download", "requires_clarification": false},
            "output_type_candidates": [{"type": "pdf", "score": 0.8, "reason": "test"}],
            "resolved_output_type_reasoning": "test",
            "document_blueprint": {"title": "test", "summary": "test", "sections": [{"title": "s", "purpose": "p", "bullets": ["b"], "estimated_length": "medium"}]},
            "constraints": {"preferred_output_type": "auto"},
            "confidence": {"score": 0.6, "label": "medium"},
            "teacher_delivery_summary": "test"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "decode failed: {:?}", result.err());
        let payload = result.unwrap();
        // Medium confidence (0.6) should NOT trigger plan mode (threshold is 0.5)
        assert!(!payload.plan_mode.active);
    }

    #[test]
    fn test_interpreted_fields_populated() {
        let json = r#"{
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "Buatkan materi pecahan untuk kelas 5 SD",
            "language": "id",
            "teacher_intent": {"type": "generate_learning_media", "goal": "test", "preferred_delivery_mode": "digital_download", "requires_clarification": false},
            "output_type_candidates": [{"type": "pdf", "score": 0.8, "reason": "test"}],
            "resolved_output_type_reasoning": "test",
            "document_blueprint": {"title": "test", "summary": "test", "sections": [{"title": "s", "purpose": "p", "bullets": ["b"], "estimated_length": "medium"}]},
            "constraints": {"preferred_output_type": "auto"},
            "confidence": {"score": 0.8, "label": "high"},
            "teacher_delivery_summary": "test",
            "interpreted_fields": {
                "target_audience": "SD Kelas 5",
                "output_type": null,
                "subject": "Matematika",
                "topic": "pecahan",
                "learning_objectives": ["Memahami konsep pecahan"],
                "page_count": null,
                "difficulty_level": null,
                "include_activities": null,
                "slide_count": null,
                "question_count": null,
                "meeting_duration": null,
                "teaching_method": null,
                "assessment_method": null,
                "visual_density": null,
                "speaker_notes": null,
                "question_type": null
            },
            "missing_fields": [
                {
                    "field_id": "output_type",
                    "field_label": "Format File",
                    "priority": "required",
                    "question": "Format file apa yang Anda inginkan?",
                    "suggestions": [{"value": "pdf", "label": "PDF"}],
                    "input_type": "select"
                }
            ],
            "plan_mode": {
                "active": true,
                "reason": "Output type not specified",
                "detected_content_type": "materi_pembelajaran",
                "content_type_confidence": 0.8
            }
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "decode failed: {:?}", result.err());
        let payload = result.unwrap();
        assert!(payload.plan_mode.active);
        assert_eq!(payload.plan_mode.detected_content_type.as_deref(), Some("materi_pembelajaran"));
        let ifields = payload.interpreted_fields.unwrap();
        assert_eq!(ifields.target_audience.as_deref(), Some("SD Kelas 5"));
        assert_eq!(ifields.topic.as_deref(), Some("pecahan"));
        assert_eq!(payload.missing_fields.len(), 1);
        assert_eq!(payload.missing_fields[0].field_id, "output_type");
    }

    #[test]
    fn test_fallback_has_plan_mode() {
        let p = fallback("Buatkan materi");
        assert!(!p.plan_mode.active);
        assert!(p.missing_fields.is_empty());
    }

    #[test]
    fn test_repair_missing_plan_mode() {
        // JSON without plan_mode should get it auto-generated
        let json = r#"{
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "Buatkan materi",
            "language": "id",
            "teacher_intent": {"type": "generate_learning_media", "goal": "test", "preferred_delivery_mode": "digital_download", "requires_clarification": false},
            "output_type_candidates": [{"type": "pdf", "score": 0.8, "reason": "test"}],
            "resolved_output_type_reasoning": "test",
            "document_blueprint": {"title": "test", "summary": "test", "sections": [{"title": "s", "purpose": "p", "bullets": ["b"], "estimated_length": "medium"}]},
            "constraints": {"preferred_output_type": "auto"},
            "confidence": {"score": 0.3, "label": "low"},
            "teacher_delivery_summary": "test"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "repair should inject plan_mode: {:?}", result.err());
        let payload = result.unwrap();
        assert!(payload.plan_mode.active, "low confidence should trigger plan mode via repair");
    }

    #[test]
    fn test_repair_missing_interpreted_fields() {
        // JSON without interpreted_fields should get them auto-populated
        let json = r#"{
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "Buatkan materi",
            "language": "id",
            "teacher_intent": {"type": "generate_learning_media", "goal": "test", "preferred_delivery_mode": "digital_download", "requires_clarification": false},
            "output_type_candidates": [{"type": "pdf", "score": 0.8, "reason": "test"}],
            "resolved_output_type_reasoning": "test",
            "document_blueprint": {"title": "test", "summary": "test", "sections": [{"title": "s", "purpose": "p", "bullets": ["b"], "estimated_length": "medium"}]},
            "constraints": {"preferred_output_type": "auto"},
            "confidence": {"score": 0.8, "label": "high"},
            "teacher_delivery_summary": "test"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "repair should inject interpreted_fields: {:?}", result.err());
        let payload = result.unwrap();
        assert!(payload.interpreted_fields.is_some());
    }
}

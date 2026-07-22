//! Prompt clarification service.
//!
//! Analyzes a teacher's prompt, detects missing fields based on content
//! standards, and generates clarification questions (max 5) for the
//! conversational UI.
//!
//! Uses rules-based detection first (fast, zero-cost), with optional LLM
//! fallback for edge cases.
//!
//! ## Flow:
//! 1. Detect content type from prompt keywords
//! 2. Get standards for detected content type
//! 3. Auto-detect known fields (target_audience, output_type, subject)
//! 4. Compute gaps (required + recommended fields not auto-detected)
//! 5. Generate clarification questions for gaps
//! 6. Build enriched prompt from detected values + answers
//!
//! ## API contract:
//! - `POST /preflight` → `ClarificationResponse`
//! - `POST /confirm` → submit enriched prompt
//! - `POST /{id}/skip-clarification` → skip all, use enriched prompt

use uuid::Uuid;

use crate::standards::content_standards::{
    detect_content_type, detect_field_value_from_prompt, detect_output_type,
    detect_target_audience, get_clarification_fields, get_minimum_requirements, ContentGap,
    ContentType, FieldDefinition, FieldPriority,
};

// ─── Types ──────────────────────────────────────────────────────────────────

/// The full preflight response returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarificationResponse {
    pub generation_id: String,
    pub detected: DetectedInfo,
    pub gaps: Vec<ContentGap>,
    pub suggested_prompt: String,
    pub is_ready: bool,
    pub total_required_gaps: usize,
    pub total_recommended_gaps: usize,
}

/// Information auto-detected from the prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedInfo {
    pub output_type: Option<String>,
    pub subject: Option<String>,
    pub subject_id: Option<i64>,
    pub audience: Option<String>,
    pub topic: Option<String>,
    pub content_type: String,
    pub confidence: f64,
}

/// Input for the preflight analysis.
#[derive(Debug, Clone)]
pub struct PreflightInput {
    pub raw_prompt: String,
    pub preferred_output_type: Option<String>,
    pub subject_id: Option<i64>,
    pub sub_subject_id: Option<i64>,
}

/// Input for LLM-based preflight (PLAN MODE).
///
/// Uses the LLM interpretation result to compare against minimum requirements
/// and generate clarification questions for missing fields.
#[derive(Debug, Clone)]
pub struct PreflightWithInterpretationInput {
    /// The raw teacher prompt.
    pub raw_prompt: String,
    /// The detected content type from LLM interpretation.
    pub detected_content_type: Option<String>,
    /// Fields that the LLM was able to interpret.
    pub interpreted_fields: serde_json::Value,
    /// Confidence score from the LLM (0.0 - 1.0).
    pub confidence_score: Option<f64>,
    /// Whether the LLM already flagged that clarification is needed.
    pub llm_requires_clarification: Option<bool>,
    /// Preferred output type from the user selection.
    pub preferred_output_type: Option<String>,
    /// Subject ID if provided.
    pub subject_id: Option<i64>,
    /// Sub-subject ID if provided.
    pub sub_subject_id: Option<i64>,
}

/// Input for the confirm (submit enriched prompt).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmInput {
    pub generation_id: String,
    pub enriched_prompt: String,
    pub answers: std::collections::HashMap<String, String>,
    pub subject_id: Option<i64>,
    pub sub_subject_id: Option<i64>,
}

/// Result of the confirm operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmResult {
    pub generation_id: String,
    pub job_id: String,
    pub status: String,
    pub poll_url: String,
}

// ─── ClarificationService ────────────────────────────────────────────────

/// Service for prompt preflight analysis and clarification.
///
/// Stateless — all state lives in the request/response cycle.
/// The frontend manages conversation state via Riverpod (in-memory).
pub struct ClarificationService;

impl ClarificationService {
    /// Analyze a raw prompt and return clarification questions.
    ///
    /// This is the core preflight logic:
    /// 1. Detect content type from keywords
    /// 2. Auto-detect fields (audience, output type, subject)
    /// 3. Compute gaps against content standards
    /// 4. Build suggested enriched prompt
    pub fn preflight(input: PreflightInput) -> ClarificationResponse {
        let generation_id = Uuid::new_v4().to_string();

        // Step 1: Detect content type
        let (content_type, confidence) = detect_content_type(&input.raw_prompt);

        // Step 2: Auto-detect known fields
        let detected_audience = detect_target_audience(&input.raw_prompt);
        let detected_output_type = input
            .preferred_output_type
            .clone()
            .filter(|s| !s.is_empty() && s != "auto")
            .or_else(|| detect_output_type(&input.raw_prompt));

        // Step 3: Get content standards and compute gaps
        let standards = get_clarification_fields(&content_type);
        let gaps = compute_gaps(
            &standards,
            &input.raw_prompt,
            detected_audience.as_deref(),
            detected_output_type.as_deref(),
            input.subject_id,
        );

        // Step 4: Count required vs recommended gaps
        let total_required_gaps = gaps
            .iter()
            .filter(|g| g.priority == FieldPriority::Required.as_str())
            .count();
        let total_recommended_gaps = gaps
            .iter()
            .filter(|g| g.priority == FieldPriority::Recommended.as_str())
            .count();

        // Step 5: Build suggested enriched prompt
        let suggested_prompt = build_suggested_prompt(
            &input.raw_prompt,
            detected_audience.as_deref(),
            detected_output_type.as_deref(),
            &content_type,
        );

        // Step 6: Determine if ready (no required gaps)
        let is_ready = total_required_gaps == 0;

        let detected = DetectedInfo {
            output_type: detected_output_type,
            subject: None, // Subject detection via taxonomy is done at the API handler level
            subject_id: input.subject_id,
            audience: detected_audience,
            topic: extract_topic(&input.raw_prompt),
            content_type: content_type.as_str().to_string(),
            confidence,
        };

        ClarificationResponse {
            generation_id,
            detected,
            gaps,
            suggested_prompt,
            is_ready,
            total_required_gaps,
            total_recommended_gaps,
        }
    }

    /// Analyze an LLM interpretation result against minimum requirements.
    ///
    /// This is the core PLAN MODE logic:
    /// 1. Parse detected content type from LLM interpretation
    /// 2. Get minimum requirements for that content type
    /// 3. Compare interpreted fields against requirements
    /// 4. Generate clarification questions for missing fields
    /// 5. Build suggested enriched prompt
    pub fn preflight_with_interpretation(
        input: PreflightWithInterpretationInput,
    ) -> ClarificationResponse {
        let generation_id = Uuid::new_v4().to_string();

        // Step 1: Determine content type from LLM interpretation
        let (content_type, confidence) = if let Some(ref ct_str) = input.detected_content_type {
            let ct = match ct_str.as_str() {
                "materi_pembelajaran" => ContentType::MateriPembelajaran,
                "slide_presentasi" => ContentType::SlidePresentasi,
                "rpp" => ContentType::Rpp,
                "lembar_kerja" => ContentType::LembarKerja,
                "silabus" => ContentType::Silabus,
                "penilaian" => ContentType::Penilaian,
                _ => detect_content_type(&input.raw_prompt).0,
            };
            let conf = input.confidence_score.unwrap_or(0.6);
            (ct, conf)
        } else {
            detect_content_type(&input.raw_prompt)
        };

        // Step 2: Get minimum requirements for this content type
        let requirements = get_minimum_requirements(&content_type);

        // Step 3: Compare interpreted fields against requirements
        let gaps = compute_gaps_from_interpretation(
            &requirements.required_fields,
            &requirements.recommended_fields,
            &input.interpreted_fields,
            &input.raw_prompt,
        );

        // Step 4: Count gaps
        let total_required_gaps = gaps
            .iter()
            .filter(|g| g.priority == FieldPriority::Required.as_str())
            .count();
        let total_recommended_gaps = gaps
            .iter()
            .filter(|g| g.priority == FieldPriority::Recommended.as_str())
            .count();

        // Step 5: Auto-detect additional fields from raw prompt
        let detected_audience = input
            .interpreted_fields
            .get("target_audience")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| detect_target_audience(&input.raw_prompt));
        let detected_output_type = input
            .preferred_output_type
            .clone()
            .filter(|s| !s.is_empty() && s != "auto")
            .or_else(|| {
                input
                    .interpreted_fields
                    .get("output_type")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty() && *s != "auto")
                    .map(|s| s.to_string())
            })
            .or_else(|| detect_output_type(&input.raw_prompt));

        // Step 6: Build suggested enriched prompt
        let suggested_prompt = build_suggested_prompt(
            &input.raw_prompt,
            detected_audience.as_deref(),
            detected_output_type.as_deref(),
            &content_type,
        );

        // Step 7: Determine if ready (no required gaps)
        let is_ready = total_required_gaps == 0;

        let topic = input
            .interpreted_fields
            .get("topic")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| extract_topic(&input.raw_prompt));

        let detected = DetectedInfo {
            output_type: detected_output_type,
            subject: None,
            subject_id: input.subject_id,
            audience: detected_audience,
            topic,
            content_type: content_type.as_str().to_string(),
            confidence,
        };

        ClarificationResponse {
            generation_id,
            detected,
            gaps,
            suggested_prompt,
            is_ready,
            total_required_gaps,
            total_recommended_gaps,
        }
    }

    /// Build an enriched prompt from the original prompt + user answers.
    ///
    /// Called by the confirm handler after the teacher answers clarification
    /// questions or clicks "Generate dengan Prompt Ini".
    pub fn enrich_prompt(
        raw_prompt: &str,
        answers: &std::collections::HashMap<String, String>,
    ) -> String {
        let raw_prompt_trimmed = raw_prompt.trim();
        let mut parts = vec![raw_prompt_trimmed.to_string()];
        let h_lower = raw_prompt_trimmed.to_lowercase();

        // Helper: check if haystack contains all tokens of the needle (case-insensitive)
        let contains_all_tokens = |haystack: &str, needle: &str| -> bool {
            let needle_clean = needle.replace('_', " ").to_lowercase();
            let tokens: Vec<&str> = needle_clean.split_whitespace().collect();
            if tokens.is_empty() {
                return false;
            }
            tokens.into_iter().all(|t| haystack.contains(t))
        };

        // Helper: check if prompt already specifies quantity (number + unit synonyms)
        let prompt_has_quantity = |haystack: &str, unit_synonyms: &[&str]| -> bool {
            let has_digit = haystack.chars().any(|c| c.is_ascii_digit());
            if !has_digit {
                return false;
            }
            unit_synonyms.iter().any(|&syn| haystack.contains(&syn.to_lowercase()))
        };

        // 1. Append audience if answered and not already in prompt
        if let Some(audience) = answers.get("target_audience") {
            let audience_clean = audience.replace('_', " ");
            if !contains_all_tokens(&h_lower, &audience_clean) {
                parts.push(format!("untuk jenjang {}", audience_clean));
            }
        }

        // 2. Append output type if answered and not already in prompt
        if let Some(output) = answers.get("output_type") {
            let output_upper = output.to_uppercase();
            let has_format = match output.to_lowercase().as_str() {
                "pptx" => ["ppt", "pptx", "powerpoint", "slide", "slides", "presentasi", "presentation", "slideshow"].iter().any(|&s| h_lower.contains(s)),
                "pdf" => ["pdf", "dokumen pdf"].iter().any(|&s| h_lower.contains(s)),
                "docx" => ["docx", "doc", "word", "dokumen word", "lembar kerja"].iter().any(|&s| h_lower.contains(s)),
                _ => h_lower.contains(&output.to_lowercase()),
            };
            if !has_format {
                parts.push(format!("dalam format {}", output_upper));
            }
        }

        // 3. Append page count if answered and not already in prompt
        if let Some(pages) = answers.get("page_count") {
            let label = match pages.as_str() {
                "short" => "2-3 halaman",
                "medium" => "5-7 halaman",
                "long" => "10+ halaman",
                _ => pages,
            };
            let has_page_qty = prompt_has_quantity(&h_lower, &["halaman", "hal", "page", "pages", "pg", "pgs"]);
            if !has_page_qty {
                parts.push(format!("sebanyak {}", label));
            }
        }

        // 4. Append slide count if answered and not already in prompt
        if let Some(slides) = answers.get("slide_count") {
            let label = match slides.as_str() {
                "short" => "8-10 slide",
                "medium" => "15-20 slide",
                "long" => "25+ slide",
                _ => slides,
            };
            let has_slide_qty = prompt_has_quantity(&h_lower, &["slide", "slides", "halaman", "hal", "ppt", "pptx", "deck", "presentasi"]);
            if !has_slide_qty {
                parts.push(format!("sebanyak {}", label));
            }
        }

        // 5. Append learning objectives if answered and not already in prompt
        if let Some(objectives) = answers.get("learning_objectives") {
            let objectives_trimmed = objectives.trim();
            if !objectives_trimmed.is_empty() {
                let obj_lower = objectives_trimmed.to_lowercase();
                if !h_lower.contains(&obj_lower) {
                    parts.push(format!("dengan tujuan pembelajaran: {}", objectives_trimmed));
                }
            }
        }

        // 6. Append difficulty level if answered and not already in prompt
        if let Some(difficulty) = answers.get("difficulty_level") {
            let diff_lower = difficulty.to_lowercase();
            if !h_lower.contains(&diff_lower) {
                parts.push(format!("tingkat kesulitan {}", difficulty));
            }
        }

        // 7. Append question count if answered and not already in prompt
        if let Some(count) = answers.get("question_count") {
            let has_q_qty = prompt_has_quantity(&h_lower, &["soal", "pertanyaan", "kuis", "quiz", "question", "questions"]);
            if !has_q_qty {
                parts.push(format!("dengan {} soal", count));
            }
        }

        // 8. Append teaching method if answered and not already in prompt
        if let Some(method) = answers.get("teaching_method") {
            let method_clean = method.replace('_', " ");
            let method_lower = method_clean.to_lowercase();
            if !h_lower.contains(&method_lower) {
                parts.push(format!("dengan metode {}", method_clean));
            }
        }

        // 9. Append include activities if answered and not already in prompt
        if let Some(activities) = answers.get("include_activities") {
            if activities == "yes" {
                let has_activities = ["latihan", "soal", "kuis", "aktivitas", "activity", "exercise", "quiz"].iter().any(|&s| h_lower.contains(s));
                if !has_activities {
                    parts.push("sertakan latihan/soal".to_string());
                }
            }
        }

        // 10. Append slide-by-slide contents if answered
        let mut slide_contents = Vec::new();
        for i in 1..=20 {
            let key = format!("slide_{}", i);
            if let Some(content) = answers.get(&key) {
                if !content.trim().is_empty() {
                    slide_contents.push(format!("Slide {}: {}", i, content));
                }
            } else {
                let key_content = format!("slide_{}_content", i);
                if let Some(content) = answers.get(&key_content) {
                    if !content.trim().is_empty() {
                        slide_contents.push(format!("Slide {}: {}", i, content));
                    }
                }
            }
        }

        if !slide_contents.is_empty() {
            parts.push(format!("Rincian isi per slide:\n{}", slide_contents.join("\n")));
        }

        // Join parts together nicely
        let mut enriched = parts[0].clone();
        if parts.len() > 1 {
            let has_slide_details = !slide_contents.is_empty();
            let other_parts_count = parts.len() - if has_slide_details { 2 } else { 1 };
            
            if other_parts_count > 0 {
                enriched.push_str(", ");
                let comma_parts: Vec<String> = parts[1..=other_parts_count].to_vec();
                enriched.push_str(&comma_parts.join(", "));
            }
            
            if has_slide_details {
                enriched.push_str(". ");
                enriched.push_str(&parts[parts.len() - 1]);
            }
        }

        // Clean up double commas, spaces, or trailing punctuation issues
        enriched
            .replace(", ,", ",")
            .replace(",.", ".")
            .replace("..", ".")
    }
}

// ─── Internal helpers ────────────────────────────────────────────────────

/// Check if a field value in JSON is non-null, non-empty, and not default 'auto'.
fn is_field_present_in_json(value: Option<&serde_json::Value>) -> bool {
    match value {
        None | Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::String(s)) => {
            let trimmed = s.trim();
            !trimmed.is_empty() && trimmed != "auto"
        }
        Some(serde_json::Value::Array(arr)) => !arr.is_empty(),
        Some(serde_json::Value::Object(obj)) => !obj.is_empty(),
        Some(serde_json::Value::Bool(_)) => true,
        Some(serde_json::Value::Number(_)) => true,
    }
}

/// Compute gaps: fields that need clarification.
fn compute_gaps(
    standards: &[FieldDefinition],
    raw_prompt: &str,
    detected_audience: Option<&str>,
    detected_output_type: Option<&str>,
    subject_id: Option<i64>,
) -> Vec<ContentGap> {
    let mut gaps = Vec::new();

    for field in standards {
        // Skip if already auto-detected
        match field.field_id {
            "target_audience" if detected_audience.is_some() => continue,
            "output_type" if detected_output_type.is_some() => continue,
            "subject" if subject_id.is_some() => continue,
            _ => {}
        }

        // Skip if field keyword/value is present in raw prompt
        if detect_field_value_from_prompt(field.field_id, raw_prompt).is_some() {
            continue;
        }

        // Build question based on field type
        let question = build_question(field);

        gaps.push(ContentGap {
            field_id: field.field_id.to_string(),
            question,
            priority: field.priority.as_str().to_string(),
            input_type: field.input_type.as_str().to_string(),
            suggestions: field.suggestions.clone(),
            detected_value: None,
        });
    }

    gaps
}

/// Compute gaps by comparing interpreted fields against minimum requirements.
///
/// This is the core PLAN MODE comparison logic. It checks each required and
/// recommended field from the content standards against what the LLM was able
/// to extract from the teacher's prompt or what was detected via keywords in prompt.
fn compute_gaps_from_interpretation(
    required_fields: &[crate::standards::content_standards::MinimumRequirementField],
    recommended_fields: &[crate::standards::content_standards::MinimumRequirementField],
    interpreted: &serde_json::Value,
    raw_prompt: &str,
) -> Vec<ContentGap> {
    let mut gaps = Vec::new();

    // Check required fields
    for field in required_fields {
        let value = interpreted.get(&field.field_id);
        let is_present = is_field_present_in_json(value);

        if !is_present {
            // Skip if field can be detected from prompt keywords (e.g. "5 halaman", "ppt/pptx/pdf/docx", grade, etc.)
            if detect_field_value_from_prompt(&field.field_id, raw_prompt).is_some() {
                continue;
            }

            gaps.push(ContentGap {
                field_id: field.field_id.clone(),
                question: build_question_from_requirement(field),
                priority: "required".to_string(),
                input_type: field.input_type.clone(),
                suggestions: field.suggestions.clone(),
                detected_value: None,
            });
        }
    }

    // Check recommended fields (up to 5 max total gaps)
    let recommended_limit = 5usize.saturating_sub(gaps.len());
    for field in recommended_fields.iter().take(recommended_limit) {
        let value = interpreted.get(&field.field_id);
        let is_present = is_field_present_in_json(value);

        if !is_present {
            // Skip if field can be detected from prompt keywords
            if detect_field_value_from_prompt(&field.field_id, raw_prompt).is_some() {
                continue;
            }

            gaps.push(ContentGap {
                field_id: field.field_id.clone(),
                question: build_question_from_requirement(field),
                priority: "recommended".to_string(),
                input_type: field.input_type.clone(),
                suggestions: field.suggestions.clone(),
                detected_value: None,
            });
        }
    }

    gaps
}

/// Build a natural language question for a minimum requirement field.
fn build_question_from_requirement(
    field: &crate::standards::content_standards::MinimumRequirementField,
) -> String {
    match field.field_id.as_str() {
        "target_audience" => "Untuk jenjang/kelas berapa materi ini ditujukan?".to_string(),
        "output_type" => "Format file apa yang Anda inginkan? (PDF, Word, PowerPoint)".to_string(),
        "learning_objectives" => "Apa tujuan pembelajaran dari konten ini?".to_string(),
        "page_count" => "Berapa jumlah halaman yang diinginkan?".to_string(),
        "slide_count" => "Berapa jumlah slide yang diinginkan?".to_string(),
        "include_activities" => "Apakah perlu disertakan latihan/soal?".to_string(),
        "meeting_duration" => "Berapa lama durasi pertemuan?".to_string(),
        "teaching_method" => "Metode pembelajaran apa yang digunakan?".to_string(),
        "assessment_method" => "Bagaimana cara penilaian siswa?".to_string(),
        "difficulty_level" => "Tingkat kesulitan materi ini?".to_string(),
        "question_count" => "Berapa jumlah soal yang diinginkan?".to_string(),
        "visual_density" => "Bagaimana tampilan slide yang diinginkan?".to_string(),
        "speaker_notes" => "Apakah perlu disertakan catatan presenter?".to_string(),
        "question_type" => "Jenis soal apa yang diinginkan?".to_string(),
        _ => format!("Informasi tambahan untuk {}?", field.field_label),
    }
}

/// Build a natural language question for a field.
fn build_question(field: &FieldDefinition) -> String {
    match field.field_id {
        "target_audience" => "Untuk jenjang/kelas berapa?".to_string(),
        "output_type" => "Format file apa yang Anda inginkan?".to_string(),
        "page_count" => "Berapa jumlah halaman yang diinginkan?".to_string(),
        "slide_count" => "Berapa jumlah slide yang diinginkan?".to_string(),
        "learning_objectives" => "Apa tujuan pembelajaran dari konten ini?".to_string(),
        "include_activities" => "Apakah perlu disertakan latihan/soal?".to_string(),
        "meeting_duration" => "Berapa lama durasi pertemuan?".to_string(),
        "teaching_method" => "Metode pembelajaran apa yang digunakan?".to_string(),
        "assessment_method" => "Bagaimana cara penilaian siswa?".to_string(),
        "difficulty_level" => "Tingkat kesulitan materi ini?".to_string(),
        "question_count" => "Berapa jumlah soal yang diinginkan?".to_string(),
        "visual_density" => "Bagaimana tampilan slide yang diinginkan?".to_string(),
        "speaker_notes" => "Apakah perlu disertakan catatan presenter?".to_string(),
        _ => format!("Informasi tambahan untuk {}?", field.label_id),
    }
}

/// Build a suggested prompt from detected values.
fn build_suggested_prompt(
    raw_prompt: &str,
    audience: Option<&str>,
    output_type: Option<&str>,
    _content_type: &ContentType,
) -> String {
    let raw = raw_prompt.trim();
    let mut result = raw.to_string();

    let lower = raw.to_lowercase();

    if let Some(audience) = audience {
        let audience_lower = audience.replace('_', " ").to_lowercase();
        let audience_mentioned = audience_lower.split_whitespace().all(|t| lower.contains(t));
        if !audience_mentioned {
            result.push_str(&format!(" untuk {}", audience));
        }
    }

    if let Some(output) = output_type {
        let format_mention: &[&str] = match output.to_lowercase().as_str() {
            "pptx" => &["ppt", "pptx", "powerpoint", "slide", "presentasi", "slideshow"],
            "pdf" => &["pdf"],
            "docx" => &["docx", "doc", "word", "dokumen"],
            _ => &[],
        };
        let format_mentioned = format_mention.iter().any(|&s| lower.contains(s));
        if !format_mentioned {
            result.push_str(&format!(" dalam format {}", output.to_uppercase()));
        }
    }

    // Capitalize first letter and ensure proper punctuation
    let mut chars = result.chars();
    match chars.next() {
        None => return String::new(),
        Some(first) => {
            let mut output = first.to_uppercase().to_string();
            output.extend(chars);
            if !output.ends_with('.') && !output.ends_with('?') && !output.ends_with('!') {
                output.push('.');
            }
            output
        }
    }
}

/// Extract a topic hint from the prompt.
pub fn extract_topic(prompt: &str) -> Option<String> {
    let lower = prompt.to_lowercase();

    // Common patterns: "tentang X", "about X", "materi X"
    let markers = ["tentang ", "about ", "materi ", "topik ", "topic "];
    for marker in &markers {
        if let Some(pos) = lower.find(marker) {
            let start = pos + marker.len();
            let rest = &prompt[start..];
            // Take until the next punctuation or end
            let end = rest
                .find(|c: char| c == '.' || c == ',' || c == '!' || c == '?')
                .unwrap_or(rest.len());
            let topic = rest[..end].trim().to_string();
            if !topic.is_empty() && topic.len() > 2 {
                return Some(topic);
            }
        }
    }

    None
}

// ─── Re-exports for serde ────────────────────────────────────────────────

use serde::{Deserialize, Serialize};

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::standards::content_standards::InputType;
    use std::collections::HashMap;

    #[test]
    fn test_preflight_basic() {
        let input = PreflightInput {
            raw_prompt: "Buatkan materi pecahan untuk kelas 5 SD".to_string(),
            preferred_output_type: None,
            subject_id: None,
            sub_subject_id: None,
        };
        let response = ClarificationService::preflight(input);

        assert!(!response.generation_id.is_empty());
        assert_eq!(response.detected.content_type, "materi_pembelajaran");
        assert_eq!(
            response.detected.audience,
            Some("SD Kelas 5".to_string())
        );
    }

    #[test]
    fn test_preflight_with_output_type() {
        let input = PreflightInput {
            raw_prompt: "Buatkan slide presentasi tentang gaya".to_string(),
            preferred_output_type: Some("pptx".to_string()),
            subject_id: None,
            sub_subject_id: None,
        };
        let response = ClarificationService::preflight(input);

        assert_eq!(response.detected.content_type, "slide_presentasi");
        assert_eq!(
            response.detected.output_type,
            Some("pptx".to_string())
        );
    }

    #[test]
    fn test_preflight_ready_when_all_required_detected() {
        let input = PreflightInput {
            raw_prompt: "Buatkan materi pecahan untuk kelas 5 SD format PDF".to_string(),
            preferred_output_type: Some("pdf".to_string()),
            subject_id: Some(1),
            sub_subject_id: None,
        };
        let response = ClarificationService::preflight(input);

        // All required fields detected → is_ready
        assert!(response.is_ready);
        assert_eq!(response.total_required_gaps, 0);
    }

    #[test]
    fn test_preflight_auto_output_type_requires_clarification() {
        let input = PreflightInput {
            raw_prompt: "Buatkan materi pecahan untuk kelas 5 SD".to_string(),
            preferred_output_type: Some("auto".to_string()),
            subject_id: None,
            sub_subject_id: None,
        };
        let response = ClarificationService::preflight(input);

        // 'auto' does not satisfy output_type requirement, so is_ready must be false
        assert!(!response.is_ready);
        assert_eq!(response.total_required_gaps, 1);
        assert!(response.gaps.iter().any(|g| g.field_id == "output_type" && g.priority == "required"));
    }

    #[test]
    fn test_preflight_not_ready_when_audience_missing() {
        let input = PreflightInput {
            raw_prompt: "Buatkan materi tentang pecahan".to_string(),
            preferred_output_type: None,
            subject_id: None,
            sub_subject_id: None,
        };
        let response = ClarificationService::preflight(input);

        assert!(!response.is_ready);
        assert!(response.total_required_gaps > 0);
    }

    #[test]
    fn test_enrich_prompt_basic() {
        let mut answers = HashMap::new();
        answers.insert("target_audience".to_string(), "SD_Kelas_5".to_string());
        answers.insert("output_type".to_string(), "pdf".to_string());

        let enriched = ClarificationService::enrich_prompt(
            "Buatkan materi pecahan",
            &answers,
        );

        assert!(enriched.contains("SD Kelas 5"));
        assert!(enriched.contains("PDF"));
    }

    #[test]
    fn test_enrich_prompt_empty_answers() {
        let answers = HashMap::new();
        let enriched = ClarificationService::enrich_prompt(
            "Buatkan materi pecahan",
            &answers,
        );

        assert_eq!(enriched, "Buatkan materi pecahan");
    }

    #[test]
    fn test_enrich_prompt_deduplication() {
        let mut answers = HashMap::new();
        answers.insert("target_audience".to_string(), "SD_Kelas_5".to_string());
        answers.insert("output_type".to_string(), "pptx".to_string());
        answers.insert("slide_count".to_string(), "medium".to_string());

        // The prompt already contains "ppt" and "5 slide" (digit + slide synonym) and "kelas 5 sd"
        let enriched = ClarificationService::enrich_prompt(
            "Buatkan aku 5 slide ppt materi pecahan untuk kelas 5 sd",
            &answers,
        );

        // It should NOT append output_type ("pptx" / "PPTX") or slide_count ("sebanyak 15-20 slide") or target_audience ("SD Kelas 5")
        assert_eq!(enriched, "Buatkan aku 5 slide ppt materi pecahan untuk kelas 5 sd");
    }

    #[test]
    fn test_extract_topic() {
        assert_eq!(
            extract_topic("Buatkan materi tentang pecahan"),
            Some("pecahan".to_string())
        );
        assert_eq!(
            extract_topic("Buatkan presentasi about gaya tarik-menarik"),
            Some("gaya tarik-menarik".to_string())
        );
        // "materi " marker matches → extracts "belajar"
        assert_eq!(
            extract_topic("Buatkan materi belajar"),
            Some("belajar".to_string())
        );
        assert_eq!(
            extract_topic("Hello world foo bar"),
            None
        );
    }

    #[test]
    fn test_build_question() {
        let field = FieldDefinition {
            field_id: "target_audience",
            label_id: "Jenjang/Kelas",
            label_en: "Grade Level",
            input_type: InputType::Select,
            priority: FieldPriority::Required,
            suggestions: vec![],
        };
        let question = build_question(&field);
        assert!(question.contains("jenjang"));
    }

    #[test]
    fn test_compute_gaps_excludes_detected() {
        let standards = get_clarification_fields(&ContentType::MateriPembelajaran);
        let gaps = compute_gaps(&standards, "test", Some("SD Kelas 5"), Some("pdf"), Some(1));

        // target_audience and output_type should be excluded (detected)
        let field_ids: Vec<&str> = gaps.iter().map(|g| g.field_id.as_str()).collect();
        assert!(!field_ids.contains(&"target_audience"));
        assert!(!field_ids.contains(&"output_type"));
    }

    #[test]
    fn test_suggested_prompt_generation() {
        let prompt = build_suggested_prompt(
            "Buatkan materi pecahan",
            Some("SD Kelas 5"),
            Some("pdf"),
            &ContentType::MateriPembelajaran,
        );
        assert!(prompt.starts_with('B'));
        assert!(prompt.contains("SD Kelas 5"));
        assert!(prompt.contains("PDF"));
        assert!(prompt.ends_with('.'));
    }

    #[test]
    fn test_detected_info_serialization() {
        let info = DetectedInfo {
            output_type: Some("pdf".to_string()),
            subject: None,
            subject_id: None,
            audience: Some("SD Kelas 5".to_string()),
            topic: Some("pecahan".to_string()),
            content_type: "materi_pembelajaran".to_string(),
            confidence: 0.8,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("materi_pembelajaran"));
        assert!(json.contains("SD Kelas 5"));
    }

    #[test]
    fn test_clarification_response_serialization() {
        let input = PreflightInput {
            raw_prompt: "Buatkan materi".to_string(),
            preferred_output_type: None,
            subject_id: None,
            sub_subject_id: None,
        };
        let response = ClarificationService::preflight(input);
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("generation_id"));
        assert!(json.contains("detected"));
        assert!(json.contains("gaps"));
        assert!(json.contains("is_ready"));
    }

    // ── PLAN MODE tests ──────────────────────────────────────────────────

    #[test]
    fn test_preflight_with_interpretation_vague_prompt() {
        let input = PreflightWithInterpretationInput {
            raw_prompt: "Buatkan materi".to_string(),
            detected_content_type: Some("materi_pembelajaran".to_string()),
            interpreted_fields: serde_json::json!({
                "target_audience": null,
                "output_type": null,
                "topic": null,
            }),
            confidence_score: Some(0.3),
            llm_requires_clarification: Some(true),
            preferred_output_type: None,
            subject_id: None,
            sub_subject_id: None,
        };
        let response = ClarificationService::preflight_with_interpretation(input);

        assert!(!response.generation_id.is_empty());
        assert_eq!(response.detected.content_type, "materi_pembelajaran");
        assert!(!response.is_ready);
        assert!(response.total_required_gaps > 0);
        // Should have gaps for target_audience and output_type
        let field_ids: Vec<&str> = response.gaps.iter().map(|g| g.field_id.as_str()).collect();
        assert!(field_ids.contains(&"target_audience"));
        assert!(field_ids.contains(&"output_type"));
    }

    #[test]
    fn test_preflight_with_interpretation_complete_prompt() {
        let input = PreflightWithInterpretationInput {
            raw_prompt: "Buatkan materi pecahan untuk kelas 5 SD format PDF".to_string(),
            detected_content_type: Some("materi_pembelajaran".to_string()),
            interpreted_fields: serde_json::json!({
                "target_audience": "SD Kelas 5",
                "output_type": "pdf",
                "topic": "pecahan",
                "learning_objectives": ["Memahami konsep pecahan"],
            }),
            confidence_score: Some(0.85),
            llm_requires_clarification: Some(false),
            preferred_output_type: Some("pdf".to_string()),
            subject_id: Some(1),
            sub_subject_id: None,
        };
        let response = ClarificationService::preflight_with_interpretation(input);

        assert!(response.is_ready);
        assert_eq!(response.total_required_gaps, 0);
    }

    #[test]
    fn test_preflight_with_interpretation_slide() {
        let input = PreflightWithInterpretationInput {
            raw_prompt: "Buatkan slide presentasi".to_string(),
            detected_content_type: Some("slide_presentasi".to_string()),
            interpreted_fields: serde_json::json!({
                "target_audience": null,
                "output_type": "pptx",
            }),
            confidence_score: Some(0.5),
            llm_requires_clarification: Some(true),
            preferred_output_type: Some("pptx".to_string()),
            subject_id: None,
            sub_subject_id: None,
        };
        let response = ClarificationService::preflight_with_interpretation(input);

        assert!(!response.is_ready);
        assert_eq!(response.detected.content_type, "slide_presentasi");
        // output_type should be detected from interpreted_fields
        let field_ids: Vec<&str> = response.gaps.iter().map(|g| g.field_id.as_str()).collect();
        assert!(field_ids.contains(&"target_audience"));
        assert!(!field_ids.contains(&"output_type")); // already detected
    }

    #[test]
    fn test_preflight_with_interpretation_auto_detect_from_prompt() {
        let input = PreflightWithInterpretationInput {
            raw_prompt: "Buatkan materi untuk kelas 7 SMP".to_string(),
            detected_content_type: Some("materi_pembelajaran".to_string()),
            interpreted_fields: serde_json::json!({
                "target_audience": null,  // LLM didn't detect it
                "output_type": null,
            }),
            confidence_score: Some(0.4),
            llm_requires_clarification: Some(true),
            preferred_output_type: None,
            subject_id: None,
            sub_subject_id: None,
        };
        let response = ClarificationService::preflight_with_interpretation(input);

        // target_audience should be auto-detected from "kelas 7 SMP" in prompt
        assert_eq!(
            response.detected.audience,
            Some("SMP Kelas 7".to_string())
        );
    }

    #[test]
    fn test_compute_gaps_from_interpretation_required_only() {
        use crate::standards::content_standards::get_minimum_requirements;

        let reqs = get_minimum_requirements(&ContentType::MateriPembelajaran);
        let interpreted = serde_json::json!({
            "target_audience": "SD Kelas 5",
            "output_type": null,
        });

        let gaps = super::compute_gaps_from_interpretation(
            &reqs.required_fields,
            &reqs.recommended_fields,
            &interpreted,
            "Buatkan materi",
        );

        // output_type is missing, target_audience is present
        let required_gaps: Vec<&str> = gaps
            .iter()
            .filter(|g| g.priority == "required")
            .map(|g| g.field_id.as_str())
            .collect();
        assert!(required_gaps.contains(&"output_type"));
        assert!(!required_gaps.contains(&"target_audience"));
    }

    #[test]
    fn test_compute_gaps_from_interpretation_all_present() {
        use crate::standards::content_standards::get_minimum_requirements;

        let reqs = get_minimum_requirements(&ContentType::MateriPembelajaran);
        let interpreted = serde_json::json!({
            "target_audience": "SD Kelas 5",
            "output_type": "pdf",
        });

        let gaps = super::compute_gaps_from_interpretation(
            &reqs.required_fields,
            &reqs.recommended_fields,
            &interpreted,
            "Buatkan materi untuk kelas 5 SD format PDF",
        );

        // No required gaps
        let required_gaps: Vec<&str> = gaps
            .iter()
            .filter(|g| g.priority == "required")
            .map(|g| g.field_id.as_str())
            .collect();
        assert!(required_gaps.is_empty());
    }

    #[test]
    fn test_build_question_from_requirement() {
        use crate::standards::content_standards::MinimumRequirementField;

        let field = MinimumRequirementField {
            field_id: "target_audience".to_string(),
            field_label: "Jenjang/Kelas".to_string(),
            priority: "required".to_string(),
            input_type: "select".to_string(),
            description: "Test".to_string(),
            suggestions: vec![],
        };
        let question = super::build_question_from_requirement(&field);
        assert!(question.contains("jenjang"));
    }

    #[test]
    fn test_preflight_skips_gaps_when_keywords_in_prompt() {
        // Prompt already contains "5 halaman", "ppt", and "kelas 5 SD"
        let input = PreflightWithInterpretationInput {
            raw_prompt: "Buatkan 5 halaman materi pecahan untuk kelas 5 SD format ppt".to_string(),
            detected_content_type: Some("materi_pembelajaran".to_string()),
            interpreted_fields: serde_json::json!({
                "target_audience": null,
                "output_type": null,
                "page_count": null,
            }),
            confidence_score: Some(0.7),
            llm_requires_clarification: Some(true),
            preferred_output_type: None,
            subject_id: None,
            sub_subject_id: None,
        };

        let response = ClarificationService::preflight_with_interpretation(input);

        // Required gaps (target_audience, output_type) and page_count should be skipped!
        let field_ids: Vec<&str> = response.gaps.iter().map(|g| g.field_id.as_str()).collect();
        assert!(!field_ids.contains(&"target_audience"));
        assert!(!field_ids.contains(&"output_type"));
        assert!(!field_ids.contains(&"page_count"));
        assert!(response.is_ready);
    }
}

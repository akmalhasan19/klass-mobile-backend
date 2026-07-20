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
    detect_content_type, detect_output_type, detect_target_audience, get_clarification_fields,
    ContentGap, ContentType, FieldDefinition, FieldPriority,
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

    /// Build an enriched prompt from the original prompt + user answers.
    ///
    /// Called by the confirm handler after the teacher answers clarification
    /// questions or clicks "Generate dengan Prompt Ini".
    pub fn enrich_prompt(
        raw_prompt: &str,
        answers: &std::collections::HashMap<String, String>,
    ) -> String {
        let mut parts = vec![raw_prompt.trim().to_string()];

        // Append audience if answered
        if let Some(audience) = answers.get("target_audience") {
            parts.push(format!("Untuk jenjang {}", audience.replace('_', " ")));
        }

        // Append output type if answered
        if let Some(output) = answers.get("output_type") {
            parts.push(format!("dalam format {}", output.to_uppercase()));
        }

        // Append page count if answered
        if let Some(pages) = answers.get("page_count") {
            let label = match pages.as_str() {
                "short" => "2-3 halaman",
                "medium" => "5-7 halaman",
                "long" => "10+ halaman",
                _ => pages,
            };
            parts.push(format!("sebanyak {} halaman", label));
        }

        // Append slide count if answered
        if let Some(slides) = answers.get("slide_count") {
            let label = match slides.as_str() {
                "short" => "8-10 slide",
                "medium" => "15-20 slide",
                "long" => "25+ slide",
                _ => slides,
            };
            parts.push(format!("sebanyak {} slide", label));
        }

        // Append learning objectives if answered
        if let Some(objectives) = answers.get("learning_objectives") {
            if !objectives.trim().is_empty() {
                parts.push(format!("dengan tujuan pembelajaran: {}", objectives));
            }
        }

        // Append difficulty level if answered
        if let Some(difficulty) = answers.get("difficulty_level") {
            parts.push(format!("tingkat kesulitan {}", difficulty));
        }

        // Append question count if answered
        if let Some(count) = answers.get("question_count") {
            parts.push(format!("dengan {} soal", count));
        }

        // Append teaching method if answered
        if let Some(method) = answers.get("teaching_method") {
            parts.push(format!("dengan metode {}", method.replace('_', " ")));
        }

        // Append include activities if answered
        if let Some(activities) = answers.get("include_activities") {
            if activities == "yes" {
                parts.push("sertakan latihan/soal".to_string());
            }
        }

        // Join with ", " for natural language flow
        let enriched = parts.join(", ");
        // Clean up double commas or trailing punctuation issues
        enriched
            .replace(", ,", ",")
            .replace(",.", ".")
            .replace("..", ".")
    }
}

// ─── Internal helpers ────────────────────────────────────────────────────

/// Compute gaps: fields that need clarification.
fn compute_gaps(
    standards: &[FieldDefinition],
    _raw_prompt: &str,
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
    let mut parts = vec![raw_prompt.trim().to_string()];

    if let Some(audience) = audience {
        parts.push(format!("untuk {}", audience));
    }

    if let Some(output) = output_type {
        parts.push(format!("format {}", output.to_uppercase()));
    }

    let result = parts.join(", ");
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
fn extract_topic(prompt: &str) -> Option<String> {
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
}

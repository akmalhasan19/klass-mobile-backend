//! Media generation output type decision service.
//!
//! Port of `MediaGenerationDecisionService` from Laravel.
//!
//! Determines the output format (pdf/docx/pptx) for a media generation based on:
//!
//! - **Teacher override**: If the teacher explicitly requested a format, use it.
//! - **Interpretation constraint**: If the LLM interpretation constrained the output, use it.
//! - **Candidate ranking**: Score each format based on LLM candidate scores + keyword signals,
//!   then pick the highest. Ties are broken by deterministic priority: pdf → docx → pptx.
//!
//! Keyword signals scan the interpretation decision haystack (teacher prompt, goal,
//! reasoning, blueprint, etc.) for format-related keywords:
//! - `pptx`: slide/slides/deck/presentasi/presentation/... → weight 0.35
//! - `pdf`: handout/printable/print/cetak/pdf/booklet → weight 0.25
//! - `docx`: editable/edit/docx/word/worksheet/lembar kerja/template → weight 0.25
//! - `pptx` (bonus): high visual density + assets → weight 0.12

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::orchestrator::submission::normalize_preferred_output_type;

// ─── Constants ──────────────────────────────────────────────────────────────

/// Schema version for the decision payload.
pub const VERSION: &str = "media_output_decision.v1";

/// Allowed output formats (matching `MediaPromptInterpretationSchema::allowedOutputFormats()`).
const ALLOWED_FORMATS: &[&str] = &["docx", "pdf", "pptx"];

/// Deterministic tie-breaking priority: lower = higher priority.
/// pdf=0, docx=1, pptx=2  →  PDF wins ties.
fn type_priority(format: &str) -> i32 {
    match format {
        "pdf" => 0,
        "docx" => 1,
        "pptx" => 2,
        _ => i32::MAX,
    }
}

/// Keyword signal definitions for format detection.
const KEYWORD_SIGNALS: &[KeywordSignalDef] = &[
    KeywordSignalDef {
        output_type: "pptx",
        weight: 0.35,
        reason_code: "slide_intent_detected",
        keywords: &["slide", "slides", "deck", "presentasi", "presentation", "slideshow", "ppt", "pptx"],
    },
    KeywordSignalDef {
        output_type: "pdf",
        weight: 0.25,
        reason_code: "printable_intent_detected",
        keywords: &["handout", "printable", "print", "cetak", "pdf", "booklet"],
    },
    KeywordSignalDef {
        output_type: "docx",
        weight: 0.25,
        reason_code: "editable_document_intent_detected",
        keywords: &["editable", "edit", "docx", "word", "worksheet", "lembar kerja", "template"],
    },
];

// ─── Types ──────────────────────────────────────────────────────────────────

/// Definition of a keyword signal.
struct KeywordSignalDef {
    output_type: &'static str,
    weight: f64,
    reason_code: &'static str,
    keywords: &'static [&'static str],
}

/// A matched keyword signal.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MatchedSignal {
    /// The output type this signal applies to (pdf, docx, pptx).
    #[serde(rename = "type")]
    pub output_type: String,
    pub reason_code: String,
    pub weight: f64,
    pub matched_keyword: Option<String>,
    pub reason: String,
}

/// A ranked format candidate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RankedCandidate {
    #[serde(rename = "type")]
    pub output_type: String,
    pub score: f64,
    pub candidate_score: f64,
    pub reason_code: String,
    pub matched_signals: Vec<MatchedSignal>,
    pub reasons: Vec<String>,
}

/// Input for the `decide` function.
#[derive(Debug, Clone)]
pub struct DecideInput {
    /// The validated interpretation payload (from `MediaPromptInterpretationSchema`).
    pub interpretation: Value,
    /// The teacher's preferred output type (or None/"auto" if no preference).
    pub preferred_output_type: Option<String>,
}

/// The output of the `decide` function.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DecisionOutput {
    pub schema_version: String,
    pub preferred_output_type: String,
    pub constraint_preferred_output_type: String,
    pub resolved_output_type: String,
    pub decision_source: String,
    pub reason_code: String,
    pub reasoning: String,
    pub ranked_candidates: Vec<RankedCandidate>,
    pub tie_breaker_applied: bool,
    pub resolved_at: String,
}

/// Error type for decision operations.
#[derive(Debug, thiserror::Error)]
pub enum DecisionError {
    /// Missing or invalid interpretation payload.
    #[error("interpretation payload is required: {0}")]
    MissingInterpretation(String),

    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// UUID parse error.
    #[error("invalid UUID: {0}")]
    InvalidUuid(String),
}

// ─── Decision Service ───────────────────────────────────────────────────────

/// Service for making output type decisions.
pub struct DecisionService {
    pool: PgPool,
}

impl DecisionService {
    /// Create a new decision service.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Resolve the output type for a generation and persist the decision.
    ///
    /// 1. Validates that the interpretation payload exists.
    /// 2. Calls `decide()` to determine the output type.
    /// 3. Builds a generation spec payload (from interpretation or draft).
    /// 4. Persists `resolved_output_type`, `decision_payload`, `generation_spec_payload`.
    ///
    /// The content draft step is skipped when `draft_payload` is `None`
    /// (matching Laravel's optional `ContentDraftingService`).
    pub async fn resolve(
        &self,
        generation_id: &str,
        interpretation: Value,
        preferred_output_type: Option<&str>,
        draft_payload: Option<Value>,
    ) -> Result<Value, DecisionError> {
        let gen_id = Uuid::parse_str(generation_id)
            .map_err(|e| DecisionError::InvalidUuid(e.to_string()))?;

        // Validate interpretation exists
        if interpretation.is_null() || !interpretation.is_object() {
            return Err(DecisionError::MissingInterpretation(
                "Interpretation payload must exist before resolving the output type.".to_string(),
            ));
        }

        // Run decide
        let input = DecideInput {
            interpretation: interpretation.clone(),
            preferred_output_type: preferred_output_type.map(|s| s.to_string()),
        };
        let decision = decide(input);

        // Build generation spec (from draft or from interpretation alone)
        let generation_spec = build_generation_spec(
            &interpretation,
            &decision.resolved_output_type,
            draft_payload.as_ref(),
        );

        // Build the full decision payload with content draft metadata
        let draft_meta = if let Some(draft) = draft_payload {
            serde_json::json!({
                "content_draft": {
                    "source": draft.get("source").cloned().unwrap_or(Value::String("interpretation_only".to_string())),
                    "schema_version": draft.pointer("/payload/schema_version").cloned(),
                    "adapter_provider": draft.pointer("/adapter_metadata/provider").cloned(),
                    "adapter_model": draft.pointer("/adapter_metadata/model").cloned(),
                    "adapter_primary_provider": draft.pointer("/adapter_metadata/primary_provider").cloned(),
                    "adapter_fallback_used": draft.pointer("/adapter_metadata/fallback_used").cloned().unwrap_or(Value::Bool(false)),
                    "adapter_fallback_reason": draft.pointer("/adapter_metadata/fallback_reason").cloned(),
                    "draft_fallback_triggered": draft.pointer("/payload/fallback/triggered").cloned().unwrap_or(Value::Bool(false)),
                    "draft_fallback_reason_code": draft.pointer("/payload/fallback/reason_code").cloned(),
                }
            })
        } else {
            serde_json::json!({
                "content_draft": {
                    "source": "interpretation_only",
                    "schema_version": Value::Null,
                    "adapter_provider": Value::Null,
                    "adapter_model": Value::Null,
                    "adapter_primary_provider": Value::Null,
                    "adapter_fallback_used": false,
                    "adapter_fallback_reason": Value::Null,
                    "draft_fallback_triggered": false,
                    "draft_fallback_reason_code": Value::Null,
                }
            })
        };

        let mut decision_value = serde_json::to_value(&decision)
            .unwrap_or_else(|_| serde_json::json!({}));

        // Merge content_draft metadata into decision payload
        if let Some(obj) = decision_value.as_object_mut() {
            if let Some(draft_obj) = draft_meta.as_object() {
                obj.insert("content_draft".to_string(), draft_obj["content_draft"].clone());
            }
        }

        // Persist to DB
        sqlx::query(
            r#"
            UPDATE media_generations
            SET resolved_output_type = $1,
                decision_payload = $2,
                generation_spec_payload = $3,
                error_code = NULL,
                error_message = NULL,
                updated_at = NOW()
            WHERE id = $4
            "#,
        )
        .bind(&decision.resolved_output_type)
        .bind(&decision_value)
        .bind(&generation_spec)
        .bind(gen_id)
        .execute(&self.pool)
        .await
        .map_err(DecisionError::Database)?;

        Ok(decision_value)
    }
}

// ─── decide — pure decision logic ───────────────────────────────────────────

/// Determine the output type for a media generation based on interpretation
/// and teacher preference.
///
/// Decision priority:
/// 1. **Teacher override** — if teacher explicitly set a non-auto output type.
/// 2. **Interpretation constraint** — if the LLM constrained the output type.
/// 3. **Candidate ranking** — score each format via LLM scores + keyword signals.
///
/// Returns a `DecisionOutput` with the resolved type, source metadata, and
/// reasoning.
pub fn decide(input: DecideInput) -> DecisionOutput {
    let interpretation = &input.interpretation;
    let preferred_output_type = input.preferred_output_type.as_deref();

    let normalized_preferred = normalize_preferred_output_type(preferred_output_type);
    let constraint_raw = interpretation
        .pointer("/constraints/preferred_output_type")
        .and_then(|v| v.as_str());
    let constraint_preferred = normalize_preferred_output_type(constraint_raw);

    let ranked_candidates = rank_candidates(interpretation);

    // Priority 1: Teacher override
    if normalized_preferred != "auto" {
        return build_decision(
            &normalized_preferred,
            &normalized_preferred,
            "teacher_override",
            "teacher_override",
            &format!(
                "Teacher override selected {}, so automatic classification was bypassed.",
                normalized_preferred.to_uppercase()
            ),
            &constraint_preferred,
            &ranked_candidates,
            false,
        );
    }

    // Priority 2: Interpretation constraint
    if constraint_preferred != "auto" {
        return build_decision(
            &normalized_preferred,
            &constraint_preferred,
            "interpretation_constraint",
            "interpretation_constraint",
            &format!(
                "Interpretation payload explicitly constrained the output to {}.",
                constraint_preferred.to_uppercase()
            ),
            &constraint_preferred,
            &ranked_candidates,
            false,
        );
    }

    // Priority 3: Candidate ranking
    let selected = &ranked_candidates[0];
    let runner_up = ranked_candidates.get(1);

    let tie_breaker_applied = runner_up.map_or(false, |r| {
        (selected.score - r.score).abs() < 0.0001
    });

    let reasoning = build_ranking_reasoning(
        selected,
        runner_up,
        tie_breaker_applied,
        interpretation
            .pointer("/resolved_output_type_reasoning")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
    );

    build_decision(
        &normalized_preferred,
        &selected.output_type,
        "candidate_ranking",
        &selected.reason_code,
        &reasoning,
        &constraint_preferred,
        &ranked_candidates,
        tie_breaker_applied,
    )
}

// ─── Private helpers ────────────────────────────────────────────────────────

/// Build the decision output from resolved values.
fn build_decision(
    preferred_output_type: &str,
    resolved_output_type: &str,
    decision_source: &str,
    reason_code: &str,
    reasoning: &str,
    constraint_preferred_output_type: &str,
    ranked_candidates: &[RankedCandidate],
    tie_breaker_applied: bool,
) -> DecisionOutput {
    DecisionOutput {
        schema_version: VERSION.to_string(),
        preferred_output_type: preferred_output_type.to_string(),
        constraint_preferred_output_type: constraint_preferred_output_type.to_string(),
        resolved_output_type: resolved_output_type.to_string(),
        decision_source: decision_source.to_string(),
        reason_code: reason_code.to_string(),
        reasoning: reasoning.to_string(),
        ranked_candidates: ranked_candidates.to_vec(),
        tie_breaker_applied,
        resolved_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    }
}

/// Rank format candidates by LLM scores + keyword signals.
fn rank_candidates(interpretation: &Value) -> Vec<RankedCandidate> {
    // Initialize scores for each format
    let mut scores: std::collections::HashMap<&str, RankedCandidate> = std::collections::HashMap::new();

    for fmt in ALLOWED_FORMATS {
        scores.insert(
            fmt,
            RankedCandidate {
                output_type: fmt.to_string(),
                score: 0.0,
                candidate_score: 0.0,
                reason_code: "highest_candidate_score".to_string(),
                matched_signals: vec![],
                reasons: vec![],
            },
        );
    }

    // Add LLM candidate scores
    if let Some(candidates) = interpretation.get("output_type_candidates").and_then(|c| c.as_array()) {
        for candidate in candidates {
            let ctype = candidate.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let cscore = candidate.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let reason = candidate
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let rounded_score = (cscore * 10000.0).round() / 10000.0;

            if let Some(entry) = scores.get_mut(ctype) {
                entry.score += rounded_score;
                entry.candidate_score = rounded_score;
                entry.reasons.push(format!(
                    "LLM candidate score {:.4}: {}",
                    rounded_score, reason
                ));
            }
        }
    }

    // Add keyword signals
    let signals = keyword_signals(interpretation);
    for signal in &signals {
        if let Some(entry) = scores.get_mut(signal.output_type.as_str()) {
            entry.score += signal.weight;
            entry.reason_code = signal.reason_code.clone();
            entry.matched_signals.push(MatchedSignal {
                output_type: signal.output_type.clone(),
                reason_code: signal.reason_code.clone(),
                weight: signal.weight,
                matched_keyword: signal.matched_keyword.clone(),
                reason: signal.reason.clone(),
            });
            entry.reasons.push(signal.reason.clone());
        }
    }

    // Convert to sorted vec
    let mut ranked: Vec<RankedCandidate> = scores.into_values().collect();

    ranked.sort_by(|a, b| {
        let score_diff = (a.score - b.score).abs();
        if score_diff >= 0.0001 {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        } else {
            // Tie: use deterministic priority
            let priority_a = type_priority(&a.output_type);
            let priority_b = type_priority(&b.output_type);
            priority_a.cmp(&priority_b)
        }
    });

    // Round scores
    for candidate in &mut ranked {
        candidate.score = (candidate.score * 10000.0).round() / 10000.0;
    }

    ranked
}

/// Detect keyword signals for format types from the interpretation.
fn keyword_signals(interpretation: &Value) -> Vec<MatchedSignal> {
    let haystack = decision_haystack(interpretation);
    let haystack_lower = haystack.to_lowercase();
    let mut signals = Vec::new();

    for signal_def in KEYWORD_SIGNALS {
        for keyword in signal_def.keywords {
            if haystack_lower.contains(keyword) {
                signals.push(MatchedSignal {
                    output_type: signal_def.output_type.to_string(),
                    reason_code: signal_def.reason_code.to_string(),
                    weight: signal_def.weight,
                    matched_keyword: Some(keyword.to_string()),
                    reason: format!(
                        "Keyword \"{}\" indicates a {} format.",
                        keyword,
                        format_type_label(signal_def.output_type)
                    ),
                });
                break; // Only first match per signal definition
            }
        }
    }

    // Bonus signal: high visual density + assets → pptx
    let visual_density = interpretation
        .pointer("/requested_media_characteristics/visual_density")
        .and_then(|v| v.as_str());
    let has_assets = interpretation
        .get("assets")
        .and_then(|a| a.as_array())
        .map_or(false, |a| !a.is_empty());

    if visual_density == Some("high") && has_assets {
        signals.push(MatchedSignal {
            output_type: "pptx".to_string(),
            reason_code: "visual_density_signal".to_string(),
            weight: 0.12,
            matched_keyword: None,
            reason: "High visual density with explicit assets favors slide-oriented output.".to_string(),
        });
    }

    signals
}

/// Build a lowercase search haystack from interpretation fields.
fn decision_haystack(interpretation: &Value) -> String {
    let mut segments: Vec<String> = Vec::new();

    if let Some(v) = interpretation.get("teacher_prompt").and_then(|v| v.as_str()) {
        if !v.trim().is_empty() {
            segments.push(v.to_lowercase());
        }
    }
    if let Some(v) = interpretation
        .pointer("/teacher_intent/goal")
        .and_then(|v| v.as_str())
    {
        if !v.trim().is_empty() {
            segments.push(v.to_lowercase());
        }
    }
    if let Some(v) = interpretation
        .get("resolved_output_type_reasoning")
        .and_then(|v| v.as_str())
    {
        if !v.trim().is_empty() {
            segments.push(v.to_lowercase());
        }
    }
    if let Some(v) = interpretation
        .pointer("/document_blueprint/title")
        .and_then(|v| v.as_str())
    {
        if !v.trim().is_empty() {
            segments.push(v.to_lowercase());
        }
    }
    if let Some(v) = interpretation
        .pointer("/document_blueprint/summary")
        .and_then(|v| v.as_str())
    {
        if !v.trim().is_empty() {
            segments.push(v.to_lowercase());
        }
    }
    // Section texts
    if let Some(sections) = interpretation
        .pointer("/document_blueprint/sections")
        .and_then(|s| s.as_array())
    {
        for section in sections {
            if let Some(t) = section.get("title").and_then(|v| v.as_str()) {
                segments.push(t.to_lowercase());
            }
            if let Some(p) = section.get("purpose").and_then(|v| v.as_str()) {
                segments.push(p.to_lowercase());
            }
            if let Some(bullets) = section.get("bullets").and_then(|b| b.as_array()) {
                for bullet in bullets {
                    if let Some(b) = bullet.as_str() {
                        segments.push(b.to_lowercase());
                    }
                }
            }
        }
    }
    // Format preferences
    if let Some(prefs) = interpretation
        .pointer("/requested_media_characteristics/format_preferences")
        .and_then(|p| p.as_array())
    {
        for pref in prefs {
            if let Some(p) = pref.as_str() {
                segments.push(p.to_lowercase());
            }
        }
    }

    segments.join(" ")
}

/// Build human-readable ranking reasoning.
fn build_ranking_reasoning(
    selected: &RankedCandidate,
    runner_up: Option<&RankedCandidate>,
    tie_breaker_applied: bool,
    interpretation_reasoning: &str,
) -> String {
    let mut reasoning = format!(
        "Auto resolution selected {} with score {:.4}. {}",
        selected.output_type.to_uppercase(),
        selected.score,
        selected.reasons.iter().take(2).cloned().collect::<Vec<_>>().join(" "),
    );

    if let Some(runner) = runner_up {
        reasoning.push_str(&format!(
            " Runner-up was {} at score {:.4}.",
            runner.output_type.to_uppercase(),
            runner.score,
        ));
    }

    if tie_breaker_applied {
        reasoning.push_str(
            " Scores tied, so the deterministic priority order PDF > DOCX > PPTX was applied.",
        );
    }

    if !interpretation_reasoning.trim().is_empty() {
        reasoning.push(' ');
        reasoning.push_str(interpretation_reasoning.trim());
    }

    reasoning.trim().to_string()
}

/// Get a human-readable label for a format type.
fn format_type_label(format_type: &str) -> &'static str {
    match format_type {
        "pptx" => "slide deck or presentation",
        "pdf" => "stable printable document",
        "docx" => "editable document",
        _ => "document",
    }
}

/// Normalize emphasis/estimated_length to a value the Python renderer accepts.
///
/// Python `Section.emphasis` is `Literal["short", "medium", "long"]`.
/// The LLM may return arbitrary strings like "long_text", "brief", etc.
fn normalize_emphasis(raw: Option<&str>) -> &'static str {
    match raw.unwrap_or("medium") {
        "short" | "brief" | "concise" | "minimal" => "short",
        "long" | "long_text" | "detailed" | "extensive" | "comprehensive" => "long",
        _ => "medium",
    }
}

/// Normalize assessment_or_activity_block type to a value the Python renderer accepts.
///
/// Python `AssessmentBlock.type` is
/// `Literal["assessment", "activity", "reflection", "quiz", "assignment"]`.
fn normalize_assessment_type(raw: &str) -> &'static str {
    match raw.to_lowercase().as_str() {
        "assessment" | "test" | "exam" | "evaluation" => "assessment",
        "activity" | "exercise" | "practice" | "task" => "activity",
        "reflection" | "reflect" | "thinking" => "reflection",
        "quiz" | "question" | "mcq" => "quiz",
        "assignment" | "homework" | "worksheet" => "assignment",
        _ => "activity",
    }
}

/// Normalize body block type to a value the Python renderer accepts.
///
/// Python `BodyBlock.type` is `Literal["paragraph", "bullet", "checklist", "note"]`.
fn normalize_body_block_type(raw: &str) -> &'static str {
    match raw.to_lowercase().as_str() {
        "paragraph" | "text" | "p" => "paragraph",
        "bullet" | "bullets" | "list" | "ul" => "bullet",
        "checklist" | "check" | "checkbox" => "checklist",
        "note" | "callout" | "info" => "note",
        _ => "paragraph",
    }
}

/// Ensure a string field is non-null and non-empty, providing a fallback.
fn require_str(val: Option<&str>, fallback: &str) -> String {
    val.unwrap_or(fallback).to_string()
}

/// Build a generation spec payload (for the Python renderer).
///
/// When a content draft is available, uses draft sections.
/// Otherwise builds from the interpretation blueprint directly.
///
/// All fields are normalized to match the Python `GenerationSpec` strict model:
/// - Required strings are never null/empty
/// - Enum fields use allowed values
/// - Sections always have ≥1 body_block with valid type
/// - Section counts match layout_hints
fn build_generation_spec(
    interpretation: &Value,
    resolved_output_type: &str,
    draft_payload: Option<&Value>,
) -> Value {
    let export_format = resolved_output_type;
    let document_mode = if export_format == "pptx" {
        "slide_deck"
    } else {
        "document"
    };
    let unit_type = if export_format == "pptx" { "slide" } else { "page" };

    // ── Extract safe string helpers ───────────────────────────────────────
    let teacher_prompt = interpretation
        .get("teacher_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("Learning material");

    let raw_title = interpretation
        .pointer("/document_blueprint/title")
        .and_then(|v| v.as_str());
    let raw_summary = interpretation
        .pointer("/document_blueprint/summary")
        .and_then(|v| v.as_str());
    let raw_delivery = interpretation
        .get("teacher_delivery_summary")
        .and_then(|v| v.as_str());
    let raw_language = interpretation
        .get("language")
        .and_then(|v| v.as_str());

    let title = require_str(raw_title, teacher_prompt);
    let summary = require_str(raw_summary, teacher_prompt);
    let teacher_delivery_summary = require_str(raw_delivery, teacher_prompt);
    let language = require_str(raw_language, "id");
    let source_schema_version = interpretation
        .get("schema_version")
        .and_then(|v| v.as_str())
        .unwrap_or("media_prompt_understanding.v1");

    // ── Build sections ────────────────────────────────────────────────────
    let sections: Vec<Value> = if let Some(draft) = draft_payload {
        // Use draft content sections
        if let Some(payload) = draft.get("payload") {
            if let Some(draft_sections) = payload.get("sections").and_then(|s| s.as_array()) {
                let built: Vec<Value> = draft_sections
                    .iter()
                    .map(|s| {
                        let s_title = s.get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Untitled Section");
                        let s_purpose = s.get("purpose")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Section content");
                        let raw_emphasis = s.get("emphasis")
                            .and_then(|v| v.as_str());

                        // Ensure body_blocks is non-empty and each block is valid
                        let body_blocks: Vec<Value> = s.get("body_blocks")
                            .and_then(|b| b.as_array())
                            .map(|blocks| {
                                let normalized: Vec<Value> = blocks
                                    .iter()
                                    .filter_map(|b| {
                                        let btype = b.get("type")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("paragraph");
                                        let content = b.get("content")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("");
                                        if content.is_empty() {
                                            None
                                        } else {
                                            Some(serde_json::json!({
                                                "type": normalize_body_block_type(btype),
                                                "content": content,
                                            }))
                                        }
                                    })
                                    .collect();
                                if normalized.is_empty() {
                                    vec![serde_json::json!({
                                        "type": "paragraph",
                                        "content": s_purpose,
                                    })]
                                } else {
                                    normalized
                                }
                            })
                            .unwrap_or_else(|| {
                                vec![serde_json::json!({
                                    "type": "paragraph",
                                    "content": s_purpose,
                                })]
                            });

                        serde_json::json!({
                            "title": s_title,
                            "purpose": s_purpose,
                            "body_blocks": body_blocks,
                            "emphasis": normalize_emphasis(raw_emphasis),
                        })
                    })
                    .collect();
                if built.is_empty() {
                    vec![default_section(teacher_prompt)]
                } else {
                    built
                }
            } else {
                vec![default_section(teacher_prompt)]
            }
        } else {
            vec![default_section(teacher_prompt)]
        }
    } else {
        // Build sections from interpretation blueprint
        interpretation
            .pointer("/document_blueprint/sections")
            .and_then(|s| s.as_array())
            .map(|bp_sections| {
                let built: Vec<Value> = bp_sections
                    .iter()
                    .map(|s| {
                        let s_title = s.get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Untitled Section");
                        let s_purpose = s.get("purpose")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Section content");
                        let raw_emphasis = s.get("estimated_length")
                            .and_then(|v| v.as_str());

                        let body_blocks: Vec<Value> = s
                            .get("bullets")
                            .and_then(|b| b.as_array())
                            .map(|bullets| {
                                let normalized: Vec<Value> = bullets
                                    .iter()
                                    .filter_map(|bullet| {
                                        let content = bullet.as_str().unwrap_or("");
                                        if content.is_empty() {
                                            None
                                        } else {
                                            Some(serde_json::json!({
                                                "type": "bullet",
                                                "content": content,
                                            }))
                                        }
                                    })
                                    .collect();
                                if normalized.is_empty() {
                                    vec![serde_json::json!({
                                        "type": "paragraph",
                                        "content": s_purpose,
                                    })]
                                } else {
                                    normalized
                                }
                            })
                            .unwrap_or_else(|| {
                                vec![serde_json::json!({
                                    "type": "paragraph",
                                    "content": s_purpose,
                                })]
                            });

                        serde_json::json!({
                            "title": s_title,
                            "purpose": s_purpose,
                            "body_blocks": body_blocks,
                            "emphasis": normalize_emphasis(raw_emphasis),
                        })
                    })
                    .collect();
                if built.is_empty() {
                    vec![default_section(teacher_prompt)]
                } else {
                    built
                }
            })
            .unwrap_or_else(|| vec![default_section(teacher_prompt)])
    };

    // ── Assessment blocks — normalize type ─────────────────────────────────
    let assessment_blocks: Vec<Value> = interpretation
        .get("assessment_or_activity_blocks")
        .and_then(|a| a.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| {
                    let a_title = b.get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Activity");
                    let raw_type = b.get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("activity");
                    let instructions = b.get("instructions")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Complete the activity.");

                    if instructions.is_empty() {
                        None
                    } else {
                        Some(serde_json::json!({
                            "title": a_title,
                            "type": normalize_assessment_type(raw_type),
                            "instructions": instructions,
                        }))
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let style_tone = interpretation
        .pointer("/requested_media_characteristics/tone")
        .and_then(|v| v.as_str())
        .or_else(|| {
            interpretation
                .pointer("/constraints/tone")
                .and_then(|v| v.as_str())
        })
        .unwrap_or("clear_and_structured");

    let mut format_prefs: Vec<Value> = interpretation
        .pointer("/requested_media_characteristics/format_preferences")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();

    if format_prefs.is_empty() {
        format_prefs.push(Value::String(export_format.to_string()));
    }

    // ── Assets — normalize type field ─────────────────────────────────────
    let assets: Vec<Value> = interpretation
        .get("assets")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    let a_type = a.get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("text");
                    let description = a.get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Asset");
                    let required = a.get("required")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let normalized_type = match a_type {
                        "text" | "image" | "table" | "chart" | "diagram" | "reference" => a_type,
                        _ => "text",
                    };
                    Some(serde_json::json!({
                        "type": normalized_type,
                        "description": description,
                        "required": required,
                    }))
                })
                .collect()
        })
        .unwrap_or_default();

    serde_json::json!({
        "schema_version": "media_generation_spec.v1",
        "source_interpretation_schema_version": source_schema_version,
        "export_format": export_format,
        "title": title,
        "language": language,
        "summary": summary,
        "learning_objectives": interpretation.get("learning_objectives").cloned().unwrap_or(Value::Array(vec![])),
        "sections": sections,
        "layout_hints": {
            "document_mode": document_mode,
            "visual_density": interpretation.pointer("/requested_media_characteristics/visual_density").and_then(|v| v.as_str()).unwrap_or("medium"),
            "section_count": sections.len() as i64,
            "asset_count": assets.len() as i64,
            "assessment_block_count": assessment_blocks.len() as i64,
        },
        "style_hints": {
            "tone": style_tone,
            "audience_level": interpretation.pointer("/target_audience/level").and_then(|v| v.as_str()).unwrap_or("general"),
            "format_preferences": format_prefs,
        },
        "page_or_slide_structure": {
            "unit_type": unit_type,
            "total_units": 1 + sections.len() as i64 + if assessment_blocks.is_empty() { 0 } else { 1 },
            "opening_unit": true,
            "section_units": sections.len() as i64,
            "closing_unit": !assessment_blocks.is_empty(),
        },
        "content_context": {
            "subject_context": interpretation.get("subject_context"),
            "sub_subject_context": interpretation.get("sub_subject_context"),
            "target_audience": interpretation.get("target_audience"),
        },
        "content_integrity": {
            "integrity_score": 1.0,
            "violations": [],
            "classification_source": "fallback",
            "metadata": { "synthetic": true },
        },
        "assets": assets,
        "assessment_or_activity_blocks": assessment_blocks,
        "teacher_delivery_summary": teacher_delivery_summary,
        "contract_versions": {
            "generator_output_metadata": "media_generator_output_metadata.v1"
        }
    })
}

/// Build a default section from the teacher prompt (used when no sections exist).
fn default_section(teacher_prompt: &str) -> Value {
    // Safely truncate to 200 chars (handles multi-byte UTF-8)
    let char_count = teacher_prompt.chars().count();
    let truncated: String = if char_count > 200 {
        let s: String = teacher_prompt.chars().take(197).collect();
        format!("{}...", s)
    } else {
        teacher_prompt.to_string()
    };
    serde_json::json!({
        "title": truncated,
        "purpose": "Deliver the requested learning material.",
        "body_blocks": [{
            "type": "paragraph",
            "content": teacher_prompt,
        }],
        "emphasis": "medium",
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_interpretation() -> Value {
        serde_json::json!({
            "schema_version": "media_prompt_understanding.v1",
            "teacher_prompt": "Buatkan materi aljabar untuk kelas 8 dengan contoh singkat.",
            "language": "id",
            "teacher_intent": {
                "type": "generate_learning_media",
                "goal": "Create a classroom resource.",
                "preferred_delivery_mode": "digital_download",
                "requires_clarification": false
            },
            "learning_objectives": ["Siswa memahami konsep dasar aljabar."],
            "constraints": {
                "preferred_output_type": "auto",
                "max_duration_minutes": 45,
                "must_include": ["contoh soal"],
                "avoid": ["istilah terlalu teknis"],
                "tone": "supportive"
            },
            "output_type_candidates": [
                { "type": "pdf", "score": 0.78, "reason": "Format printable paling cocok." },
                { "type": "docx", "score": 0.61, "reason": "Masih cocok jika guru ingin mengedit." },
                { "type": "pptx", "score": 0.22, "reason": "Slide deck tidak menjadi kebutuhan utama." }
            ],
            "resolved_output_type_reasoning": "Sesuai untuk materi yang ingin tampil konsisten.",
            "document_blueprint": {
                "title": "Materi Aljabar Kelas 8",
                "summary": "Ringkasan singkat aljabar dasar dengan latihan cepat.",
                "sections": [{
                    "title": "Konsep Dasar",
                    "purpose": "Memperkenalkan istilah inti aljabar.",
                    "bullets": ["Pengertian variabel", "Contoh ekspresi sederhana"],
                    "estimated_length": "short"
                }]
            },
            "subject_context": { "subject_name": "Matematika", "subject_slug": "mathematics" },
            "sub_subject_context": { "sub_subject_name": "Aljabar", "sub_subject_slug": "algebra" },
            "target_audience": { "label": "Siswa kelas 8", "level": "middle_school", "age_range": "13-14" },
            "requested_media_characteristics": {
                "tone": "supportive",
                "format_preferences": [],
                "visual_density": "medium"
            },
            "assets": [],
            "assessment_or_activity_blocks": [],
            "teacher_delivery_summary": "Gunakan materi ini untuk pembuka sebelum latihan mandiri.",
            "confidence": { "score": 0.88, "label": "high", "rationale": "Prompt cukup jelas." },
            "fallback": { "triggered": false, "reason_code": null, "action": null }
        })
    }

    // ── decide — teacher override ──────────────────────────────────────────

    #[test]
    fn test_teacher_override_prioritizes_over_ranking() {
        let input = DecideInput {
            interpretation: sample_interpretation(),
            preferred_output_type: Some("pptx".to_string()),
        };
        let decision = decide(input);
        assert_eq!(decision.resolved_output_type, "pptx");
        assert_eq!(decision.decision_source, "teacher_override");
        assert_eq!(decision.reason_code, "teacher_override");
        assert!(!decision.tie_breaker_applied);
    }

    #[test]
    fn test_teacher_override_normalizes_case() {
        let input = DecideInput {
            interpretation: sample_interpretation(),
            preferred_output_type: Some("PDF".to_string()),
        };
        let decision = decide(input);
        assert_eq!(decision.resolved_output_type, "pdf");
        assert_eq!(decision.decision_source, "teacher_override");
    }

    #[test]
    fn test_teacher_override_auto_falls_through_to_ranking() {
        let input = DecideInput {
            interpretation: sample_interpretation(),
            preferred_output_type: Some("auto".to_string()),
        };
        let decision = decide(input);
        // Should use candidate ranking (pdf wins with 0.78)
        assert_eq!(decision.resolved_output_type, "pdf");
        assert_eq!(decision.decision_source, "candidate_ranking");
    }

    // ── decide — interpretation constraint ─────────────────────────────────

    #[test]
    fn test_interpretation_constraint_used_when_no_teacher_override() {
        let mut interp = sample_interpretation();
        interp["constraints"]["preferred_output_type"] = Value::String("docx".to_string());

        let input = DecideInput {
            interpretation: interp,
            preferred_output_type: Some("auto".to_string()),
        };
        let decision = decide(input);
        assert_eq!(decision.resolved_output_type, "docx");
        assert_eq!(decision.decision_source, "interpretation_constraint");
    }

    // ── decide — candidate ranking ─────────────────────────────────────────

    #[test]
    fn test_candidate_ranking_selects_highest_score() {
        let input = DecideInput {
            interpretation: sample_interpretation(),
            preferred_output_type: None,
        };
        let decision = decide(input);
        // pdf has 0.78, docx has 0.61, pptx has 0.22
        assert_eq!(decision.resolved_output_type, "pdf");
        assert_eq!(decision.decision_source, "candidate_ranking");
    }

    #[test]
    fn test_tie_breaker_applies_deterministic_priority() {
        let mut interp = sample_interpretation();
        interp["teacher_prompt"] = Value::String(
            "Buatkan media pembelajaran kelas 8 dengan ringkasan dan latihan singkat.".to_string(),
        );
        interp["teacher_intent"]["goal"] =
            Value::String("Create a short classroom resource.".to_string());
        interp["resolved_output_type_reasoning"] =
            Value::String("Kedua format tertinggi sama-sama layak.".to_string());
        interp["requested_media_characteristics"]["format_preferences"] = Value::Array(vec![]);
        interp["document_blueprint"]["title"] = Value::String("Materi Kelas 8".to_string());
        interp["document_blueprint"]["summary"] =
            Value::String("Ringkasan singkat untuk pembuka materi kelas.".to_string());
        interp["teacher_delivery_summary"] =
            Value::String("Gunakan materi ini untuk pembuka dan latihan kelas.".to_string());
        interp["output_type_candidates"] = serde_json::json!([
            { "type": "docx", "score": 0.71, "reason": "Dokumen editable cukup cocok." },
            { "type": "pdf", "score": 0.71, "reason": "Dokumen printable juga sama kuatnya." },
            { "type": "pptx", "score": 0.41, "reason": "Slide deck kurang prioritas." },
        ]);

        let input = DecideInput {
            interpretation: interp,
            preferred_output_type: Some("auto".to_string()),
        };
        let decision = decide(input);
        // pdf wins tie-breaker (priority 0 < docx priority 1)
        assert_eq!(decision.resolved_output_type, "pdf");
        assert_eq!(decision.decision_source, "candidate_ranking");
        assert!(decision.tie_breaker_applied);
        assert!(decision.reasoning.contains("Scores tied"));
    }

    // ── decide — keyword signals ───────────────────────────────────────────

    #[test]
    fn test_keyword_signal_slide_intent() {
        let mut interp = sample_interpretation();
        interp["teacher_prompt"] =
            Value::String("Buatkan slide presentasi untuk materi pecahan.".to_string());
        interp["output_type_candidates"] = serde_json::json!([
            { "type": "pdf", "score": 0.50, "reason": "Printable." },
            { "type": "pptx", "score": 0.45, "reason": "Slide relevan." },
            { "type": "docx", "score": 0.30, "reason": "Editable." },
        ]);

        let input = DecideInput {
            interpretation: interp,
            preferred_output_type: Some("auto".to_string()),
        };
        let decision = decide(input);
        // pptx should get 0.35 bonus for "presentasi" keyword
        assert_eq!(decision.resolved_output_type, "pptx");
        assert_eq!(decision.decision_source, "candidate_ranking");
    }

    #[test]
    fn test_keyword_signal_printable_intent() {
        let mut interp = sample_interpretation();
        interp["teacher_prompt"] = Value::String("Buatkan handout printable materi pecahan.".to_string());
        interp["output_type_candidates"] = serde_json::json!([
            { "type": "pptx", "score": 0.50, "reason": "Slide deck lumayan." },
            { "type": "pdf", "score": 0.48, "reason": "Printable juga cukup." },
            { "type": "docx", "score": 0.40, "reason": "Editable juga oke." },
        ]);

        let input = DecideInput {
            interpretation: interp,
            preferred_output_type: Some("auto".to_string()),
        };
        let decision = decide(input);
        // pdf should win with 0.48 + 0.25 keyword bonus = 0.73 vs pptx 0.50
        assert_eq!(decision.resolved_output_type, "pdf");
    }

    #[test]
    fn test_visual_density_signal() {
        let mut interp = sample_interpretation();
        interp["requested_media_characteristics"]["visual_density"] = Value::String("high".to_string());
        interp["assets"] = serde_json::json!([
            { "type": "image", "description": "Diagram", "required": true }
        ]);
        interp["output_type_candidates"] = serde_json::json!([
            { "type": "pdf", "score": 0.50, "reason": "Printable." },
            { "type": "pptx", "score": 0.45, "reason": "Slide juga relevan." },
            { "type": "docx", "score": 0.30, "reason": "Editable." },
        ]);

        let input = DecideInput {
            interpretation: interp,
            preferred_output_type: Some("auto".to_string()),
        };
        let decision = decide(input);
        // pptx: 0.45 + 0.12 visual_density = 0.57 vs pdf: 0.50 → pptx wins
        assert_eq!(decision.resolved_output_type, "pptx");
    }

    // ── DecisionOutput fields ──────────────────────────────────────────────

    #[test]
    fn test_decision_output_schema_version() {
        let input = DecideInput {
            interpretation: sample_interpretation(),
            preferred_output_type: None,
        };
        let decision = decide(input);
        assert_eq!(decision.schema_version, VERSION);
    }

    #[test]
    fn test_decision_output_preferred_output_type() {
        let input = DecideInput {
            interpretation: sample_interpretation(),
            preferred_output_type: Some("pdf".to_string()),
        };
        let decision = decide(input);
        assert_eq!(decision.preferred_output_type, "pdf");
    }

    #[test]
    fn test_decision_output_constratint_preferred_type() {
        let input = DecideInput {
            interpretation: sample_interpretation(),
            preferred_output_type: None,
        };
        let decision = decide(input);
        assert_eq!(decision.constraint_preferred_output_type, "auto");
    }

    #[test]
    fn test_decision_output_ranked_candidates() {
        let input = DecideInput {
            interpretation: sample_interpretation(),
            preferred_output_type: None,
        };
        let decision = decide(input);
        assert_eq!(decision.ranked_candidates.len(), 3);
        // Should be sorted by score descending
        assert_eq!(decision.ranked_candidates[0].output_type, "pdf");
        assert_eq!(decision.ranked_candidates[1].output_type, "docx");
        assert_eq!(decision.ranked_candidates[2].output_type, "pptx");
    }

    #[test]
    fn test_decision_output_resolved_at_is_iso8601() {
        let input = DecideInput {
            interpretation: sample_interpretation(),
            preferred_output_type: None,
        };
        let decision = decide(input);
        // Should be a valid ISO 8601 with milliseconds
        assert!(!decision.resolved_at.is_empty());
        assert!(decision.resolved_at.contains('T'));
        assert!(decision.resolved_at.contains('Z') || decision.resolved_at.contains('+'));
    }

    // ── build_generation_spec ──────────────────────────────────────────────

    #[test]
    fn test_generation_spec_from_interpretation() {
        let interp = sample_interpretation();
        let spec = build_generation_spec(&interp, "pdf", None);
        assert_eq!(spec["schema_version"], "media_generation_spec.v1");
        assert_eq!(spec["export_format"], "pdf");
        assert_eq!(spec["layout_hints"]["document_mode"], "document");
        assert_eq!(spec["page_or_slide_structure"]["unit_type"], "page");
    }

    #[test]
    fn test_generation_spec_pptx_mode() {
        let interp = sample_interpretation();
        let spec = build_generation_spec(&interp, "pptx", None);
        assert_eq!(spec["layout_hints"]["document_mode"], "slide_deck");
        assert_eq!(spec["page_or_slide_structure"]["unit_type"], "slide");
    }

    #[test]
    fn test_generation_spec_sections_from_draft() {
        let interp = sample_interpretation();
        let draft = serde_json::json!({
            "payload": {
                "sections": [
                    {
                        "title": "Custom Title",
                        "purpose": "Custom Purpose",
                        "body_blocks": [{"type": "paragraph", "content": "Hello"}],
                        "emphasis": "medium"
                    }
                ],
                "schema_version": "media_content_draft.v1"
            },
            "source": "adapter"
        });
        let spec = build_generation_spec(&interp, "pdf", Some(&draft));
        assert_eq!(spec["sections"][0]["title"], "Custom Title");
    }

    // ── rank_candidates ────────────────────────────────────────────────────

    #[test]
    fn test_rank_candidates_returns_three_formats() {
        let ranked = rank_candidates(&sample_interpretation());
        assert_eq!(ranked.len(), 3);
    }

    #[test]
    fn test_rank_candidates_order_by_score_desc() {
        let ranked = rank_candidates(&sample_interpretation());
        for i in 1..ranked.len() {
            assert!(
                ranked[i].score <= ranked[i - 1].score + 0.0001,
                "ranked[{}].score ({}) should be <= ranked[{}].score ({})",
                i,
                ranked[i].score,
                i - 1,
                ranked[i - 1].score,
            );
        }
    }

    // ── decision_haystack ──────────────────────────────────────────────────

    #[test]
    fn test_decision_haystack_includes_prompt_and_goal() {
        let interp = sample_interpretation();
        let haystack = decision_haystack(&interp);
        assert!(haystack.contains("buatkan materi aljabar"));
        assert!(haystack.contains("create a classroom resource"));
    }

    // ── build_ranking_reasoning ────────────────────────────────────────────

    #[test]
    fn test_build_ranking_reasoning_no_runner_up() {
        let selected = RankedCandidate {
            output_type: "pdf".to_string(),
            score: 0.78,
            candidate_score: 0.78,
            reason_code: "highest_candidate_score".to_string(),
            matched_signals: vec![],
            reasons: vec!["LLM candidate score 0.7800: OK.".to_string()],
        };
        let reasoning = build_ranking_reasoning(&selected, None, false, "Good choice.");
        assert!(reasoning.contains("PDF with score 0.7800"));
        assert!(reasoning.contains("Good choice."));
        assert!(!reasoning.contains("Runner-up"));
    }

    #[test]
    fn test_build_ranking_reasoning_with_runner_up() {
        let selected = RankedCandidate {
            output_type: "pdf".to_string(),
            score: 0.78,
            candidate_score: 0.78,
            reason_code: "highest_candidate_score".to_string(),
            matched_signals: vec![],
            reasons: vec!["LLM candidate score 0.7800: OK.".to_string()],
        };
        let runner = RankedCandidate {
            output_type: "docx".to_string(),
            score: 0.61,
            candidate_score: 0.61,
            reason_code: "highest_candidate_score".to_string(),
            matched_signals: vec![],
            reasons: vec![],
        };
        let reasoning = build_ranking_reasoning(&selected, Some(&runner), false, "");
        assert!(reasoning.contains("Runner-up"));
        assert!(reasoning.contains("DOCX at score 0.6100"));
    }

    #[test]
    fn test_build_ranking_reasoning_tie_breaker() {
        let selected = RankedCandidate {
            output_type: "pdf".to_string(),
            score: 0.71,
            candidate_score: 0.71,
            reason_code: "highest_candidate_score".to_string(),
            matched_signals: vec![],
            reasons: vec![],
        };
        let runner = RankedCandidate {
            output_type: "docx".to_string(),
            score: 0.71,
            candidate_score: 0.71,
            reason_code: "highest_candidate_score".to_string(),
            matched_signals: vec![],
            reasons: vec![],
        };
        let reasoning = build_ranking_reasoning(&selected, Some(&runner), true, "");
        assert!(reasoning.contains("Scores tied"));
        assert!(reasoning.contains("PDF > DOCX > PPTX"));
    }

    // ── keyword_signals ────────────────────────────────────────────────────

    #[test]
    fn test_keyword_signals_empty_when_no_keywords() {
        let mut interp = sample_interpretation();
        interp["teacher_prompt"] = Value::String("Buatkan materi pecahan.".to_string());
        let signals = keyword_signals(&interp);
        // "materi" and "pecahan" are not format signal keywords
        assert!(signals.is_empty());
    }

    #[test]
    fn test_keyword_signals_slide_presentasi() {
        let mut interp = sample_interpretation();
        interp["teacher_prompt"] = Value::String("Buatkan presentasi untuk materi ini.".to_string());
        let signals = keyword_signals(&interp);
        let pptx_signals: Vec<_> = signals.iter().filter(|s| s.reason_code == "slide_intent_detected").collect();
        assert_eq!(pptx_signals.len(), 1);
        assert!((pptx_signals[0].weight - 0.35).abs() < 0.001);
    }

    #[test]
    fn test_keyword_signals_handout_printable() {
        let mut interp = sample_interpretation();
        interp["teacher_prompt"] = Value::String("Buatkan handout untuk kelas.".to_string());
        let signals = keyword_signals(&interp);
        let pdf_signals: Vec<_> = signals.iter().filter(|s| s.reason_code == "printable_intent_detected").collect();
        assert_eq!(pdf_signals.len(), 1);
        assert!((pdf_signals[0].weight - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_visual_density_pptx_bonus() {
        let mut interp = sample_interpretation();
        interp["requested_media_characteristics"]["visual_density"] = Value::String("high".to_string());
        interp["assets"] = serde_json::json!([
            { "type": "image", "description": "Diagram", "required": true }
        ]);
        let signals = keyword_signals(&interp);
        let visual_signal: Vec<_> = signals.iter().filter(|s| s.reason_code == "visual_density_signal").collect();
        assert_eq!(visual_signal.len(), 1);
        assert!((visual_signal[0].weight - 0.12).abs() < 0.001);
    }

    #[test]
    fn test_visual_density_bonus_only_with_assets() {
        let mut interp = sample_interpretation();
        interp["requested_media_characteristics"]["visual_density"] = Value::String("high".to_string());
        interp["assets"] = Value::Array(vec![]); // No assets
        let signals = keyword_signals(&interp);
        let visual_signal: Vec<_> = signals.iter().filter(|s| s.reason_code == "visual_density_signal").collect();
        assert_eq!(visual_signal.len(), 0);
    }

    // ── DecisionError ──────────────────────────────────────────────────────

    #[test]
    fn test_decision_error_display() {
        let err = DecisionError::MissingInterpretation("test".to_string());
        assert!(err.to_string().contains("interpretation"));
    }

    // ── normalize_preferred_output_type ────────────────────────────────────

    #[test]
    fn test_normalize_output_type_auto() {
        assert_eq!(normalize_preferred_output_type(Some("auto")), "auto");
        assert_eq!(normalize_preferred_output_type(Some("AUTO")), "auto");
    }

    #[test]
    fn test_normalize_output_type_none_returns_auto() {
        assert_eq!(normalize_preferred_output_type(None), "auto");
    }

    #[test]
    fn test_normalize_output_type_valid() {
        assert_eq!(normalize_preferred_output_type(Some("pdf")), "pdf");
        assert_eq!(normalize_preferred_output_type(Some("DOCX")), "docx");
        assert_eq!(normalize_preferred_output_type(Some("PPTX")), "pptx");
    }

    #[test]
    fn test_normalize_output_type_invalid_fallsback() {
        assert_eq!(normalize_preferred_output_type(Some("html")), "auto");
    }
}

use std::sync::Arc;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;

// ─── Stopwords ──────────────────────────────────────────────────────────────

const STOPWORDS: &[&str] = &[
    "agar", "ajar", "aku", "analisis", "buat", "buatkan", "contoh", "dan",
    "dengan", "di", "dokumen", "file", "guru", "handout", "kelas", "kuis",
    "latihan", "lembar", "materi", "modul", "pada", "pdf", "pelajaran",
    "pembelajaran", "ppt", "pptx", "ringkas", "saya", "semester", "sebuah",
    "slide", "siswa", "soal", "tentang", "untuk", "yang",
];

const SCORE_NORMALIZER: f64 = 24.0;
const MINIMUM_CONFIDENCE_SCORE: f64 = 0.25;
const VERSION: &str = "media_prompt_taxonomy_inference.v1";

// ─── Embedded copy of `kurikulum_merdeka_structure.json` ────────────────────

/// Holds the Kurikulum Merdeka structure reference data, loaded at compile time.
#[derive(Debug, Clone)]
pub struct TaxonomyCatalog {
    pub raw: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum TaxonomyError {
    #[error("failed to parse taxonomy JSON: {0}")]
    Parse(serde_json::Error),
}

impl TaxonomyCatalog {
    /// Load taxonomy from the compile-time embedded JSON data.
    pub fn load_default() -> Self {
        let raw: Value = serde_json::from_str(include_str!(
            "data/kurikulum_merdeka_structure.json"
        ))
        .expect("embedded kurikulum_merdeka_structure.json must be valid JSON");
        Self { raw }
    }

    /// Run taxonomy inference on a teacher prompt.
    pub fn infer(&self, teacher_prompt: &str) -> Option<TaxonomyInferenceResult> {
        SubjectsJsonTaxonomyCatalog::infer(teacher_prompt)
    }
}

pub type SharedTaxonomyCatalog = Arc<TaxonomyCatalog>;

// ─── Subjects JSON taxonomy catalog ─────────────────────────────────────────
// Port of `App\\MediaGeneration\\SubjectsJsonTaxonomyCatalog` + `MediaPromptTaxonomyInferenceService`

/// A single normalized entry from `subjects.json`.
#[derive(Debug, Clone, Serialize)]
pub struct TaxonomyEntry {
    pub catalog_index: usize,
    pub jenjang: Option<String>,
    pub subject_name: String,
    pub subject_slug: String,
    pub kelas: Option<i64>,
    pub semester: Option<i64>,
    pub bab: Option<i64>,
    pub sub_subject_name: String,
    pub sub_subject_slug: String,
    pub description: String,
    pub is_active: bool,
    pub content_structure: String,
    pub structure_items: Vec<String>,
    #[serde(skip)]
    pub normalized_subject: String,
    #[serde(skip)]
    pub normalized_sub_subject: String,
}

/// Context extracted from the teacher prompt.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PromptContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jenjang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kelas: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semester: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bab: Option<i64>,
}

/// Matching signals computed for a candidate entry.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MatchSignal {
    pub subject_phrase_match: bool,
    pub sub_subject_phrase_match: bool,
    pub subject_overlap: usize,
    pub sub_subject_overlap: usize,
    pub description_overlap: usize,
    pub structure_overlap: usize,
    pub jenjang_match: Option<bool>,
    pub kelas_match: Option<bool>,
    pub semester_match: Option<bool>,
    pub bab_match: Option<bool>,
}

/// Scored candidate entry.
#[derive(Debug, Clone, Serialize)]
pub struct CandidateScore {
    pub entry: TaxonomyEntry,
    pub raw_score: f64,
    pub normalized_score: f64,
    pub signal: MatchSignal,
}

/// The inference result returned by `infer()`.
#[derive(Debug, Clone, Serialize)]
pub struct TaxonomyInferenceResult {
    pub schema_version: &'static str,
    pub source: &'static str,
    pub prompt_context: PromptContext,
    pub confidence: ConfidenceInfo,
    pub best_match: BestMatch,
    pub candidate_matches: Vec<CandidateSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfidenceInfo {
    pub score: f64,
    pub label: &'static str,
    pub sub_subject_attached: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BestMatch {
    pub jenjang: Option<String>,
    pub kelas: Option<i64>,
    pub semester: Option<i64>,
    pub bab: Option<i64>,
    pub subject_name: String,
    pub subject_slug: String,
    pub subject_id: Option<i64>,
    pub sub_subject_name: Option<String>,
    pub sub_subject_slug: Option<String>,
    pub sub_subject_id: Option<i64>,
    pub description: String,
    pub content_structure: String,
    pub structure_items: Vec<String>,
    pub matched_signals: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CandidateSummary {
    pub subject_name: String,
    pub subject_slug: String,
    pub sub_subject_name: String,
    pub sub_subject_slug: String,
    pub jenjang: Option<String>,
    pub kelas: Option<i64>,
    pub score: f64,
    pub label: &'static str,
}

// ─── SubjectsJsonTaxonomyCatalog ─────────────────────────────────────────────

pub struct SubjectsJsonTaxonomyCatalog;

impl SubjectsJsonTaxonomyCatalog {
    /// Return all parsed entries from the embedded `subjects.json`.
    /// Parsed once and cached via `OnceLock`.
    pub fn entries() -> &'static Vec<TaxonomyEntry> {
        use std::sync::OnceLock;
        static ENTRIES: OnceLock<Vec<TaxonomyEntry>> = OnceLock::new();
        ENTRIES.get_or_init(|| {
            let raw: Vec<Value> = serde_json::from_str(include_str!("data/subjects.json"))
                .expect("embedded subjects.json must be valid JSON array");
            let mut entries = Vec::with_capacity(raw.len());

            for (idx, row) in raw.iter().enumerate() {
                if let Some(entry) = Self::normalize_entry(row, idx) {
                    entries.push(entry);
                }
            }

            entries
        })
    }

    fn normalize_entry(row: &Value, index: usize) -> Option<TaxonomyEntry> {
        let subject_name = row.get("subject")?.as_str()?.trim().to_string();
        let sub_subject_name = row.get("sub_subject")?.as_str()?.trim().to_string();

        if subject_name.is_empty() || sub_subject_name.is_empty() {
            return None;
        }

        let description = Self::normalize_text_value(row.get("deskripsi_singkat").or_else(|| row.get("description")));
        let raw_content_structure = row
            .get("Structure of content")
            .or_else(|| row.get("structure_of_content"))
            .map(|v| text_value(Some(v)))
            .unwrap_or_default();
        let content_structure = Self::normalize_text_value(Some(&Value::String(raw_content_structure.clone())));
        let structure_items = Self::structure_items(&raw_content_structure);

        Some(TaxonomyEntry {
            catalog_index: index + 1,
            jenjang: row.get("jenjang").and_then(|v| v.as_str()).map(|s| s.to_uppercase().trim().to_string()).filter(|s| !s.is_empty()),
            subject_name: subject_name.clone(),
            subject_slug: slugify(row.get("subject_slug").and_then(|v| v.as_str()).unwrap_or(&subject_name)),
            kelas: row.get("kelas").and_then(|v| v.as_i64()),
            semester: row.get("semester").and_then(|v| v.as_i64()),
            bab: row.get("bab").and_then(|v| v.as_i64()),
            sub_subject_name: sub_subject_name.clone(),
            sub_subject_slug: slugify(row.get("sub_subject_slug").and_then(|v| v.as_str()).unwrap_or(&sub_subject_name)),
            description,
            is_active: row.get("is_active").and_then(|v| v.as_bool()).unwrap_or(true),
            content_structure,
            structure_items,
            normalized_subject: Self::normalize_search_text(&subject_name),
            normalized_sub_subject: Self::normalize_search_text(&sub_subject_name),
        })
    }

    /// Normalize search text: ASCII-fy, lowercase, collapse non-alphanumeric to spaces.
    pub fn normalize_search_text(value: &str) -> String {
        let normalized = value.to_lowercase();
        // Replace non-alphanumeric, non-unicode sequences with spaces
        let re = Regex::new(r"[^\p{L}\p{N}]+").unwrap();
        let result = re.replace_all(&normalized, " ");
        let result = result.trim();
        let re_space = Regex::new(r"\s+").unwrap();
        re_space.replace_all(result, " ").to_string()
    }

    /// Tokenize search text into an array of tokens.
    pub fn tokenize_search_text(value: &str) -> Vec<String> {
        let normalized = Self::normalize_search_text(value);
        if normalized.is_empty() {
            return vec![];
        }
        normalized
            .split_whitespace()
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string())
            .collect()
    }

    fn normalize_text_value(value: Option<&Value>) -> String {
        match value {
            Some(Value::String(s)) => s.trim().to_string(),
            Some(Value::Array(arr)) => {
                let parts: Vec<String> = arr
                    .iter()
                    .filter_map(|v| {
                        let t = text_value(Some(v));
                        if t.is_empty() { None } else { Some(t) }
                    })
                    .collect();
                parts.join(", ")
            }
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::Bool(b)) => b.to_string(),
            _ => String::new(),
        }
    }

    fn structure_items(content_structure: &str) -> Vec<String> {
        if content_structure.is_empty() {
            return vec![];
        }

        // Replace " dan " or " and " with ", "
        let re = Regex::new(r"\s+(dan|and)\s+").unwrap();
        let normalized = re.replace_all(content_structure, ", ");

        normalized
            .split(',')
            .map(|part| {
                let re_space = Regex::new(r"\s+").unwrap();
                re_space.replace_all(part.trim(), " ").to_string()
            })
            .filter(|part| !part.is_empty())
            .collect()
    }

    // ─── Inference ───────────────────────────────────────────────────────

    /// Run full taxonomy inference on a teacher prompt.
    pub fn infer(teacher_prompt: &str) -> Option<TaxonomyInferenceResult> {
        let teacher_prompt = teacher_prompt.trim();

        if teacher_prompt.is_empty() {
            return None;
        }

        let prompt_ctx = Self::prompt_context(teacher_prompt);
        let normalized_prompt = Self::normalize_search_text(teacher_prompt);
        let prompt_tokens = Self::filtered_tokens(teacher_prompt);
        let entries = Self::entries();

        let mut ranked: Vec<CandidateScore> = Vec::new();

        for entry in entries.iter() {
            let candidate = Self::score_entry(entry, &normalized_prompt, &prompt_tokens, &prompt_ctx);
            if candidate.raw_score > 0.0 {
                ranked.push(candidate);
            }
        }

        if ranked.is_empty() {
            return None;
        }

        // Sort: descending raw_score, then ascending catalog_index
        ranked.sort_by(|a, b| {
            b.raw_score
                .partial_cmp(&a.raw_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.entry.catalog_index.cmp(&b.entry.catalog_index))
        });

        let best = &ranked[0];

        // Threshold check: normalized < 0.25 AND no phrase match → reject
        if best.normalized_score < MINIMUM_CONFIDENCE_SCORE
            && !best.signal.subject_phrase_match
            && !best.signal.sub_subject_phrase_match
        {
            return None;
        }

        // Determine best subject candidate (first with same subject_slug)
        let best_subject = ranked
            .iter()
            .find(|c| c.entry.subject_slug == best.entry.subject_slug)
            .unwrap_or(best);

        let include_sub_subject = Self::should_attach_sub_subject(best, ranked.get(1));

        // Resolve DB models — currently returns None until DB integration
        let subject_id: Option<i64> = None;
        let sub_subject_id: Option<i64> = None;

        let matched_signals = Self::matched_signals(&best.signal);

        let confidence_label = Self::confidence_label(best.normalized_score);

        let candidate_matches: Vec<CandidateSummary> = ranked
            .iter()
            .take(3)
            .map(|c| CandidateSummary {
                subject_name: c.entry.subject_name.clone(),
                subject_slug: c.entry.subject_slug.clone(),
                sub_subject_name: c.entry.sub_subject_name.clone(),
                sub_subject_slug: c.entry.sub_subject_slug.clone(),
                jenjang: c.entry.jenjang.clone(),
                kelas: c.entry.kelas,
                score: c.normalized_score,
                label: Self::confidence_label(c.normalized_score),
            })
            .collect();

        Some(TaxonomyInferenceResult {
            schema_version: VERSION,
            source: "subjects.json",
            prompt_context: PromptContext {
                jenjang: prompt_ctx.jenjang.clone(),
                kelas: prompt_ctx.kelas,
                semester: prompt_ctx.semester,
                bab: prompt_ctx.bab,
            },
            confidence: ConfidenceInfo {
                score: best.normalized_score,
                label: confidence_label,
                sub_subject_attached: include_sub_subject,
            },
            best_match: BestMatch {
                jenjang: best.entry.jenjang.clone(),
                kelas: best.entry.kelas,
                semester: best.entry.semester,
                bab: best.entry.bab,
                subject_name: best_subject.entry.subject_name.clone(),
                subject_slug: best_subject.entry.subject_slug.clone(),
                subject_id,
                sub_subject_name: if include_sub_subject {
                    Some(best.entry.sub_subject_name.clone())
                } else {
                    None
                },
                sub_subject_slug: if include_sub_subject {
                    Some(best.entry.sub_subject_slug.clone())
                } else {
                    None
                },
                sub_subject_id: if include_sub_subject { sub_subject_id } else { None },
                description: best.entry.description.clone(),
                content_structure: best.entry.content_structure.clone(),
                structure_items: best.entry.structure_items.clone(),
                matched_signals,
            },
            candidate_matches,
        })
    }

    fn score_entry(
        entry: &TaxonomyEntry,
        normalized_prompt: &str,
        prompt_tokens: &[String],
        prompt_ctx: &PromptContext,
    ) -> CandidateScore {
        let subject_tokens = Self::filtered_tokens(&entry.subject_name);
        let sub_subject_tokens = Self::filtered_tokens(&entry.sub_subject_name);
        let description_tokens = Self::filtered_tokens(&entry.description);

        let mut structure_tokens: Vec<String> = Vec::new();
        for item in &entry.structure_items {
            structure_tokens.extend(Self::filtered_tokens(item));
        }
        structure_tokens.sort();
        structure_tokens.dedup();

        let signal = MatchSignal {
            subject_phrase_match: contains_phrase(normalized_prompt, &entry.normalized_subject),
            sub_subject_phrase_match: contains_phrase(normalized_prompt, &entry.normalized_sub_subject),
            subject_overlap: overlap_count(prompt_tokens, &subject_tokens),
            sub_subject_overlap: overlap_count(prompt_tokens, &sub_subject_tokens),
            description_overlap: usize::min(4, overlap_count(prompt_tokens, &description_tokens)),
            structure_overlap: usize::min(3, overlap_count(prompt_tokens, &structure_tokens)),
            jenjang_match: matches_context(&prompt_ctx.jenjang, &entry.jenjang),
            kelas_match: matches_context_i64(&prompt_ctx.kelas, entry.kelas),
            semester_match: matches_context_i64(&prompt_ctx.semester, entry.semester),
            bab_match: matches_context_i64(&prompt_ctx.bab, entry.bab),
        };

        let mut raw_score = 0.0;

        if signal.subject_phrase_match {
            raw_score += 7.0;
        }
        if signal.sub_subject_phrase_match {
            raw_score += 12.0;
        }

        raw_score += signal.subject_overlap as f64 * 1.5;
        raw_score += signal.sub_subject_overlap as f64 * 2.75;
        raw_score += signal.description_overlap as f64 * 0.75;
        raw_score += signal.structure_overlap as f64 * 0.35;

        raw_score += match signal.jenjang_match {
            Some(true) => 4.5,
            Some(false) => -6.0,
            None => 0.0,
        };
        raw_score += match signal.kelas_match {
            Some(true) => 5.5,
            Some(false) => -7.0,
            None => 0.0,
        };
        raw_score += match signal.semester_match {
            Some(true) => 1.5,
            Some(false) => -1.0,
            None => 0.0,
        };
        raw_score += match signal.bab_match {
            Some(true) => 1.5,
            Some(false) => -0.75,
            None => 0.0,
        };

        // Bonus: kelas_match + subject_phrase_match
        if prompt_ctx.kelas.is_some() && signal.kelas_match == Some(true) && signal.subject_phrase_match {
            raw_score += 1.5;
        }

        // Bonus: jenjang_match + subject_overlap > 0
        if prompt_ctx.jenjang.is_some() && signal.jenjang_match == Some(true) && signal.subject_overlap > 0 {
            raw_score += 1.0;
        }

        raw_score = f64::max(0.0, round_4(raw_score));

        CandidateScore {
            entry: entry.clone(),
            raw_score,
            normalized_score: normalize_score(raw_score),
            signal,
        }
    }

    fn should_attach_sub_subject(best: &CandidateScore, runner_up: Option<&CandidateScore>) -> bool {
        let signal = &best.signal;

        if signal.sub_subject_phrase_match {
            return true;
        }

        if signal.sub_subject_overlap >= 2 || signal.description_overlap >= 2 {
            return true;
        }

        if signal.semester_match == Some(true) || signal.bab_match == Some(true) {
            return true;
        }

        if !signal.subject_phrase_match && signal.subject_overlap == 0 {
            return false;
        }

        let runner_up = match runner_up {
            Some(r) => r,
            None => return false,
        };

        best.normalized_score >= 0.7
            && (best.raw_score - runner_up.raw_score) >= 3.0
            && best.entry.subject_slug != runner_up.entry.subject_slug
    }

    fn matched_signals(signal: &MatchSignal) -> Vec<String> {
        let mut matched = Vec::new();

        if signal.subject_phrase_match {
            matched.push("subject_phrase".to_string());
        }
        if signal.sub_subject_phrase_match {
            matched.push("sub_subject_phrase".to_string());
        }
        if signal.subject_overlap > 0 {
            matched.push("subject_tokens".to_string());
        }
        if signal.sub_subject_overlap > 0 {
            matched.push("sub_subject_tokens".to_string());
        }
        if signal.description_overlap > 0 {
            matched.push("description_tokens".to_string());
        }
        if signal.structure_overlap > 0 {
            matched.push("content_structure".to_string());
        }

        for dim in &["jenjang", "kelas", "semester", "bab"] {
            let val = match *dim {
                "jenjang" => signal.jenjang_match,
                "kelas" => signal.kelas_match,
                "semester" => signal.semester_match,
                "bab" => signal.bab_match,
                _ => None,
            };
            if val == Some(true) {
                matched.push(dim.to_string());
            }
        }

        matched
    }

    /// Extract prompt context: jenjang, kelas, semester, bab.
    fn prompt_context(text: &str) -> PromptContext {
        PromptContext {
            jenjang: Self::detect_jenjang(text),
            kelas: Self::detect_class_number(text),
            semester: Self::detect_number_by_label(text, &["semester"]),
            bab: Self::detect_number_by_label(text, &["bab", "chapter"]),
        }
    }

    fn detect_jenjang(text: &str) -> Option<String> {
        let normalized = Self::normalize_search_text(text);

        if contains_word(&normalized, "smk") || normalized.contains("sekolah menengah kejuruan") {
            return Some("SMK".to_string());
        }
        if contains_word(&normalized, "sma") || normalized.contains("sekolah menengah atas") {
            return Some("SMA".to_string());
        }
        if contains_word(&normalized, "smp") || normalized.contains("sekolah menengah pertama") {
            return Some("SMP".to_string());
        }
        if contains_word(&normalized, "sd") || normalized.contains("sekolah dasar") {
            return Some("SD".to_string());
        }

        None
    }

    fn detect_class_number(text: &str) -> Option<i64> {
        let re = Regex::new(r"(?i)\b(?:kelas|grade)\s+([0-9]{1,2}|xii|xi|ix|viii|vii|vi|x|v|iv|iii|ii|i)\b").unwrap();

        if let Some(caps) = re.captures(text) {
            let token = caps.get(1)?.as_str().to_lowercase();

            if let Ok(n) = token.parse::<i64>() {
                return Some(n);
            }

            return roman_to_int(&token);
        }

        None
    }

    fn detect_number_by_label(text: &str, labels: &[&str]) -> Option<i64> {
        let pattern = format!(
            r"(?i)\b(?:{})\s+([0-9]{{1,2}})\b",
            labels.iter().map(|l| regex::escape(l)).collect::<Vec<_>>().join("|")
        );
        let re = Regex::new(&pattern).ok()?;

        if let Some(caps) = re.captures(text) {
            return caps.get(1)?.as_str().parse::<i64>().ok();
        }

        None
    }

    fn filtered_tokens(text: &str) -> Vec<String> {
        let tokens = Self::tokenize_search_text(text);

        tokens
            .into_iter()
            .filter(|token| {
                if token.is_empty() || token.chars().all(|c| c.is_ascii_digit()) {
                    return false;
                }
                if STOPWORDS.contains(&token.as_str()) {
                    return false;
                }
                token.chars().count() >= 2
            })
            .collect()
    }

    fn confidence_label(score: f64) -> &'static str {
        if score >= 0.75 {
            "high"
        } else if score >= 0.45 {
            "medium"
        } else {
            "low"
        }
    }
}

// ─── Helper functions ───────────────────────────────────────────────────────

fn contains_phrase(haystack: &str, needle: &str) -> bool {
    let haystack = haystack.trim();
    let needle = needle.trim();

    if haystack.is_empty() || needle.is_empty() {
        return false;
    }

    let haystack_padded = format!(" {haystack} ");
    let needle_padded = format!(" {needle} ");
    haystack_padded.contains(&needle_padded)
}

fn contains_word(text: &str, word: &str) -> bool {
    let pattern = format!(r"\b{}\b", regex::escape(word));
    Regex::new(&pattern).map_or(false, |re| re.is_match(text))
}

fn overlap_count(prompt_tokens: &[String], candidate_tokens: &[String]) -> usize {
    if prompt_tokens.is_empty() || candidate_tokens.is_empty() {
        return 0;
    }

    let mut deduped = candidate_tokens.to_vec();
    deduped.sort();
    deduped.dedup();

    prompt_tokens
        .iter()
        .filter(|t| deduped.contains(t))
        .count()
}

fn normalize_score(raw_score: f64) -> f64 {
    round_4(f64::min(1.0, f64::max(0.0, raw_score / SCORE_NORMALIZER)))
}

fn round_4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn roman_to_int(token: &str) -> Option<i64> {
    match token.to_uppercase().as_str() {
        "I" => Some(1),
        "II" => Some(2),
        "III" => Some(3),
        "IV" => Some(4),
        "V" => Some(5),
        "VI" => Some(6),
        "VII" => Some(7),
        "VIII" => Some(8),
        "IX" => Some(9),
        "X" => Some(10),
        "XI" => Some(11),
        "XII" => Some(12),
        _ => None,
    }
}

fn slugify(value: &str) -> String {
    let re = Regex::new(r"[^a-zA-Z0-9]+").unwrap();
    let slug = re.replace_all(value, "-");
    let slug = slug.trim_matches('-');
    slug.to_lowercase()
}

fn text_value(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

fn matches_context(prompt_val: &Option<String>, entry_val: &Option<String>) -> Option<bool> {
    match (prompt_val, entry_val) {
        (Some(p), Some(e)) if !p.is_empty() => Some(p.to_lowercase() == e.to_lowercase()),
        _ => None,
    }
}

fn matches_context_i64(prompt_val: &Option<i64>, entry_val: Option<i64>) -> Option<bool> {
    match (prompt_val, entry_val) {
        (Some(p), Some(e)) => Some(p == &e),
        _ => None,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_rejects_empty_prompt() {
        assert!(SubjectsJsonTaxonomyCatalog::infer("").is_none());
        assert!(SubjectsJsonTaxonomyCatalog::infer("   ").is_none());
    }

    #[test]
    fn test_normalize_search_text() {
        let result = SubjectsJsonTaxonomyCatalog::normalize_search_text("  Hello World!  ");
        assert_eq!(result, "hello world");

        let result = SubjectsJsonTaxonomyCatalog::normalize_search_text("ILMU Pengetahuan Alam (IPA)");
        assert_eq!(result, "ilmu pengetahuan alam ipa");
    }

    #[test]
    fn test_tokenize_search_text() {
        let tokens = SubjectsJsonTaxonomyCatalog::tokenize_search_text("modul matematika kelas 5");
        assert!(tokens.contains(&"modul".to_string()));
        assert!(tokens.contains(&"matematika".to_string()));
        assert!(tokens.contains(&"kelas".to_string()));
        assert!(tokens.contains(&"5".to_string()));
    }

    #[test]
    fn test_detect_jenjang() {
        assert_eq!(
            SubjectsJsonTaxonomyCatalog::detect_jenjang("handout matematika sd kelas 5"),
            Some("SD".to_string())
        );
        assert_eq!(
            SubjectsJsonTaxonomyCatalog::detect_jenjang("modul ipa smp kelas 7"),
            Some("SMP".to_string())
        );
        assert_eq!(
            SubjectsJsonTaxonomyCatalog::detect_jenjang("Buatkan soal SMA kelas 10"),
            Some("SMA".to_string())
        );
        assert_eq!(
            SubjectsJsonTaxonomyCatalog::detect_jenjang("job sheet SMK kelas 10 teknik otomotif"),
            Some("SMK".to_string())
        );
        assert_eq!(
            SubjectsJsonTaxonomyCatalog::detect_jenjang("sekolah dasar kelas 1"),
            Some("SD".to_string())
        );
        assert!(SubjectsJsonTaxonomyCatalog::detect_jenjang("materi tentang gaya").is_none());
    }

    #[test]
    fn test_detect_class_number() {
        assert_eq!(
            SubjectsJsonTaxonomyCatalog::detect_class_number("kelas 5 sd"),
            Some(5)
        );
        assert_eq!(
            SubjectsJsonTaxonomyCatalog::detect_class_number("KELAS 10 SMA"),
            Some(10)
        );
        // Roman numeral
        assert_eq!(
            SubjectsJsonTaxonomyCatalog::detect_class_number("kelas XII sma"),
            Some(12)
        );
        assert!(SubjectsJsonTaxonomyCatalog::detect_class_number("tanpa kelas").is_none());
    }

    #[test]
    fn test_roman_to_int() {
        assert_eq!(roman_to_int("I"), Some(1));
        assert_eq!(roman_to_int("V"), Some(5));
        assert_eq!(roman_to_int("X"), Some(10));
        assert_eq!(roman_to_int("XII"), Some(12));
        assert_eq!(roman_to_int("xii"), Some(12));
        assert!(roman_to_int("invalid").is_none());
    }

    #[test]
    fn test_contains_phrase() {
        assert!(contains_phrase("buatkan modul matematika", "modul matematika"));
        assert!(!contains_phrase("buatkan modul", "modul matematika"));
    }

    #[test]
    fn test_filtered_tokens_removes_stopwords() {
        let tokens = SubjectsJsonTaxonomyCatalog::filtered_tokens("buatkan modul matematika untuk sd");
        // "buatkan", "untuk" are stopwords
        assert!(!tokens.contains(&"buatkan".to_string()));
        assert!(!tokens.contains(&"untuk".to_string()));
        assert!(tokens.contains(&"modul".to_string()));
        assert!(tokens.contains(&"matematika".to_string()));
        assert!(tokens.contains(&"sd".to_string()));
    }

    #[test]
    fn test_structure_items_parsing() {
        let items = SubjectsJsonTaxonomyCatalog::structure_items("Konsep, Hukum/Rumus, Contoh fenomena, dan Eksperimen aman");
        assert!(items.contains(&"Konsep".to_string()));
        assert!(items.contains(&"Eksperimen aman".to_string()));
    }

    #[test]
    fn test_overlap_count() {
        let a = vec!["modul".to_string(), "matematika".to_string(), "sd".to_string()];
        let b = vec!["matematika".to_string(), "sd".to_string(), "fisika".to_string()];
        assert_eq!(overlap_count(&a, &b), 2);
    }

    #[test]
    fn test_confidence_label() {
        assert_eq!(SubjectsJsonTaxonomyCatalog::confidence_label(0.80), "high");
        assert_eq!(SubjectsJsonTaxonomyCatalog::confidence_label(0.60), "medium");
        assert_eq!(SubjectsJsonTaxonomyCatalog::confidence_label(0.30), "low");
    }

    #[test]
    fn test_normalize_score() {
        assert!((normalize_score(24.0) - 1.0).abs() < 0.001);
        assert!((normalize_score(0.0) - 0.0).abs() < 0.001);
        assert!((normalize_score(12.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_entries_loaded() {
        let entries = SubjectsJsonTaxonomyCatalog::entries();
        assert!(!entries.is_empty(), "subjects.json should have entries");
        // Verify structure
        let entry = &entries[0];
        assert!(!entry.subject_name.is_empty());
        assert!(!entry.sub_subject_name.is_empty());
        assert!(!entry.subject_slug.is_empty());
    }

    #[test]
    fn test_infer_returns_result_for_known_prompt() {
        // This should match IPAS SD kelas 4 → gaya-sekitar-kita-kelas-4
        let result = SubjectsJsonTaxonomyCatalog::infer(
            "Buatkan PDF pembelajaran IPAS kelas 4 tentang Gaya di Sekitar Kita"
        );
        assert!(result.is_some(), "Should find a match for IPAS prompt");
        let result = result.unwrap();
        assert_eq!(result.schema_version, "media_prompt_taxonomy_inference.v1");
        assert!(!result.candidate_matches.is_empty());
    }

    #[test]
    fn test_infer_returns_none_for_gibberish() {
        let result = SubjectsJsonTaxonomyCatalog::infer("asdfghjkl qwerty zxcvbnm");
        assert!(result.is_none(), "Gibberish should not match anything");
    }
}

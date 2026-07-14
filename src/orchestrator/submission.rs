//! Media generation submission service.
//!
//! Port of `MediaGenerationSubmissionService` from Laravel.
//!
//! Manages creation and deduplication of media generation requests:
//!
//! - `create_or_reuse()` — Computes a `request_fingerprint` (SHA-256 over teacher,
//!   normalized prompt, preferred output type, subject IDs). Within a DB transaction,
//!   uses `SELECT ... FOR UPDATE` to find an active (non-terminal) duplicate.
//!   If found, returns the existing generation. Otherwise inserts a new one.
//!   On a unique-constraint race (PG error 23505), retries the lookup.
//!
//! - `create_regeneration()` — Creates a new generation linked to a parent for
//!   prompt refinement, with combined prompt text.

use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

// ─── Constants ──────────────────────────────────────────────────────────────

/// Allowed preferred output types (matching `MediaPromptInterpretationSchema::allowedPreferredOutputTypes()`).
const ALLOWED_OUTPUT_TYPES: &[&str] = &["auto", "docx", "pdf", "pptx"];

// ─── Types ──────────────────────────────────────────────────────────────────

/// Error type for submission operations.
#[derive(Debug, thiserror::Error)]
pub enum SubmissionError {
    /// Teacher not found.
    #[error("teacher not found: {0}")]
    TeacherNotFound(String),

    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// UUID parse error.
    #[error("invalid UUID: {0}")]
    InvalidUuid(String),
}

/// Input for `create_or_reuse`.
#[derive(Debug, Clone)]
pub struct CreateInput {
    pub teacher_id: i64,
    pub raw_prompt: String,
    pub preferred_output_type: Option<String>,
    pub subject_id: Option<i64>,
    pub sub_subject_id: Option<i64>,
    pub provider_metadata: ProviderMetadata,
}

/// Provider metadata extracted from the request (stored in media_generations columns).
#[derive(Debug, Clone, Default)]
pub struct ProviderMetadata {
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub generator_provider: Option<String>,
    pub generator_model: Option<String>,
}

/// Result of `create_or_reuse`.
#[derive(Debug)]
pub struct CreateResult {
    /// The generation UUID (either newly inserted or existing).
    pub id: Uuid,
    /// Whether this generation was freshly created (true) or reused (false).
    pub was_created: bool,
    /// The fingerprint used for deduplication.
    pub request_fingerprint: String,
}

// ─── SubmissionService ──────────────────────────────────────────────────────

/// Service for submitting media generation requests with deduplication via request_fingerprint.
pub struct SubmissionService {
    pool: PgPool,
}

impl SubmissionService {
    /// Create a new submission service.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new media generation or reuse an existing active duplicate.
    ///
    /// 1. Normalizes the preferred output type (→ `normalize_preferred_output_type`).
    /// 2. Computes `request_fingerprint` (SHA-256 of canonicalised input).
    /// 3. In a DB transaction, acquires `FOR UPDATE` lock on potential duplicates.
    /// 4. If an active (non-terminal) generation with the same fingerprint exists, returns it.
    /// 5. Otherwise inserts a new generation with status `queued`.
    /// 6. On unique constraint violation (PG 23505 — `active_duplicate_key`), retries lookup.
    pub async fn create_or_reuse(&self, input: CreateInput) -> Result<CreateResult, SubmissionError> {
        let normalized_type = normalize_preferred_output_type(input.preferred_output_type.as_deref());
        let request_fingerprint = make_request_fingerprint(
            input.teacher_id,
            &input.raw_prompt,
            &normalized_type,
            input.subject_id,
            input.sub_subject_id,
        );

        // active_duplicate_key is always set for new generations (status=queued, non-terminal)
        // Matching Laravel's shouldPreventDuplicateSubmission() which returns true when
        // fingerprint is non-empty and status is not terminal.
        let active_duplicate_key = Some(&request_fingerprint);

        let mut tx = self.pool.begin().await.map_err(SubmissionError::Database)?;

        // Step 1: SELECT FOR UPDATE on active duplicates
        let existing = sqlx::query_as::<_, (Uuid,)>(
            r#"
            SELECT id
            FROM media_generations
            WHERE teacher_id = $1
              AND request_fingerprint = $2
              AND status NOT IN ('completed', 'failed', 'cancelled')
            ORDER BY created_at DESC, id DESC
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(input.teacher_id)
        .bind(&request_fingerprint)
        .fetch_optional(&mut *tx)
        .await
        .map_err(SubmissionError::Database)?;

        if let Some((existing_id,)) = existing {
            tx.commit().await.map_err(SubmissionError::Database)?;
            return Ok(CreateResult {
                id: existing_id,
                was_created: false,
                request_fingerprint,
            });
        }

        // Step 2: Try to insert a new generation
        let insert_result = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO media_generations (
                teacher_id,
                subject_id,
                sub_subject_id,
                raw_prompt,
                request_fingerprint,
                active_duplicate_key,
                preferred_output_type,
                status,
                llm_provider,
                llm_model,
                generator_provider,
                generator_model
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, 'queued',
                $8, $9, $10, $11
            )
            RETURNING id
            "#,
        )
        .bind(input.teacher_id)
        .bind(input.subject_id)
        .bind(input.sub_subject_id)
        .bind(&input.raw_prompt)
        .bind(&request_fingerprint)
        .bind(active_duplicate_key)
        .bind(&normalized_type)
        .bind(&input.provider_metadata.llm_provider)
        .bind(&input.provider_metadata.llm_model)
        .bind(&input.provider_metadata.generator_provider)
        .bind(&input.provider_metadata.generator_model)
        .fetch_optional(&mut *tx)
        .await;

        match insert_result {
            Ok(Some(new_id)) => {
                tx.commit().await.map_err(SubmissionError::Database)?;
                Ok(CreateResult {
                    id: new_id,
                    was_created: true,
                    request_fingerprint,
                })
            }
            Ok(None) => {
                // Should not happen with RETURNING clause, but handle gracefully
                tx.rollback().await.map_err(SubmissionError::Database)?;
                Err(SubmissionError::Database(sqlx::Error::Protocol(
                    "INSERT RETURNING returned no rows".into(),
                )))
            }
            Err(sqlx::Error::Database(db_err))
                if db_err.code().as_deref() == Some("23505") =>
            {
                // Unique constraint violation on active_duplicate_key — retry lookup
                tx.rollback().await.map_err(SubmissionError::Database)?;

                // Retry lookup in a new transaction
                let mut retry_tx = self.pool.begin().await.map_err(SubmissionError::Database)?;

                let duplicate = sqlx::query_as::<_, (Uuid,)>(
                    r#"
                    SELECT id
                    FROM media_generations
                    WHERE teacher_id = $1
                      AND request_fingerprint = $2
                      AND status NOT IN ('completed', 'failed', 'cancelled')
                    ORDER BY created_at DESC, id DESC
                    LIMIT 1
                    FOR UPDATE
                    "#,
                )
                .bind(input.teacher_id)
                .bind(&request_fingerprint)
                .fetch_optional(&mut *retry_tx)
                .await
                .map_err(SubmissionError::Database)?;

                if let Some((dup_id,)) = duplicate {
                    retry_tx.commit().await.map_err(SubmissionError::Database)?;
                    Ok(CreateResult {
                        id: dup_id,
                        was_created: false,
                        request_fingerprint,
                    })
                } else {
                    // The race was not a duplicate — some other constraint failed
                    retry_tx
                        .rollback()
                        .await
                        .map_err(SubmissionError::Database)?;
                    Err(SubmissionError::Database(
                        sqlx::Error::Database(db_err),
                    ))
                }
            }
            Err(e) => {
                tx.rollback().await.map_err(SubmissionError::Database)?;
                Err(SubmissionError::Database(e))
            }
        }
    }

    /// Create a regeneration from an existing (terminal) parent generation.
    ///
    /// Combines the parent's prompt with the additional refinement prompt
    /// in the format: `"Original Request:\n{parent_prompt}\n\nRefinement / Additional context:\n{additional_prompt}"`
    /// The new generation is linked via `generated_from_id` and `is_regeneration = true`.
    pub async fn create_regeneration(
        &self,
        parent_generation_id: &str,
        additional_prompt: &str,
    ) -> Result<Uuid, SubmissionError> {
        let parent_id = parse_uuid(parent_generation_id).map_err(|e| {
            SubmissionError::InvalidUuid(e.to_string())
        })?;

        // Fetch the parent generation's data
        let parent = sqlx::query_as::<_, (i64, String, Option<i64>, Option<i64>, Option<String>)>(
            r#"
            SELECT teacher_id, raw_prompt, subject_id, sub_subject_id, preferred_output_type
            FROM media_generations
            WHERE id = $1
            "#,
        )
        .bind(parent_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(SubmissionError::Database)?;

        let (teacher_id, raw_prompt, subject_id, sub_subject_id, preferred_output_type) = parent
            .ok_or_else(|| {
                SubmissionError::InvalidUuid(format!(
                    "parent generation not found: {}",
                    parent_generation_id
                ))
            })?;

        let normalized_type = preferred_output_type
            .as_deref()
            .map(|t| t.to_string())
            .unwrap_or_else(|| "auto".to_string());

        let combined_prompt = format!(
            "Original Request:\n{}\n\nRefinement / Additional context:\n{}",
            raw_prompt, additional_prompt
        );

        // Compute request_fingerprint from parent's teacher_id + combined prompt
        // (matching Laravel's booted saving handler)
        let regeneration_fingerprint = make_request_fingerprint(
            teacher_id,
            &combined_prompt,
            &normalized_type,
            subject_id,
            sub_subject_id,
        );

        let new_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO media_generations (
                teacher_id,
                generated_from_id,
                is_regeneration,
                subject_id,
                sub_subject_id,
                raw_prompt,
                request_fingerprint,
                active_duplicate_key,
                preferred_output_type,
                status
            )
            SELECT
                teacher_id,
                $1,
                TRUE,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                'queued'
            FROM media_generations
            WHERE id = $1
            RETURNING id
            "#,
        )
        .bind(parent_id)
        .bind(subject_id)
        .bind(sub_subject_id)
        .bind(&combined_prompt)
        .bind(&regeneration_fingerprint)
        .bind(Some(&regeneration_fingerprint))
        .bind(&normalized_type)
        .fetch_one(&self.pool)
        .await
        .map_err(SubmissionError::Database)?;

        Ok(new_id)
    }
}

// ─── Helper functions ───────────────────────────────────────────────────────

/// Normalize a preferred output type.
///
/// Trims and lowercases the value. Falls back to `"auto"` if:
/// - The value is `None`, empty, or whitespace-only.
/// - The value is not in the allowed set `["auto", "docx", "pdf", "pptx"]`.
///
/// Matches Laravel's `MediaGeneration::normalizePreferredOutputType()` EXACTLY.
pub fn normalize_preferred_output_type(preferred_output_type: Option<&str>) -> String {
    let Some(raw) = preferred_output_type else {
        return "auto".to_string();
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "auto".to_string();
    }

    let normalized = trimmed.to_lowercase();
    if !ALLOWED_OUTPUT_TYPES.contains(&normalized.as_str()) {
        return "auto".to_string();
    }

    normalized
}

/// Compute a deterministic request fingerprint for deduplication.
///
/// The fingerprint is SHA-256 hex of the canonical form:
/// `{teacher_id}|{normalized_preferred_output_type}|{subject_id or 'none'}|{sub_subject_id or 'none'}|{squished_lowercase_prompt}`
///
/// Matches Laravel's `MediaGeneration::makeRequestFingerprint()` EXACTLY.
pub fn make_request_fingerprint(
    teacher_id: i64,
    raw_prompt: &str,
    preferred_output_type: &str,
    subject_id: Option<i64>,
    sub_subject_id: Option<i64>,
) -> String {
    let squished_prompt = squish_and_lower(raw_prompt);
    let subject_str = subject_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "none".to_string());
    let sub_subject_str = sub_subject_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "none".to_string());

    let canonical = format!(
        "{}|{}|{}|{}|{}",
        teacher_id, preferred_output_type, subject_str, sub_subject_str, squished_prompt
    );

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

/// Squish (collapse whitespace, trim) and lowercase a string.
///
/// Matches Laravel's `Str::of($s)->squish()->lower()`.
fn squish_and_lower(s: &str) -> String {
    let collapsed: String = s
        .chars()
        .fold((String::with_capacity(s.len()), false), |(mut acc, prev_ws), c| {
            if c.is_whitespace() {
                if !prev_ws {
                    acc.push(' ');
                }
                (acc, true)
            } else {
                for lower_c in c.to_lowercase() {
                    acc.push(lower_c);
                }
                (acc, false)
            }
        })
        .0;

    collapsed.trim().to_string()
}

/// Check if a generation status is terminal (completed, failed, cancelled).
pub fn is_terminal_status(status: &str) -> bool {
    matches!(status, "completed" | "failed" | "cancelled")
}

/// Parse a UUID string.
fn parse_uuid(s: &str) -> Result<Uuid, uuid::Error> {
    Uuid::parse_str(s)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── normalize_preferred_output_type ──────────────────────────────────────

    #[test]
    fn test_normalize_auto_returns_auto() {
        assert_eq!(normalize_preferred_output_type(Some("auto")), "auto");
        assert_eq!(normalize_preferred_output_type(Some("AUTO")), "auto");
        assert_eq!(normalize_preferred_output_type(Some(" Auto ")), "auto");
    }

    #[test]
    fn test_normalize_docx() {
        assert_eq!(normalize_preferred_output_type(Some("docx")), "docx");
        assert_eq!(normalize_preferred_output_type(Some("DOCX")), "docx");
    }

    #[test]
    fn test_normalize_pdf() {
        assert_eq!(normalize_preferred_output_type(Some("pdf")), "pdf");
    }

    #[test]
    fn test_normalize_pptx() {
        assert_eq!(normalize_preferred_output_type(Some("pptx")), "pptx");
    }

    #[test]
    fn test_normalize_none_returns_auto() {
        assert_eq!(normalize_preferred_output_type(None), "auto");
    }

    #[test]
    fn test_normalize_empty_returns_auto() {
        assert_eq!(normalize_preferred_output_type(Some("")), "auto");
        assert_eq!(normalize_preferred_output_type(Some("   ")), "auto");
    }

    #[test]
    fn test_normalize_invalid_falls_back_to_auto() {
        assert_eq!(normalize_preferred_output_type(Some("invalid")), "auto");
        assert_eq!(normalize_preferred_output_type(Some("html")), "auto");
        assert_eq!(normalize_preferred_output_type(Some("xlsx")), "auto");
    }

    // ── make_request_fingerprint ─────────────────────────────────────────────

    #[test]
    fn test_fingerprint_is_deterministic() {
        let fp1 = make_request_fingerprint(1, "Buatkan materi pecahan", "auto", Some(2), Some(5));
        let fp2 = make_request_fingerprint(1, "Buatkan materi pecahan", "auto", Some(2), Some(5));
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_fingerprint_changes_with_teacher() {
        let fp1 = make_request_fingerprint(1, "Buatkan materi pecahan", "auto", Some(2), Some(5));
        let fp2 = make_request_fingerprint(2, "Buatkan materi pecahan", "auto", Some(2), Some(5));
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_fingerprint_changes_with_prompt() {
        let fp1 = make_request_fingerprint(1, "Buatkan materi pecahan", "auto", Some(2), Some(5));
        let fp2 = make_request_fingerprint(1, "Buatkan materi bangun datar", "auto", Some(2), Some(5));
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_fingerprint_changes_with_output_type() {
        let fp1 = make_request_fingerprint(1, "Buatkan materi pecahan", "auto", Some(2), Some(5));
        let fp2 = make_request_fingerprint(1, "Buatkan materi pecahan", "pdf", Some(2), Some(5));
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_fingerprint_changes_with_subject() {
        let fp1 = make_request_fingerprint(1, "Buatkan materi pecahan", "auto", Some(2), Some(5));
        let fp2 = make_request_fingerprint(1, "Buatkan materi pecahan", "auto", Some(3), Some(5));
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_fingerprint_is_64_char_hex() {
        let fp = make_request_fingerprint(1, "test prompt", "auto", None, None);
        assert_eq!(fp.len(), 64);
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_fingerprint_uses_none_when_ids_missing() {
        let fp = make_request_fingerprint(1, "test", "auto", None, None);
        // Should not panic with None IDs
        assert_eq!(fp.len(), 64);
    }

    #[test]
    fn test_fingerprint_prompt_normalization() {
        // Different casing + extra whitespace should produce same fingerprint
        let fp1 = make_request_fingerprint(1, "Buatkan Materi Pecahan", "auto", None, None);
        let fp2 = make_request_fingerprint(1, "  buatkan   materi   pecahan  ", "auto", None, None);
        assert_eq!(fp1, fp2);
    }

    // ── squish_and_lower ─────────────────────────────────────────────────────

    #[test]
    fn test_squish_and_lower_collapses_whitespace() {
        assert_eq!(squish_and_lower("Hello    World"), "hello world");
        assert_eq!(squish_and_lower("  leading and trailing  "), "leading and trailing");
    }

    #[test]
    fn test_squish_and_lower_lowercases() {
        assert_eq!(squish_and_lower("HELLO World"), "hello world");
        assert_eq!(squish_and_lower("Buatkan Materi"), "buatkan materi");
    }

    #[test]
    fn test_squish_and_lower_empty() {
        assert_eq!(squish_and_lower(""), "");
        assert_eq!(squish_and_lower("   "), "");
    }

    // ── is_terminal_status ──────────────────────────────────────────────────

    #[test]
    fn test_is_terminal_status_true() {
        assert!(is_terminal_status("completed"));
        assert!(is_terminal_status("failed"));
        assert!(is_terminal_status("cancelled"));
    }

    #[test]
    fn test_is_terminal_status_false() {
        assert!(!is_terminal_status("queued"));
        assert!(!is_terminal_status("interpreting"));
        assert!(!is_terminal_status("generating"));
        assert!(!is_terminal_status(""));
    }

    // ── parse_uuid ──────────────────────────────────────────────────────────

    #[test]
    fn test_parse_uuid_valid() {
        let id = "00000000-0000-0000-0000-000000000001";
        assert!(parse_uuid(id).is_ok());
    }

    #[test]
    fn test_parse_uuid_invalid() {
        assert!(parse_uuid("not-a-uuid").is_err());
    }

    // ── CreateResult ────────────────────────────────────────────────────────

    #[test]
    fn test_create_result_fields() {
        let id = Uuid::new_v4();
        let result = CreateResult {
            id,
            was_created: true,
            request_fingerprint: "abc".to_string(),
        };
        assert_eq!(result.id, id);
        assert!(result.was_created);
        assert_eq!(result.request_fingerprint, "abc");
    }

    #[test]
    fn test_create_result_not_created() {
        let result = CreateResult {
            id: Uuid::new_v4(),
            was_created: false,
            request_fingerprint: "def".to_string(),
        };
        assert!(!result.was_created);
    }
}

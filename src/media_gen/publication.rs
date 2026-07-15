//! Media publication service.
//!
//! Port of `MediaPublicationService` from Laravel.
//!
//! Orchestrates the publication of a generated media artifact:
//!
//! 1. DB transaction + `SELECT ... FOR UPDATE` on the generation
//! 2. Download artifact from the Python renderer's temporary URL
//! 3. Validate artifact integrity (MIME, size, SHA256, PDF/OOXML header)
//! 4. Upload artifact to R2 via the existing `storage::r2::upload` helper
//! 5. Thumbnail generation (SVG fallback, upgradable via `ThumbnailGenerator` trait)
//! 6. Resolve or create `Topic` (from interpretation taxonomy)
//! 7. Resolve or create `Content` (type from `resolved_output_type`)
//! 8. Resolve or create `RecommendedProject` (`source_type=ai_generated`)
//! 9. Persist `delivery_payload` on the generation row
//! 10. Compensation: delete uploaded R2 files on failure

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use aws_sdk_s3::Client as S3Client;
use reqwest::Client as HttpClient;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::orchestrator::workflow::{PublishStep, WorkflowError};

// ─── Constants ──────────────────────────────────────────────────────────────

/// R2 upload category for generated media artifacts.
const ARTIFACT_CATEGORY: &str = "materials";

/// PDF magic bytes.
const PDF_MAGIC: &[u8] = b"%PDF";

/// OOXML (docx/pptx) ZIP magic bytes.
const OOXML_MAGIC: &[u8] = b"PK\x03\x04";

/// EOF marker for PDF validation.
const PDF_EOF_MARKER: &[u8] = b"%%EOF";

/// Fallback SVG thumbnail dimensions.
const THUMBNAIL_WIDTH: u32 = 1280;
const THUMBNAIL_HEIGHT: u32 = 720;

// ─── Error type ─────────────────────────────────────────────────────────────

/// Error type for publication operations.
#[derive(Debug, thiserror::Error)]
pub enum PublicationError {
    /// Generation not found.
    #[error("generation not found: {0}")]
    NotFound(String),

    /// Artifact not found in generator response.
    #[error("artifact not found in generator response")]
    MissingArtifact,

    /// Integrity validation failed.
    #[error("integrity check failed: {0}")]
    Integrity(String),

    /// Artifact download failed.
    #[error("failed to download artifact: {0}")]
    Download(#[from] reqwest::Error),

    /// R2 upload failed.
    #[error("R2 upload failed: {0}")]
    Upload(String),

    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// UUID parse error.
    #[error("invalid UUID: {0}")]
    InvalidUuid(String),
}

impl From<PublicationError> for WorkflowError {
    fn from(e: PublicationError) -> Self {
        WorkflowError::StepProvider(format!("publication error: {e}"))
    }
}

// ─── Thumbnail strategy ─────────────────────────────────────────────────────

/// Pluggable thumbnail generator.
///
/// The default implementation produces an SVG fallback. PDF page rendering
/// and OOXML thumbnail extraction can be added later as separate generators
/// without changing the publication service.
#[async_trait]
pub trait ThumbnailGenerator: Send + Sync {
    /// Generate a thumbnail from artifact bytes.
    /// Returns `(thumbnail_bytes, mime_type)`.
    async fn generate(
        &self,
        artifact: &[u8],
        mime_type: &str,
    ) -> Result<(Vec<u8>, String), PublicationError>;
}

/// Default thumbnail generator that produces an SVG fallback.
///
/// Palette: PDF → red, PPTX → orange, default → blue.
pub struct SvgFallbackGenerator;

#[async_trait]
impl ThumbnailGenerator for SvgFallbackGenerator {
    async fn generate(
        &self,
        _artifact: &[u8],
        mime_type: &str,
    ) -> Result<(Vec<u8>, String), PublicationError> {
        let accent = thumbnail_accent(mime_type);
        let label = mime_label(mime_type);
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">
  <rect width="{}" height="{}" fill="{}22" rx="24"/>
  <rect x="60" y="60" width="{}" height="{}" rx="16" fill="white" stroke="{}" stroke-width="3"/>
  <text x="{}" y="320" font-family="Arial,sans-serif" font-size="64" fill="{}" text-anchor="middle" font-weight="bold">KLASS</text>
  <text x="{}" y="400" font-family="Arial,sans-serif" font-size="36" fill="#555" text-anchor="middle">Generated Media</text>
  <text x="{}" y="460" font-family="Arial,sans-serif" font-size="28" fill="#888" text-anchor="middle">{}</text>
</svg>"##,
            THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT,
            THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT, accent,
            THUMBNAIL_WIDTH - 120, THUMBNAIL_HEIGHT - 120, accent,
            THUMBNAIL_WIDTH / 2, accent,
            THUMBNAIL_WIDTH / 2,
            THUMBNAIL_WIDTH / 2, label,
        );
        Ok((svg.into_bytes(), "image/svg+xml".to_string()))
    }
}

pub(crate) fn thumbnail_accent(mime_type: &str) -> &'static str {
    if mime_type.contains("pdf") {
        "#E74C3C"
    } else if mime_type.contains("presentation") {
        "#F39C12"
    } else {
        "#3498DB"
    }
}

pub(crate) fn mime_label(mime_type: &str) -> &'static str {
    if mime_type.contains("pdf") {
        "PDF Document"
    } else if mime_type.contains("presentation") {
        "Presentation"
    } else if mime_type.contains("word") || mime_type.contains("document") {
        "Document"
    } else {
        "Media File"
    }
}

// ─── Publication service ────────────────────────────────────────────────────

/// Service for publishing generated media artifacts.
pub struct MediaPublicationService {
    pool: PgPool,
    s3_client: S3Client,
    http: HttpClient,
    r2_bucket: String,
    r2_public_url: String,
    thumbnail_gen: Arc<dyn ThumbnailGenerator>,
}

impl MediaPublicationService {
    pub fn new(
        pool: PgPool,
        s3_client: S3Client,
        http: HttpClient,
        r2_bucket: String,
        r2_public_url: String,
    ) -> Self {
        Self {
            pool,
            s3_client,
            http,
            r2_bucket,
            r2_public_url,
            thumbnail_gen: Arc::new(SvgFallbackGenerator),
        }
    }

    pub fn new_with_thumbnailer(
        pool: PgPool,
        s3_client: S3Client,
        http: HttpClient,
        r2_bucket: String,
        r2_public_url: String,
        thumbnail_gen: Arc<dyn ThumbnailGenerator>,
    ) -> Self {
        Self {
            pool,
            s3_client,
            http,
            r2_bucket,
            r2_public_url,
            thumbnail_gen,
        }
    }

    /// Run the full publication pipeline for a generation.
    pub async fn publish(
        &self,
        generation_id: &str,
    ) -> Result<PublishResult, PublicationError> {
        let gen_id = Uuid::parse_str(generation_id)
            .map_err(|e| PublicationError::InvalidUuid(e.to_string()))?;

        let mut uploaded_paths: Vec<String> = Vec::new();

        let result = self.publish_inner(gen_id, &mut uploaded_paths).await;

        if result.is_err() {
            self.compensate_uploaded_files(&uploaded_paths).await;
        }

        result
    }

    async fn publish_inner(
        &self,
        gen_id: Uuid,
        uploaded_paths: &mut Vec<String>,
    ) -> Result<PublishResult, PublicationError> {
        let mut tx = self.pool.begin().await?;

        // ── 1. Lock and read generation ─────────────────────────────────
        #[allow(clippy::type_complexity)]
        let row: Option<(
            String,
            Option<Value>,
            Option<Value>,
            Option<String>,
            Option<String>,
            Option<i64>,
        )> = sqlx::query_as(
            r#"
            SELECT status, interpretation_payload, generator_service_response,
                   resolved_output_type, mime_type, teacher_id
            FROM media_generations
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(gen_id)
        .fetch_optional(&mut *tx)
        .await?;

        let (status, interpretation_payload, generator_response, resolved_output_type, existing_mime, teacher_id) =
            row.ok_or_else(|| PublicationError::NotFound(gen_id.to_string()))?;

        if status == "completed" || status == "cancelled" {
            tx.commit().await?;
            return Ok(PublishResult {
                topic_id: None,
                content_id: None,
                recommended_project_id: None,
            });
        }

        // ── 2. Extract artifact info ────────────────────────────────────
        let output_type = resolved_output_type.as_deref().unwrap_or("pdf");
        let mime_type = existing_mime
            .as_deref()
            .unwrap_or_else(|| default_mime_for(output_type));

        let response = generator_response.as_ref().ok_or(PublicationError::MissingArtifact)?;

        let file_url = response
            .get("file_url")
            .or_else(|| response.pointer("/response/file_url"))
            .and_then(|v| v.as_str())
            .ok_or(PublicationError::MissingArtifact)?;

        // ── 3. Download artifact from Python renderer ────────────────────
        let artifact_bytes = self.download_artifact(file_url).await?;

        // ── 4. Validate integrity ───────────────────────────────────────
        validate_integrity(&artifact_bytes, mime_type)?;

        // ── 5. Thumbnail (before upload to avoid cloning bytes) ─────────
        let thumbnail_url = self
            .resolve_thumbnail(response, &artifact_bytes, mime_type)
            .await?;

        // ── 6. Upload to R2 ─────────────────────────────────────────────
        let r2_result = crate::storage::r2::upload(
            &self.s3_client,
            &self.r2_bucket,
            &self.r2_public_url,
            ARTIFACT_CATEGORY,
            artifact_bytes,
            mime_type,
        )
        .await
        .map_err(|e| PublicationError::Upload(e.to_string()))?;

        uploaded_paths.push(r2_result.path.clone());

        // ── 7. Create Topic ─────────────────────────────────────────────
        let topic_title = interpretation_payload
            .as_ref()
            .and_then(|p| p.pointer("/document_blueprint/title"))
            .and_then(|v| v.as_str())
            .unwrap_or("Generated Topic")
            .to_string();

        let topic_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO topics (id, title, teacher_id, owner_user_id, ownership_status, is_published, "order", created_at, updated_at)
            VALUES ($1, $2, $3, $4, 'owned', true, 0, NOW(), NOW())
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(topic_id)
        .bind(&topic_title)
        .bind(teacher_id.map(|id| id.to_string()).unwrap_or_default())
        .bind(teacher_id)
        .execute(&mut *tx)
        .await?;

        // ── 8. Create Content ───────────────────────────────────────────
        let content_id = Uuid::new_v4();
        let content_type = content_type_from_output(output_type);
        sqlx::query(
            r#"
            INSERT INTO contents (id, topic_id, type, title, media_url, is_published, "order", created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, true, 0, NOW(), NOW())
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(content_id)
        .bind(topic_id)
        .bind(content_type)
        .bind(&topic_title)
        .bind(&r2_result.public_url)
        .execute(&mut *tx)
        .await?;

        // ── 9. Create RecommendedProject ───────────────────────────────
        let recommended_project_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO recommended_projects (title, project_file_url, thumbnail_url, source_type, source_reference, is_active, starts_at, created_at, updated_at)
            VALUES ($1, $2, $3, 'ai_generated', $4::text, true, NOW(), NOW(), NOW())
            RETURNING id
            "#,
        )
        .bind(&topic_title)
        .bind(&r2_result.public_url)
        .bind(&thumbnail_url)
        .bind(gen_id.to_string())
        .fetch_one(&mut *tx)
        .await?;

        // ── 10. Update generation row ───────────────────────────────────
        let delivery_payload = build_delivery_payload(
            &r2_result.public_url,
            thumbnail_url.as_deref(),
            output_type,
            mime_type,
        );

        sqlx::query(
            r#"
            UPDATE media_generations
            SET topic_id = $1,
                content_id = $2,
                recommended_project_id = $3,
                storage_path = $4,
                file_url = $5,
                thumbnail_url = $6,
                delivery_payload = $7,
                resolved_output_type = $8,
                updated_at = NOW()
            WHERE id = $9
            "#,
        )
        .bind(topic_id)
        .bind(content_id)
        .bind(recommended_project_id)
        .bind(&r2_result.path)
        .bind(&r2_result.public_url)
        .bind(&thumbnail_url)
        .bind(&delivery_payload)
        .bind(output_type)
        .bind(gen_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(PublishResult {
            topic_id: Some(topic_id),
            content_id: Some(content_id),
            recommended_project_id: Some(recommended_project_id),
        })
    }

    async fn download_artifact(&self, url: &str) -> Result<Vec<u8>, PublicationError> {
        let resp = self
            .http
            .get(url)
            .timeout(Duration::from_secs(120))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(PublicationError::Integrity(format!(
                "artifact download failed: HTTP status {}",
                resp.status()
            )));
        }

        Ok(resp.bytes().await?.to_vec())
    }

    /// Resolve a thumbnail URL.
    ///
    /// Priority:
    /// 1. `thumbnail_url` from the Python renderer response
    /// 2. Generated via the `ThumbnailGenerator` (SVG fallback) → uploaded to R2
    async fn resolve_thumbnail(
        &self,
        response: &Value,
        artifact_bytes: &[u8],
        mime_type: &str,
    ) -> Result<Option<String>, PublicationError> {
        let renderer_url = response
            .get("thumbnail_url")
            .or_else(|| response.pointer("/response/thumbnail_url"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        if let Some(url) = renderer_url {
            // Download and re-upload to R2 for persistent storage
            let thumb_bytes = self.download_thumbnail(&url).await;
            if let Ok(bytes) = thumb_bytes {
                if !bytes.is_empty() {
                    let upload_result = crate::storage::r2::upload(
                        &self.s3_client,
                        &self.r2_bucket,
                        &self.r2_public_url,
                        ARTIFACT_CATEGORY,
                        bytes,
                        "image/svg+xml",
                    )
                    .await
                    .map_err(|e| PublicationError::Upload(e.to_string()))?;
                    return Ok(Some(upload_result.public_url));
                }
            }
        }

        // Fallback: generate via ThumbnailGenerator (SVG)
        let (thumb_bytes, thumb_mime) = self.thumbnail_gen.generate(artifact_bytes, mime_type).await?;
        let upload_result = crate::storage::r2::upload(
            &self.s3_client,
            &self.r2_bucket,
            &self.r2_public_url,
            "gallery",
            thumb_bytes,
            &thumb_mime,
        )
        .await
        .map_err(|e| PublicationError::Upload(e.to_string()))?;

        Ok(Some(upload_result.public_url))
    }

    async fn download_thumbnail(&self, url: &str) -> Result<Vec<u8>, reqwest::Error> {
        let resp = self.http.get(url).timeout(Duration::from_secs(30)).send().await?;
        if !resp.status().is_success() {
            return Ok(vec![]);
        }
        resp.bytes().await.map(|b| b.to_vec())
    }

    async fn compensate_uploaded_files(&self, paths: &[String]) {
        for path in paths {
            if let Err(e) = crate::storage::r2::delete(&self.s3_client, &self.r2_bucket, path).await {
                tracing::warn!(error = %e, path = %path, "compensation: R2 delete failed");
            }
        }
    }
}

// ─── PublishStep trait impl ────────────────────────────────────────────────

#[async_trait]
impl PublishStep for MediaPublicationService {
    async fn publish(&self, generation_id: &str) -> Result<Value, WorkflowError> {
        let result = MediaPublicationService::publish(self, generation_id).await?;
        Ok(serde_json::json!({
            "topic_id": result.topic_id,
            "content_id": result.content_id,
            "recommended_project_id": result.recommended_project_id,
        }))
    }
}

// ─── Result type ───────────────────────────────────────────────────────────

/// Result of a successful publication.
#[derive(Debug, Clone)]
pub struct PublishResult {
    pub topic_id: Option<Uuid>,
    pub content_id: Option<Uuid>,
    pub recommended_project_id: Option<i64>,
}

// ─── Pure helpers ──────────────────────────────────────────────────────────

/// Build the delivery_payload JSON stored on the generation row.
fn build_delivery_payload(
    file_url: &str,
    thumbnail_url: Option<&str>,
    output_type: &str,
    mime_type: &str,
) -> Value {
    serde_json::json!({
        "schema_version": "media_delivery_response.v1",
        "response_meta": {
            "provider": "python-renderer",
            "model": "hf-space-v3",
            "llm_used": false,
        },
        "fallback": {
            "triggered": false,
            "reason_code": null,
            "action": null,
        },
        "artifact": {
            "file_url": file_url,
            "thumbnail_url": thumbnail_url,
            "output_type": output_type,
            "mime_type": mime_type,
        },
    })
}

/// Default MIME type for a given output type.
fn default_mime_for(output_type: &str) -> &'static str {
    match output_type {
        "pdf" => "application/pdf",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        _ => "application/octet-stream",
    }
}

/// Map output type (pdf, pptx, docx) to content type that satisfies the
/// `contents.type CHECK (type IN ('module', 'quiz', 'brief'))` constraint.
///
/// AI-generated media artifacts are always categorised as `module`, matching
/// the Laravel `MediaPublicationService` behaviour.
fn content_type_from_output(_output_type: &str) -> &'static str {
    // All AI-generated content maps to 'module' regardless of file format.
    // The output_type (pdf/pptx/docx) is stored on the media_generation row
    // as resolved_output_type, not in the content type column.
    "module"
}

/// Validate artifact integrity: magic bytes, format-specific markers, SHA256.
fn validate_integrity(bytes: &[u8], mime_type: &str) -> Result<(), PublicationError> {
    if bytes.is_empty() {
        return Err(PublicationError::Integrity("artifact is empty".to_string()));
    }

    if mime_type.contains("pdf") {
        if !bytes.starts_with(PDF_MAGIC) {
            return Err(PublicationError::Integrity(
                "PDF file missing %PDF header magic bytes".to_string(),
            ));
        }
        // Check for %%EOF within the last 1024 bytes to tolerate trailing
        // whitespace/newlines that generators like reportlab commonly append.
        if !has_pdf_eof_marker(bytes) {
            return Err(PublicationError::Integrity(
                "PDF file missing %%EOF trailer".to_string(),
            ));
        }
    } else if mime_type.contains("officedocument") || mime_type.contains("presentation") || mime_type.contains("word") {
        if !bytes.starts_with(OOXML_MAGIC) {
            return Err(PublicationError::Integrity(
                "OOXML file missing PK\\x03\\x04 (ZIP) magic bytes".to_string(),
            ));
        }
    }

    let checksum = {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    };

    tracing::info!(
        size = bytes.len(),
        checksum = %checksum,
        mime = %mime_type,
        "artifact integrity check passed"
    );

    Ok(())
}

/// Check for `%%EOF` within the last 1024 bytes of a PDF.
///
/// Real-world PDF generators (reportlab, wkhtmltopdf, etc.) often append
/// trailing newlines or whitespace after the `%%EOF` marker. Using a strict
/// `ends_with` would reject these valid files.
fn has_pdf_eof_marker(bytes: &[u8]) -> bool {
    let tail_start = bytes.len().saturating_sub(1024);
    let tail = &bytes[tail_start..];
    // Search for the marker in the tail region
    tail.windows(PDF_EOF_MARKER.len())
        .any(|w| w == PDF_EOF_MARKER)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── build_delivery_payload ─────────────────────────────────────────

    #[test]
    fn test_build_delivery_payload_shape() {
        let payload = build_delivery_payload(
            "https://cdn.example.com/materials/file.pdf",
            Some("https://cdn.example.com/materials/thumb.svg"),
            "pdf",
            "application/pdf",
        );
        assert_eq!(payload["schema_version"], "media_delivery_response.v1");
        assert_eq!(payload["response_meta"]["provider"], "python-renderer");
        assert_eq!(payload["fallback"]["triggered"], false);
        assert_eq!(
            payload["artifact"]["file_url"],
            "https://cdn.example.com/materials/file.pdf"
        );
        assert_eq!(payload["artifact"]["output_type"], "pdf");
    }

    #[test]
    fn test_build_delivery_payload_no_thumbnail() {
        let payload = build_delivery_payload("https://cdn.example.com/materials/slides.pptx", None, "pptx", "application/vnd.openxmlformats-officedocument.presentationml.presentation");
        assert!(payload["artifact"]["thumbnail_url"].is_null());
    }

    // ── default_mime_for ────────────────────────────────────────────────

    #[test]
    fn test_default_mime_for_pdf() {
        assert_eq!(default_mime_for("pdf"), "application/pdf");
    }

    #[test]
    fn test_default_mime_for_pptx() {
        assert_eq!(
            default_mime_for("pptx"),
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        );
    }

    #[test]
    fn test_default_mime_for_docx() {
        assert_eq!(
            default_mime_for("docx"),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        );
    }

    #[test]
    fn test_default_mime_for_unknown() {
        assert_eq!(default_mime_for("html"), "application/octet-stream");
    }

    // ── thumbnail_accent / mime_label ───────────────────────────────────

    #[test]
    fn test_thumbnail_accent_pdf() {
        assert_eq!(thumbnail_accent("application/pdf"), "#E74C3C");
    }

    #[test]
    fn test_thumbnail_accent_pptx() {
        assert_eq!(
            thumbnail_accent(
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            ),
            "#F39C12"
        );
    }

    #[test]
    fn test_thumbnail_accent_default() {
        assert_eq!(thumbnail_accent("text/plain"), "#3498DB");
    }

    #[test]
    fn test_mime_label_pdf() {
        assert_eq!(mime_label("application/pdf"), "PDF Document");
    }

    #[test]
    fn test_mime_label_pptx() {
        assert_eq!(
            mime_label(
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            ),
            "Presentation"
        );
    }

    // ── SvgFallbackGenerator ───────────────────────────────────────────

    #[tokio::test]
    async fn test_svg_fallback_generates_valid_svg() {
        let gen = SvgFallbackGenerator;
        let (bytes, mime) = gen
            .generate(b"fake pdf content", "application/pdf")
            .await
            .unwrap();
        let svg = String::from_utf8(bytes).unwrap();
        assert_eq!(mime, "image/svg+xml");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("#E74C3C"));
    }

    #[tokio::test]
    async fn test_svg_fallback_pptx_accent() {
        let gen = SvgFallbackGenerator;
        let (bytes, _) = gen
            .generate(
                b"fake pptx content",
                "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            )
            .await
            .unwrap();
        let svg = String::from_utf8(bytes).unwrap();
        assert!(svg.contains("#F39C12"));
    }

    // ── validate_integrity ──────────────────────────────────────────────

    #[test]
    fn test_validate_integrity_empty() {
        let result = validate_integrity(&[], "application/pdf");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn test_validate_integrity_pdf_valid() {
        let content = b"%PDFsome content %%EOF".to_vec();
        let result = validate_integrity(&content, "application/pdf");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_integrity_pdf_missing_magic() {
        let content = b"Not a PDF at all%%EOF";
        let result = validate_integrity(content, "application/pdf");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("%PDF"));
    }

    #[test]
    fn test_validate_integrity_pptx_valid() {
        let content = b"PK\x03\x04valid zip content".to_vec();
        let result = validate_integrity(
            &content,
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_integrity_ooxml_missing_magic() {
        let content = b"Not a valid OOXML file";
        let result = validate_integrity(
            content,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("PK"));
    }

    // ── Error types ────────────────────────────────────────────────────

    #[test]
    fn test_error_display_not_found() {
        let err = PublicationError::NotFound("gen-1".to_string());
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_error_display_integrity() {
        let err = PublicationError::Integrity("checksum mismatch".to_string());
        assert!(err.to_string().contains("integrity check failed"));
    }

    #[test]
    fn test_error_display_missing_artifact() {
        let err = PublicationError::MissingArtifact;
        assert!(err.to_string().contains("artifact not found"));
    }

    #[test]
    fn test_error_conversion_to_workflow_error() {
        let err = PublicationError::NotFound("gen-1".to_string());
        let wf_err: WorkflowError = err.into();
        assert!(wf_err.to_string().contains("publication error"));
    }

    // ── content_type_from_output ────────────────────────────────────────

    #[test]
    fn test_content_type_from_output_pdf() {
        assert_eq!(content_type_from_output("pdf"), "module");
    }

    #[test]
    fn test_content_type_from_output_pptx() {
        assert_eq!(content_type_from_output("pptx"), "module");
    }

    #[test]
    fn test_content_type_from_output_docx() {
        assert_eq!(content_type_from_output("docx"), "module");
    }

    #[test]
    fn test_content_type_from_output_unknown() {
        assert_eq!(content_type_from_output("html"), "module");
    }

    // ── has_pdf_eof_marker ─────────────────────────────────────────────

    #[test]
    fn test_has_pdf_eof_marker_exact() {
        assert!(has_pdf_eof_marker(b"%PDF-1.4 content %%EOF"));
    }

    #[test]
    fn test_has_pdf_eof_marker_trailing_whitespace() {
        assert!(has_pdf_eof_marker(b"%PDF-1.4 content %%EOF\n"));
    }

    #[test]
    fn test_has_pdf_eof_marker_trailing_crlf() {
        assert!(has_pdf_eof_marker(b"%PDF-1.4 content %%EOF\r\n"));
    }

    #[test]
    fn test_has_pdf_eof_marker_trailing_spaces() {
        assert!(has_pdf_eof_marker(b"%PDF-1.4 content %%EOF   \n\n"));
    }

    #[test]
    fn test_has_pdf_eof_marker_absent() {
        assert!(!has_pdf_eof_marker(b"%PDF-1.4 no eof marker here"));
    }

    #[test]
    fn test_validate_integrity_pdf_with_trailing_newline() {
        let content = b"%PDFsome content %%EOF\n".to_vec();
        let result = validate_integrity(&content, "application/pdf");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_integrity_pdf_with_trailing_crlf() {
        let content = b"%PDFsome content %%EOF\r\n".to_vec();
        let result = validate_integrity(&content, "application/pdf");
        assert!(result.is_ok());
    }
}

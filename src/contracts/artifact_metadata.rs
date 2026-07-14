//! `MediaArtifactMetadataContract` — metadata returned by the Python renderer.
//!
//! Describes the generated artifact file (size, checksum, page count, etc.).

use garde::Validate;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "media_artifact_metadata.v1";

// ─── Main payload ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ArtifactMetadata {
    #[garde(length(min = 1))]
    pub schema_version: String,
    /// Original filename.
    #[garde(length(min = 1, max = 255))]
    pub filename: String,
    /// MIME type of the artifact.
    #[garde(length(min = 1, max = 100))]
    pub mime_type: String,
    /// File size in bytes.
    #[garde(skip)]
    pub size_bytes: i64,
    /// SHA-256 checksum (hex).
    #[garde(length(min = 64, max = 64))]
    pub checksum_sha256: String,
    /// Output type (pdf, docx, pptx).
    #[garde(length(min = 1, max = 20))]
    pub output_type: String,
    /// Number of pages (PDF) or slides (PPTX).
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_count: Option<i32>,
    /// List of supported features/extras.
    #[garde(skip)]
    #[serde(default)]
    pub features: Vec<String>,
    /// Provider info.
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator_provider: Option<String>,
    /// Model version used.
    #[garde(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator_model: Option<String>,
}

// ─── Public API ─────────────────────────────────────────────────────────────

use crate::contracts::prompt_interpretation::ContractValidationError;

/// Decode a raw JSON string into validated `ArtifactMetadata`.
pub fn decode_and_validate(raw_json: &str) -> Result<ArtifactMetadata, ContractValidationError> {
    let trimmed = raw_json.trim();
    if trimmed.is_empty() {
        return Err(ContractValidationError {
            code: "empty_completion",
            message: "Artifact metadata was empty.".to_string(),
            details: serde_json::json!({"reason": "empty_completion"}),
            raw_completion: raw_json.to_string(),
        });
    }

    let metadata: ArtifactMetadata = serde_json::from_str(trimmed).map_err(|e| {
        ContractValidationError {
            code: "artifact_metadata_invalid",
            message: format!("Failed to decode artifact metadata as JSON: {}", e),
            details: serde_json::json!({"json_error": e.to_string()}),
            raw_completion: raw_json.to_string(),
        }
    })?;

    if let Err(errors) = metadata.validate() {
        return Err(ContractValidationError {
            code: "artifact_metadata_invalid",
            message: "Artifact metadata failed validation.".to_string(),
            details: serde_json::json!({"errors": errors.to_string()}),
            raw_completion: raw_json.to_string(),
        });
    }

    Ok(metadata)
}

/// Build a fallback artifact metadata when the renderer response is missing or invalid.
pub fn fallback(filename: &str, mime_type: &str) -> ArtifactMetadata {
    ArtifactMetadata {
        schema_version: SCHEMA_VERSION.to_string(),
        filename: filename.to_string(),
        mime_type: mime_type.to_string(),
        size_bytes: 0,
        checksum_sha256: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        output_type: if mime_type.contains("pdf") {
            "pdf".to_string()
        } else if mime_type.contains("presentation") || mime_type.contains("pptx") {
            "pptx".to_string()
        } else {
            "docx".to_string()
        },
        page_count: None,
        features: vec![],
        generator_provider: None,
        generator_model: None,
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_version() {
        assert_eq!(SCHEMA_VERSION, "media_artifact_metadata.v1");
    }

    #[test]
    fn test_fallback_defaults() {
        let m = fallback("output.pdf", "application/pdf");
        assert_eq!(m.output_type, "pdf");
        assert_eq!(m.size_bytes, 0);
    }

    #[test]
    fn test_decode_valid_metadata() {
        let json = r#"{
            "schema_version": "media_artifact_metadata.v1",
            "filename": "materi-pecahan.pdf",
            "mime_type": "application/pdf",
            "size_bytes": 204800,
            "checksum_sha256": "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
            "output_type": "pdf",
            "page_count": 5,
            "features": ["watermark"],
            "generator_provider": "python-renderer",
            "generator_model": "v2"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_ok(), "decode failed: {:?}", result.err());
        let meta = result.unwrap();
        assert_eq!(meta.filename, "materi-pecahan.pdf");
        assert_eq!(meta.size_bytes, 204800);
        assert_eq!(meta.page_count, Some(5));
    }

    #[test]
    fn test_decode_invalid_checksum_length() {
        let json = r#"{
            "schema_version": "media_artifact_metadata.v1",
            "filename": "test.pdf",
            "mime_type": "application/pdf",
            "size_bytes": 100,
            "checksum_sha256": "tooshort",
            "output_type": "pdf"
        }"#;
        let result = decode_and_validate(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_empty_returns_error() {
        let result = decode_and_validate("");
        assert!(result.is_err());
    }
}

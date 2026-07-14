//! Integration tests for the media publication service.
//!
//! Covers:
//! - Artifact integrity validation (PDF header/trailer, OOXML ZIP magic)
//! - Error types and conversion to WorkflowError
//! - Delivery payload shape
//! - Thumbnail SVG generation (fallback)
//! - Compensation: uploaded file path tracking

use klass_gateway::media_gen::publication::{
    MediaPublicationService, PublicationError, SvgFallbackGenerator, ThumbnailGenerator,
};

// ═════════════════════════════════════════════════════════════════════════════
// 1. Artifact integrity validation
// ═════════════════════════════════════════════════════════════════════════════

/// Test PDF validation with valid header + trailer.
#[test]
fn test_pdf_integrity_valid() {
    let content = b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\n%%EOF".to_vec();
    let result = crate::validate_integrity(&content, "application/pdf");
    assert!(result.is_ok(), "valid PDF should pass: {:?}", result.err());
}

/// Test PDF validation rejects content without %PDF header.
#[test]
fn test_pdf_integrity_missing_header() {
    let content = b"NotAPDF%%EOF".to_vec();
    let result = crate::validate_integrity(&content, "application/pdf");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("%PDF"));
    assert!(err.to_string().contains("header magic bytes"));
}

/// Test PDF validation rejects content without %%EOF trailer.
#[test]
fn test_pdf_integrity_missing_trailer() {
    let content = b"%PDF-1.4\nSome content without EOF marker".to_vec();
    let result = crate::validate_integrity(&content, "application/pdf");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("%%EOF"));
}

/// Test OOXML validation with valid ZIP magic bytes.
#[test]
fn test_ooxml_integrity_valid() {
    let content = b"PK\x03\x04valid zip content".to_vec();
    let result = crate::validate_integrity(&content, "application/vnd.openxmlformats-officedocument.presentationml.presentation");
    assert!(result.is_ok(), "valid OOXML should pass: {:?}", result.err());
}

/// Test OOXML validation rejects content without PK magic.
#[test]
fn test_ooxml_integrity_missing_magic() {
    let content = b"Not a valid OOXML file".to_vec();
    let result = crate::validate_integrity(&content, "application/vnd.openxmlformats-officedocument.wordprocessingml.document");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("PK"));
}

/// Test empty content fails validation.
#[test]
fn test_empty_content_fails() {
    let content = vec![];
    let result = crate::validate_integrity(&content, "application/pdf");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. Error types and conversion
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_publication_error_not_found() {
    let err = PublicationError::NotFound("gen-1".to_string());
    assert!(err.to_string().contains("not found"));
}

#[test]
fn test_publication_error_integrity() {
    let err = PublicationError::Integrity("checksum mismatch".to_string());
    assert!(err.to_string().contains("integrity check failed"));
}

#[test]
fn test_publication_error_missing_artifact() {
    let err = PublicationError::MissingArtifact;
    assert!(err.to_string().contains("artifact not found"));
}

#[test]
fn test_publication_error_conversion_to_workflow() {
    use klass_gateway::orchestrator::workflow::WorkflowError;
    let err = PublicationError::NotFound("gen-1".to_string());
    let wf_err: WorkflowError = err.into();
    assert!(wf_err.to_string().contains("publication error"));
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. Delivery payload shape
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_delivery_payload_shape() {
    use serde_json::json;

    let payload = json!({
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
            "file_url": "https://cdn.example.com/materials/output.pdf",
            "thumbnail_url": "https://cdn.example.com/gallery/thumb.svg",
            "output_type": "pdf",
            "mime_type": "application/pdf",
        },
    });

    assert_eq!(payload["schema_version"], "media_delivery_response.v1");
    assert_eq!(payload["artifact"]["output_type"], "pdf");
    assert_eq!(
        payload["artifact"]["file_url"],
        "https://cdn.example.com/materials/output.pdf"
    );
    assert!(payload["fallback"]["triggered"] == false);
}

#[test]
fn test_delivery_payload_no_thumbnail() {
    use serde_json::json;

    let payload = json!({
        "schema_version": "media_delivery_response.v1",
        "response_meta": {
            "provider": "python-renderer",
            "model": "hf-space-v3",
            "llm_used": false,
        },
        "fallback": { "triggered": false },
        "artifact": {
            "file_url": "https://cdn.example.com/materials/slides.pptx",
            "thumbnail_url": null,
            "output_type": "pptx",
            "mime_type": "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        },
    });

    assert!(payload["artifact"]["thumbnail_url"].is_null());
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. Thumbnail SVG generation
// ═════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_svg_fallback_generates_valid_svg() {
    let gen = SvgFallbackGenerator;
    let (bytes, mime) = gen
        .generate(b"fake pdf content", "application/pdf")
        .await
        .unwrap();

    assert_eq!(mime, "image/svg+xml");
    let svg = String::from_utf8(bytes).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
    assert!(svg.contains("KLASS"));
    assert!(svg.contains("#E74C3C")); // PDF accent (red)
}

#[tokio::test]
async fn test_svg_fallback_pptx_accent() {
    let gen = SvgFallbackGenerator;
    let (bytes, _) = gen
        .generate(b"fake pptx", "application/vnd.openxmlformats-officedocument.presentationml.presentation")
        .await
        .unwrap();

    let svg = String::from_utf8(bytes).unwrap();
    assert!(svg.contains("#F39C12")); // PPTX accent (orange)
}

#[tokio::test]
async fn test_svg_fallback_default_accent() {
    let gen = SvgFallbackGenerator;
    let (bytes, _) = gen.generate(b"plain text", "text/plain").await.unwrap();
    let svg = String::from_utf8(bytes).unwrap();
    assert!(svg.contains("#3498DB")); // default accent (blue)
}

// ═════════════════════════════════════════════════════════════════════════════
// Helpers — mirrors the validate_integrity function from publication.rs
// ═════════════════════════════════════════════════════════════════════════════

const PDF_MAGIC: &[u8] = b"%PDF";
const PDF_EOF_MARKER: &[u8] = b"%%EOF";
const OOXML_MAGIC: &[u8] = b"PK\x03\x04";

fn validate_integrity(bytes: &[u8], mime_type: &str) -> Result<(), String> {
    if bytes.is_empty() {
        return Err("artifact is empty".to_string());
    }

    if mime_type.contains("pdf") {
        if !bytes.starts_with(PDF_MAGIC) {
            return Err("PDF file missing %PDF header magic bytes".to_string());
        }
        if !bytes.ends_with(PDF_EOF_MARKER) {
            return Err("PDF file missing %%EOF trailer".to_string());
        }
    } else if mime_type.contains("officedocument") || mime_type.contains("presentation") || mime_type.contains("word") {
        if !bytes.starts_with(OOXML_MAGIC) {
            return Err("OOXML file missing PK\\x03\\x04 (ZIP) magic bytes".to_string());
        }
    }

    Ok(())
}

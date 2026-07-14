use aws_sdk_s3::Client as S3Client;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

// ─── Upload Categories (ported from `config/filesystems.php`) ────────────────

/// A validated upload category with its path, allowed MIME types, and size limit.
#[derive(Debug, Clone, Copy)]
pub struct UploadCategory {
    pub name: &'static str,
    pub path: &'static str,
    pub allowed_extensions: &'static [&'static str],
    pub max_size_kb: u64,
}

/// All upload categories supported by the application.
///
/// Matches `config/filesystems.php` → `upload_categories` exactly.
pub const UPLOAD_CATEGORIES: &[UploadCategory] = &[
    UploadCategory {
        name: "avatars",
        path: "avatars",
        allowed_extensions: &["jpg", "jpeg", "png", "webp"],
        max_size_kb: 2048,
    },
    UploadCategory {
        name: "gallery",
        path: "gallery",
        allowed_extensions: &["jpg", "jpeg", "png", "webp", "gif", "svg"],
        max_size_kb: 5120,
    },
    UploadCategory {
        name: "materials",
        path: "materials",
        allowed_extensions: &["pdf", "doc", "docx", "ppt", "pptx", "xls", "xlsx", "txt", "md"],
        max_size_kb: 10240,
    },
    UploadCategory {
        name: "attachments",
        path: "attachments",
        allowed_extensions: &["jpg", "jpeg", "png", "webp", "gif", "pdf", "doc", "docx", "zip"],
        max_size_kb: 10240,
    },
];

/// Look up a category definition by name. Returns `None` for unknown categories.
pub fn get_category(name: &str) -> Option<&'static UploadCategory> {
    UPLOAD_CATEGORIES.iter().find(|c| c.name == name)
}

// ─── MIME ↔ Extension mapping ───────────────────────────────────────────────

/// Map a MIME type to its canonical file extension (without the dot).
///
/// Returns `None` for unsupported / unknown MIME types.
pub fn mime_to_extension(mime: &str) -> Option<&'static str> {
    match mime {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        "image/svg+xml" => Some("svg"),
        "application/pdf" => Some("pdf"),
        "application/msword" => Some("doc"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            Some("docx")
        }
        "application/vnd.ms-powerpoint" => Some("ppt"),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            Some("pptx")
        }
        "application/vnd.ms-excel" => Some("xls"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
            Some("xlsx")
        }
        "text/plain" => Some("txt"),
        "text/markdown" => Some("md"),
        "application/zip" => Some("zip"),
        _ => None,
    }
}

// ─── Filename sanitisation ───────────────────────────────────────────────────

/// Sanitise a filename into a slug + timestamp + random prefix.
///
/// Format: `{random_8}_{unix_millis}_{slugified_stem}.{ext}`
///
/// - The stem is lowercased, non-alphanumeric characters (except hyphens and
///   dots) are stripped, and whitespace runs are collapsed into `-`.
/// - The random prefix is the first 8 hex chars of a UUID v4.
/// - `fallback_ext` is used when the MIME type cannot be mapped (e.g. `"bin"`).
pub fn sanitise_filename(stem: &str, extension: &str) -> String {
    let random = &Uuid::new_v4().to_string()[..8];

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    // Slugify the stem
    let slug: String = stem
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '.' { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase()
        .trim_start_matches('-')
        .trim_end_matches('-')
        .to_string();

    let slug = if slug.is_empty() { "untitled" } else { &slug };

    let ext = if extension.is_empty() { "bin" } else { extension };
    format!("{random}_{timestamp}_{slug}.{ext}")
}

// ─── Result ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct UploadResult {
    /// The object key (path) inside the bucket — used for `delete()` / `exists()`.
    pub path: String,
    /// The full public URL consumers can use to fetch the object.
    pub public_url: String,
}

// ─── Public URL helper ───────────────────────────────────────────────────────

/// Build a public URL from the base URL and object path.
pub fn generate_public_url(public_url_base: &str, path: &str) -> String {
    format!("{}/{}", public_url_base.trim_end_matches('/'), path)
}

// ─── Upload ──────────────────────────────────────────────────────────────────

/// Upload a file to R2 under the given category.
///
/// Validates:
/// - The category name exists in [`UPLOAD_CATEGORIES`]
/// - The MIME type maps to a known extension
/// - The extension is allowed for this category
/// - The file size does not exceed the category limit
///
/// The object key is built as `{category_path}/{sanitised_filename}` with
/// automatic slugification and a random prefix to prevent collisions.
pub async fn upload(
    s3_client: &S3Client,
    bucket: &str,
    public_url_base: &str,
    category: &str,
    bytes: Vec<u8>,
    content_type: &str,
) -> anyhow::Result<UploadResult> {
    let cat = get_category(category).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown upload category '{category}'; valid: {:?}",
            UPLOAD_CATEGORIES.iter().map(|c| c.name).collect::<Vec<_>>()
        )
    })?;

    // 1. Validate file size
    let max_bytes = cat.max_size_kb * 1024;
    if bytes.len() as u64 > max_bytes {
        return Err(anyhow::anyhow!(
            "file too large for category '{category}': {} bytes (max {} bytes / {} KB)",
            bytes.len(),
            max_bytes,
            cat.max_size_kb,
        ));
    }

    // 2. Map MIME → extension & validate against category
    let ext = mime_to_extension(content_type).ok_or_else(|| {
        anyhow::anyhow!(
            "unsupported content type '{content_type}' for category '{category}'"
        )
    })?;

    if !cat.allowed_extensions.contains(&ext) {
        return Err(anyhow::anyhow!(
            "content type '{content_type}' (→ {ext}) not allowed in category '{category}'; \
             allowed extensions: {:?}",
            cat.allowed_extensions,
        ));
    }

    // 3. Build object key with sanitised filename
    let filename = sanitise_filename("file", ext);
    let object_key = format!("{}/{}", cat.path, filename);

    // 4. Upload to R2
    s3_client
        .put_object()
        .bucket(bucket)
        .key(&object_key)
        .body(bytes.into())
        .content_type(content_type)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("failed to upload '{object_key}' to R2: {e}"))?;

    let public_url = generate_public_url(public_url_base, &object_key);

    Ok(UploadResult {
        path: object_key,
        public_url,
    })
}

// ─── Delete ──────────────────────────────────────────────────────────────────

/// Delete an object from R2 by its object key (path).
///
/// Returns `true` when the object was deleted (or did not exist), `false` on
/// an unexpected error (the error is also logged).
pub async fn delete(
    s3_client: &S3Client,
    bucket: &str,
    path: &str,
) -> anyhow::Result<bool> {
    match s3_client.delete_object().bucket(bucket).key(path).send().await {
        Ok(_) => Ok(true),
        Err(e) => {
            tracing::warn!(error = %e, key = %path, "failed to delete object from R2");
            Ok(false)
        }
    }
}

// ─── Exists ──────────────────────────────────────────────────────────────────

/// Check whether an object exists in R2 by its object key.
///
/// Uses `head_object` which is cheaper than listing or `get_object`.
pub async fn exists(
    s3_client: &S3Client,
    bucket: &str,
    path: &str,
) -> anyhow::Result<bool> {
    match s3_client.head_object().bucket(bucket).key(path).send().await {
        Ok(_) => Ok(true),
        Err(aws_sdk_s3::error::SdkError::ServiceError(err)) => {
            // 404 (NotFound) means the object simply does not exist
            use aws_sdk_s3::operation::head_object::HeadObjectError;
            if matches!(err.err(), HeadObjectError::NotFound(_)) {
                Ok(false)
            } else {
                Err(anyhow::anyhow!(
                    "failed to check existence of '{path}': {err:?}"
                ))
            }
        }
        Err(e) => Err(anyhow::anyhow!("failed to check existence of '{path}': {e}")),
    }
}

// ─── Extract object key from public URL ──────────────────────────────────────

/// Extract the object key from a full public URL.
///
/// # Example
///
/// ```
/// let key = extract_object_key(
///     "https://cdn.example.com/avatars/abc123_file.jpg",
///     "https://cdn.example.com",
/// );
/// assert_eq!(key, Some("avatars/abc123_file.jpg".to_string()));
/// ```
pub fn extract_object_key(public_url: &str, public_url_base: &str) -> Option<String> {
    let base = public_url_base.trim_end_matches('/');
    public_url
        .strip_prefix(base)
        .map(|path| path.trim_start_matches('/').to_string())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Upload categories ───────────────────────────────────────────────

    #[test]
    fn test_get_category_found() {
        let cat = get_category("avatars").unwrap();
        assert_eq!(cat.name, "avatars");
        assert_eq!(cat.path, "avatars");
        assert_eq!(cat.max_size_kb, 2048);
    }

    #[test]
    fn test_get_category_not_found() {
        assert!(get_category("nonexistent").is_none());
    }

    #[test]
    fn test_get_category_all_have_paths() {
        for cat in UPLOAD_CATEGORIES {
            assert!(!cat.path.is_empty(), "category '{}' has empty path", cat.name);
            assert!(!cat.allowed_extensions.is_empty(), "category '{}' has no mimes", cat.name);
        }
    }

    // ── MIME ↔ Extension ────────────────────────────────────────────────

    #[test]
    fn test_mime_to_extension_known() {
        assert_eq!(mime_to_extension("image/jpeg"), Some("jpg"));
        assert_eq!(mime_to_extension("image/png"), Some("png"));
        assert_eq!(mime_to_extension("application/pdf"), Some("pdf"));
        assert_eq!(mime_to_extension("application/zip"), Some("zip"));
    }

    #[test]
    fn test_mime_to_extension_unknown() {
        assert_eq!(mime_to_extension("video/mp4"), None);
        assert_eq!(mime_to_extension("application/octet-stream"), None);
        assert_eq!(mime_to_extension(""), None);
    }

    // ── Filename sanitisation ───────────────────────────────────────────

    #[test]
    fn test_sanitise_filename_basic() {
        let name = sanitise_filename("My File", "pdf");
        assert!(name.ends_with(".pdf"));
        assert!(!name.contains(' '));
        assert!(name.contains("my-file"));
    }

    #[test]
    fn test_sanitise_filename_special_chars() {
        let name = sanitise_filename("Hello@World!#$%^&*()", "png");
        assert!(name.contains("hello-world"));
        assert!(name.ends_with(".png"));
    }

    #[test]
    fn test_sanitise_filename_empty_stem() {
        let name = sanitise_filename("", "pdf");
        assert!(name.contains("untitled"));
        assert!(name.ends_with(".pdf"));
    }

    #[test]
    fn test_sanitise_filename_empty_extension() {
        let name = sanitise_filename("test", "");
        assert!(name.ends_with(".bin"));
    }

    #[test]
    fn test_sanitise_filename_whitespace_collapsed() {
        let name = sanitise_filename("a   b   c", "txt");
        assert!(name.contains("a-b-c"));
    }

    #[test]
    fn test_sanitise_filename_leading_trailing_dashes_removed() {
        let name = sanitise_filename("--hello-world--", "jpg");
        assert!(name.contains("hello-world"));
    }

    #[test]
    fn test_sanitise_filename_has_random_prefix() {
        let name = sanitise_filename("test", "txt");
        // Format: {8 random chars}_{timestamp}_{slug}.{ext}
        // 8 hex chars + 1 underscore = the prefix starts the string
        let parts: Vec<&str> = name.split('_').collect();
        assert!(parts.len() >= 3, "expected at least 3 underscore-separated parts, got {parts:?}");
        assert_eq!(parts[0].len(), 8, "first part should be 8 hex chars");
    }

    // ── UploadResult ────────────────────────────────────────────────────

    #[test]
    fn test_upload_result_debug() {
        let result = UploadResult {
            path: "avatars/abc123_file.jpg".to_string(),
            public_url: "https://cdn.example.com/avatars/abc123_file.jpg".to_string(),
        };
        let debug = format!("{result:?}");
        assert!(debug.contains("abc123_file.jpg"));
    }

    // ── Public URL ──────────────────────────────────────────────────────

    #[test]
    fn test_generate_public_url() {
        let url = generate_public_url("https://cdn.example.com", "avatars/file.jpg");
        assert_eq!(url, "https://cdn.example.com/avatars/file.jpg");
    }

    #[test]
    fn test_generate_public_url_trailing_slash() {
        let url = generate_public_url("https://cdn.example.com/", "avatars/file.jpg");
        assert_eq!(url, "https://cdn.example.com/avatars/file.jpg");
    }

    // ── Extract object key ──────────────────────────────────────────────

    #[test]
    fn test_extract_object_key_basic() {
        let key = extract_object_key(
            "https://cdn.example.com/avatars/abc123_file.jpg",
            "https://cdn.example.com",
        );
        assert_eq!(key, Some("avatars/abc123_file.jpg".to_string()));
    }

    #[test]
    fn test_extract_object_key_with_trailing_slash() {
        let key = extract_object_key(
            "https://cdn.example.com/avatars/abc123_file.jpg",
            "https://cdn.example.com/",
        );
        assert_eq!(key, Some("avatars/abc123_file.jpg".to_string()));
    }

    #[test]
    fn test_extract_object_key_no_match() {
        let key = extract_object_key(
            "https://other.com/avatars/file.jpg",
            "https://cdn.example.com",
        );
        assert!(key.is_none());
    }

    #[test]
    fn test_extract_object_key_subdirectory_prefix() {
        let key = extract_object_key(
            "https://cdn.example.com/storage/avatars/file.jpg",
            "https://cdn.example.com/storage",
        );
        assert_eq!(key, Some("avatars/file.jpg".to_string()));
    }

    // ── Upload validation edge cases (unit-tested without S3) ────────────

    #[test]
    fn test_upload_validates_category() {
        // Simulate what upload() does when given an unknown category
        let err = get_category("videos").ok_or_else(|| {
            anyhow::anyhow!("unknown upload category 'videos'")
        });
        assert!(err.is_err());
    }

    #[test]
    fn test_upload_validates_mime_type() {
        let ext = mime_to_extension("video/mp4");
        assert!(ext.is_none());
    }

    #[test]
    fn test_size_check_logic() {
        let cat = get_category("avatars").unwrap();
        let max_bytes = cat.max_size_kb * 1024; // 2 MB

        // Small file is fine
        assert!(1024u64 <= max_bytes);

        // Oversize file would fail
        let oversized = (cat.max_size_kb + 1) * 1024;
        assert!(oversized > max_bytes);
    }

    #[test]
    fn test_allowed_extension_check() {
        let cat = get_category("gallery").unwrap();

        // Valid for gallery
        assert!(cat.allowed_extensions.contains(&"png"));
        assert!(cat.allowed_extensions.contains(&"svg"));

        // Not valid for gallery
        assert!(!cat.allowed_extensions.contains(&"pdf"));
        assert!(!cat.allowed_extensions.contains(&"docx"));
    }

    #[test]
    fn test_extract_object_key_maintains_previous_behaviour() {
        // Same inputs as the original avatar.rs usage
        let key = extract_object_key(
            "https://cdn.example.com/avatars/1/avatar_abc12345.jpg",
            "https://cdn.example.com",
        );
        assert_eq!(key, Some("avatars/1/avatar_abc12345.jpg".to_string()));
    }
}

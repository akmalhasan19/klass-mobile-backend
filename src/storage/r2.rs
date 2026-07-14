use aws_sdk_s3::Client as S3Client;
use uuid::Uuid;

const MAX_AVATAR_SIZE_BYTES: usize = 2 * 1024 * 1024; // 2MB (matches Laravel `StoreAvatarRequest::max:2048`)
const ALLOWED_AVATAR_MIME_TYPES: &[&str] = &["image/jpeg", "image/png", "image/webp"];

#[derive(Debug)]
pub struct UploadResult {
    pub path: String,
    pub public_url: String,
}

/// Upload avatar to R2 storage.
/// Returns the public URL of the uploaded avatar.
pub async fn upload_avatar(
    s3_client: &S3Client,
    bucket: &str,
    public_url_base: &str,
    user_id: i64,
    bytes: Vec<u8>,
    content_type: &str,
) -> anyhow::Result<UploadResult> {
    if bytes.len() > MAX_AVATAR_SIZE_BYTES {
        return Err(anyhow::anyhow!(
            "file too large: {} bytes (max {} bytes)",
            bytes.len(),
            MAX_AVATAR_SIZE_BYTES
        ));
    }

    if !ALLOWED_AVATAR_MIME_TYPES.contains(&content_type) {
        return Err(anyhow::anyhow!(
            "invalid content type: {} (allowed: {:?})",
            content_type,
            ALLOWED_AVATAR_MIME_TYPES
        ));
    }

    let extension = match content_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        _ => return Err(anyhow::anyhow!("unsupported mime type")),
    };

    let random_id = Uuid::new_v4().to_string();
    let short_id = &random_id[..8];
    let object_key = format!("avatars/{}/avatar_{}.{}", user_id, short_id, extension);

    s3_client
        .put_object()
        .bucket(bucket)
        .key(&object_key)
        .body(bytes.into())
        .content_type(content_type)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("failed to upload to R2: {e}"))?;

    let public_url = format!("{}/{}", public_url_base.trim_end_matches('/'), object_key);

    Ok(UploadResult {
        path: object_key,
        public_url,
    })
}

/// Delete object from R2 storage.
pub async fn delete_object(
    s3_client: &S3Client,
    bucket: &str,
    object_key: &str,
) -> anyhow::Result<bool> {
    match s3_client
        .delete_object()
        .bucket(bucket)
        .key(object_key)
        .send()
        .await
    {
        Ok(_) => Ok(true),
        Err(e) => {
            tracing::warn!(error = %e, key = %object_key, "failed to delete object from R2");
            Ok(false)
        }
    }
}

/// Extract object key from a full public URL.
/// E.g., "https://cdn.example.com/avatars/1/avatar_abc12345.jpg" -> "avatars/1/avatar_abc12345.jpg"
pub fn extract_object_key(public_url: &str, public_url_base: &str) -> Option<String> {
    let base = public_url_base.trim_end_matches('/');
    public_url
        .strip_prefix(base)
        .map(|path| path.trim_start_matches('/').to_string())
}

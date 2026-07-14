use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::auth::middleware::Principal;
use crate::error::{AppError, AppResult};
use crate::governance::activity_log::record_activity;
use crate::state::AppState;
use crate::storage::r2;

use super::require_admin;
use super::super::response;

// ─── Query params ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct DeleteUploadQuery {
    pub path: String,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// POST /admin/upload/{category}
///
/// Accepts multipart form with a single file field named `file`.
/// Validates against the category's allowed MIME types and size limits
/// (defined in `UPLOAD_CATEGORIES` inside `src/storage/r2.rs`).
///
/// Supported categories: `avatars`, `gallery`, `materials`, `attachments`.
pub async fn upload(
    State(state): State<AppState>,
    principal: Principal,
    Path(category): Path<String>,
    mut multipart: axum::extract::Multipart,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    // Validate category exists
    if r2::get_category(&category).is_none() {
        return Err(AppError::Validation(format!(
            "Kategori upload '{}' tidak valid. Kategori yang diizinkan: avatars, gallery, materials, attachments.",
            category
        )));
    }

    let mut file_bytes: Option<Vec<u8>> = None;
    let mut content_type: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation(format!("gagal membaca field multipart: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
            content_type = field.content_type().map(|ct| ct.to_string());

            let data = field
                .bytes()
                .await
                .map_err(|e| AppError::Validation(format!("gagal membaca data file: {e}")))?;

            file_bytes = Some(data.to_vec());
            break;
        }
    }

    let bytes = file_bytes
        .ok_or_else(|| AppError::Validation("Field 'file' wajib dikirim.".into()))?;

    let mime = content_type
        .ok_or_else(|| AppError::Validation("Content type file tidak ditemukan.".into()))?;

    // Upload to R2 — validation (size, mime type, category) is handled inside r2::upload
    let upload_result = r2::upload(
        &state.s3_client,
        &state.config.r2_bucket_name,
        &state.config.r2_public_url,
        &category,
        bytes,
        &mime,
    )
    .await
    .map_err(|e| AppError::Internal(format!("Gagal meng-upload file: {e}")))?;

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "upload_file",
        Some("media_file"),
        None,
        Some(serde_json::json!({
            "category": category,
            "path": upload_result.path,
            "url": upload_result.public_url,
            "content_type": mime,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("gagal mencatat aktivitas: {e}")))?;

    Ok(response::created_with_message(
        "File berhasil di-upload.",
        serde_json::json!({
            "path": upload_result.path,
            "url": upload_result.public_url,
            "category": category,
        }),
    ))
}

/// DELETE /admin/upload/{category}?path=...
///
/// Deletes a file from the storage bucket by its object key (path).
/// The `category` parameter is validated but not used for the actual deletion
/// (the path itself encodes the category). It serves as a safety guard and
/// for consistent routing.
pub async fn delete(
    State(state): State<AppState>,
    principal: Principal,
    Path(category): Path<String>,
    Query(params): Query<DeleteUploadQuery>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    // Validate category exists
    if r2::get_category(&category).is_none() {
        return Err(AppError::Validation(format!(
            "Kategori upload '{}' tidak valid. Kategori yang diizinkan: avatars, gallery, materials, attachments.",
            category
        )));
    }

    let path = params.path.trim().to_string();
    if path.is_empty() {
        return Err(AppError::Validation("Parameter 'path' wajib dikirim.".into()));
    }

    let deleted = r2::delete(&state.s3_client, &state.config.r2_bucket_name, &path)
        .await
        .map_err(|e| AppError::Internal(format!("Gagal menghapus file: {e}")))?;

    if !deleted {
        return Err(AppError::NotFound("File tidak ditemukan.".into()));
    }

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "delete_file",
        Some("media_file"),
        None,
        Some(serde_json::json!({
            "category": category,
            "path": path,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("gagal mencatat aktivitas: {e}")))?;

    Ok(response::ok_with_message(
        "File berhasil dihapus.",
        serde_json::json!({}),
    ))
}

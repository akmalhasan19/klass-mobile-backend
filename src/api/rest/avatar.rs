use axum::extract::{Multipart, State};
use axum::Json;
use serde::Serialize;

use crate::auth::middleware::Principal;
use crate::db::repositories::users::UsersRepo;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::storage::r2;

use super::auth::UserResource;
use super::response;

#[derive(Serialize, utoipa::ToSchema)]
pub struct AvatarData {
    pub user: UserResource,
    pub avatar_url: String,
}

/// POST /user/avatar
///
/// Accepts multipart form with a single file field named `file` (matches
/// Laravel `StoreAvatarRequest` and the Flutter client `AuthApi.uploadAvatar`).
/// Supported formats: JPEG, PNG, WebP. Max size: 2MB.
#[utoipa::path(
    post,
    path = "/api/v1/user/avatar",
    tag = "user",
    responses(
        (status = 200, body = AvatarData),
        (status = 422, description = "Validation error"),
    ),
    security(
        ("bearer_auth" = [])
    ),
)]
pub async fn upload_avatar(
    State(state): State<AppState>,
    principal: Principal,
    mut multipart: Multipart,
) -> AppResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut content_type: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation(format!("failed to read multipart field: {e}")))? 
    {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
            content_type = field.content_type().map(|ct| ct.to_string());

            let data = field
                .bytes()
                .await
                .map_err(|e| AppError::Validation(format!("failed to read file data: {e}")))?;

            file_bytes = Some(data.to_vec());
            break;
        }
    }

    let bytes = file_bytes
        .ok_or_else(|| AppError::Validation("missing 'file' field in multipart form".into()))?;

    let mime = content_type
        .ok_or_else(|| AppError::Validation("missing content type for avatar file".into()))?;

    // Use the generalized upload() with the "avatars" category.
    // Validation (size, mime type) is handled inside r2::upload against the
    // avatars category config (2 MB max, JPEG/PNG/WebP only).
    let upload_result = r2::upload(
        &state.s3_client,
        &state.config.r2_bucket_name,
        &state.config.r2_public_url,
        "avatars",
        bytes,
        &mime,
    )
    .await
    .map_err(|e| AppError::Internal(format!("upload failed: {e}")))?;

    let repo = crate::db::repositories::users::PgUsersRepo::new(state.db_pool.clone());

    let user = repo
        .find_by_id(principal.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("user not found".into()))?;

    // Delete the old avatar from R2 if present
    if let Some(old_url) = &user.avatar_url {
        if let Some(old_key) = r2::extract_object_key(old_url, &state.config.r2_public_url) {
            let _ = r2::delete(&state.s3_client, &state.config.r2_bucket_name, &old_key).await;
        }
    }

    repo.update_avatar(principal.user_id, &upload_result.public_url)
        .await?;

    // Re-fetch the user so the returned `UserResource` reflects the new `avatar_url`.
    let user = repo
        .find_by_id(principal.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("user not found after avatar update".into()))?;

    Ok(response::ok_with_message(
        "avatar uploaded successfully",
        AvatarData {
            user: UserResource::from(user),
            avatar_url: upload_result.public_url,
        },
    ))
}

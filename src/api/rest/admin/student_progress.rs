use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::middleware::Principal;
use crate::db::repositories::student_progress::{
    CreateStudentProgressPayload, PgStudentProgressRepo, StudentProgressRepo,
    UpdateStudentProgressPayload,
};
use crate::error::{AppError, AppResult};
use crate::governance::activity_log::record_activity;
use crate::state::AppState;

use super::require_admin;
use super::super::response;

// ─── Request bodies ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateStudentProgressRequest {
    pub student_name: String,
    pub score: i32,
    pub completion_date: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateStudentProgressRequest {
    pub student_name: Option<String>,
    pub score: Option<i32>,
    pub completion_date: Option<Option<String>>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// POST /admin/student-progress
pub async fn create(
    State(state): State<AppState>,
    principal: Principal,
    Json(payload): Json<CreateStudentProgressRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    // Validate student_name
    let student_name = payload.student_name.trim().to_string();
    if student_name.is_empty() {
        return Err(AppError::Validation("Nama siswa wajib diisi.".into()));
    }
    if student_name.len() > 255 {
        return Err(AppError::Validation(
            "Nama siswa maksimal 255 karakter.".into(),
        ));
    }

    // Validate score range (0-100)
    if payload.score < 0 || payload.score > 100 {
        return Err(AppError::Validation(
            "Skor harus antara 0 dan 100.".into(),
        ));
    }

    // Validate completion_date if provided
    let completion_date = if let Some(ref date_str) = payload.completion_date {
        Some(
            DateTime::parse_from_rfc3339(date_str)
                .map_err(|_| {
                    AppError::Validation(
                        "Format tanggal selesai tidak valid. Gunakan format RFC 3339 (ISO 8601)."
                            .into(),
                    )
                })?
                .with_timezone(&Utc),
        )
    } else {
        None
    };

    let repo = PgStudentProgressRepo::new(state.db_pool.clone());

    let create_payload = CreateStudentProgressPayload {
        student_name,
        score: payload.score,
        completion_date,
    };

    let record = repo
        .insert(&create_payload)
        .await
        .map_err(|e| AppError::Internal(format!("gagal membuat progress siswa: {e}")))?;

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "create_student_progress",
        Some("student_progress"),
        None, // student_progress uses UUID, not BIGINT
        Some(serde_json::json!({
            "progress_id": record.id.to_string(),
            "student_name": record.student_name,
            "score": record.score,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("gagal mencatat aktivitas: {e}")))?;

    Ok(response::created_with_message(
        "Progress siswa berhasil dibuat.",
        serde_json::json!({
            "id": record.id,
            "student_name": record.student_name,
            "score": record.score,
            "completion_date": record.completion_date.map(|d| d.to_rfc3339()),
            "created_at": record.created_at.to_rfc3339(),
            "updated_at": record.updated_at.to_rfc3339(),
        }),
    ))
}

/// PATCH /admin/student-progress/{id}
pub async fn update(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateStudentProgressRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    // Validate student_name if provided
    if let Some(ref name) = payload.student_name {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(AppError::Validation("Nama siswa tidak boleh kosong.".into()));
        }
        if trimmed.len() > 255 {
            return Err(AppError::Validation(
                "Nama siswa maksimal 255 karakter.".into(),
            ));
        }
    }

    // Validate score range if provided
    if let Some(score) = payload.score {
        if score < 0 || score > 100 {
            return Err(AppError::Validation(
                "Skor harus antara 0 dan 100.".into(),
            ));
        }
    }

    // Validate completion_date if provided
    let completion_date = if let Some(ref date_opt) = payload.completion_date {
        match date_opt {
            Some(date_str) => Some(Some(
                DateTime::parse_from_rfc3339(date_str)
                    .map_err(|_| {
                        AppError::Validation(
                            "Format tanggal selesai tidak valid. Gunakan format RFC 3339 (ISO 8601)."
                                .into(),
                        )
                    })?
                    .with_timezone(&Utc),
            )),
            None => Some(None),
        }
    } else {
        None
    };

    let repo = PgStudentProgressRepo::new(state.db_pool.clone());

    let update_payload = UpdateStudentProgressPayload {
        student_name: payload.student_name.map(|n| n.trim().to_string()),
        score: payload.score,
        completion_date,
    };

    let record = repo
        .update(id, &update_payload)
        .await
        .map_err(|e| {
            if e.to_string().contains("not found") {
                AppError::NotFound("Progress siswa tidak ditemukan.".into())
            } else {
                AppError::Internal(format!("gagal memperbarui progress siswa: {e}"))
            }
        })?;

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "update_student_progress",
        Some("student_progress"),
        None,
        Some(serde_json::json!({
            "progress_id": record.id.to_string(),
            "student_name": record.student_name,
            "score": record.score,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("gagal mencatat aktivitas: {e}")))?;

    Ok(response::ok_with_message(
        "Progress siswa berhasil diperbarui.",
        serde_json::json!({
            "id": record.id,
            "student_name": record.student_name,
            "score": record.score,
            "completion_date": record.completion_date.map(|d| d.to_rfc3339()),
            "created_at": record.created_at.to_rfc3339(),
            "updated_at": record.updated_at.to_rfc3339(),
        }),
    ))
}

/// DELETE /admin/student-progress/{id}
pub async fn delete(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;
    let repo = PgStudentProgressRepo::new(state.db_pool.clone());

    let deleted = repo
        .delete(id)
        .await
        .map_err(|e| AppError::Internal(format!("gagal menghapus progress siswa: {e}")))?;

    if !deleted {
        return Err(AppError::NotFound("Progress siswa tidak ditemukan.".into()));
    }

    // Record activity
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "delete_student_progress",
        Some("student_progress"),
        None,
        Some(serde_json::json!({
            "progress_id": id.to_string(),
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("gagal mencatat aktivitas: {e}")))?;

    Ok(response::ok_with_message(
        "Progress siswa berhasil dihapus.",
        serde_json::json!({}),
    ))
}

use axum::{http::StatusCode, Json};
use serde::Serialize;

use crate::db::pagination::PaginationMeta;

pub fn ok<T: Serialize>(data: T) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(serde_json::json!({ "success": true, "data": data })),
    )
}

pub fn created<T: Serialize>(data: T) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "success": true, "data": data })),
    )
}

pub fn ok_with_message<T: Serialize>(
    message: &str,
    data: T,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": message,
            "data": data,
        })),
    )
}

pub fn created_with_message<T: Serialize>(
    message: &str,
    data: T,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "message": message,
            "data": data,
        })),
    )
}

pub fn paginated<T: Serialize>(
    data: Vec<T>,
    meta: PaginationMeta,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "Berhasil.",
            "data": data,
            "meta": meta,
            "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        })),
    )
}

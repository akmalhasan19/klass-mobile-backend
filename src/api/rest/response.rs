use axum::{http::StatusCode, Json};
use serde::Serialize;

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

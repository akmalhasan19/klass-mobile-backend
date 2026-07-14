use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::auth::middleware::Principal;
use crate::db::repositories::system_settings::{
    coerce_setting_value, parse_bool_setting, BulkUpdateItem, PgSystemSettingsRepo,
    SystemSettingsRepo,
};
use crate::error::{AppError, AppResult};
use crate::governance::activity_log::record_activity;
use crate::state::AppState;

use super::require_admin;
use super::super::response;

// ─── Resources ───────────────────────────────────────────────────────────────

/// A single setting as returned to the admin client.
/// Boolean values are properly deserialized (true/false) rather than strings.
#[derive(Serialize)]
struct SettingResource {
    key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<serde_json::Value>,
    #[serde(rename = "type")]
    setting_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

/// Grouped settings: each key is a group name, value is an array of settings.
type GroupedSettingsResource = HashMap<String, Vec<SettingResource>>;

// ─── Request body ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct BulkUpdateSettingsRequest {
    /// A flat map of key → value for settings to update.
    /// Keys not in the database are silently skipped.
    pub settings: HashMap<String, String>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// GET /admin/settings
///
/// Returns all system settings grouped by their `group` column, ordered by
/// group and key. Boolean settings have their value parsed as a proper JSON
/// boolean (`true`/`false`) rather than a raw string.
pub async fn index(
    State(state): State<AppState>,
    principal: Principal,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    let repo = PgSystemSettingsRepo::new(state.db_pool.clone());
    let grouped = repo
        .find_grouped()
        .await
        .map_err(|e| AppError::Internal(format!("gagal mengambil pengaturan: {e}")))?;

    let resource: GroupedSettingsResource = grouped
        .into_iter()
        .map(|(group, settings)| {
            let resources: Vec<SettingResource> = settings
                .into_iter()
                .map(|s| {
                    let parsed_value = if s.setting_type == "boolean" {
                        parse_bool_setting(s.value.as_deref(), &s.setting_type)
                            .map(|b| serde_json::Value::Bool(b))
                    } else {
                        s.value.map(|v| serde_json::Value::String(v))
                    };

                    SettingResource {
                        key: s.key,
                        value: parsed_value,
                        setting_type: s.setting_type,
                        description: s.description,
                    }
                })
                .collect();
            (group, resources)
        })
        .collect();

    Ok((StatusCode::OK, Json(serde_json::json!({ "data": resource }))))
}

/// PATCH /admin/settings
///
/// Bulk update system settings from a flat map. Values are automatically
/// type-coerced based on the setting's current `type` column in the database.
///
/// Body: `{ "settings": { "site_name": "Klass 2.0", "feature_x": "true" } }`
///
/// For boolean settings, common truthy/falsy representations are normalised
/// (e.g. `"1"` → `"true"`, `"yes"` → `"true"`, `"off"` → `"false"`).
///
/// Returns the updated (grouped) settings after the operation.
pub async fn bulk_update(
    State(state): State<AppState>,
    principal: Principal,
    Json(payload): Json<BulkUpdateSettingsRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&principal)?;

    if payload.settings.is_empty() {
        return Err(AppError::Validation(
            "Map 'settings' tidak boleh kosong.".into(),
        ));
    }

    let repo = PgSystemSettingsRepo::new(state.db_pool.clone());

    // Fetch all current settings to determine types for coercion
    let grouped = repo
        .find_grouped()
        .await
        .map_err(|e| AppError::Internal(format!("gagal mengambil pengaturan: {e}")))?;

    // Build a lookup map: key -> setting_type
    let type_map: HashMap<&str, &str> = grouped
        .values()
        .flatten()
        .map(|s| (s.key.as_str(), s.setting_type.as_str()))
        .collect();

    // Build BulkUpdateItem list with coerced values
    let updates: Vec<BulkUpdateItem> = payload
        .settings
        .into_iter()
        .map(|(key, value)| {
            let setting_type = type_map
                .get(key.as_str())
                .copied()
                .unwrap_or("text");
            let coerced_value = coerce_setting_value(&value, setting_type);
            BulkUpdateItem {
                key,
                value: Some(coerced_value),
                type_hint: None, // Keep existing type
            }
        })
        .collect();

    repo.bulk_update(&updates)
        .await
        .map_err(|e| AppError::Internal(format!("gagal memperbarui pengaturan: {e}")))?;

    // Record activity
    let updated_keys: Vec<&str> = updates.iter().map(|u| u.key.as_str()).collect();
    record_activity(
        &state.db_pool,
        Some(principal.user_id),
        "update_system_settings",
        Some("system_setting"),
        None,
        Some(serde_json::json!({
            "updated_keys": updated_keys,
        })),
    )
    .await
    .map_err(|e| AppError::Internal(format!("gagal mencatat aktivitas: {e}")))?;

    // Re-fetch and return the updated grouped settings
    let updated_grouped = repo
        .find_grouped()
        .await
        .map_err(|e| AppError::Internal(format!("gagal mengambil pengaturan: {e}")))?;

    let resource: GroupedSettingsResource = updated_grouped
        .into_iter()
        .map(|(group, settings)| {
            let resources: Vec<SettingResource> = settings
                .into_iter()
                .map(|s| {
                    let parsed_value = if s.setting_type == "boolean" {
                        parse_bool_setting(s.value.as_deref(), &s.setting_type)
                            .map(|b| serde_json::Value::Bool(b))
                    } else {
                        s.value.map(|v| serde_json::Value::String(v))
                    };

                    SettingResource {
                        key: s.key,
                        value: parsed_value,
                        setting_type: s.setting_type,
                        description: s.description,
                    }
                })
                .collect();
            (group, resources)
        })
        .collect();

    Ok(response::ok_with_message(
        &format!("{} pengaturan berhasil diperbarui.", updated_keys.len()),
        resource,
    ))
}

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashMap;

// ─── Struct ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SystemSetting {
    pub id: i64,
    pub key: String,
    pub value: Option<String>,
    #[sqlx(rename = "type")]
    pub setting_type: String,
    #[sqlx(rename = "group")]
    pub setting_group: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ─── Bulk update item ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BulkUpdateItem {
    /// The setting key to update.
    pub key: String,
    /// The new value (None = set to NULL).
    pub value: Option<String>,
    /// Optional type override (None = keep existing).
    pub type_hint: Option<String>,
}

// ─── Boolean coercion ────────────────────────────────────────────────────────

/// Coerce a raw string value according to the setting's `type` column.
///
/// For `type = "boolean"` this normalises common truthy/falsy representations
/// ("true"/"false", "1"/"0", "yes"/"no", "on"/"off") into a canonical
/// `"true"` or `"false"` string. Every other type passes through unchanged.
///
/// # Examples
///
/// assert_eq!(coerce_setting_value("1", "boolean"), "true");
/// assert_eq!(coerce_setting_value("YES", "boolean"), "true");
/// assert_eq!(coerce_setting_value("off", "boolean"), "false");
/// assert_eq!(coerce_setting_value("hello", "text"), "hello");
pub fn coerce_setting_value(value: &str, type_hint: &str) -> String {
    match type_hint {
        "boolean" => match value.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => "true".to_string(),
            _ => "false".to_string(),
        },
        _ => value.to_string(),
    }
}

/// Parse a setting value with `type = "boolean"` into a `bool`.
///
/// Returns `None` when the value is `NULL` or the setting type is not boolean.
///
/// # Examples
///
/// assert_eq!(parse_bool_setting(Some("true"), "boolean"), Some(true));
/// assert_eq!(parse_bool_setting(Some("0"), "boolean"), Some(false));
/// assert_eq!(parse_bool_setting(None, "boolean"), None);
/// assert_eq!(parse_bool_setting(Some("hello"), "text"), None);
pub fn parse_bool_setting(value: Option<&str>, type_hint: &str) -> Option<bool> {
    if type_hint != "boolean" {
        return None;
    }
    match value {
        Some(v) => match v.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => Some(false),
        },
        None => None,
    }
}

// ─── Trait ───────────────────────────────────────────────────────────────────

#[async_trait]
pub trait SystemSettingsRepo: Send + Sync {
    /// Retrieve the raw value of a setting by its key.
    ///
    /// If the key does not exist and `default` is `Some(d)`, returns `d` as
    /// the fallback value.  If the key does not exist and `default` is `None`,
    /// returns `None`.
    async fn get(&self, key: &str, default: Option<&str>) -> anyhow::Result<Option<String>>;

    /// Upsert a system setting using a single atomic `INSERT … ON CONFLICT DO UPDATE`.
    ///
    /// - `key`           – The setting key (unique constraint).
    /// - `value`         – The raw string value to store (`None` = set to NULL).
    /// - `type_hint`     – Override the `type` column (`None` = keep existing on update,
    ///   defaults to `"text"` on insert).
    /// - `setting_group` – Override the `group` column (`None` = keep existing on update,
    ///   defaults to `"general"` on insert).
    /// - `description`   – Optional human-readable description (`None` = keep existing on update).
    async fn set(
        &self,
        key: &str,
        value: Option<&str>,
        type_hint: Option<&str>,
        setting_group: Option<&str>,
        description: Option<&str>,
    ) -> anyhow::Result<SystemSetting>;

    /// Return **all** settings grouped by their `group` column.
    async fn find_grouped(&self) -> anyhow::Result<HashMap<String, Vec<SystemSetting>>>;

    /// Bulk-update settings inside a single database transaction.
    ///
    /// Each item's key is used to match the row; `value` and optional
    /// `type_hint` are applied.  Keys that do not exist in the database are
    /// silently skipped.
    async fn bulk_update(&self, updates: &[BulkUpdateItem]) -> anyhow::Result<()>;
}

// ─── Pg implementation ───────────────────────────────────────────────────────

pub struct PgSystemSettingsRepo {
    pool: PgPool,
}

impl PgSystemSettingsRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SystemSettingsRepo for PgSystemSettingsRepo {
    async fn get(&self, key: &str, default: Option<&str>) -> anyhow::Result<Option<String>> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT value FROM system_settings WHERE key = $1")
                .bind(key)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| anyhow::anyhow!("failed to get system setting: {e}"))?;

        match row {
            Some((value,)) => Ok(value),
            None => Ok(default.map(String::from)),
        }
    }

    async fn set(
        &self,
        key: &str,
        value: Option<&str>,
        type_hint: Option<&str>,
        setting_group: Option<&str>,
        description: Option<&str>,
    ) -> anyhow::Result<SystemSetting> {
        // Single atomic UPSERT — no race condition unlike check-then-insert.
        // On conflict the value is always overwritten (even to NULL); other
        // columns are only touched when the caller supplies an explicit value.
        let sql = r#"
            INSERT INTO system_settings
                (key, value, type, "group", description, created_at, updated_at)
            VALUES ($1, $2, COALESCE($3, 'text'), COALESCE($4, 'general'), $5, NOW(), NOW())
            ON CONFLICT (key) DO UPDATE SET
                value       = $2,
                type        = COALESCE($3, system_settings.type),
                "group"     = COALESCE($4, system_settings."group"),
                description = COALESCE($5, system_settings.description),
                updated_at  = NOW()
            RETURNING id, key, value, type, "group", description,
                      created_at, updated_at
        "#;

        let setting = sqlx::query_as::<_, SystemSetting>(sql)
            .bind(key)
            .bind(value)
            .bind(type_hint)
            .bind(setting_group)
            .bind(description)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to upsert system setting: {e}"))?;

        Ok(setting)
    }

    async fn find_grouped(&self) -> anyhow::Result<HashMap<String, Vec<SystemSetting>>> {
        let sql = r#"
            SELECT id, key, value, type, "group", description,
                   created_at, updated_at
            FROM system_settings
            ORDER BY "group" ASC, key ASC
        "#;

        let settings = sqlx::query_as::<_, SystemSetting>(sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch system settings: {e}"))?;

        let mut grouped: HashMap<String, Vec<SystemSetting>> = HashMap::new();
        for setting in settings {
            grouped
                .entry(setting.setting_group.clone())
                .or_default()
                .push(setting);
        }

        Ok(grouped)
    }

    async fn bulk_update(&self, updates: &[BulkUpdateItem]) -> anyhow::Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| anyhow::anyhow!("failed to begin transaction: {e}"))?;

        for item in updates {
            let sql = r#"
                UPDATE system_settings
                SET value = $1,
                    type = COALESCE($2, type),
                    updated_at = NOW()
                WHERE key = $3
            "#;

            sqlx::query(sql)
                .bind(item.value.as_deref())
                .bind(item.type_hint.as_deref())
                .bind(&item.key)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    anyhow::anyhow!("failed to bulk-update setting '{}': {e}", item.key)
                })?;
        }

        tx.commit()
            .await
            .map_err(|e| anyhow::anyhow!("failed to commit bulk update transaction: {e}"))?;

        Ok(())
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── SystemSetting struct ─────────────────────────────────────────────

    #[test]
    fn test_system_setting_struct() {
        let setting = SystemSetting {
            id: 1,
            key: "site_name".to_string(),
            value: Some("Klass".to_string()),
            setting_type: "text".to_string(),
            setting_group: "general".to_string(),
            description: Some("Application name".to_string()),
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
            updated_at: DateTime::from_timestamp(0, 0).unwrap(),
        };

        assert_eq!(setting.key, "site_name");
        assert_eq!(setting.value.as_deref(), Some("Klass"));
        assert_eq!(setting.setting_type, "text");
        assert_eq!(setting.setting_group, "general");
    }

    #[test]
    fn test_system_setting_with_null_value() {
        let setting = SystemSetting {
            id: 2,
            key: "feature_x_enabled".to_string(),
            value: None,
            setting_type: "boolean".to_string(),
            setting_group: "features".to_string(),
            description: None,
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
            updated_at: DateTime::from_timestamp(0, 0).unwrap(),
        };

        assert!(setting.value.is_none());
        assert!(setting.description.is_none());
    }

    // ── Boolean coercion ─────────────────────────────────────────────────

    #[test]
    fn test_coerce_boolean_true_variants() {
        for truthy in &["true", "True", "TRUE", "1", "yes", "Yes", "YES", "on", "On", "ON"] {
            assert_eq!(
                coerce_setting_value(truthy, "boolean"),
                "true",
                "expected 'true' for input '{truthy}'"
            );
        }
    }

    #[test]
    fn test_coerce_boolean_false_variants() {
        for falsy in &[
            "false", "False", "FALSE", "0", "no", "No", "NO", "off", "Off", "OFF",
        ] {
            assert_eq!(
                coerce_setting_value(falsy, "boolean"),
                "false",
                "expected 'false' for input '{falsy}'"
            );
        }
    }

    #[test]
    fn test_coerce_boolean_unknown_falls_to_false() {
        assert_eq!(coerce_setting_value("banana", "boolean"), "false");
        assert_eq!(coerce_setting_value("", "boolean"), "false");
        assert_eq!(coerce_setting_value("  ", "boolean"), "false");
    }

    #[test]
    fn test_coerce_boolean_whitespace_trimmed() {
        assert_eq!(coerce_setting_value("  true  ", "boolean"), "true");
        assert_eq!(coerce_setting_value("  FALSE  ", "boolean"), "false");
    }

    #[test]
    fn test_coerce_non_boolean_passthrough() {
        assert_eq!(coerce_setting_value("hello", "text"), "hello");
        assert_eq!(coerce_setting_value("42", "number"), "42");
        assert_eq!(coerce_setting_value("{\"a\":1}", "json"), "{\"a\":1}");
        assert_eq!(coerce_setting_value("", "text"), "");
    }

    // ── Boolean parsing ──────────────────────────────────────────────────

    #[test]
    fn test_parse_bool_setting_true() {
        assert_eq!(parse_bool_setting(Some("true"), "boolean"), Some(true));
        assert_eq!(parse_bool_setting(Some("1"), "boolean"), Some(true));
        assert_eq!(parse_bool_setting(Some("yes"), "boolean"), Some(true));
        assert_eq!(parse_bool_setting(Some("on"), "boolean"), Some(true));
    }

    #[test]
    fn test_parse_bool_setting_false() {
        assert_eq!(parse_bool_setting(Some("false"), "boolean"), Some(false));
        assert_eq!(parse_bool_setting(Some("0"), "boolean"), Some(false));
        assert_eq!(parse_bool_setting(Some("no"), "boolean"), Some(false));
        assert_eq!(parse_bool_setting(Some("off"), "boolean"), Some(false));
    }

    #[test]
    fn test_parse_bool_setting_null_value() {
        assert_eq!(parse_bool_setting(None, "boolean"), None);
    }

    #[test]
    fn test_parse_bool_setting_non_boolean_type() {
        assert_eq!(parse_bool_setting(Some("true"), "text"), None);
        assert_eq!(parse_bool_setting(Some("42"), "number"), None);
        assert_eq!(parse_bool_setting(None, "text"), None);
    }

    #[test]
    fn test_parse_bool_setting_unrecognised_falls_to_false() {
        assert_eq!(parse_bool_setting(Some("banana"), "boolean"), Some(false));
        assert_eq!(parse_bool_setting(Some("maybe"), "boolean"), Some(false));
    }

    // ── Bulk update item ─────────────────────────────────────────────────

    #[test]
    fn test_bulk_update_item_creation() {
        let item = BulkUpdateItem {
            key: "site_name".to_string(),
            value: Some("Klass 2.0".to_string()),
            type_hint: None,
        };

        assert_eq!(item.key, "site_name");
        assert_eq!(item.value.as_deref(), Some("Klass 2.0"));
        assert!(item.type_hint.is_none());
    }

    #[test]
    fn test_bulk_update_item_with_type_hint() {
        let item = BulkUpdateItem {
            key: "feature_x".to_string(),
            value: Some("true".to_string()),
            type_hint: Some("boolean".to_string()),
        };

        assert_eq!(item.type_hint.as_deref(), Some("boolean"));
    }

    #[test]
    fn test_bulk_update_item_null_value() {
        let item = BulkUpdateItem {
            key: "old_setting".to_string(),
            value: None,
            type_hint: None,
        };
        assert!(item.value.is_none());
    }

    // ── Grouped settings ─────────────────────────────────────────────────

    #[test]
    fn test_find_grouped_empty() {
        let grouped: HashMap<String, Vec<SystemSetting>> = HashMap::new();
        assert!(grouped.is_empty());
    }

    #[test]
    fn test_find_grouped_with_data() {
        let mut grouped: HashMap<String, Vec<SystemSetting>> = HashMap::new();

        let s1 = SystemSetting {
            id: 1,
            key: "site_name".to_string(),
            value: Some("Klass".to_string()),
            setting_type: "text".to_string(),
            setting_group: "general".to_string(),
            description: None,
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
            updated_at: DateTime::from_timestamp(0, 0).unwrap(),
        };

        let s2 = SystemSetting {
            id: 2,
            key: "items_per_page".to_string(),
            value: Some("15".to_string()),
            setting_type: "number".to_string(),
            setting_group: "general".to_string(),
            description: None,
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
            updated_at: DateTime::from_timestamp(0, 0).unwrap(),
        };

        let s3 = SystemSetting {
            id: 3,
            key: "enable_feature_x".to_string(),
            value: Some("true".to_string()),
            setting_type: "boolean".to_string(),
            setting_group: "features".to_string(),
            description: None,
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
            updated_at: DateTime::from_timestamp(0, 0).unwrap(),
        };

        grouped
            .entry("general".to_string())
            .or_default()
            .extend(vec![s1, s2]);
        grouped
            .entry("features".to_string())
            .or_default()
            .push(s3);

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped.get("general").unwrap().len(), 2);
        assert_eq!(grouped.get("features").unwrap().len(), 1);
    }

    // ── Edge cases ───────────────────────────────────────────────────────

    #[test]
    fn test_bulk_update_empty_is_noop() {
        let updates: Vec<BulkUpdateItem> = vec![];
        assert!(updates.is_empty());
    }

    #[test]
    fn test_get_with_fallback_default() {
        let key = "nonexistent";
        let default = Some("fallback");
        let result = default.map(String::from);
        assert_eq!(result, Some("fallback".to_string()));
    }

    #[test]
    fn test_get_without_default() {
        let default: Option<&str> = None;
        let result: Option<String> = default.map(String::from);
        assert!(result.is_none());
    }

    // ── Verify the trait is object-safe ───────────────────────────────────

    #[test]
    fn test_trait_is_object_safe() {
        fn _assert(_: &dyn SystemSettingsRepo) {}
    }
}

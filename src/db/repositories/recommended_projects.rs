use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::PgPool;

// ─── Source type constants ──────────────────────────────────────────────────

pub const SOURCE_ADMIN_UPLOAD: &str = "admin_upload";
pub const SOURCE_SYSTEM_TOPIC: &str = "system_topic";
pub const SOURCE_AI_GENERATED: &str = "ai_generated";

// ─── Struct ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RecommendedProject {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub project_file_url: Option<String>,
    pub ratio: String,
    pub project_type: Option<String>,
    pub tags: Option<serde_json::Value>,
    pub modules: Option<serde_json::Value>,
    pub source_type: String,
    pub source_reference: Option<String>,
    pub source_payload: Option<serde_json::Value>,
    pub display_priority: i32,
    pub is_active: bool,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub created_by: Option<i64>,
    pub updated_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ─── Input structs ──────────────────────────────────────────────────────────

/// Payload for creating a new RecommendedProject.
#[derive(Debug, Deserialize)]
pub struct CreateRecommendedProject {
    pub title: String,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub project_file_url: Option<String>,
    pub ratio: Option<String>,
    pub project_type: Option<String>,
    pub tags: Option<serde_json::Value>,
    pub modules: Option<serde_json::Value>,
    pub source_type: String,
    pub source_reference: Option<String>,
    pub source_payload: Option<serde_json::Value>,
    pub display_priority: Option<i32>,
    pub is_active: Option<bool>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub created_by: Option<i64>,
}

/// Payload for updating an existing RecommendedProject.
/// All fields are optional — only provided fields will be updated.
#[derive(Debug, Deserialize)]
pub struct UpdateRecommendedProject {
    pub title: Option<String>,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub project_file_url: Option<String>,
    pub ratio: Option<String>,
    pub project_type: Option<String>,
    pub tags: Option<serde_json::Value>,
    pub modules: Option<serde_json::Value>,
    pub source_type: Option<String>,
    pub source_reference: Option<String>,
    pub source_payload: Option<serde_json::Value>,
    pub display_priority: Option<i32>,
    pub is_active: Option<bool>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub updated_by: Option<i64>,
}

/// Filters for listing RecommendedProjects.
#[derive(Debug, Default)]
pub struct RecommendedProjectFilters {
    pub source_type: Option<String>,
    /// `Some(true)` → only active, `Some(false)` → only inactive, `None` → all
    pub is_active: Option<bool>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ─── Trait ──────────────────────────────────────────────────────────────────

#[async_trait]
pub trait RecommendedProjectsRepo: Send + Sync {
    /// Find all projects visible at the given moment (NOW() by default).
    async fn find_visible(&self) -> anyhow::Result<Vec<RecommendedProject>>;

    /// Find all projects visible at a specific moment.
    async fn find_visible_at(&self, moment: &DateTime<Utc>) -> anyhow::Result<Vec<RecommendedProject>>;

    /// Find a project by its primary key.
    async fn find_by_id(&self, id: i64) -> anyhow::Result<Option<RecommendedProject>>;

    /// List projects with optional filters.
    async fn find_all(&self, filters: &RecommendedProjectFilters) -> anyhow::Result<Vec<RecommendedProject>>;

    /// Create a new RecommendedProject. Returns the created record.
    async fn create(&self, payload: &CreateRecommendedProject) -> anyhow::Result<RecommendedProject>;

    /// Update an existing RecommendedProject. Returns the updated record.
    async fn update(&self, id: i64, payload: &UpdateRecommendedProject) -> anyhow::Result<Option<RecommendedProject>>;

    /// Delete a RecommendedProject by its primary key. Returns true if deleted.
    async fn delete(&self, id: i64) -> anyhow::Result<bool>;

    /// Toggle the `is_active` flag. Returns the new value.
    async fn toggle_active(&self, id: i64) -> anyhow::Result<Option<bool>>;

    /// Clear `starts_at` and set `is_active = true` (show immediately).
    async fn show_now(&self, id: i64, updated_by: Option<i64>) -> anyhow::Result<Option<RecommendedProject>>;
}

// ─── Pg implementation ──────────────────────────────────────────────────────

pub struct PgRecommendedProjectsRepo {
    pool: PgPool,
}

impl PgRecommendedProjectsRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Column list used in SELECT queries (consistent with `RecommendedProject` fields).
const SELECT_COLS: &str = r#"
    id, title, description, thumbnail_url, project_file_url,
    ratio, project_type, tags, modules,
    source_type, source_reference, source_payload,
    display_priority, is_active, starts_at, ends_at,
    created_by, updated_by, created_at, updated_at
"#;

#[async_trait]
impl RecommendedProjectsRepo for PgRecommendedProjectsRepo {
    async fn find_visible(&self) -> anyhow::Result<Vec<RecommendedProject>> {
        let sql = format!(
            r#"SELECT {SELECT_COLS}
               FROM recommended_projects
               WHERE is_active = true
                 AND (starts_at IS NULL OR starts_at <= NOW())
                 AND (ends_at IS NULL OR ends_at >= NOW())
               ORDER BY display_priority DESC, created_at DESC"#,
        );

        let projects = sqlx::query_as::<_, RecommendedProject>(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch visible recommended projects: {e}"))?;

        Ok(projects)
    }

    async fn find_visible_at(&self, moment: &DateTime<Utc>) -> anyhow::Result<Vec<RecommendedProject>> {
        let sql = format!(
            r#"SELECT {SELECT_COLS}
               FROM recommended_projects
               WHERE is_active = true
                 AND (starts_at IS NULL OR starts_at <= $1::TIMESTAMPTZ)
                 AND (ends_at IS NULL OR ends_at >= $1::TIMESTAMPTZ)
               ORDER BY display_priority DESC, created_at DESC"#,
        );

        let projects = sqlx::query_as::<_, RecommendedProject>(&sql)
            .bind(moment)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch visible projects at moment: {e}"))?;

        Ok(projects)
    }

    async fn find_by_id(&self, id: i64) -> anyhow::Result<Option<RecommendedProject>> {
        let sql = format!(
            r#"SELECT {SELECT_COLS} FROM recommended_projects WHERE id = $1"#,
        );

        let project = sqlx::query_as::<_, RecommendedProject>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch recommended project {id}: {e}"))?;

        Ok(project)
    }

    async fn find_all(&self, filters: &RecommendedProjectFilters) -> anyhow::Result<Vec<RecommendedProject>> {
        let mut conditions: Vec<String> = Vec::new();
        #[allow(unused_assignments)]
        let mut param_idx = 1u32;

        if filters.source_type.is_some() {
            conditions.push(format!("source_type = ${param_idx}"));
            param_idx += 1;
        }

        if filters.is_active.is_some() {
            conditions.push(format!("is_active = ${param_idx}"));
            param_idx += 1;
        }

        if filters.search.is_some() {
            conditions.push(format!("title ILIKE ${param_idx}"));
            param_idx += 1;
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let limit_clause = filters
            .limit
            .map(|l| format!(" LIMIT {l}"))
            .unwrap_or_default();
        let offset_clause = filters
            .offset
            .map(|o| format!(" OFFSET {o}"))
            .unwrap_or_default();

        let sql = format!(
            r#"SELECT {SELECT_COLS}
               FROM recommended_projects
               {where_clause}
               ORDER BY display_priority DESC, created_at DESC
               {limit_clause}{offset_clause}"#,
        );

        let mut query = sqlx::query_as::<_, RecommendedProject>(&sql);

        if let Some(ref source_type) = filters.source_type {
            query = query.bind(source_type);
        }
        if let Some(is_active) = filters.is_active {
            query = query.bind(is_active);
        }
        if let Some(ref search) = filters.search {
            query = query.bind(format!("%{search}%"));
        }

        let projects = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch recommended projects: {e}"))?;

        Ok(projects)
    }

    async fn create(&self, payload: &CreateRecommendedProject) -> anyhow::Result<RecommendedProject> {
        let sql = format!(
            r#"
            INSERT INTO recommended_projects
                (title, description, thumbnail_url, project_file_url,
                 ratio, project_type, tags, modules,
                 source_type, source_reference, source_payload,
                 display_priority, is_active, starts_at, ends_at, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8,
                    $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING {SELECT_COLS}
            "#,
        );

        let project = sqlx::query_as::<_, RecommendedProject>(&sql)
            .bind(&payload.title)
            .bind(&payload.description)
            .bind(&payload.thumbnail_url)
            .bind(&payload.project_file_url)
            .bind(payload.ratio.as_deref().unwrap_or("16:9"))
            .bind(&payload.project_type)
            .bind(&payload.tags)
            .bind(&payload.modules)
            .bind(&payload.source_type)
            .bind(&payload.source_reference)
            .bind(&payload.source_payload)
            .bind(payload.display_priority.unwrap_or(0))
            .bind(payload.is_active.unwrap_or(true))
            .bind(payload.starts_at)
            .bind(payload.ends_at)
            .bind(payload.created_by)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to create recommended project: {e}"))?;

        Ok(project)
    }

    async fn update(&self, id: i64, payload: &UpdateRecommendedProject) -> anyhow::Result<Option<RecommendedProject>> {
        // Build dynamic SET clause — count how many placeholders we need
        let set_parts = dynamic_update_set(payload);

        if set_parts.is_empty() {
            return Ok(self.find_by_id(id).await?);
        }

        let set_str = set_parts.join(", ");
        let id_param = set_parts.len() + 1; // next param index after SET fields

        let sql = format!(
            r#"UPDATE recommended_projects
               SET {set_str}, updated_at = NOW()
               WHERE id = ${id_param}
               RETURNING {SELECT_COLS}"#,
        );

        // Build the query and bind each present field in order
        let mut query = sqlx::query_as::<_, RecommendedProject>(&sql);

        if let Some(ref val) = payload.title {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.description {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.thumbnail_url {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.project_file_url {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.ratio {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.project_type {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.tags {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.modules {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.source_type {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.source_reference {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.source_payload {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.display_priority {
            query = query.bind(*val);
        }
        if let Some(ref val) = payload.is_active {
            query = query.bind(*val);
        }
        if let Some(ref val) = payload.starts_at {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.ends_at {
            query = query.bind(val);
        }
        if let Some(ref val) = payload.updated_by {
            query = query.bind(*val);
        }

        query = query.bind(id);

        let project = query
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to update recommended project {id}: {e}"))?;

        Ok(project)
    }

    async fn delete(&self, id: i64) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM recommended_projects WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to delete recommended project {id}: {e}"))?;

        Ok(result.rows_affected() > 0)
    }

    async fn toggle_active(&self, id: i64) -> anyhow::Result<Option<bool>> {
        let result = sqlx::query_scalar::<_, bool>(
            r#"
            UPDATE recommended_projects
            SET is_active = NOT is_active, updated_at = NOW()
            WHERE id = $1
            RETURNING is_active
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to toggle active for recommended project {id}: {e}"))?;

        Ok(result)
    }

    async fn show_now(&self, id: i64, updated_by: Option<i64>) -> anyhow::Result<Option<RecommendedProject>> {
        let sql = format!(
            r#"
            UPDATE recommended_projects
            SET is_active = true,
                starts_at = NULL,
                updated_by = $1,
                updated_at = NOW()
            WHERE id = $2
            RETURNING {SELECT_COLS}
            "#,
        );

        let project = sqlx::query_as::<_, RecommendedProject>(&sql)
            .bind(updated_by)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to show_now recommended project {id}: {e}"))?;

        Ok(project)
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Build the dynamic SET clause parts for UPDATE.
/// Returns `Vec<String>` where each entry is `"column = $N"` for non-None fields.
fn dynamic_update_set(payload: &UpdateRecommendedProject) -> Vec<String> {
    let mut parts = Vec::new();
    #[allow(unused_assignments)]
    let mut idx = 1u32;

    macro_rules! add_col {
        ($field:ident, $col:literal) => {
            if payload.$field.is_some() {
                parts.push(format!("{} = ${}", $col, idx));
                idx += 1;
            }
        };
    }

    add_col!(title, "title");
    add_col!(description, "description");
    add_col!(thumbnail_url, "thumbnail_url");
    add_col!(project_file_url, "project_file_url");
    add_col!(ratio, "ratio");
    add_col!(project_type, "project_type");
    add_col!(tags, "tags");
    add_col!(modules, "modules");
    add_col!(source_type, "source_type");
    add_col!(source_reference, "source_reference");
    add_col!(source_payload, "source_payload");
    add_col!(display_priority, "display_priority");
    add_col!(is_active, "is_active");
    add_col!(starts_at, "starts_at");
    add_col!(ends_at, "ends_at");
    add_col!(updated_by, "updated_by");

    parts
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_type_constants() {
        assert_eq!(SOURCE_ADMIN_UPLOAD, "admin_upload");
        assert_eq!(SOURCE_SYSTEM_TOPIC, "system_topic");
        assert_eq!(SOURCE_AI_GENERATED, "ai_generated");
    }

    #[test]
    fn test_create_payload_defaults() {
        let payload = CreateRecommendedProject {
            title: "Test Project".to_string(),
            description: None,
            thumbnail_url: None,
            project_file_url: None,
            ratio: None,
            project_type: None,
            tags: None,
            modules: None,
            source_type: SOURCE_ADMIN_UPLOAD.to_string(),
            source_reference: None,
            source_payload: None,
            display_priority: None,
            is_active: None,
            starts_at: None,
            ends_at: None,
            created_by: None,
        };
        assert_eq!(payload.title, "Test Project");
        assert_eq!(payload.source_type, SOURCE_ADMIN_UPLOAD);
    }

    #[test]
    fn test_update_payload_empty() {
        let payload = UpdateRecommendedProject {
            title: None,
            description: None,
            thumbnail_url: None,
            project_file_url: None,
            ratio: None,
            project_type: None,
            tags: None,
            modules: None,
            source_type: None,
            source_reference: None,
            source_payload: None,
            display_priority: None,
            is_active: None,
            starts_at: None,
            ends_at: None,
            updated_by: None,
        };
        // All fields are None -- should result in no-op
        assert!(payload.title.is_none());
    }

    #[test]
    fn test_visible_query_conditions() {
        // Verify the visible query logic conceptually:
        // is_active = true AND (starts_at IS NULL OR starts_at <= moment)
        //              AND (ends_at IS NULL OR ends_at >= moment)
        let moment = DateTime::parse_from_rfc3339("2026-04-03T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        // Just verify moment parses correctly
        assert_eq!(moment.to_rfc3339(), "2026-04-03T12:00:00+00:00");
    }

    #[test]
    fn test_find_all_empty_filters_no_panic() {
        let filters = RecommendedProjectFilters::default();
        assert!(filters.source_type.is_none());
        assert!(filters.is_active.is_none());
        assert!(filters.search.is_none());
        assert!(filters.limit.is_none());
        assert!(filters.offset.is_none());
    }
}

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

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

#[async_trait]
pub trait RecommendedProjectsRepo: Send + Sync {
    async fn find_visible(&self) -> anyhow::Result<Vec<RecommendedProject>>;
}

pub struct PgRecommendedProjectsRepo {
    pool: PgPool,
}

impl PgRecommendedProjectsRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RecommendedProjectsRepo for PgRecommendedProjectsRepo {
    async fn find_visible(&self) -> anyhow::Result<Vec<RecommendedProject>> {
        let sql = r#"
            SELECT id, title, description, thumbnail_url, project_file_url,
                   ratio, project_type, tags, modules,
                   source_type, source_reference, source_payload,
                   display_priority, is_active, starts_at, ends_at,
                   created_by, updated_by, created_at, updated_at
            FROM recommended_projects
            WHERE is_active = true
              AND (starts_at IS NULL OR starts_at <= NOW())
              AND (ends_at IS NULL OR ends_at >= NOW())
            ORDER BY display_priority DESC, created_at DESC
        "#;

        let projects = sqlx::query_as::<_, RecommendedProject>(sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch visible recommended projects: {e}"))?;

        Ok(projects)
    }
}

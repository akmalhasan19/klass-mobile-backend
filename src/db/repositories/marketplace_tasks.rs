use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::pagination::PaginationQuery;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MarketplaceTask {
    pub id: Uuid,
    pub content_id: Uuid,
    pub status: String,
    pub task_type: String,
    pub description: Option<String>,
    pub creator_id: Option<String>,
    pub suggested_freelancer_id: Option<i64>,
    pub attachment_url: Option<String>,
    pub media_generation_id: Option<Uuid>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ContentSummary {
    pub id: Uuid,
    pub topic_id: Uuid,
    #[sqlx(rename = "type")]
    pub content_type: String,
    pub title: Option<String>,
    pub data: Option<serde_json::Value>,
    pub media_url: Option<String>,
    pub is_published: bool,
    pub order: i32,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct MarketplaceTaskWithContent {
    pub task: MarketplaceTask,
    pub content: ContentSummary,
}

#[derive(Debug, Clone, Default)]
pub struct MarketplaceTaskFilters {
    pub search: Option<String>,
    pub status: Option<String>,
    pub content_id: Option<Uuid>,
}

#[async_trait]
pub trait MarketplaceTasksRepo: Send + Sync {
    async fn find_many(
        &self,
        filters: &MarketplaceTaskFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<MarketplaceTaskWithContent>, i64)>;
    async fn find_by_id(&self, id: Uuid) -> anyhow::Result<Option<MarketplaceTaskWithContent>>;
}

pub struct PgMarketplaceTasksRepo {
    pool: PgPool,
}

impl PgMarketplaceTasksRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MarketplaceTasksRepo for PgMarketplaceTasksRepo {
    async fn find_many(
        &self,
        filters: &MarketplaceTaskFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<MarketplaceTaskWithContent>, i64)> {
        let search_pattern = filters.search.as_ref().map(|s| format!("%{}%", s));

        let count_sql = r#"
            SELECT COUNT(*)
            FROM marketplace_tasks mt
            WHERE ($1::text IS NULL OR EXISTS (
                SELECT 1 FROM contents c
                WHERE c.id = mt.content_id
                  AND c.title ILIKE $1
            ))
              AND ($2::text IS NULL OR mt.status = $2)
              AND ($3::uuid IS NULL OR mt.content_id = $3)
        "#;

        let total: i64 = sqlx::query_scalar(count_sql)
            .bind(search_pattern.as_deref())
            .bind(filters.status.as_deref())
            .bind(filters.content_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to count marketplace tasks: {e}"))?;

        let data_sql = r#"
            SELECT mt.id, mt.content_id, mt.status, mt.task_type, mt.description,
                   mt.creator_id, mt.suggested_freelancer_id, mt.attachment_url,
                   mt.media_generation_id, mt.created_at, mt.updated_at
            FROM marketplace_tasks mt
            WHERE ($1::text IS NULL OR EXISTS (
                SELECT 1 FROM contents c
                WHERE c.id = mt.content_id
                  AND c.title ILIKE $1
            ))
              AND ($2::text IS NULL OR mt.status = $2)
              AND ($3::uuid IS NULL OR mt.content_id = $3)
            ORDER BY mt.created_at DESC
            LIMIT $4 OFFSET $5
        "#;

        let tasks = sqlx::query_as::<_, MarketplaceTask>(data_sql)
            .bind(search_pattern.as_deref())
            .bind(filters.status.as_deref())
            .bind(filters.content_id)
            .bind(pagination.limit())
            .bind(pagination.offset())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch marketplace tasks: {e}"))?;

        let content_ids: Vec<Uuid> = tasks.iter().map(|t| t.content_id).collect();

        let contents = if content_ids.is_empty() {
            Vec::new()
        } else {
            let contents_sql = r#"
                SELECT id, topic_id, type, title, data, media_url,
                       is_published, "order", created_at, updated_at
                FROM contents
                WHERE id = ANY($1)
            "#;

            sqlx::query_as::<_, ContentSummary>(contents_sql)
                .bind(&content_ids)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| anyhow::anyhow!("failed to fetch contents: {e}"))?
        };

        let tasks_with_content: Vec<MarketplaceTaskWithContent> = tasks
            .into_iter()
            .filter_map(|task| {
                let content = contents.iter().find(|c| c.id == task.content_id)?;
                Some(MarketplaceTaskWithContent {
                    task,
                    content: content.clone(),
                })
            })
            .collect();

        Ok((tasks_with_content, total))
    }

    async fn find_by_id(&self, id: Uuid) -> anyhow::Result<Option<MarketplaceTaskWithContent>> {
        let task_sql = r#"
            SELECT id, content_id, status, task_type, description, creator_id,
                   suggested_freelancer_id, attachment_url, media_generation_id,
                   created_at, updated_at
            FROM marketplace_tasks
            WHERE id = $1
        "#;

        let task = sqlx::query_as::<_, MarketplaceTask>(task_sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch marketplace task: {e}"))?;

        let task = match task {
            Some(t) => t,
            None => return Ok(None),
        };

        let content_sql = r#"
            SELECT id, topic_id, type, title, data, media_url,
                   is_published, "order", created_at, updated_at
            FROM contents
            WHERE id = $1
        "#;

        let content = sqlx::query_as::<_, ContentSummary>(content_sql)
            .bind(task.content_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch content: {e}"))?;

        let content = match content {
            Some(c) => c,
            None => return Ok(None),
        };

        Ok(Some(MarketplaceTaskWithContent { task, content }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_marketplace_task_filters_default() {
        let filters = MarketplaceTaskFilters::default();
        assert!(filters.search.is_none());
        assert!(filters.status.is_none());
        assert!(filters.content_id.is_none());
    }

    #[test]
    fn test_marketplace_task_filters_custom() {
        let content_id = Uuid::new_v4();
        let filters = MarketplaceTaskFilters {
            search: Some("math".to_string()),
            status: Some("open".to_string()),
            content_id: Some(content_id),
        };
        assert_eq!(filters.search.as_deref(), Some("math"));
        assert_eq!(filters.status.as_deref(), Some("open"));
        assert_eq!(filters.content_id, Some(content_id));
    }

    #[test]
    fn test_marketplace_task_with_content_structure() {
        let task = MarketplaceTask {
            id: Uuid::new_v4(),
            content_id: Uuid::new_v4(),
            status: "open".to_string(),
            task_type: "bid".to_string(),
            description: Some("Test task".to_string()),
            creator_id: Some("123".to_string()),
            suggested_freelancer_id: None,
            attachment_url: Some("https://example.com/file.pdf".to_string()),
            media_generation_id: None,
            created_at: None,
            updated_at: None,
        };

        let content = ContentSummary {
            id: task.content_id,
            topic_id: Uuid::new_v4(),
            content_type: "module".to_string(),
            title: Some("Test Content".to_string()),
            data: None,
            media_url: None,
            is_published: true,
            order: 0,
            created_at: None,
            updated_at: None,
        };

        let mtc = MarketplaceTaskWithContent {
            task: task.clone(),
            content: content.clone(),
        };

        assert_eq!(mtc.task.id, task.id);
        assert_eq!(mtc.content.id, content.id);
        assert_eq!(mtc.task.content_id, mtc.content.id);
    }

    #[test]
    fn test_marketplace_task_status_values() {
        let statuses = vec!["open", "taken", "done"];
        for status in statuses {
            let task = MarketplaceTask {
                id: Uuid::new_v4(),
                content_id: Uuid::new_v4(),
                status: status.to_string(),
                task_type: "bid".to_string(),
                description: None,
                creator_id: None,
                suggested_freelancer_id: None,
                attachment_url: None,
                media_generation_id: None,
                created_at: None,
                updated_at: None,
            };
            assert_eq!(task.status, status);
        }
    }

    #[test]
    fn test_marketplace_task_type_values() {
        let task_types = vec!["bid", "suggestion"];
        for task_type in task_types {
            let task = MarketplaceTask {
                id: Uuid::new_v4(),
                content_id: Uuid::new_v4(),
                status: "open".to_string(),
                task_type: task_type.to_string(),
                description: None,
                creator_id: None,
                suggested_freelancer_id: None,
                attachment_url: None,
                media_generation_id: None,
                created_at: None,
                updated_at: None,
            };
            assert_eq!(task.task_type, task_type);
        }
    }
}

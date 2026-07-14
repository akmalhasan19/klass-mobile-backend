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

/// Payload for creating a new marketplace task.
#[derive(Debug, Clone)]
pub struct CreateMarketplaceTaskPayload {
    pub content_id: Uuid,
    pub status: String,
    pub task_type: String,
    pub description: Option<String>,
    pub creator_id: Option<String>,
    pub suggested_freelancer_id: Option<i64>,
    pub attachment_url: Option<String>,
    pub media_generation_id: Option<Uuid>,
}

/// Payload for partial update of a marketplace task.
/// Nullable fields use `Option<Option<T>>` to distinguish between:
/// - `None` — field not provided, leave unchanged
/// - `Some(None)` — explicitly set to NULL
/// - `Some(Some(v))` — set to value `v`
#[derive(Debug, Clone, Default)]
pub struct UpdateMarketplaceTaskPayload {
    pub content_id: Option<Uuid>,
    pub task_type: Option<String>,
    pub description: Option<Option<String>>,
    pub creator_id: Option<Option<String>>,
    pub suggested_freelancer_id: Option<Option<i64>>,
    pub attachment_url: Option<Option<String>>,
    pub media_generation_id: Option<Option<Uuid>>,
}

#[async_trait]
pub trait MarketplaceTasksRepo: Send + Sync {
    async fn find_many(
        &self,
        filters: &MarketplaceTaskFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<MarketplaceTaskWithContent>, i64)>;
    async fn find_by_id(&self, id: Uuid) -> anyhow::Result<Option<MarketplaceTaskWithContent>>;

    /// Insert a new marketplace task. Returns the created MarketplaceTask.
    async fn insert(&self, payload: &CreateMarketplaceTaskPayload) -> anyhow::Result<MarketplaceTask>;

    /// Update a marketplace task with partial fields. Returns the updated MarketplaceTask.
    async fn update(&self, id: Uuid, payload: &UpdateMarketplaceTaskPayload) -> anyhow::Result<MarketplaceTask>;

    /// Delete a marketplace task by ID. Returns `true` if a row was deleted.
    async fn delete(&self, id: Uuid) -> anyhow::Result<bool>;

    /// Update only the status field. Returns the updated MarketplaceTask.
    async fn update_status(&self, id: Uuid, status: &str) -> anyhow::Result<MarketplaceTask>;
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

    async fn insert(&self, payload: &CreateMarketplaceTaskPayload) -> anyhow::Result<MarketplaceTask> {
        let id = Uuid::new_v4();
        let task = sqlx::query_as::<_, MarketplaceTask>(
            r#"
            INSERT INTO marketplace_tasks
                (id, content_id, status, task_type, description, creator_id,
                 suggested_freelancer_id, attachment_url, media_generation_id,
                 created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())
            RETURNING id, content_id, status, task_type, description, creator_id,
                      suggested_freelancer_id, attachment_url, media_generation_id,
                      created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(payload.content_id)
        .bind(&payload.status)
        .bind(&payload.task_type)
        .bind(&payload.description)
        .bind(&payload.creator_id)
        .bind(payload.suggested_freelancer_id)
        .bind(&payload.attachment_url)
        .bind(payload.media_generation_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to insert marketplace task: {e}"))?;

        Ok(task)
    }

    async fn update(&self, id: Uuid, payload: &UpdateMarketplaceTaskPayload) -> anyhow::Result<MarketplaceTask> {
        let current = sqlx::query_as::<_, MarketplaceTask>(
            r#"SELECT id, content_id, status, task_type, description, creator_id,
                     suggested_freelancer_id, attachment_url, media_generation_id,
                     created_at, updated_at
              FROM marketplace_tasks WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("marketplace task not found"))?;

        let new_content_id = payload.content_id.unwrap_or(current.content_id);
        let new_task_type = payload
            .task_type
            .clone()
            .unwrap_or_else(|| current.task_type.clone());
        let new_description = payload.description.clone().unwrap_or(current.description);
        let new_creator_id = payload.creator_id.clone().unwrap_or(current.creator_id);
        let new_suggested_freelancer_id = payload
            .suggested_freelancer_id
            .unwrap_or(current.suggested_freelancer_id);
        let new_attachment_url = payload
            .attachment_url
            .clone()
            .unwrap_or(current.attachment_url);
        let new_media_generation_id = payload
            .media_generation_id
            .unwrap_or(current.media_generation_id);

        let updated = sqlx::query_as::<_, MarketplaceTask>(
            r#"
            UPDATE marketplace_tasks
            SET content_id = $2,
                task_type = $3,
                description = $4,
                creator_id = $5,
                suggested_freelancer_id = $6,
                attachment_url = $7,
                media_generation_id = $8,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, content_id, status, task_type, description, creator_id,
                      suggested_freelancer_id, attachment_url, media_generation_id,
                      created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(new_content_id)
        .bind(&new_task_type)
        .bind(&new_description)
        .bind(&new_creator_id)
        .bind(new_suggested_freelancer_id)
        .bind(&new_attachment_url)
        .bind(new_media_generation_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to update marketplace task: {e}"))?;

        Ok(updated)
    }

    async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(r#"DELETE FROM marketplace_tasks WHERE id = $1"#)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to delete marketplace task: {e}"))?;

        Ok(result.rows_affected() > 0)
    }

    async fn update_status(&self, id: Uuid, status: &str) -> anyhow::Result<MarketplaceTask> {
        let updated = sqlx::query_as::<_, MarketplaceTask>(
            r#"
            UPDATE marketplace_tasks
            SET status = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, content_id, status, task_type, description, creator_id,
                      suggested_freelancer_id, attachment_url, media_generation_id,
                      created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(status)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to update marketplace task status: {e}"))?;

        updated.ok_or_else(|| anyhow::anyhow!("marketplace task not found"))
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

    #[test]
    fn test_create_marketplace_task_payload() {
        let payload = CreateMarketplaceTaskPayload {
            content_id: Uuid::new_v4(),
            status: "open".to_string(),
            task_type: "bid".to_string(),
            description: Some("Test description".to_string()),
            creator_id: Some("123".to_string()),
            suggested_freelancer_id: Some(1),
            attachment_url: Some("https://example.com/file.pdf".to_string()),
            media_generation_id: None,
        };
        assert_eq!(payload.status, "open");
        assert_eq!(payload.task_type, "bid");
    }

    #[test]
    fn test_update_marketplace_task_payload_default() {
        let payload = UpdateMarketplaceTaskPayload::default();
        assert!(payload.content_id.is_none());
        assert!(payload.task_type.is_none());
        assert!(payload.description.is_none());
        assert!(payload.creator_id.is_none());
        assert!(payload.suggested_freelancer_id.is_none());
        assert!(payload.attachment_url.is_none());
        assert!(payload.media_generation_id.is_none());
    }

    #[test]
    fn test_update_marketplace_task_payload_tri_state() {
        // None = don't update
        let payload = UpdateMarketplaceTaskPayload {
            content_id: None,
            task_type: None,
            description: None,
            creator_id: None,
            suggested_freelancer_id: None,
            attachment_url: None,
            media_generation_id: None,
        };
        assert!(payload.description.is_none());

        // Some(None) = explicitly set to NULL
        let payload = UpdateMarketplaceTaskPayload {
            content_id: None,
            task_type: None,
            description: Some(None),
            creator_id: None,
            suggested_freelancer_id: None,
            attachment_url: None,
            media_generation_id: None,
        };
        assert_eq!(payload.description, Some(None));

        // Some(Some(v)) = set to value
        let payload = UpdateMarketplaceTaskPayload {
            content_id: None,
            task_type: None,
            description: Some(Some("new desc".to_string())),
            creator_id: None,
            suggested_freelancer_id: None,
            attachment_url: None,
            media_generation_id: None,
        };
        assert_eq!(payload.description, Some(Some("new desc".to_string())));
    }
}

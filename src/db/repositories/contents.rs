use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::pagination::PaginationQuery;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Content {
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

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TopicSummary {
    pub id: Uuid,
    pub title: String,
    pub teacher_id: String,
    pub sub_subject_id: Option<i64>,
    pub thumbnail_url: Option<String>,
    pub is_published: bool,
    pub order: i32,
    pub owner_user_id: Option<i64>,
    pub ownership_status: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MarketplaceTaskSummary {
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

#[derive(Debug, Clone)]
pub struct ContentWithRelations {
    pub content: Content,
    pub topic: TopicSummary,
    pub tasks: Vec<MarketplaceTaskSummary>,
}

#[derive(Debug, Clone, Default)]
pub struct ContentFilters {
    pub search: Option<String>,
    pub topic_id: Option<Uuid>,
    pub content_type: Option<String>,
}

/// Payload for creating a new content.
#[derive(Debug, Clone)]
pub struct CreateContentPayload {
    pub topic_id: Uuid,
    pub content_type: String,
    pub title: Option<String>,
    pub data: Option<serde_json::Value>,
    pub media_url: Option<String>,
}

/// Payload for partial update of a content.
/// Each nullable field is `Option<Option<T>>` to distinguish between:
/// - `None` — field not provided, leave unchanged
/// - `Some(None)` — explicitly set to NULL
/// - `Some(Some(v))` — set to value `v`
#[derive(Debug, Clone, Default)]
pub struct UpdateContentPayload {
    pub topic_id: Option<Uuid>,
    pub content_type: Option<String>,
    pub title: Option<Option<String>>,
    pub data: Option<Option<serde_json::Value>>,
    pub media_url: Option<Option<String>>,
}

#[async_trait]
pub trait ContentsRepo: Send + Sync {
    async fn find_many(
        &self,
        filters: &ContentFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<Content>, i64)>;
    async fn find_by_id(&self, id: Uuid) -> anyhow::Result<Option<ContentWithRelations>>;

    /// Insert a new content row. Returns the created Content.
    async fn insert(&self, payload: &CreateContentPayload) -> anyhow::Result<Content>;

    /// Update a content with partial fields. Returns the updated Content.
    async fn update(&self, id: Uuid, payload: &UpdateContentPayload) -> anyhow::Result<Content>;

    /// Delete a content by ID. Returns `true` if a row was deleted.
    async fn delete(&self, id: Uuid) -> anyhow::Result<bool>;

    /// Set the `is_published` flag explicitly. Returns the updated value.
    async fn set_publish(&self, id: Uuid, is_published: bool) -> anyhow::Result<bool>;

    /// Reorder a content up or down by swapping its `order` with the adjacent content
    /// within the same topic_id group. `direction` must be `"up"` or `"down"`.
    async fn reorder(&self, id: Uuid, direction: &str) -> anyhow::Result<()>;
}

pub struct PgContentsRepo {
    pool: PgPool,
}

impl PgContentsRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ContentsRepo for PgContentsRepo {
    async fn find_many(
        &self,
        filters: &ContentFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<Content>, i64)> {
        let search_pattern = filters.search.as_ref().map(|s| format!("%{}%", s));

        let count_sql = r#"
            SELECT COUNT(*)
            FROM contents c
            WHERE ($1::text IS NULL OR c.title ILIKE $1)
              AND ($2::uuid IS NULL OR c.topic_id = $2)
              AND ($3::text IS NULL OR c.type = $3)
        "#;

        let total: i64 = sqlx::query_scalar(count_sql)
            .bind(search_pattern.as_deref())
            .bind(filters.topic_id)
            .bind(filters.content_type.as_deref())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to count contents: {e}"))?;

        let data_sql = r#"
            SELECT id, topic_id, type, title, data, media_url,
                   is_published, "order", created_at, updated_at
            FROM contents c
            WHERE ($1::text IS NULL OR c.title ILIKE $1)
              AND ($2::uuid IS NULL OR c.topic_id = $2)
              AND ($3::text IS NULL OR c.type = $3)
            ORDER BY c."order" ASC, c.created_at DESC
            LIMIT $4 OFFSET $5
        "#;

        let contents = sqlx::query_as::<_, Content>(data_sql)
            .bind(search_pattern.as_deref())
            .bind(filters.topic_id)
            .bind(filters.content_type.as_deref())
            .bind(pagination.limit())
            .bind(pagination.offset())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch contents: {e}"))?;

        Ok((contents, total))
    }

    async fn find_by_id(&self, id: Uuid) -> anyhow::Result<Option<ContentWithRelations>> {
        let content_sql = r#"
            SELECT id, topic_id, type, title, data, media_url,
                   is_published, "order", created_at, updated_at
            FROM contents
            WHERE id = $1
        "#;

        let content = sqlx::query_as::<_, Content>(content_sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch content: {e}"))?;

        let content = match content {
            Some(c) => c,
            None => return Ok(None),
        };

        let topic_sql = r#"
            SELECT id, title, teacher_id, sub_subject_id, thumbnail_url,
                   is_published, "order", owner_user_id, ownership_status,
                   created_at, updated_at
            FROM topics
            WHERE id = $1
        "#;

        let topic = sqlx::query_as::<_, TopicSummary>(topic_sql)
            .bind(content.topic_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch topic: {e}"))?;

        let topic = match topic {
            Some(t) => t,
            None => return Ok(None),
        };

        let tasks_sql = r#"
            SELECT id, content_id, status, task_type, description, creator_id,
                   suggested_freelancer_id, attachment_url, media_generation_id,
                   created_at, updated_at
            FROM marketplace_tasks
            WHERE content_id = $1
        "#;

        let tasks = sqlx::query_as::<_, MarketplaceTaskSummary>(tasks_sql)
            .bind(id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch tasks: {e}"))?;

        Ok(Some(ContentWithRelations {
            content,
            topic,
            tasks,
        }))
    }

    async fn insert(&self, payload: &CreateContentPayload) -> anyhow::Result<Content> {
        let id = Uuid::new_v4();
        let content = sqlx::query_as::<_, Content>(
            r#"
            INSERT INTO contents (id, topic_id, type, title, data, media_url,
                                  is_published, "order", created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, TRUE, 0, NOW(), NOW())
            RETURNING id, topic_id, type, title, data, media_url,
                      is_published, "order", created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(payload.topic_id)
        .bind(&payload.content_type)
        .bind(&payload.title)
        .bind(&payload.data)
        .bind(&payload.media_url)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to insert content: {e}"))?;

        Ok(content)
    }

    async fn update(&self, id: Uuid, payload: &UpdateContentPayload) -> anyhow::Result<Content> {
        let current = sqlx::query_as::<_, Content>(
            r#"SELECT id, topic_id, type, title, data, media_url,
                     is_published, "order", created_at, updated_at
              FROM contents WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("content not found"))?;

        let new_topic_id = payload.topic_id.unwrap_or(current.topic_id);
        let new_content_type = payload
            .content_type
            .clone()
            .unwrap_or_else(|| current.content_type.clone());
        let new_title = payload.title.clone().unwrap_or(current.title);
        let new_data = payload.data.clone().unwrap_or(current.data);
        let new_media_url = payload.media_url.clone().unwrap_or(current.media_url);

        let updated = sqlx::query_as::<_, Content>(
            r#"
            UPDATE contents
            SET topic_id = $2,
                type = $3,
                title = $4,
                data = $5,
                media_url = $6,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, topic_id, type, title, data, media_url,
                      is_published, "order", created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(new_topic_id)
        .bind(&new_content_type)
        .bind(&new_title)
        .bind(&new_data)
        .bind(&new_media_url)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to update content: {e}"))?;

        Ok(updated)
    }

    async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(r#"DELETE FROM contents WHERE id = $1"#)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to delete content: {e}"))?;

        Ok(result.rows_affected() > 0)
    }

    async fn set_publish(&self, id: Uuid, is_published: bool) -> anyhow::Result<bool> {
        let row: Option<(bool,)> = sqlx::query_as(
            r#"
            UPDATE contents
            SET is_published = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING is_published
            "#,
        )
        .bind(id)
        .bind(is_published)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to set content publish: {e}"))?;

        row.map(|r| r.0)
            .ok_or_else(|| anyhow::anyhow!("content not found"))
    }

    async fn reorder(&self, id: Uuid, direction: &str) -> anyhow::Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| anyhow::anyhow!("failed to begin transaction: {e}"))?;

        // Lock the current content row and get its topic_id + order
        let current: (Uuid, i32) = sqlx::query_as(
            r#"SELECT topic_id, "order" FROM contents WHERE id = $1 FOR UPDATE"#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("content not found"))?;

        let (topic_id, current_order) = current;

        let neighbor = match direction {
            "up" => {
                // Find the content with the highest order that is still < current_order
                // within the same topic_id group
                sqlx::query_as::<_, (Uuid, i32)>(
                    r#"SELECT id, "order" FROM contents
                       WHERE topic_id = $1 AND "order" < $2
                       ORDER BY "order" DESC
                       LIMIT 1
                       FOR UPDATE"#,
                )
                .bind(topic_id)
                .bind(current_order)
                .fetch_optional(&mut *tx)
                .await?
            }
            "down" => {
                // Find the content with the lowest order that is still > current_order
                // within the same topic_id group
                sqlx::query_as::<_, (Uuid, i32)>(
                    r#"SELECT id, "order" FROM contents
                       WHERE topic_id = $1 AND "order" > $2
                       ORDER BY "order" ASC
                       LIMIT 1
                       FOR UPDATE"#,
                )
                .bind(topic_id)
                .bind(current_order)
                .fetch_optional(&mut *tx)
                .await?
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "invalid direction '{}': must be 'up' or 'down'",
                    direction
                ));
            }
        };

        let (neighbor_id, neighbor_order) = neighbor
            .ok_or_else(|| anyhow::anyhow!("cannot move {} further — already at the edge", direction))?;

        // Swap the two order values
        sqlx::query(r#"UPDATE contents SET "order" = $2, updated_at = NOW() WHERE id = $1"#)
            .bind(id)
            .bind(neighbor_order)
            .execute(&mut *tx)
            .await?;

        sqlx::query(r#"UPDATE contents SET "order" = $2, updated_at = NOW() WHERE id = $1"#)
            .bind(neighbor_id)
            .bind(current_order)
            .execute(&mut *tx)
            .await?;

        tx.commit()
            .await
            .map_err(|e| anyhow::anyhow!("failed to commit reorder transaction: {e}"))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_filters_default() {
        let filters = ContentFilters::default();
        assert!(filters.search.is_none());
        assert!(filters.topic_id.is_none());
        assert!(filters.content_type.is_none());
    }

    #[test]
    fn test_content_filters_custom() {
        let topic_id = Uuid::new_v4();
        let filters = ContentFilters {
            search: Some("module".to_string()),
            topic_id: Some(topic_id),
            content_type: Some("quiz".to_string()),
        };
        assert_eq!(filters.search.as_deref(), Some("module"));
        assert_eq!(filters.topic_id, Some(topic_id));
        assert_eq!(filters.content_type.as_deref(), Some("quiz"));
    }

    #[test]
    fn test_content_with_relations_structure() {
        let content = Content {
            id: Uuid::new_v4(),
            topic_id: Uuid::new_v4(),
            content_type: "module".to_string(),
            title: Some("Test Content".to_string()),
            data: Some(serde_json::json!({"key": "value"})),
            media_url: Some("https://example.com/file.pdf".to_string()),
            is_published: true,
            order: 1,
            created_at: None,
            updated_at: None,
        };

        let topic = TopicSummary {
            id: content.topic_id,
            title: "Test Topic".to_string(),
            teacher_id: "123".to_string(),
            sub_subject_id: Some(1),
            thumbnail_url: None,
            is_published: true,
            order: 0,
            owner_user_id: Some(1),
            ownership_status: "normalized".to_string(),
            created_at: None,
            updated_at: None,
        };

        let task = MarketplaceTaskSummary {
            id: Uuid::new_v4(),
            content_id: content.id,
            status: "open".to_string(),
            task_type: "bid".to_string(),
            description: Some("Test task".to_string()),
            creator_id: None,
            suggested_freelancer_id: None,
            attachment_url: None,
            media_generation_id: None,
            created_at: None,
            updated_at: None,
        };

        let cwr = ContentWithRelations {
            content: content.clone(),
            topic: topic.clone(),
            tasks: vec![task.clone()],
        };

        assert_eq!(cwr.content.id, content.id);
        assert_eq!(cwr.topic.id, topic.id);
        assert_eq!(cwr.tasks.len(), 1);
        assert_eq!(cwr.tasks[0].id, task.id);
    }

    #[test]
    fn test_content_data_preserve_order() {
        let json_str = r#"{"z":1,"a":2,"m":3}"#;
        let data: serde_json::Value = serde_json::from_str(json_str).unwrap();

        let content = Content {
            id: Uuid::new_v4(),
            topic_id: Uuid::new_v4(),
            content_type: "module".to_string(),
            title: None,
            data: Some(data.clone()),
            media_url: None,
            is_published: true,
            order: 0,
            created_at: None,
            updated_at: None,
        };

        let serialized = serde_json::to_string(content.data.as_ref().unwrap()).unwrap();
        assert_eq!(serialized, json_str);

        if let serde_json::Value::Object(map) = content.data.as_ref().unwrap() {
            let keys: Vec<&String> = map.keys().collect();
            assert_eq!(keys, vec!["z", "a", "m"]);
        } else {
            panic!("Expected object");
        }
    }
}

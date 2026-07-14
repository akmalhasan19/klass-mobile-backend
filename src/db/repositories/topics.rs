use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::pagination::PaginationQuery;

/// Payload for partial update of a topic.
/// Each field is `Option<Option<T>>` to distinguish between:
/// - `None` — field not provided, leave unchanged
/// - `Some(None)` — explicitly set to NULL
/// - `Some(Some(v))` — set to value `v`
#[derive(Debug, Clone, Default)]
pub struct UpdateTopicPayload {
    pub title: Option<String>,
    pub sub_subject_id: Option<Option<i64>>,
    pub thumbnail_url: Option<Option<String>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Topic {
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

#[derive(Debug, Clone)]
pub struct ContentWithTasks {
    pub content: Content,
    pub tasks: Vec<MarketplaceTask>,
}

#[derive(Debug, Clone)]
pub struct TopicWithContents {
    pub topic: Topic,
    pub contents: Vec<ContentWithTasks>,
}

#[derive(Debug, Clone, Default)]
pub struct TopicFilters {
    pub search: Option<String>,
    pub teacher_id: Option<String>,
    pub subject_id: Option<i64>,
    pub sub_subject_id: Option<i64>,
    pub is_published: Option<bool>,
}

#[async_trait]
pub trait TopicsRepo: Send + Sync {
    async fn find_many(
        &self,
        filters: &TopicFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<Topic>, i64)>;
    async fn find_by_id(&self, id: Uuid) -> anyhow::Result<Option<TopicWithContents>>;

    /// Update a topic with partial fields.
    async fn update(&self, id: Uuid, payload: &UpdateTopicPayload) -> anyhow::Result<Topic>;

    /// Delete a topic by ID. Returns `true` if a row was deleted.
    async fn delete(&self, id: Uuid) -> anyhow::Result<bool>;

    /// Set the `is_published` flag explicitly. Returns the updated value.
    async fn set_publish(&self, id: Uuid, is_published: bool) -> anyhow::Result<bool>;

    /// Reorder a topic up or down by swapping its `order` with the adjacent topic.
    /// `direction` must be `"up"` or `"down"`.
    async fn reorder(&self, id: Uuid, direction: &str) -> anyhow::Result<()>;
}

pub struct PgTopicsRepo {
    pool: PgPool,
}

impl PgTopicsRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TopicsRepo for PgTopicsRepo {
    async fn find_many(
        &self,
        filters: &TopicFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<Topic>, i64)> {
        let search_pattern = filters.search.as_ref().map(|s| format!("%{}%", s));

        let count_sql = r#"
            SELECT COUNT(*)
            FROM topics t
            LEFT JOIN sub_subjects ss ON t.sub_subject_id = ss.id
            WHERE ($1::text IS NULL OR t.title ILIKE $1)
              AND ($2::text IS NULL OR t.teacher_id = $2)
              AND ($3::bigint IS NULL OR ss.subject_id = $3)
              AND ($4::bigint IS NULL OR t.sub_subject_id = $4)
              AND ($5::boolean IS NULL OR t.is_published = $5)
        "#;

        let total: i64 = sqlx::query_scalar(count_sql)
            .bind(search_pattern.as_deref())
            .bind(filters.teacher_id.as_deref())
            .bind(filters.subject_id)
            .bind(filters.sub_subject_id)
            .bind(filters.is_published)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to count topics: {e}"))?;

        let data_sql = r#"
            SELECT t.id, t.title, t.teacher_id, t.sub_subject_id, t.thumbnail_url,
                   t.is_published, t."order", t.owner_user_id, t.ownership_status,
                   t.created_at, t.updated_at
            FROM topics t
            LEFT JOIN sub_subjects ss ON t.sub_subject_id = ss.id
            WHERE ($1::text IS NULL OR t.title ILIKE $1)
              AND ($2::text IS NULL OR t.teacher_id = $2)
              AND ($3::bigint IS NULL OR ss.subject_id = $3)
              AND ($4::bigint IS NULL OR t.sub_subject_id = $4)
              AND ($5::boolean IS NULL OR t.is_published = $5)
            ORDER BY t."order" ASC, t.created_at DESC
            LIMIT $6 OFFSET $7
        "#;

        let topics = sqlx::query_as::<_, Topic>(data_sql)
            .bind(search_pattern.as_deref())
            .bind(filters.teacher_id.as_deref())
            .bind(filters.subject_id)
            .bind(filters.sub_subject_id)
            .bind(filters.is_published)
            .bind(pagination.limit())
            .bind(pagination.offset())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch topics: {e}"))?;

        Ok((topics, total))
    }

    async fn update(&self, id: Uuid, payload: &UpdateTopicPayload) -> anyhow::Result<Topic> {
        let current = sqlx::query_as::<_, Topic>(
            r#"SELECT id, title, teacher_id, sub_subject_id, thumbnail_url,
                     is_published, "order", owner_user_id, ownership_status,
                     created_at, updated_at
              FROM topics WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("topic not found"))?;

        let new_title = payload
            .title
            .clone()
            .unwrap_or_else(|| current.title.clone());
        let new_sub_subject_id = payload.sub_subject_id.unwrap_or(current.sub_subject_id);
        let new_thumbnail_url = payload.thumbnail_url.clone().unwrap_or(current.thumbnail_url);

        let updated = sqlx::query_as::<_, Topic>(
            r#"
            UPDATE topics
            SET title = $2,
                sub_subject_id = $3,
                thumbnail_url = $4,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, title, teacher_id, sub_subject_id, thumbnail_url,
                      is_published, "order", owner_user_id, ownership_status,
                      created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&new_title)
        .bind(new_sub_subject_id)
        .bind(&new_thumbnail_url)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to update topic: {e}"))?;

        Ok(updated)
    }

    async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(r#"DELETE FROM topics WHERE id = $1"#)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to delete topic: {e}"))?;

        Ok(result.rows_affected() > 0)
    }

    async fn set_publish(&self, id: Uuid, is_published: bool) -> anyhow::Result<bool> {
        let row: Option<(bool,)> = sqlx::query_as(
            r#"
            UPDATE topics
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
        .map_err(|e| anyhow::anyhow!("failed to set topic publish: {e}"))?;

        row.map(|r| r.0)
            .ok_or_else(|| anyhow::anyhow!("topic not found"))
    }

    async fn reorder(&self, id: Uuid, direction: &str) -> anyhow::Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| anyhow::anyhow!("failed to begin transaction: {e}"))?;

        // Lock the current topic row
        let current: (i32,) = sqlx::query_as(
            r#"SELECT "order" FROM topics WHERE id = $1 FOR UPDATE"#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("topic not found"))?;

        let current_order = current.0;

        let neighbor = match direction {
            "up" => {
                // Find the topic with the highest order that is still < current_order
                sqlx::query_as::<_, (Uuid, i32)>(
                    r#"SELECT id, "order" FROM topics
                       WHERE "order" < $1
                       ORDER BY "order" DESC
                       LIMIT 1
                       FOR UPDATE"#,
                )
                .bind(current_order)
                .fetch_optional(&mut *tx)
                .await?
            }
            "down" => {
                // Find the topic with the lowest order that is still > current_order
                sqlx::query_as::<_, (Uuid, i32)>(
                    r#"SELECT id, "order" FROM topics
                       WHERE "order" > $1
                       ORDER BY "order" ASC
                       LIMIT 1
                       FOR UPDATE"#,
                )
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
        sqlx::query(r#"UPDATE topics SET "order" = $2, updated_at = NOW() WHERE id = $1"#)
            .bind(id)
            .bind(neighbor_order)
            .execute(&mut *tx)
            .await?;

        sqlx::query(r#"UPDATE topics SET "order" = $2, updated_at = NOW() WHERE id = $1"#)
            .bind(neighbor_id)
            .bind(current_order)
            .execute(&mut *tx)
            .await?;

        tx.commit()
            .await
            .map_err(|e| anyhow::anyhow!("failed to commit reorder transaction: {e}"))?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> anyhow::Result<Option<TopicWithContents>> {
        let topic_sql = r#"
            SELECT id, title, teacher_id, sub_subject_id, thumbnail_url,
                   is_published, "order", owner_user_id, ownership_status,
                   created_at, updated_at
            FROM topics
            WHERE id = $1
        "#;

        let topic = sqlx::query_as::<_, Topic>(topic_sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch topic: {e}"))?;

        let topic = match topic {
            Some(t) => t,
            None => return Ok(None),
        };

        let contents_sql = r#"
            SELECT id, topic_id, type, title, data, media_url,
                   is_published, "order", created_at, updated_at
            FROM contents
            WHERE topic_id = $1
            ORDER BY "order" ASC, created_at ASC
        "#;

        let contents = sqlx::query_as::<_, Content>(contents_sql)
            .bind(id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch contents: {e}"))?;

        let content_ids: Vec<Uuid> = contents.iter().map(|c| c.id).collect();

        let tasks = if content_ids.is_empty() {
            Vec::new()
        } else {
            let tasks_sql = r#"
                SELECT id, content_id, status, task_type, description, creator_id,
                       suggested_freelancer_id, attachment_url, media_generation_id,
                       created_at, updated_at
                FROM marketplace_tasks
                WHERE content_id = ANY($1)
            "#;

            sqlx::query_as::<_, MarketplaceTask>(tasks_sql)
                .bind(&content_ids)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| anyhow::anyhow!("failed to fetch tasks: {e}"))?
        };

        let contents_with_tasks: Vec<ContentWithTasks> = contents
            .into_iter()
            .map(|content| {
                let content_tasks = tasks
                    .iter()
                    .filter(|t| t.content_id == content.id)
                    .cloned()
                    .collect();
                ContentWithTasks {
                    content,
                    tasks: content_tasks,
                }
            })
            .collect();

        Ok(Some(TopicWithContents {
            topic,
            contents: contents_with_tasks,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_filters_default() {
        let filters = TopicFilters::default();
        assert!(filters.search.is_none());
        assert!(filters.teacher_id.is_none());
        assert!(filters.subject_id.is_none());
        assert!(filters.sub_subject_id.is_none());
        assert!(filters.is_published.is_none());
    }

    #[test]
    fn test_topic_filters_custom() {
        let filters = TopicFilters {
            search: Some("math".to_string()),
            teacher_id: Some("123".to_string()),
            subject_id: Some(1),
            sub_subject_id: Some(2),
            is_published: Some(true),
        };
        assert_eq!(filters.search.as_deref(), Some("math"));
        assert_eq!(filters.teacher_id.as_deref(), Some("123"));
        assert_eq!(filters.subject_id, Some(1));
        assert_eq!(filters.sub_subject_id, Some(2));
        assert_eq!(filters.is_published, Some(true));
    }

    #[test]
    fn test_content_with_tasks_structure() {
        let content = Content {
            id: Uuid::new_v4(),
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

        let task = MarketplaceTask {
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

        let cwt = ContentWithTasks {
            content: content.clone(),
            tasks: vec![task.clone()],
        };

        assert_eq!(cwt.content.id, content.id);
        assert_eq!(cwt.tasks.len(), 1);
        assert_eq!(cwt.tasks[0].id, task.id);
    }

    #[test]
    fn test_topic_with_contents_structure() {
        let topic = Topic {
            id: Uuid::new_v4(),
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

        let twc = TopicWithContents {
            topic: topic.clone(),
            contents: vec![],
        };

        assert_eq!(twc.topic.id, topic.id);
        assert!(twc.contents.is_empty());
    }
}

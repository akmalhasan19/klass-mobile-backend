use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::pagination::PaginationQuery;

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

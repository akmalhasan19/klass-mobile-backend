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

#[async_trait]
pub trait ContentsRepo: Send + Sync {
    async fn find_many(
        &self,
        filters: &ContentFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<Content>, i64)>;
    async fn find_by_id(&self, id: Uuid) -> anyhow::Result<Option<ContentWithRelations>>;
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

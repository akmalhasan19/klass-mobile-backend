use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::pagination::PaginationQuery;
use crate::db::repositories::contents::Content;

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

#[derive(Debug, Clone)]
pub struct GalleryItem {
    pub content: Content,
    pub topic: Option<TopicSummary>,
}

#[derive(Debug, Clone, Default)]
pub struct GalleryFilters {
    pub search: Option<String>,
    pub content_type: Option<String>,
    pub topic_id: Option<Uuid>,
}

#[async_trait]
pub trait GalleryRepo: Send + Sync {
    async fn find_many(
        &self,
        filters: &GalleryFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<GalleryItem>, i64)>;
}

pub struct PgGalleryRepo {
    pool: PgPool,
}

impl PgGalleryRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GalleryRepo for PgGalleryRepo {
    async fn find_many(
        &self,
        filters: &GalleryFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<GalleryItem>, i64)> {
        let search_pattern = filters.search.as_ref().map(|s| format!("%{}%", s));

        let count_sql = r#"
            SELECT COUNT(*)
            FROM contents c
            WHERE c.media_url IS NOT NULL
              AND c.media_url != ''
              AND ($1::text IS NULL OR c.title ILIKE $1)
              AND ($2::text IS NULL OR c.type = $2)
              AND ($3::uuid IS NULL OR c.topic_id = $3)
        "#;

        let total: i64 = sqlx::query_scalar(count_sql)
            .bind(search_pattern.as_deref())
            .bind(filters.content_type.as_deref())
            .bind(filters.topic_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to count gallery items: {e}"))?;

        let data_sql = r#"
            SELECT c.id, c.topic_id, c.type, c.title, c.data, c.media_url,
                   c.is_published, c."order", c.created_at, c.updated_at
            FROM contents c
            WHERE c.media_url IS NOT NULL
              AND c.media_url != ''
              AND ($1::text IS NULL OR c.title ILIKE $1)
              AND ($2::text IS NULL OR c.type = $2)
              AND ($3::uuid IS NULL OR c.topic_id = $3)
            ORDER BY c.created_at DESC
            LIMIT $4 OFFSET $5
        "#;

        let contents = sqlx::query_as::<_, Content>(data_sql)
            .bind(search_pattern.as_deref())
            .bind(filters.content_type.as_deref())
            .bind(filters.topic_id)
            .bind(pagination.limit())
            .bind(pagination.offset())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch gallery items: {e}"))?;

        let topic_ids: Vec<Uuid> = contents.iter().map(|c| c.topic_id).collect();

        let topics = if topic_ids.is_empty() {
            Vec::new()
        } else {
            let topics_sql = r#"
                SELECT id, title, teacher_id, sub_subject_id, thumbnail_url,
                       is_published, "order", owner_user_id, ownership_status,
                       created_at, updated_at
                FROM topics
                WHERE id = ANY($1)
            "#;

            sqlx::query_as::<_, TopicSummary>(topics_sql)
                .bind(&topic_ids)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| anyhow::anyhow!("failed to fetch topics: {e}"))?
        };

        let gallery_items: Vec<GalleryItem> = contents
            .into_iter()
            .map(|content| {
                let topic = topics.iter().find(|t| t.id == content.topic_id).cloned();
                GalleryItem { content, topic }
            })
            .collect();

        Ok((gallery_items, total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gallery_filters_default() {
        let filters = GalleryFilters::default();
        assert!(filters.search.is_none());
        assert!(filters.content_type.is_none());
        assert!(filters.topic_id.is_none());
    }

    #[test]
    fn test_gallery_filters_custom() {
        let topic_id = Uuid::new_v4();
        let filters = GalleryFilters {
            search: Some("math".to_string()),
            content_type: Some("module".to_string()),
            topic_id: Some(topic_id),
        };
        assert_eq!(filters.search.as_deref(), Some("math"));
        assert_eq!(filters.content_type.as_deref(), Some("module"));
        assert_eq!(filters.topic_id, Some(topic_id));
    }

    #[test]
    fn test_gallery_item_structure() {
        let content = Content {
            id: Uuid::new_v4(),
            topic_id: Uuid::new_v4(),
            content_type: "module".to_string(),
            title: Some("Test Content".to_string()),
            data: None,
            media_url: Some("https://example.com/file.pdf".to_string()),
            is_published: true,
            order: 0,
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

        let item = GalleryItem {
            content: content.clone(),
            topic: Some(topic.clone()),
        };

        assert_eq!(item.content.id, content.id);
        assert_eq!(item.topic.as_ref().unwrap().id, topic.id);
    }

    #[test]
    fn test_gallery_item_without_topic() {
        let content = Content {
            id: Uuid::new_v4(),
            topic_id: Uuid::new_v4(),
            content_type: "quiz".to_string(),
            title: Some("Test Quiz".to_string()),
            data: None,
            media_url: Some("https://example.com/quiz.pdf".to_string()),
            is_published: true,
            order: 0,
            created_at: None,
            updated_at: None,
        };

        let item = GalleryItem {
            content: content.clone(),
            topic: None,
        };

        assert_eq!(item.content.id, content.id);
        assert!(item.topic.is_none());
    }
}

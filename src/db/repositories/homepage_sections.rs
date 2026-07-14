use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct HomepageSection {
    pub id: Uuid,
    pub key: String,
    pub label: String,
    pub position: i32,
    pub is_enabled: bool,
    pub data_source: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[async_trait]
pub trait HomepageSectionsRepo: Send + Sync {
    async fn find_enabled_ordered(&self) -> anyhow::Result<Vec<HomepageSection>>;
    async fn find_by_key(&self, key: &str) -> anyhow::Result<Option<HomepageSection>>;
}

pub struct PgHomepageSectionsRepo {
    pool: PgPool,
}

impl PgHomepageSectionsRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl HomepageSectionsRepo for PgHomepageSectionsRepo {
    async fn find_enabled_ordered(&self) -> anyhow::Result<Vec<HomepageSection>> {
        let sql = r#"
            SELECT id, key, label, position, is_enabled, data_source, created_at, updated_at
            FROM homepage_sections
            WHERE is_enabled = true
            ORDER BY position ASC
        "#;

        let sections = sqlx::query_as::<_, HomepageSection>(sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch enabled homepage sections: {e}"))?;

        Ok(sections)
    }

    async fn find_by_key(&self, key: &str) -> anyhow::Result<Option<HomepageSection>> {
        let sql = r#"
            SELECT id, key, label, position, is_enabled, data_source, created_at, updated_at
            FROM homepage_sections
            WHERE key = $1
        "#;

        let section = sqlx::query_as::<_, HomepageSection>(sql)
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch homepage section by key: {e}"))?;

        Ok(section)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_homepage_section_structure() {
        let section = HomepageSection {
            id: Uuid::new_v4(),
            key: "project_recommendations".to_string(),
            label: "Project Recommendations".to_string(),
            position: 1,
            is_enabled: true,
            data_source: Some("api/v1/homepage-recommendations".to_string()),
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
        };

        assert_eq!(section.key, "project_recommendations");
        assert_eq!(section.label, "Project Recommendations");
        assert_eq!(section.position, 1);
        assert!(section.is_enabled);
        assert_eq!(
            section.data_source,
            Some("api/v1/homepage-recommendations".to_string())
        );
    }

    #[test]
    fn test_homepage_section_without_data_source() {
        let section = HomepageSection {
            id: Uuid::new_v4(),
            key: "custom_section".to_string(),
            label: "Custom Section".to_string(),
            position: 5,
            is_enabled: false,
            data_source: None,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
        };

        assert_eq!(section.key, "custom_section");
        assert!(!section.is_enabled);
        assert!(section.data_source.is_none());
    }

    #[test]
    fn test_homepage_section_ordering() {
        let sections = vec![
            HomepageSection {
                id: Uuid::new_v4(),
                key: "section_a".to_string(),
                label: "Section A".to_string(),
                position: 2,
                is_enabled: true,
                data_source: None,
                created_at: chrono::NaiveDateTime::default(),
                updated_at: chrono::NaiveDateTime::default(),
            },
            HomepageSection {
                id: Uuid::new_v4(),
                key: "section_b".to_string(),
                label: "Section B".to_string(),
                position: 1,
                is_enabled: true,
                data_source: None,
                created_at: chrono::NaiveDateTime::default(),
                updated_at: chrono::NaiveDateTime::default(),
            },
        ];

        let mut sorted = sections.clone();
        sorted.sort_by_key(|s| s.position);

        assert_eq!(sorted[0].key, "section_b");
        assert_eq!(sorted[1].key, "section_a");
    }
}

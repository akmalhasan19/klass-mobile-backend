use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::pagination::PaginationQuery;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StudentProgress {
    pub id: Uuid,
    pub student_name: String,
    pub score: Option<i32>,
    pub completion_date: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct StudentProgressFilters {
    pub search: Option<String>,
}

#[async_trait]
pub trait StudentProgressRepo: Send + Sync {
    async fn find_many(
        &self,
        filters: &StudentProgressFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<StudentProgress>, i64)>;
    async fn find_by_id(&self, id: Uuid) -> anyhow::Result<Option<StudentProgress>>;
}

pub struct PgStudentProgressRepo {
    pool: PgPool,
}

impl PgStudentProgressRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StudentProgressRepo for PgStudentProgressRepo {
    async fn find_many(
        &self,
        filters: &StudentProgressFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<StudentProgress>, i64)> {
        let search_pattern = filters.search.as_ref().map(|s| format!("%{}%", s));

        let count_sql = r#"
            SELECT COUNT(*)
            FROM student_progress
            WHERE ($1::text IS NULL OR student_name ILIKE $1)
        "#;

        let total: i64 = sqlx::query_scalar(count_sql)
            .bind(search_pattern.as_deref())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to count student progress: {e}"))?;

        let data_sql = r#"
            SELECT id, student_name, score, completion_date, created_at, updated_at
            FROM student_progress
            WHERE ($1::text IS NULL OR student_name ILIKE $1)
            ORDER BY completion_date DESC
            LIMIT $2 OFFSET $3
        "#;

        let records = sqlx::query_as::<_, StudentProgress>(data_sql)
            .bind(search_pattern.as_deref())
            .bind(pagination.limit())
            .bind(pagination.offset())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch student progress: {e}"))?;

        Ok((records, total))
    }

    async fn find_by_id(&self, id: Uuid) -> anyhow::Result<Option<StudentProgress>> {
        let sql = r#"
            SELECT id, student_name, score, completion_date, created_at, updated_at
            FROM student_progress
            WHERE id = $1
        "#;

        let record = sqlx::query_as::<_, StudentProgress>(sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch student progress: {e}"))?;

        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_student_progress_filters_default() {
        let filters = StudentProgressFilters::default();
        assert!(filters.search.is_none());
    }

    #[test]
    fn test_student_progress_filters_custom() {
        let filters = StudentProgressFilters {
            search: Some("john".to_string()),
        };
        assert_eq!(filters.search.as_deref(), Some("john"));
    }

    #[test]
    fn test_student_progress_structure() {
        let record = StudentProgress {
            id: Uuid::new_v4(),
            student_name: "John Doe".to_string(),
            score: Some(85),
            completion_date: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert_eq!(record.student_name, "John Doe");
        assert_eq!(record.score, Some(85));
        assert!(record.completion_date.is_some());
    }

    #[test]
    fn test_student_progress_nullable_fields() {
        let record = StudentProgress {
            id: Uuid::new_v4(),
            student_name: "Jane Smith".to_string(),
            score: None,
            completion_date: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert_eq!(record.score, None);
        assert!(record.completion_date.is_none());
    }

    #[test]
    fn test_student_progress_score_boundary_values() {
        let scores = vec![Some(0), Some(50), Some(100), None];
        for score in scores {
            let record = StudentProgress {
                id: Uuid::new_v4(),
                student_name: "Test".to_string(),
                score,
                completion_date: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            assert_eq!(record.score, score);
        }
    }
}

use async_trait::async_trait;

use sqlx::PgPool;
use uuid::Uuid;

use crate::db::pagination::PaginationQuery;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StudentProgress {
    pub id: Uuid,
    pub student_name: String,
    pub score: Option<i32>,
    pub completion_date: Option<chrono::NaiveDateTime>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Clone, Default)]
pub struct StudentProgressFilters {
    pub search: Option<String>,
}

/// Payload for creating a new student progress record.
#[derive(Debug, Clone)]
pub struct CreateStudentProgressPayload {
    pub student_name: String,
    pub score: i32,
    pub completion_date: Option<chrono::NaiveDateTime>,
}

/// Payload for partial update of a student progress record.
/// Each nullable field is `Option<Option<T>>` to distinguish between:
/// - `None` — field not provided, leave unchanged
/// - `Some(None)` — explicitly set to NULL
/// - `Some(Some(v))` — set to value `v`
#[derive(Debug, Clone, Default)]
pub struct UpdateStudentProgressPayload {
    pub student_name: Option<String>,
    pub score: Option<i32>,
    pub completion_date: Option<Option<chrono::NaiveDateTime>>,
}

#[async_trait]
pub trait StudentProgressRepo: Send + Sync {
    async fn find_many(
        &self,
        filters: &StudentProgressFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<StudentProgress>, i64)>;
    async fn find_by_id(&self, id: Uuid) -> anyhow::Result<Option<StudentProgress>>;

    /// Insert a new student progress record. Returns the created record.
    async fn insert(&self, payload: &CreateStudentProgressPayload) -> anyhow::Result<StudentProgress>;

    /// Update a student progress record with partial fields. Returns the updated record.
    async fn update(&self, id: Uuid, payload: &UpdateStudentProgressPayload) -> anyhow::Result<StudentProgress>;

    /// Delete a student progress record by ID. Returns `true` if a row was deleted.
    async fn delete(&self, id: Uuid) -> anyhow::Result<bool>;
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

    async fn insert(&self, payload: &CreateStudentProgressPayload) -> anyhow::Result<StudentProgress> {
        let id = Uuid::new_v4();
        let record = sqlx::query_as::<_, StudentProgress>(
            r#"
            INSERT INTO student_progress
                (id, student_name, score, completion_date, created_at, updated_at)
            VALUES ($1, $2, $3, $4, NOW(), NOW())
            RETURNING id, student_name, score, completion_date, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&payload.student_name)
        .bind(payload.score)
        .bind(payload.completion_date)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to insert student progress: {e}"))?;

        Ok(record)
    }

    async fn update(&self, id: Uuid, payload: &UpdateStudentProgressPayload) -> anyhow::Result<StudentProgress> {
        // Fetch current values first to use as fallback
        let current = sqlx::query_as::<_, StudentProgress>(
            r#"SELECT id, student_name, score, completion_date, created_at, updated_at
               FROM student_progress WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("student progress not found"))?;

        let new_student_name = payload
            .student_name
            .clone()
            .unwrap_or_else(|| current.student_name.clone());
        let new_score = payload.score.unwrap_or(current.score.unwrap_or(0));
        let new_completion_date = payload.completion_date.unwrap_or(current.completion_date);

        let updated = sqlx::query_as::<_, StudentProgress>(
            r#"
            UPDATE student_progress
            SET student_name = $2,
                score = $3,
                completion_date = $4,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, student_name, score, completion_date, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&new_student_name)
        .bind(new_score)
        .bind(new_completion_date)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to update student progress: {e}"))?;

        Ok(updated)
    }

    async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(r#"DELETE FROM student_progress WHERE id = $1"#)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to delete student progress: {e}"))?;

        Ok(result.rows_affected() > 0)
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
            completion_date: Some(chrono::Utc::now().naive_utc()),
            created_at: Some(chrono::Utc::now().naive_utc()),
            updated_at: Some(chrono::Utc::now().naive_utc()),
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
            created_at: Some(chrono::Utc::now().naive_utc()),
            updated_at: Some(chrono::Utc::now().naive_utc()),
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
                created_at: Some(chrono::Utc::now().naive_utc()),
                updated_at: Some(chrono::Utc::now().naive_utc()),
            };
            assert_eq!(record.score, score);
        }
    }

    #[test]
    fn test_create_payload_structure() {
        let payload = CreateStudentProgressPayload {
            student_name: "Budi".to_string(),
            score: 85,
            completion_date: Some(chrono::Utc::now().naive_utc()),
        };
        assert_eq!(payload.student_name, "Budi");
        assert_eq!(payload.score, 85);
        assert!(payload.completion_date.is_some());
    }

    #[test]
    fn test_create_payload_null_completion_date() {
        let payload = CreateStudentProgressPayload {
            student_name: "Ani".to_string(),
            score: 90,
            completion_date: None,
        };
        assert!(payload.completion_date.is_none());
    }

    #[test]
    fn test_update_payload_default() {
        let payload = UpdateStudentProgressPayload::default();
        assert!(payload.student_name.is_none());
        assert!(payload.score.is_none());
        assert!(payload.completion_date.is_none());
    }

    #[test]
    fn test_update_payload_partial() {
        let payload = UpdateStudentProgressPayload {
            student_name: Some("Updated Name".to_string()),
            score: None,
            completion_date: Some(None),
        };
        assert_eq!(payload.student_name.as_deref(), Some("Updated Name"));
        assert!(payload.score.is_none());
        assert_eq!(payload.completion_date, Some(None));
    }
}

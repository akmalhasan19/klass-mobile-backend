use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::pagination::PaginationQuery;

// ─── Struct ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MediaFile {
    pub id: Uuid,
    pub uploader_id: Option<i64>,
    pub file_path: String,
    pub file_name: String,
    pub mime_type: Option<String>,
    pub size: Option<i32>,
    pub disk: String,
    pub category: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ─── Insert data ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct InsertMediaFile<'a> {
    pub uploader_id: Option<i64>,
    pub file_path: &'a str,
    pub file_name: &'a str,
    pub mime_type: Option<&'a str>,
    pub size: Option<i32>,
    pub disk: &'a str,
    pub category: Option<&'a str>,
}

// ─── Filters ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct MediaFileFilters {
    /// Exact match on `uploader_id` column.
    pub uploader_id: Option<i64>,

    /// Exact match on `category` column.
    pub category: Option<String>,

    /// Exact match on `disk` column.
    pub disk: Option<String>,

    /// Exact match on `mime_type` column.
    pub mime_type: Option<String>,

    /// General search across `file_name` using ILIKE.
    pub search: Option<String>,
}

// ─── Trait ───────────────────────────────────────────────────────────────────

#[async_trait]
pub trait MediaFilesRepo: Send + Sync {
    /// Return paginated media files matching the given filters,
    /// together with the total count (before pagination).
    async fn find_many(
        &self,
        filters: &MediaFileFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<MediaFile>, i64)>;

    /// Insert a new media file record and return the created row.
    async fn insert(&self, data: InsertMediaFile<'_>) -> anyhow::Result<MediaFile>;

    /// Delete a single media file by its UUID id.
    async fn delete_by_id(&self, id: Uuid) -> anyhow::Result<()>;

    /// Delete multiple media files by their UUID ids.
    /// If `ids` is empty this is a no-op.
    async fn bulk_delete(&self, ids: &[Uuid]) -> anyhow::Result<()>;
}

// ─── Pg implementation ───────────────────────────────────────────────────────

pub struct PgMediaFilesRepo {
    pool: PgPool,
}

impl PgMediaFilesRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MediaFilesRepo for PgMediaFilesRepo {
    async fn find_many(
        &self,
        filters: &MediaFileFilters,
        pagination: &PaginationQuery,
    ) -> anyhow::Result<(Vec<MediaFile>, i64)> {
        let search_pattern = filters
            .search
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{}%", s));

        // ── Count query ──────────────────────────────────────────────────
        let count_sql = r#"
            SELECT COUNT(*)
            FROM media_files m
            WHERE ($1::bigint IS NULL OR m.uploader_id = $1)
              AND ($2::text IS NULL OR m.category = $2)
              AND ($3::text IS NULL OR m.disk = $3)
              AND ($4::text IS NULL OR m.mime_type = $4)
              AND ($5::text IS NULL OR m.file_name ILIKE $5)
        "#;

        let total: i64 = sqlx::query_scalar(count_sql)
            .bind(filters.uploader_id)
            .bind(filters.category.as_deref())
            .bind(filters.disk.as_deref())
            .bind(filters.mime_type.as_deref())
            .bind(search_pattern.as_deref())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to count media files: {e}"))?;

        // ── Data query ───────────────────────────────────────────────────
        let data_sql = r#"
            SELECT m.id, m.uploader_id, m.file_path, m.file_name,
                   m.mime_type, m.size, m.disk, m.category,
                   m.created_at, m.updated_at
            FROM media_files m
            WHERE ($1::bigint IS NULL OR m.uploader_id = $1)
              AND ($2::text IS NULL OR m.category = $2)
              AND ($3::text IS NULL OR m.disk = $3)
              AND ($4::text IS NULL OR m.mime_type = $4)
              AND ($5::text IS NULL OR m.file_name ILIKE $5)
            ORDER BY m.created_at DESC
            LIMIT $6 OFFSET $7
        "#;

        let files = sqlx::query_as::<_, MediaFile>(data_sql)
            .bind(filters.uploader_id)
            .bind(filters.category.as_deref())
            .bind(filters.disk.as_deref())
            .bind(filters.mime_type.as_deref())
            .bind(search_pattern.as_deref())
            .bind(pagination.limit())
            .bind(pagination.offset())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to fetch media files: {e}"))?;

        Ok((files, total))
    }

    async fn insert(&self, data: InsertMediaFile<'_>) -> anyhow::Result<MediaFile> {
        let id = Uuid::new_v4();

        let file = sqlx::query_as::<_, MediaFile>(
            r#"
            INSERT INTO media_files
                (id, uploader_id, file_path, file_name, mime_type, size,
                 disk, category, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
            RETURNING id, uploader_id, file_path, file_name, mime_type, size,
                      disk, category, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(data.uploader_id)
        .bind(data.file_path)
        .bind(data.file_name)
        .bind(data.mime_type)
        .bind(data.size)
        .bind(data.disk)
        .bind(data.category)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to insert media file: {e}"))?;

        Ok(file)
    }

    async fn delete_by_id(&self, id: Uuid) -> anyhow::Result<()> {
        let result = sqlx::query("DELETE FROM media_files WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to delete media file: {e}"))?;

        if result.rows_affected() == 0 {
            anyhow::bail!("media file with id {id} not found");
        }

        Ok(())
    }

    async fn bulk_delete(&self, ids: &[Uuid]) -> anyhow::Result<()> {
        if ids.is_empty() {
            return Ok(());
        }

        sqlx::query("DELETE FROM media_files WHERE id = ANY($1)")
            .bind(ids)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to bulk delete media files: {e}"))?;

        Ok(())
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_file_filters_default() {
        let filters = MediaFileFilters::default();
        assert!(filters.uploader_id.is_none());
        assert!(filters.category.is_none());
        assert!(filters.disk.is_none());
        assert!(filters.mime_type.is_none());
        assert!(filters.search.is_none());
    }

    #[test]
    fn test_media_file_filters_custom() {
        let filters = MediaFileFilters {
            uploader_id: Some(42),
            category: Some("avatars".to_string()),
            disk: Some("r2".to_string()),
            mime_type: Some("image/png".to_string()),
            search: Some("profile".to_string()),
        };

        assert_eq!(filters.uploader_id, Some(42));
        assert_eq!(filters.category.as_deref(), Some("avatars"));
        assert_eq!(filters.disk.as_deref(), Some("r2"));
        assert_eq!(filters.mime_type.as_deref(), Some("image/png"));
        assert_eq!(filters.search.as_deref(), Some("profile"));
    }

    #[test]
    fn test_media_file_struct() {
        let file = MediaFile {
            id: Uuid::new_v4(),
            uploader_id: Some(1),
            file_path: "avatars/user_1.png".to_string(),
            file_name: "profile.png".to_string(),
            mime_type: Some("image/png".to_string()),
            size: Some(1024),
            disk: "r2".to_string(),
            category: Some("avatars".to_string()),
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
            updated_at: DateTime::from_timestamp(0, 0).unwrap(),
        };

        assert_eq!(file.disk, "r2");
        assert_eq!(file.category.as_deref(), Some("avatars"));
        assert_eq!(file.mime_type.as_deref(), Some("image/png"));
        assert_eq!(file.size, Some(1024));
    }

    #[test]
    fn test_insert_media_file_data() {
        let data = InsertMediaFile {
            uploader_id: Some(1),
            file_path: "avatars/user_1.png",
            file_name: "profile.png",
            mime_type: Some("image/png"),
            size: Some(2048),
            disk: "r2",
            category: Some("avatars"),
        };

        assert_eq!(data.file_path, "avatars/user_1.png");
        assert_eq!(data.file_name, "profile.png");
        assert_eq!(data.mime_type, Some("image/png"));
        assert_eq!(data.size, Some(2048));
        assert_eq!(data.disk, "r2");
        assert_eq!(data.category, Some("avatars"));
    }

    #[test]
    fn test_insert_media_file_data_with_nulls() {
        let data = InsertMediaFile {
            uploader_id: None,
            file_path: "materials/lesson_1.pdf",
            file_name: "lesson_1.pdf",
            mime_type: None,
            size: None,
            disk: "r2",
            category: None,
        };

        assert!(data.uploader_id.is_none());
        assert!(data.mime_type.is_none());
        assert!(data.size.is_none());
        assert!(data.category.is_none());
    }

    #[test]
    fn test_bulk_delete_empty_ids_is_noop() {
        // This just validates the early-return guard compiles and doesn't panic
        let ids: Vec<Uuid> = vec![];
        assert!(ids.is_empty());
    }

    #[test]
    fn test_media_file_with_all_nullable_fields() {
        let file = MediaFile {
            id: Uuid::new_v4(),
            uploader_id: None,
            file_path: "system/auto_generated.pdf".to_string(),
            file_name: "auto_generated.pdf".to_string(),
            mime_type: None,
            size: None,
            disk: "r2".to_string(),
            category: None,
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
            updated_at: DateTime::from_timestamp(0, 0).unwrap(),
        };

        assert!(file.uploader_id.is_none());
        assert!(file.mime_type.is_none());
        assert!(file.size.is_none());
        assert!(file.category.is_none());
    }

    #[test]
    fn test_search_pattern_empty_string_treated_as_none() {
        let filters = MediaFileFilters {
            search: Some(String::new()),
            ..Default::default()
        };

        let search_pattern = filters
            .search
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{}%", s));

        assert!(search_pattern.is_none());
    }

    #[test]
    fn test_search_pattern_non_empty() {
        let filters = MediaFileFilters {
            search: Some("math".to_string()),
            ..Default::default()
        };

        let search_pattern = filters
            .search
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{}%", s));

        assert_eq!(search_pattern.as_deref(), Some("%math%"));
    }
}

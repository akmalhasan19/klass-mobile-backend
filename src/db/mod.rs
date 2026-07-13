// Database layer
// - sqlx::PgPool connection
// - Repository traits and implementations per entity
// - 15 Rust structs with #[derive(FromRow)]
// - Migration runner (sqlx migrate)

use sqlx::PgPool;

pub async fn health_check(pool: &PgPool) -> Result<bool, sqlx::Error> {
    let row: (bool,) = sqlx::query_as("SELECT true")
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

pub async fn get_user_by_id(pool: &PgPool, id: i64) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as!(
        UserRow,
        r#"SELECT
            id,
            name,
            email,
            avatar_url,
            role,
            primary_subject_id,
            created_at as "created_at!: chrono::DateTime<chrono::Utc>",
            updated_at as "updated_at!: chrono::DateTime<chrono::Utc>"
        FROM users WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub avatar_url: Option<String>,
    pub role: String,
    pub primary_subject_id: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

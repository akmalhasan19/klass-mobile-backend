pub mod pagination;
pub mod repositories;
pub mod seed;

use sqlx::PgPool;

pub async fn health_check(pool: &PgPool) -> Result<bool, sqlx::Error> {
    let row: (bool,) = sqlx::query_as("SELECT true").fetch_one(pool).await?;
    Ok(row.0)
}

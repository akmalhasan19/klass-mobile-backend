use async_trait::async_trait;
use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PersonalAccessToken {
    pub id: i64,
    pub tokenable_type: String,
    pub tokenable_id: i64,
    pub name: String,
    pub token: String,
    pub abilities: Option<String>,
    pub last_used_at: Option<chrono::NaiveDateTime>,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug)]
pub struct InsertToken<'a> {
    pub tokenable_type: &'a str,
    pub tokenable_id: i64,
    pub name: &'a str,
    pub token: &'a str,
    pub abilities: Option<&'a str>,
}

#[async_trait]
pub trait PersonalAccessTokensRepo: Send + Sync {
    async fn find_by_id(&self, id: i64) -> anyhow::Result<Option<PersonalAccessToken>>;
    async fn find_by_token_hash(&self, token_hash: &str) -> anyhow::Result<Option<PersonalAccessToken>>;
    async fn insert(&self, data: InsertToken<'_>) -> anyhow::Result<PersonalAccessToken>;
    async fn delete_by_id(&self, id: i64) -> anyhow::Result<()>;
    async fn delete_by_tokenable(&self, tokenable_type: &str, tokenable_id: i64) -> anyhow::Result<()>;
    async fn update_last_used_at(&self, id: i64) -> anyhow::Result<()>;
}

pub struct PgPersonalAccessTokensRepo {
    pool: PgPool,
}

impl PgPersonalAccessTokensRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

const SELECT_COLUMNS: &str = r#"id, tokenable_type, tokenable_id, name, token,
                                 abilities, last_used_at, expires_at, created_at, updated_at"#;

#[async_trait]
impl PersonalAccessTokensRepo for PgPersonalAccessTokensRepo {
    async fn find_by_id(&self, id: i64) -> anyhow::Result<Option<PersonalAccessToken>> {
        let row = sqlx::query_as::<_, PersonalAccessToken>(&format!(
            "SELECT {SELECT_COLUMNS} FROM personal_access_tokens WHERE id = $1"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to find token by id: {e}"))?;

        Ok(row)
    }

    async fn find_by_token_hash(&self, token_hash: &str) -> anyhow::Result<Option<PersonalAccessToken>> {
        let row = sqlx::query_as::<_, PersonalAccessToken>(&format!(
            "SELECT {SELECT_COLUMNS} FROM personal_access_tokens WHERE token = $1"
        ))
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to find token by hash: {e}"))?;

        Ok(row)
    }

    async fn insert(&self, data: InsertToken<'_>) -> anyhow::Result<PersonalAccessToken> {
        let row = sqlx::query_as::<_, PersonalAccessToken>(&format!(
            r#"INSERT INTO personal_access_tokens
                   (tokenable_type, tokenable_id, name, token, abilities, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               RETURNING {SELECT_COLUMNS}"#
        ))
        .bind(data.tokenable_type)
        .bind(data.tokenable_id)
        .bind(data.name)
        .bind(data.token)
        .bind(data.abilities)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to insert token: {e}"))?;

        Ok(row)
    }

    async fn delete_by_id(&self, id: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM personal_access_tokens WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to delete token: {e}"))?;

        Ok(())
    }

    async fn delete_by_tokenable(&self, tokenable_type: &str, tokenable_id: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM personal_access_tokens WHERE tokenable_type = $1 AND tokenable_id = $2")
            .bind(tokenable_type)
            .bind(tokenable_id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to delete tokens for tokenable: {e}"))?;

        Ok(())
    }

    async fn update_last_used_at(&self, id: i64) -> anyhow::Result<()> {
        sqlx::query("UPDATE personal_access_tokens SET last_used_at = NOW(), updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to update last_used_at: {e}"))?;

        Ok(())
    }
}

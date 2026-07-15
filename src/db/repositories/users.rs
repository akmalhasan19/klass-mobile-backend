use async_trait::async_trait;
use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub email_verified_at: Option<chrono::NaiveDateTime>,
    pub password: String,
    pub avatar_url: Option<String>,
    pub primary_subject_id: Option<i64>,
    pub role: String,
    pub remember_token: Option<String>,
    pub security_question: Option<String>,
    pub security_answer: Option<String>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug)]
pub struct InsertUser<'a> {
    pub name: &'a str,
    pub email: &'a str,
    pub password: &'a str,
    pub role: &'a str,
}

#[async_trait]
pub trait UsersRepo: Send + Sync {
    async fn find_by_id(&self, id: i64) -> anyhow::Result<Option<User>>;
    async fn find_by_email(&self, email: &str) -> anyhow::Result<Option<User>>;
    async fn insert(&self, data: InsertUser<'_>) -> anyhow::Result<User>;
    async fn update_avatar(&self, id: i64, avatar_url: &str) -> anyhow::Result<()>;
    async fn update_password(&self, id: i64, password_hash: &str) -> anyhow::Result<()>;
    async fn set_role(&self, id: i64, role: &str) -> anyhow::Result<()>;
    async fn set_security_qa(
        &self,
        id: i64,
        question: &str,
        answer_hash: &str,
    ) -> anyhow::Result<()>;
}

pub struct PgUsersRepo {
    pool: PgPool,
}

impl PgUsersRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UsersRepo for PgUsersRepo {
    async fn find_by_id(&self, id: i64) -> anyhow::Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"SELECT id, name, email, email_verified_at, password, avatar_url,
                      primary_subject_id, role, remember_token,
                      security_question, security_answer, created_at, updated_at
               FROM users WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to find user by id: {e}"))?;

        Ok(user)
    }

    async fn find_by_email(&self, email: &str) -> anyhow::Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"SELECT id, name, email, email_verified_at, password, avatar_url,
                      primary_subject_id, role, remember_token,
                      security_question, security_answer, created_at, updated_at
               FROM users WHERE email = $1"#,
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to find user by email: {e}"))?;

        Ok(user)
    }

    async fn insert(&self, data: InsertUser<'_>) -> anyhow::Result<User> {
        let user = sqlx::query_as::<_, User>(
            r#"INSERT INTO users (name, email, password, role, created_at, updated_at)
               VALUES ($1, $2, $3, $4, NOW(), NOW())
               RETURNING id, name, email, email_verified_at, password, avatar_url,
                         primary_subject_id, role, remember_token,
                         security_question, security_answer, created_at, updated_at"#,
        )
        .bind(data.name)
        .bind(data.email)
        .bind(data.password)
        .bind(data.role)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to insert user: {e}"))?;

        Ok(user)
    }

    async fn update_avatar(&self, id: i64, avatar_url: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET avatar_url = $1, updated_at = NOW() WHERE id = $2")
            .bind(avatar_url)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to update avatar: {e}"))?;

        Ok(())
    }

    async fn update_password(&self, id: i64, password_hash: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET password = $1, updated_at = NOW() WHERE id = $2")
            .bind(password_hash)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to update password: {e}"))?;

        Ok(())
    }

    async fn set_role(&self, id: i64, role: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET role = $1, updated_at = NOW() WHERE id = $2")
            .bind(role)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to set role: {e}"))?;

        Ok(())
    }

    async fn set_security_qa(
        &self,
        id: i64,
        question: &str,
        answer_hash: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE users SET security_question = $1, security_answer = $2, updated_at = NOW() WHERE id = $3",
        )
        .bind(question)
        .bind(answer_hash)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to set security qa: {e}"))?;

        Ok(())
    }
}

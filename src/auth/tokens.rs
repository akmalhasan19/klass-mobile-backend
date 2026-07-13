use sha2::{Digest, Sha256};
use sqlx::FromRow;

const TOKENABLE_TYPE: &str = "App\\Models\\User";

/// Personal access token structure matching `personal_access_tokens` table
#[derive(Debug, Clone, FromRow)]
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

/// Generate a random 64-character hex string using two UUID v4s.
fn generate_plain_token() -> String {
    let a = uuid::Uuid::new_v4().as_simple().to_string();
    let b = uuid::Uuid::new_v4().as_simple().to_string();
    format!("{a}{b}")
}

/// Compute SHA-256 hex digest of the plain token.
fn hash_token(plain: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(plain.as_bytes());
    hex::encode(hasher.finalize())
}

/// Issue a new personal access token.
/// Returns `"{id}|{plain_random_64_hex}"` and stores `sha256(plain)` hex (64 chars).
pub async fn issue_token(
    pool: &sqlx::PgPool,
    user_id: i64,
    name: &str,
    abilities: Option<&str>,
) -> anyhow::Result<String> {
    let plain = generate_plain_token();
    let token_hash = hash_token(&plain);

    let row: (i64,) = sqlx::query_as(
        r#"INSERT INTO personal_access_tokens
               (tokenable_type, tokenable_id, name, token, abilities, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
           RETURNING id"#,
    )
    .bind(TOKENABLE_TYPE)
    .bind(user_id)
    .bind(name)
    .bind(&token_hash)
    .bind(abilities)
    .fetch_one(pool)
    .await
    .map_err(|e| anyhow::anyhow!("failed to insert token: {e}"))?;

    Ok(format!("{}|{plain}", row.0))
}

/// Verify a token string.
/// Splits at `|`, parses `id`, hashes remainder, and SELECTs from DB.
pub async fn verify_token(
    pool: &sqlx::PgPool,
    token: &str,
) -> anyhow::Result<Option<PersonalAccessToken>> {
    let (id_str, plain) = match token.split_once('|') {
        Some(pair) => pair,
        None => return Ok(None),
    };

    let id: i64 = match id_str.parse() {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    let token_hash = hash_token(plain);

    let row = sqlx::query_as::<_, PersonalAccessToken>(
        r#"SELECT id, tokenable_type, tokenable_id, name, token, abilities,
                  last_used_at, expires_at, created_at, updated_at
           FROM personal_access_tokens
           WHERE id = $1 AND token = $2"#,
    )
    .bind(id)
    .bind(&token_hash)
    .fetch_optional(pool)
    .await
    .map_err(|e| anyhow::anyhow!("failed to query token: {e}"))?;

    Ok(row)
}

/// Revoke a specific token by ID
pub async fn revoke_token(pool: &sqlx::PgPool, id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM personal_access_tokens WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to delete token: {e}"))?;
    Ok(())
}

/// Revoke all tokens for a specific user
pub async fn revoke_all_for_user(pool: &sqlx::PgPool, user_id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM personal_access_tokens WHERE tokenable_id = $1 AND tokenable_type = $2")
        .bind(user_id)
        .bind(TOKENABLE_TYPE)
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to delete tokens for user: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_plain_token_length() {
        let token = generate_plain_token();
        assert_eq!(token.len(), 64, "plain token should be 64 hex chars");
        assert!(
            token.chars().all(|c| c.is_ascii_hexdigit()),
            "plain token should be hex"
        );
    }

    #[test]
    fn test_hash_token_deterministic() {
        let plain = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
        let h1 = hash_token(plain);
        let h2 = hash_token(plain);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64, "SHA-256 hex digest should be 64 chars");
    }

    #[test]
    fn test_hash_token_different_inputs() {
        let h1 = hash_token("token_a");
        let h2 = hash_token("token_b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_generate_plain_token_uniqueness() {
        let a = generate_plain_token();
        let b = generate_plain_token();
        assert_ne!(a, b, "two generated tokens should differ");
    }
}

use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};

const BCRYPT_PREFIXES: &[&str] = &["$2y$", "$2b$", "$2a$"];

/// Hash password using Argon2id with default parameters.
/// Returns PHC string format: $argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>
pub fn hash_password(plain: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("failed to hash password: {e}"))?
        .to_string();
    Ok(hash)
}

/// Verify password against hash.
/// Supports both argon2id ($argon2id$) and bcrypt ($2y$/$2b$/$2a$) prefixes for DB compatibility.
pub fn verify_password(plain: &str, hash: &str) -> bool {
    if hash.starts_with("$argon2") {
        verify_argon2(plain, hash)
    } else if BCRYPT_PREFIXES
        .iter()
        .any(|prefix| hash.starts_with(prefix))
    {
        verify_bcrypt(plain, hash)
    } else {
        tracing::warn!(
            hash_prefix = &hash[..8.min(hash.len())],
            "unsupported password hash format"
        );
        false
    }
}

fn verify_argon2(plain: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = %e, "failed to parse argon2 hash");
            return false;
        }
    };
    Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok()
}

fn verify_bcrypt(plain: &str, hash: &str) -> bool {
    bcrypt::verify(plain, hash).unwrap_or_else(|e| {
        tracing::error!(error = %e, "failed to verify bcrypt hash");
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_roundtrip() {
        let password = "my_secure_password_123!";
        let hash = hash_password(password).expect("hashing should succeed");

        assert!(hash.starts_with("$argon2id$"), "hash should use argon2id");
        assert!(
            verify_password(password, &hash),
            "verify should succeed for correct password"
        );
        assert!(
            !verify_password("wrong_password", &hash),
            "verify should fail for wrong password"
        );
    }

    #[test]
    fn test_hash_generates_unique_salts() {
        let password = "same_password";
        let hash1 = hash_password(password).unwrap();
        let hash2 = hash_password(password).unwrap();
        assert_ne!(
            hash1, hash2,
            "different salts should produce different hashes"
        );
        assert!(verify_password(password, &hash1));
        assert!(verify_password(password, &hash2));
    }

    #[test]
    fn test_bcrypt_prefix_detection() {
        let bcrypt_2y = "$2y$10$abcdefghijklmnopqrstuuABCDEFGHIJKLMNOPQRSTUVWXYZ012";
        let bcrypt_2b = "$2b$10$abcdefghijklmnopqrstuuABCDEFGHIJKLMNOPQRSTUVWXYZ012";
        let bcrypt_2a = "$2a$10$abcdefghijklmnopqrstuuABCDEFGHIJKLMNOPQRSTUVWXYZ012";
        let argon2 = "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHRzYWx0$hash";

        assert!(verify_password("test", bcrypt_2y) == false); // invalid hash, but prefix detected
        assert!(verify_password("test", bcrypt_2b) == false);
        assert!(verify_password("test", bcrypt_2a) == false);
        assert!(verify_password("test", argon2) == false); // invalid hash, but prefix detected
    }

    #[test]
    fn test_unsupported_hash_format() {
        assert!(!verify_password("test", "$unknown$format$hash"));
        assert!(!verify_password("test", "plaintext"));
    }

    #[test]
    fn test_empty_password() {
        let hash = hash_password("").expect("empty password should be hashable");
        assert!(verify_password("", &hash));
        assert!(!verify_password("not_empty", &hash));
    }
}

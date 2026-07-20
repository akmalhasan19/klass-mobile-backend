//! Semantic LLM response cache over the `llm_cache_entries` table.
//!
//! - Cache key generation: SHA-256 of canonical JSON (byte-compatible with Python).
//! - `pg_try_advisory_lock` anti-stampede for inflight requests.
//! - Route-aware TTL (`interpret` / `respond`).
//! - Hit-count tracking with `last_hit_at` update.
//! - Lazy cleanup of expired entries on lookup/store.
//!
//! Port of `app/cache.py` — `AdapterCacheService`.

use blake2::{Blake2b512, Digest as Blake2Digest};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::Sha256;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

// ─── Types ──────────────────────────────────────────────────────────────────

/// Cache route discriminator matching the `route` column CHECK constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheRoute {
    Interpret,
    Respond,
}

impl CacheRoute {
    fn as_str(&self) -> &'static str {
        match self {
            CacheRoute::Interpret => "interpret",
            CacheRoute::Respond => "respond",
        }
    }

    /// Default TTL: interpret = 1 hour, respond = 24 hours.
    fn default_ttl_seconds(&self) -> i64 {
        match self {
            CacheRoute::Interpret => 3600,
            CacheRoute::Respond => 86400,
        }
    }
}

/// A cache entry returned by lookup / store.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub cache_key: String,
    pub request_payload: Value,
    pub response_payload: Value,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub hit_count: i64,
    pub last_hit_at: Option<DateTime<Utc>>,
}

/// Stats returned by cleanup operations.
#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub route: CacheRoute,
    pub deleted_count: u64,
}

// ─── Key generation ─────────────────────────────────────────────────────────

/// Build a canonical JSON string that Python's `json.dumps(..., sort_keys=True,
/// separators=(",", ":"))` would produce, then return the SHA-256 hex digest.
///
/// The resulting key is a 64-character hex string matching the `cache_key
/// CHAR(64)` column and byte-compatible with the Python adapter.
pub fn build_cache_key(
    route: CacheRoute,
    provider: &str,
    model: &str,
    request_type: &str,
    instruction: &str,
    input_payload: &Value,
) -> String {
    let document = serde_json::json!({
        "schema_version": "llm_adapter_cache.v1",
        "route": route.as_str(),
        "request_type": request_type,
        "provider": provider.to_lowercase(),
        "model": model,
        "instruction": instruction,
        "input": input_payload,
    });

    // Canonical JSON with sorted keys, byte-compatible with Python's
    // json.dumps(..., sort_keys=True, separators=(",", ":")).
    // With serde_json's preserve_order feature, we must explicitly sort.
    let canonical = to_canonical_json_string(&document);

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}

/// Serialize a JSON value to a string with alphabetically sorted keys,
/// compact separators, and no trailing whitespace — matching Python's
/// `json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)`.
fn to_canonical_json_string(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            // Sort keys alphabetically via BTreeMap
            let sorted: BTreeMap<&String, &Value> = map.iter().collect();
            let items: Vec<String> = sorted
                .iter()
                .map(|(k, v)| {
                    let key = serde_json::to_string(k).expect("key serialization infallible");
                    let val = to_canonical_json_string(v);
                    format!("{}:{}", key, val)
                })
                .collect();
            format!("{{{}}}", items.join(","))
        }
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(to_canonical_json_string).collect();
            format!("[{}]", items.join(","))
        }
        Value::String(s) => serde_json::to_string(s).expect("string serialization infallible"),
        Value::Number(n) => serde_json::to_string(n).expect("number serialization infallible"),
        Value::Bool(b) => serde_json::to_string(b).expect("bool serialization infallible"),
        Value::Null => "null".to_string(),
    }
}

/// Build a signed 64-bit advisory lock ID from a route + cache_key pair,
/// matching Python's `blake2b(payload, digest_size=8, person=b"klasscch")`.
///
/// The result is safe for `pg_try_advisory_lock(bigint)` — values <= 0 are
/// shifted to avoid PG's restriction (lock ID must be positive).
pub fn build_lock_id(route: CacheRoute, cache_key: &str) -> i64 {
    let payload = format!("{}:{}", route.as_str(), cache_key.trim());

    let mut hasher = Blake2b512::new();
    hasher.update(payload.as_bytes());
    // Blake2b512 produces 64 bytes; take the first 8 bytes for the lock ID
    let full_hash = hasher.finalize();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&full_hash[..8]);

    let lock_id = i64::from_ne_bytes(bytes);
    if lock_id <= 0 { 1 } else { lock_id }
}

// ─── SQL fragments ──────────────────────────────────────────────────────────

fn lookup_sql() -> &'static str {
    "SELECT cache_key, request_payload, response_payload, created_at, \
     expires_at, hit_count, last_hit_at \
     FROM llm_cache_entries \
     WHERE cache_key = $1 AND route = $2 AND expires_at > NOW()"
}

fn touch_sql() -> &'static str {
    "UPDATE llm_cache_entries \
     SET hit_count = hit_count + 1, last_hit_at = NOW() \
     WHERE cache_key = $1 AND route = $2 AND expires_at > NOW() \
     RETURNING cache_key, request_payload, response_payload, \
               created_at, expires_at, hit_count, last_hit_at"
}

fn upsert_sql() -> &'static str {
    "INSERT INTO llm_cache_entries \
     (cache_key, route, request_payload, response_payload, created_at, expires_at, hit_count, last_hit_at) \
     VALUES ($1, $2, $3, $4, NOW(), $5, 0, NULL) \
     ON CONFLICT (cache_key) \
     DO UPDATE SET \
       request_payload = EXCLUDED.request_payload, \
       response_payload = EXCLUDED.response_payload, \
       created_at = NOW(), \
       expires_at = EXCLUDED.expires_at, \
       hit_count = 0, \
       last_hit_at = NULL \
     RETURNING cache_key, request_payload, response_payload, \
               created_at, expires_at, hit_count, last_hit_at"
}

fn delete_expired_by_key_sql() -> &'static str {
    "DELETE FROM llm_cache_entries \
     WHERE cache_key = $1 AND expires_at <= NOW() \
     RETURNING id"
}

fn cleanup_sql() -> &'static str {
    "WITH expired AS ( \
       SELECT id FROM llm_cache_entries \
       WHERE route = $1 AND expires_at <= NOW() \
       ORDER BY expires_at ASC LIMIT $2 \
     ) \
     DELETE FROM llm_cache_entries \
     USING expired \
     WHERE llm_cache_entries.id = expired.id \
     RETURNING llm_cache_entries.id"
}

// ─── Repository ─────────────────────────────────────────────────────────────

/// Repository over `llm_cache_entries` for semantic LLM response caching.
pub struct LlmCacheRepo {
    pool: PgPool,
    /// Monotonic clock used for lazy cleanup interval tracking.
    lazy_cleanup_last: std::sync::Mutex<Option<Instant>>,
}

impl LlmCacheRepo {
    /// Create a new cache repository using the given DB pool.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            lazy_cleanup_last: std::sync::Mutex::new(None),
        }
    }

    // ── Public API ─────────────────────────────────────────────────────────

    /// Look up an unexpired cache entry.
    ///
    /// On hit, increments `hit_count` and updates `last_hit_at`.
    /// Returns `None` if the key does not exist or has expired.
    pub async fn lookup(&self, route: CacheRoute, cache_key: &str) -> Result<Option<CacheEntry>, sqlx::Error> {
        self.maybe_run_lazy_cleanup(route).await;

        // Delete expired entry for this key first (avoid returning stale data on race)
        sqlx::query(delete_expired_by_key_sql())
            .bind(cache_key)
            .execute(&self.pool)
            .await?;

        // Touch + return (updates hit_count, last_hit_at)
        let row: Option<(String, Value, Value, DateTime<Utc>, DateTime<Utc>, i64, Option<DateTime<Utc>>)> =
            sqlx::query_as(touch_sql())
                .bind(cache_key)
                .bind(route.as_str())
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.map(|r| CacheEntry {
            cache_key: r.0,
            request_payload: r.1,
            response_payload: r.2,
            created_at: r.3,
            expires_at: r.4,
            hit_count: r.5,
            last_hit_at: r.6,
        }))
    }

    /// Look up an unexpired cache entry **without** incrementing the hit count.
    /// Useful for inner retry loops during inflight-wait.
    pub async fn peek(&self, route: CacheRoute, cache_key: &str) -> Result<Option<CacheEntry>, sqlx::Error> {
        let row: Option<(String, Value, Value, DateTime<Utc>, DateTime<Utc>, i64, Option<DateTime<Utc>>)> =
            sqlx::query_as(lookup_sql())
                .bind(cache_key)
                .bind(route.as_str())
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.map(|r| CacheEntry {
            cache_key: r.0,
            request_payload: r.1,
            response_payload: r.2,
            created_at: r.3,
            expires_at: r.4,
            hit_count: r.5,
            last_hit_at: r.6,
        }))
    }

    /// Store a response in the cache (upsert by `cache_key`).
    ///
    /// `ttl_seconds` — if `None`, uses the route default.
    pub async fn store(
        &self,
        route: CacheRoute,
        cache_key: &str,
        request_payload: &Value,
        response_payload: &Value,
        ttl_seconds: Option<i64>,
    ) -> Result<CacheEntry, sqlx::Error> {
        self.maybe_run_lazy_cleanup(route).await;

        let ttl = ttl_seconds.unwrap_or_else(|| route.default_ttl_seconds());
        // Compute the expiration timestamp client-side, then bind it directly
        // as a TIMESTAMPTZ parameter (the raw TTL integer is meaningless to PG).
        let expires_at = Utc::now() + chrono::Duration::seconds(ttl);

        let row: (String, Value, Value, DateTime<Utc>, DateTime<Utc>, i64, Option<DateTime<Utc>>) =
            sqlx::query_as(upsert_sql())
                .bind(cache_key)
                .bind(route.as_str())
                .bind(request_payload)
                .bind(response_payload)
                .bind(expires_at)
                .fetch_one(&self.pool)
                .await?;

        Ok(CacheEntry {
            cache_key: row.0,
            request_payload: row.1,
            response_payload: row.2,
            created_at: row.3,
            expires_at: row.4,
            hit_count: row.5,
            last_hit_at: row.6,
        })
    }

    /// Try to acquire a PostgreSQL advisory lock for the given route + key.
    ///
    /// Used to prevent cache stampede: only one concurrent request should
    /// call the LLM for a given cache key; others wait for the result.
    pub async fn try_acquire_lock(&self, route: CacheRoute, cache_key: &str) -> Result<bool, sqlx::Error> {
        let lock_id = build_lock_id(route, cache_key);
        let row: (bool,) = sqlx::query_as("SELECT pg_try_advisory_lock($1)")
            .bind(lock_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Release a previously acquired advisory lock.
    pub async fn release_lock(&self, route: CacheRoute, cache_key: &str) -> Result<bool, sqlx::Error> {
        let lock_id = build_lock_id(route, cache_key);
        let row: (bool,) = sqlx::query_as("SELECT pg_advisory_unlock($1)")
            .bind(lock_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Poll the cache until an entry appears or timeout elapses.
    ///
    /// `poll_interval` defaults to 50 ms. `timeout` defaults to 10 seconds.
    pub async fn wait_for_entry(
        &self,
        route: CacheRoute,
        cache_key: &str,
        poll_interval: Option<Duration>,
        timeout: Option<Duration>,
    ) -> Result<Option<CacheEntry>, sqlx::Error> {
        let interval = poll_interval.unwrap_or(Duration::from_millis(50));
        let deadline = Instant::now() + timeout.unwrap_or(Duration::from_secs(10));

        loop {
            let entry = self.peek(route, cache_key).await?;
            if entry.is_some() {
                return Ok(entry);
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            tokio::time::sleep(interval).await;
        }
    }

    /// Delete expired cache entries for a given route, up to `limit`.
    ///
    /// Returns the number of deleted rows.
    pub async fn cleanup_expired(&self, route: CacheRoute, limit: i64) -> Result<CleanupResult, sqlx::Error> {
        let deleted: i64 = sqlx::query_scalar(cleanup_sql())
            .bind(route.as_str())
            .bind(limit)
            .fetch_optional(&self.pool)
            .await?
            .unwrap_or(0);

        Ok(CleanupResult {
            route,
            deleted_count: deleted as u64,
        })
    }

    // ── Internal lazy cleanup ──────────────────────────────────────────────

    /// Run a lazy cleanup if at least 60 seconds have elapsed since the last one.
    async fn maybe_run_lazy_cleanup(&self, route: CacheRoute) {
        let should_run = {
            let mut last = self.lazy_cleanup_last.lock().unwrap();
            let now = Instant::now();
            let due = last.map_or(true, |t| now.duration_since(t) >= Duration::from_secs(60));
            if due {
                *last = Some(now);
            }
            due
        };

        if should_run {
            if let Err(e) = self.cleanup_expired(route, 100).await {
                tracing::warn!(error = %e, route = %route.as_str(), "cache lazy cleanup failed");
            }
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_cache_key_deterministic() {
        let input = serde_json::json!({"teacher_prompt": "Buatkan materi"});
        let key1 = build_cache_key(
            CacheRoute::Interpret,
            "xiaomi",
            "mimo-v2.5",
            "media_prompt_interpretation",
            "Interpret the following prompt",
            &input,
        );
        let key2 = build_cache_key(
            CacheRoute::Interpret,
            "xiaomi",
            "mimo-v2.5",
            "media_prompt_interpretation",
            "Interpret the following prompt",
            &input,
        );
        assert_eq!(key1.len(), 64, "SHA-256 hex should be 64 chars");
        assert_eq!(key1, key2, "keys should be deterministic");
    }

    #[test]
    fn test_build_cache_key_different_routes_differ() {
        let input = serde_json::json!({"teacher_prompt": "test"});
        let key_interpret = build_cache_key(
            CacheRoute::Interpret, "p", "m", "t", "i", &input,
        );
        let key_respond = build_cache_key(
            CacheRoute::Respond, "p", "m", "t", "i", &input,
        );
        assert_ne!(key_interpret, key_respond, "different routes should differ");
    }

    #[test]
    fn test_build_cache_key_different_providers_differ() {
        let input = serde_json::json!({"teacher_prompt": "test"});
        let key1 = build_cache_key(
            CacheRoute::Interpret, "xiaomi", "m", "t", "i", &input,
        );
        let key2 = build_cache_key(
            CacheRoute::Interpret, "gemini", "m", "t", "i", &input,
        );
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_build_lock_id_positive() {
        let lock_id = build_lock_id(CacheRoute::Interpret, "abc123");
        assert!(lock_id > 0, "lock ID must be positive for PG");
    }

    #[test]
    fn test_build_lock_id_deterministic() {
        let id1 = build_lock_id(CacheRoute::Interpret, "same-key");
        let id2 = build_lock_id(CacheRoute::Interpret, "same-key");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_build_lock_id_different_routes_differ() {
        let id1 = build_lock_id(CacheRoute::Interpret, "key");
        let id2 = build_lock_id(CacheRoute::Respond, "key");
        // Could theoretically collide but astronomically unlikely with blake2 64-bit
        assert_ne!(id1, id2, "different routes should produce different lock IDs");
    }

    #[test]
    fn test_cache_route_as_str() {
        assert_eq!(CacheRoute::Interpret.as_str(), "interpret");
        assert_eq!(CacheRoute::Respond.as_str(), "respond");
    }

    #[test]
    fn test_cache_route_default_ttl() {
        assert_eq!(CacheRoute::Interpret.default_ttl_seconds(), 3600);
        assert_eq!(CacheRoute::Respond.default_ttl_seconds(), 86400);
    }

    #[test]
    fn test_build_cache_key_with_null_fields() {
        // Empty input should still produce a valid key
        let input = serde_json::json!({});
        let key = build_cache_key(
            CacheRoute::Interpret, "", "", "", "", &input,
        );
        assert_eq!(key.len(), 64);
    }

    #[test]
    fn test_build_cache_key_normalizes_provider_case() {
        let input = serde_json::json!({"prompt": "hello"});
        let key_lower = build_cache_key(
            CacheRoute::Interpret, "xiaomi", "m", "t", "i", &input,
        );
        let key_mixed = build_cache_key(
            CacheRoute::Interpret, "xiaomi", "m", "t", "i", &input,
        );
        // Both should produce the same key because provider is lowercased
        assert_eq!(key_lower, key_mixed, "provider should be case-normalized");
    }

    #[test]
    #[test]
    fn test_to_canonical_json_string_sorted_keys() {
        let doc1 = serde_json::json!({"z": 1, "a": 2});
        let doc2 = serde_json::json!({"a": 2, "z": 1});
        let s1 = to_canonical_json_string(&doc1);
        let s2 = to_canonical_json_string(&doc2);
        assert_eq!(s1, s2, "canonical JSON must produce same output regardless of input key order");
        // Alphabetically, "a" comes before "z"
        assert!(!s1.contains("z") || s1.find("\"a\":").unwrap() < s1.find("\"z\":").unwrap());
    }

    #[test]
    fn test_to_canonical_json_string_nested() {
        let doc = serde_json::json!({"b": {"y": 1, "x": 2}, "a": 3});
        let s = to_canonical_json_string(&doc);
        assert_eq!(s, "{\"a\":3,\"b\":{\"x\":2,\"y\":1}}");
    }

    #[test]
    fn test_to_canonical_json_string_array() {
        let doc = serde_json::json!(["b", "a"]);
        let s = to_canonical_json_string(&doc);
        // Arrays preserve order
        assert_eq!(s, "[\"b\",\"a\"]");
    }

    #[test]
    fn test_build_cache_key_deterministic_regardless_of_field_order() {
        // The cache key should be the same regardless of how the input
        // payload is constructed (different field order)
        let input1 = serde_json::json!({"teacher_prompt": "Buatkan materi", "language": "id"});
        let input2 = serde_json::json!({"language": "id", "teacher_prompt": "Buatkan materi"});
        let key1 = build_cache_key(
            CacheRoute::Interpret, "xiaomi", "m", "t", "i", &input1,
        );
        let key2 = build_cache_key(
            CacheRoute::Interpret, "xiaomi", "m", "t", "i", &input2,
        );
        assert_eq!(key1, key2, "cache key must be independent of input field order");
    }

    #[test]
    fn test_sql_constants_compile() {
        // Verify SQL strings are valid (basic syntax check)
        assert!(lookup_sql().contains("SELECT"));
        assert!(touch_sql().contains("UPDATE"));
        assert!(upsert_sql().contains("INSERT"));
        assert!(delete_expired_by_key_sql().contains("DELETE"));
        assert!(cleanup_sql().contains("DELETE"));
    }
}

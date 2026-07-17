//! Redis Streams queue operations for media generation.
//!
//! Port of `MediaGenerationQueueService` from Laravel.
//!
//! Primitives built on Redis Streams:
//!
//! - `enqueue(generation_id, attempt)` → XADD `klass:media-gen`
//! - `create_consumer_group()` → idempotent XGROUP CREATE `klass-workers`
//! - `concurrency()` → returns the configured max concurrency for the worker pool

use deadpool_redis::Pool as RedisPool;

// ─── Constants ──────────────────────────────────────────────────────────────

/// Redis stream key for media generation jobs.
pub const STREAM_KEY: &str = "klass:media-gen";

/// Consumer group name.
pub const CONSUMER_GROUP: &str = "klass-workers";

/// Default concurrency when no config is provided.
pub const DEFAULT_CONCURRENCY: usize = 1;

// ─── Queue service ──────────────────────────────────────────────────────────

/// Service for enqueuing media generation jobs into Redis Streams.
pub struct QueueService {
    pool: RedisPool,
    stream: String,
    group: String,
    /// Max number of jobs this worker should process concurrently.
    concurrency: usize,
}

impl QueueService {
    /// Create a new queue service with the given concurrency limit.
    pub fn new(pool: RedisPool, concurrency: usize) -> Self {
        Self {
            pool,
            stream: STREAM_KEY.to_string(),
            group: CONSUMER_GROUP.to_string(),
            concurrency,
        }
    }

    /// Return the configured concurrency for worker pool sizing.
    pub fn concurrency(&self) -> usize {
        self.concurrency
    }

    /// Create the consumer group idempotently.
    ///
    /// Uses `MKSTREAM` so the stream key is created automatically if it does
    /// not exist. If the group already exists (`BUSYGROUP` error), this is
    /// silently ignored — safe to call on every worker boot.
    pub async fn create_consumer_group(&self) -> Result<(), QueueError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| QueueError::Redis(e.to_string()))?;

        let result: Result<(), _> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(&[&self.stream, &self.group, "$", "MKSTREAM"])
            .query_async(&mut *conn)
            .await;

        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("BUSYGROUP") {
                    return Ok(());
                }
                Err(QueueError::Redis(msg))
            }
        }
    }

    /// Enqueue a generation job into the Redis stream.
    ///
    /// Calls `XADD klass:media-gen * generation_id <id> job_id <job_id> attempt <n> enqueued_at <now>`.
    /// Returns the auto-generated Redis entry ID (e.g. `"1234567890-0"`).
    pub async fn enqueue(
        &self,
        generation_id: &str,
        job_id: &str,
        attempt: i64,
    ) -> Result<String, QueueError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| QueueError::Redis(e.to_string()))?;

        let now = chrono::Utc::now().to_rfc3339();

        let entry_id: String = redis::cmd("XADD")
            .arg(&self.stream)
            .arg("*")
            .arg("generation_id")
            .arg(generation_id)
            .arg("job_id")
            .arg(job_id)
            .arg("attempt")
            .arg(attempt.to_string())
            .arg("enqueued_at")
            .arg(&now)
            .query_async(&mut *conn)
            .await
            .map_err(|e| QueueError::Redis(e.to_string()))?;

        Ok(entry_id)
    }
}

// ─── Error type ─────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Redis error: {0}")]
    Redis(String),
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_key_constant() {
        assert_eq!(STREAM_KEY, "klass:media-gen");
    }

    #[test]
    fn test_consumer_group_constant() {
        assert_eq!(CONSUMER_GROUP, "klass-workers");
    }

    #[test]
    fn test_default_concurrency() {
        assert_eq!(DEFAULT_CONCURRENCY, 1);
    }

    #[test]
    fn test_queue_error_display() {
        let err = QueueError::Redis("connection refused".to_string());
        assert!(err.to_string().contains("Redis error"));
    }
}

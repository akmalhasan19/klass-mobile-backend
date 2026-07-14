//! Redis Streams worker for consuming media generation jobs.
//!
//! Port of `MediaGenerationWorker` from Laravel.
//!
//! Event loop:
//! 1. `XREADGROUP GROUP klass-workers consumer-name > COUNT 10 BLOCK 5000`
//! 2. Dispatch each message to `WorkflowService::process()`
//! 3. `XACK` on success
//! 4. `XCLAIM` after idle timeout 300s for recovery
//! 5. Max tries 3 → DLQ via `dead_letter`

use std::sync::Arc;
use std::time::Duration;

use deadpool_redis::Pool as RedisPool;
use serde_json::Value;
use tokio::sync::Semaphore;

use crate::orchestrator::audit_trail::AuditTrailService;
use crate::orchestrator::workflow::{
    ComposeStep, DraftStep, GenerateStep, InterpretStep, PublishStep, WorkflowService,
};
use crate::queue::dead_letter::DeadLetterService;

// ─── Constants ──────────────────────────────────────────────────────────────

/// Redis stream key (must match `redis_streams::STREAM_KEY`).
pub const STREAM_KEY: &str = "klass:media-gen";

/// Consumer group name (must match `redis_streams::CONSUMER_GROUP`).
pub const CONSUMER_GROUP: &str = "klass-workers";

/// Default consumer name prefix.
pub const CONSUMER_PREFIX: &str = "worker";

/// Max messages to read per XREADGROUP call.
pub const BATCH_SIZE: usize = 10;

/// Block time in seconds for XREADGROUP.
pub const BLOCK_SECS: u64 = 5;

/// Idle timeout in seconds before another worker can claim a pending message.
pub const IDLE_TIMEOUT_SECS: u64 = 300;

/// Max retry attempts before moving to DLQ.
pub const MAX_TRIES: i64 = 3;

/// How often (in loop iterations) to run XCLAIM for pending message recovery.
const CLAIM_INTERVAL: u32 = 6; // every ~30 seconds with 5s BLOCK

// ─── Worker ─────────────────────────────────────────────────────────────────

/// Worker that consumes media generation jobs from Redis Streams.
pub struct Worker {
    pool: RedisPool,
    pool_pg: sqlx::PgPool,
    dead_letter: Arc<DeadLetterService>,
    stream: String,
    group: String,
    consumer: String,
    concurrency: Arc<Semaphore>,
    claim_counter: std::sync::atomic::AtomicU32,
}

impl Worker {
    /// Create a new worker.
    pub fn new(
        pool: RedisPool,
        pool_pg: sqlx::PgPool,
        consumer_name: String,
        max_concurrency: usize,
    ) -> Self {
        let dead_letter = Arc::new(DeadLetterService::new(pool.clone()));
        Self {
            pool,
            pool_pg,
            dead_letter,
            stream: STREAM_KEY.to_string(),
            group: CONSUMER_GROUP.to_string(),
            consumer: consumer_name,
            concurrency: Arc::new(Semaphore::new(max_concurrency)),
            claim_counter: std::sync::atomic::AtomicU32::new(0),
        }
    }

    /// Run the worker event loop.
    ///
    /// Processes messages from the Redis stream, dispatching each to the
    /// workflow pipeline. Runs indefinitely until cancelled.
    pub async fn run(
        &self,
        _workflow: &WorkflowService,
        interpret: Arc<dyn InterpretStep>,
        draft: Arc<dyn DraftStep>,
        generate: Arc<dyn GenerateStep>,
        publish: Arc<dyn PublishStep>,
        compose: Arc<dyn ComposeStep>,
    ) -> Result<(), WorkerError> {
        // Ensure consumer group exists
        let svc = crate::queue::redis_streams::QueueService::new(self.pool.clone(), 1);
        svc.create_consumer_group().await.map_err(|e| WorkerError::Redis(e.to_string()))?;

        loop {
            // Periodically claim pending messages from failed/abandoned workers
            self.claim_pending().await?;

            // Read new messages
            let entries = self.read_messages().await?;

            for entry in entries {
                let permit = self.concurrency.clone().acquire_owned().await;
                let pool = self.pool.clone();
                let pool_pg = self.pool_pg.clone();
                let dead_letter = self.dead_letter.clone();
                let stream = self.stream.clone();
                let group = self.group.clone();
                let interpret = interpret.clone();
                let draft = draft.clone();
                let generate = generate.clone();
                let publish = publish.clone();
                let compose = compose.clone();

                tokio::spawn(async move {
                    let _permit = permit;
                    let audit = AuditTrailService::new(pool_pg.clone());
                    let wf = WorkflowService::new(pool_pg);

                    // Build job_context for the audit trail
                    let job_context = serde_json::json!({
                        "connection": "redis",
                        "queue": STREAM_KEY,
                        "tries": MAX_TRIES,
                        "timeout_seconds": 300,
                        "backoff_seconds": 30,
                    });

                    let result = wf
                        .process(
                            &entry.generation_id,
                            Some(entry.attempt as i32),
                            Some(job_context),
                            &*interpret,
                            &*draft,
                            &*generate,
                            &*publish,
                            &*compose,
                        )
                        .await;

                    match result {
                        Ok(()) => {
                            // XACK on success
                            let mut conn = match pool.get().await {
                                Ok(c) => c,
                                Err(e) => {
                                    tracing::error!(
                                        generation_id = %entry.generation_id,
                                        error = %e,
                                        "worker: failed to get Redis connection for XACK"
                                    );
                                    return;
                                }
                            };
                            let _: Result<(), _> = redis::cmd("XACK")
                                .arg(&[&stream, &group, &entry.id])
                                .query_async(&mut *conn)
                                .await;
                        }
                        Err(e) => {
                            // Determine if this is a retryable error
                            let error_msg = e.to_string();
                            let new_attempt = entry.attempt + 1;

                            if new_attempt > MAX_TRIES {
                                // Exhausted retries → DLQ
                                tracing::error!(
                                    generation_id = %entry.generation_id,
                                    attempt = new_attempt,
                                    max_tries = MAX_TRIES,
                                    error = %error_msg,
                                    "worker: max retries exceeded, moving to DLQ"
                                );

                                let _ = dead_letter
                                    .send_to_dlq(
                                        &entry.generation_id,
                                        new_attempt,
                                        "WORKFLOW_FAILED",
                                        &error_msg,
                                        &audit,
                                    )
                                    .await;

                                // XACK to remove from PEL
                                let mut conn = match pool.get().await {
                                    Ok(c) => c,
                                    Err(e) => {
                                        tracing::error!(
                                            generation_id = %entry.generation_id,
                                            error = %e,
                                            "worker: failed to XACK after DLQ"
                                        );
                                        return;
                                    }
                                };
                                let _: Result<(), _> = redis::cmd("XACK")
                                    .arg(&[&stream, &group, &entry.id])
                                    .query_async(&mut *conn)
                                    .await;
                            } else {
                                // Retryable: XACK + re-enqueue with incremented attempt
                                tracing::warn!(
                                    generation_id = %entry.generation_id,
                                    attempt = new_attempt,
                                    error = %error_msg,
                                    "worker: workflow failed, re-enqueuing"
                                );

                                let mut conn = match pool.get().await {
                                    Ok(c) => c,
                                    Err(e) => {
                                        tracing::error!(
                                            generation_id = %entry.generation_id,
                                            error = %e,
                                            "worker: failed to get Redis connection for re-enqueue"
                                        );
                                        return;
                                    }
                                };

                                // XACK current entry
                                let _: Result<(), _> = redis::cmd("XACK")
                                    .arg(&[&stream, &group, &entry.id])
                                    .query_async(&mut *conn)
                                    .await;

                                // Re-enqueue with incremented attempt
                                let now = chrono::Utc::now().to_rfc3339();
                                let _: Result<(), _> = redis::cmd("XADD")
                                    .arg(&stream)
                                    .arg("*")
                                    .arg("generation_id")
                                    .arg(&entry.generation_id)
                                    .arg("attempt")
                                    .arg(new_attempt.to_string())
                                    .arg("enqueued_at")
                                    .arg(&now)
                                    .query_async(&mut *conn)
                                    .await;
                            }
                        }
                    }
                });
            }
        }
    }

    /// Claim pending messages from other workers that may have crashed.
    ///
    /// Uses `XCLAIM` to transfer ownership of messages that have been idle
    /// longer than `IDLE_TIMEOUT_SECS`. Runs every `CLAIM_INTERVAL` iterations.
    async fn claim_pending(&self) -> Result<(), WorkerError> {
        let counter = self
            .claim_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if counter % CLAIM_INTERVAL != 0 {
            return Ok(());
        }

        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| WorkerError::Redis(e.to_string()))?;

        // Get list of pending message IDs (XPENDING returns bulk strings)
        let pending: Option<redis::Value> = redis::cmd("XPENDING")
            .arg(&[&self.stream, &self.group])
            .arg("-")
            .arg("+")
            .arg(BATCH_SIZE as i64)
            .query_async(&mut *conn)
            .await
            .map_err(|e| WorkerError::Redis(e.to_string()))?;

        // Parse XPENDING result: array of [entry_id, consumer, idle_ms, deliveries]
        let entry_ids: Vec<String> = match pending {
            Some(redis::Value::Array(items)) => items
                .iter()
                .filter_map(|item| match item {
                    redis::Value::Array(parts) if parts.len() >= 1 => {
                        Some(value_to_str(&parts[0]).unwrap_or_default())
                    }
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };

        for entry_id in &entry_ids {
            if entry_id.is_empty() {
                continue;
            }
            // XCLAIM the message: reassign to this consumer
            let idle_ms = (IDLE_TIMEOUT_SECS * 1000).to_string();
            let _: Result<(), _> = redis::cmd("XCLAIM")
                .arg(&[
                    &self.stream,
                    &self.group,
                    &self.consumer,
                    &idle_ms,
                    entry_id,
                ])
                .query_async(&mut *conn)
                .await;
        }

        Ok(())
    }

    /// Read new messages from the stream.
    ///
    /// XREADGROUP GROUP `group` `consumer` > COUNT `BATCH_SIZE` BLOCK `BLOCK_SECS * 1000`
    async fn read_messages(&self) -> Result<Vec<StreamEntry>, WorkerError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| WorkerError::Redis(e.to_string()))?;

        let raw: Option<redis::Value> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(&self.group)
            .arg(&self.consumer)
            .arg("COUNT")
            .arg(BATCH_SIZE as i64)
            .arg("BLOCK")
            .arg((BLOCK_SECS * 1000) as i64)
            .arg("STREAMS")
            .arg(&self.stream)
            .arg(">")
            .query_async(&mut *conn)
            .await
            .map_err(|e| WorkerError::Redis(e.to_string()))?;

        let streams = match raw {
            Some(redis::Value::Array(a)) => a,
            _ => return Ok(Vec::new()),
        };

        let mut entries = Vec::new();

        for stream_result in &streams {
            let msgs = match extract_messages(stream_result) {
                Some(m) => m,
                None => continue,
            };
            for (entry_id, fields) in msgs {
                let generation_id = fields
                    .get("generation_id")
                    .cloned()
                    .unwrap_or_default();
                let attempt: i64 = fields
                    .get("attempt")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1);
                let payload = serde_json::json!({
                    "generation_id": generation_id,
                    "attempt": attempt,
                    "entry_id": entry_id,
                });
                entries.push(StreamEntry {
                    id: entry_id,
                    generation_id,
                    attempt,
                    payload,
                });
            }
        }

        Ok(entries)
    }
}

// ─── Response parser ────────────────────────────────────────────────────────

/// Extract messages from a raw XREADGROUP stream result.
///
/// `stream_result` is a `redis::Value::Array` with two elements:
///   [stream_name, messages]
///
/// Each message is:
///   [entry_id, fields]
/// where `fields` is a `Value::Array` of alternating key-value strings:
///   [key1, val1, key2, val2, ...]
fn extract_messages(
    stream_result: &redis::Value,
) -> Option<Vec<(String, std::collections::HashMap<String, String>)>> {
    let arr = match stream_result {
        redis::Value::Array(a) => a,
        _ => return None,
    };
    if arr.len() < 2 {
        return None;
    }
    let messages_arr = match &arr[1] {
        redis::Value::Array(a) => a,
        _ => return None,
    };

    let mut result = Vec::new();
    for msg in messages_arr {
        let msg_arr = match msg {
            redis::Value::Array(a) => a,
            _ => continue,
        };
        if msg_arr.len() < 2 {
            continue;
        }
        let entry_id = value_to_str(&msg_arr[0])?;

        let fields_arr = match &msg_arr[1] {
            redis::Value::Array(a) => a,
            _ => continue,
        };

        let mut fields = std::collections::HashMap::new();
        for chunk in fields_arr.chunks(2) {
            if chunk.len() == 2 {
                let key = value_to_str(&chunk[0]);
                let val = value_to_str(&chunk[1]);
                if let Some(k) = key {
                    fields.insert(k, val.unwrap_or_default());
                }
            }
        }

        result.push((entry_id, fields));
    }

    Some(result)
}

/// Extract a string from a `redis::Value`, handling multiple representations.
fn value_to_str(v: &redis::Value) -> Option<String> {
    match v {
        redis::Value::BulkString(d) => Some(String::from_utf8_lossy(d).to_string()),
        redis::Value::SimpleString(s) => Some(s.clone()),
        redis::Value::Int(i) => Some(i.to_string()),
        _ => None,
    }
}

// ─── Types ──────────────────────────────────────────────────────────────────

/// A single stream entry parsed from XREADGROUP.
#[derive(Debug, Clone)]
pub struct StreamEntry {
    pub id: String,
    pub generation_id: String,
    pub attempt: i64,
    pub payload: Value,
}

// ─── Error type ─────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("Redis error: {0}")]
    Redis(String),
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(STREAM_KEY, "klass:media-gen");
        assert_eq!(CONSUMER_GROUP, "klass-workers");
        assert_eq!(CONSUMER_PREFIX, "worker");
        assert_eq!(BATCH_SIZE, 10);
        assert_eq!(BLOCK_SECS, 5);
        assert_eq!(IDLE_TIMEOUT_SECS, 300);
        assert_eq!(MAX_TRIES, 3);
    }

    #[test]
    fn test_stream_entry_construction() {
        let entry = StreamEntry {
            id: "1234567890-0".to_string(),
            generation_id: "gen-1".to_string(),
            attempt: 1,
            payload: serde_json::json!({"generation_id": "gen-1", "attempt": 1}),
        };
        assert_eq!(entry.generation_id, "gen-1");
        assert_eq!(entry.attempt, 1);
        assert!(!entry.id.is_empty());
    }

    #[test]
    fn test_extract_messages_empty() {
        let result = extract_messages(&redis::Value::Array(vec![]));
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_messages_single_entry() {
        let stream = redis::Value::Array(vec![
            redis::Value::BulkString(b"klass:media-gen".to_vec()),
            redis::Value::Array(vec![
                redis::Value::Array(vec![
                    redis::Value::BulkString(b"1234567890-0".to_vec()),
                    redis::Value::Array(vec![
                        redis::Value::BulkString(b"generation_id".to_vec()),
                        redis::Value::BulkString(b"gen-1".to_vec()),
                        redis::Value::BulkString(b"attempt".to_vec()),
                        redis::Value::BulkString(b"1".to_vec()),
                    ]),
                ]),
            ]),
        ]);

        let result = extract_messages(&stream);
        assert!(result.is_some());
        let msgs = result.unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].0, "1234567890-0");
        assert_eq!(msgs[0].1.get("generation_id").unwrap(), "gen-1");
        assert_eq!(msgs[0].1.get("attempt").unwrap(), "1");
    }

    #[test]
    fn test_extract_messages_multiple_entries() {
        let stream = redis::Value::Array(vec![
            redis::Value::BulkString(b"klass:media-gen".to_vec()),
            redis::Value::Array(vec![
                redis::Value::Array(vec![
                    redis::Value::BulkString(b"entry-1".to_vec()),
                    redis::Value::Array(vec![
                        redis::Value::BulkString(b"generation_id".to_vec()),
                        redis::Value::BulkString(b"gen-1".to_vec()),
                    ]),
                ]),
                redis::Value::Array(vec![
                    redis::Value::BulkString(b"entry-2".to_vec()),
                    redis::Value::Array(vec![
                        redis::Value::BulkString(b"generation_id".to_vec()),
                        redis::Value::BulkString(b"gen-2".to_vec()),
                    ]),
                ]),
            ]),
        ]);

        let result = extract_messages(&stream);
        assert!(result.is_some());
        let msgs = result.unwrap();
        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn test_extract_messages_skips_badly_formed() {
        let stream = redis::Value::Array(vec![
            redis::Value::BulkString(b"stream".to_vec()),
            redis::Value::Array(vec![
                redis::Value::Array(vec![
                    redis::Value::BulkString(b"entry-1".to_vec()),
                    redis::Value::Array(vec![
                        redis::Value::BulkString(b"generation_id".to_vec()),
                        redis::Value::BulkString(b"gen-1".to_vec()),
                        redis::Value::BulkString(b"orphan".to_vec()),
                    ]),
                ]),
            ]),
        ]);

        let result = extract_messages(&stream);
        assert!(result.is_some());
        let msgs = result.unwrap();
        assert_eq!(msgs.len(), 1);
        // "orphan" has no value pair, so it's skipped
        assert_eq!(msgs[0].1.len(), 1);
        assert_eq!(msgs[0].1.get("generation_id").unwrap(), "gen-1");
    }
}

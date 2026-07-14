//! Redis Streams queue for async media generation processing.
//!
//! - `redis_streams` — XADD / XREADGROUP / XACK primitives
//! - `worker` — `Worker::run()` event loop dispatching to `WorkflowService`
//! - `dead_letter` — DLQ and admin retry helpers

pub mod dead_letter;
pub mod redis_streams;
pub mod worker;

//! Contract schemas for LLM provider requests and responses.
//!
//! Each module defines:
//! - `SCHEMA_VERSION` constant (e.g. `"media_prompt_understanding.v1"`)
//! - `decode_and_validate(raw_json: &str) -> Result<Schema, Error>` 
//! - `fallback()` or `fallback_from_*()` constructors for graceful degradation
//!
//! Uses `serde` for deserialization and `garde` for validation rules.

pub mod artifact_metadata;
pub mod content_draft;
pub mod delivery;
pub mod generation_spec;
pub mod prompt_interpretation;
pub mod taxonomy_inference;

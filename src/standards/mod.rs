//! Content standards module.
//!
//! Defines content standards per media type: which fields are required,
//! recommended, or optional. Provides content type detection from raw prompts
//! and suggestion chip definitions for the clarification UI.

pub mod content_standards;

pub use content_standards::{
    detect_content_type, detect_output_type, detect_target_audience, get_clarification_fields,
    get_standards_for_content_type, ContentGap, ContentType, FieldDefinition, FieldPriority,
    InputType, SuggestionChip,
};

//! Media generation lifecycle state machine.
//!
//! Defines the 9-state lifecycle matching the `GenerationStatus` proto enum
//! and the Laravel `MediaGenerationLifecycle` class EXACTLY.
//!
//! States:
//!   QUEUED → INTERPRETING → CLASSIFIED → GENERATING → UPLOADING → PUBLISHING → COMPLETED
//!   Any non-terminal state may also transition to FAILED or CANCELLED.
//!
//! Terminal states: COMPLETED, FAILED, CANCELLED
//!
//! Invariants:
//!   - `can_transition(from, to)` enforces the matrix from Laravel.
//!   - `StatusBefore` trait prevents status regression (state should not move backwards).

use std::fmt;

// ─── Enum ─────────────────────────────────────────────────────────────────────

/// The 9-state lifecycle for media generation.
///
/// Discriminant values match `GenerationStatus` proto enum:
///   0 = Unspecified (not used in lifecycle)
///   1 = QUEUED,  2 = INTERPRETING,  3 = CLASSIFIED,
///   4 = GENERATING,  5 = UPLOADING,  6 = PUBLISHING,
///   7 = COMPLETED,  8 = FAILED,  9 = CANCELLED
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaGenerationStatus {
    /// Initial state — generation created but not yet processed.
    Queued = 1,
    /// LLM interpreting the prompt (subject, objectives, blueprint).
    Interpreting = 2,
    /// Interpretation done, decision made (output type, blueprint).
    Classified = 3,
    /// Media generator producing the file (.pdf/.docx/.pptx).
    Generating = 4,
    /// File being uploaded to R2/S3.
    Uploading = 5,
    /// Publication entities being created (topic, content, project).
    Publishing = 6,
    /// Terminal — all steps completed successfully.
    Completed = 7,
    /// Terminal — unrecoverable failure.
    Failed = 8,
    /// Terminal — cancelled by user.
    Cancelled = 9,
}

impl MediaGenerationStatus {
    /// All 9 statuses in order.
    pub const ALL: [MediaGenerationStatus; 9] = [
        MediaGenerationStatus::Queued,
        MediaGenerationStatus::Interpreting,
        MediaGenerationStatus::Classified,
        MediaGenerationStatus::Generating,
        MediaGenerationStatus::Uploading,
        MediaGenerationStatus::Publishing,
        MediaGenerationStatus::Completed,
        MediaGenerationStatus::Failed,
        MediaGenerationStatus::Cancelled,
    ];

    /// The 8 minimum statuses (excludes CANCELLED — matching Laravel's `minimumStatuses()`).
    pub const MINIMUM: [MediaGenerationStatus; 8] = [
        MediaGenerationStatus::Queued,
        MediaGenerationStatus::Interpreting,
        MediaGenerationStatus::Classified,
        MediaGenerationStatus::Generating,
        MediaGenerationStatus::Uploading,
        MediaGenerationStatus::Publishing,
        MediaGenerationStatus::Completed,
        MediaGenerationStatus::Failed,
    ];

    /// Terminal states that end the lifecycle.
    pub const TERMINAL: [MediaGenerationStatus; 3] = [
        MediaGenerationStatus::Completed,
        MediaGenerationStatus::Failed,
        MediaGenerationStatus::Cancelled,
    ];

    /// Check if this status is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, MediaGenerationStatus::Completed | MediaGenerationStatus::Failed | MediaGenerationStatus::Cancelled)
    }

    /// Check if this status can transition to `to`.
    ///
    /// Matrix matches Laravel `statusDefinitions()` EXACTLY:
    /// - QUEUED → INTERPRETING, FAILED, CANCELLED
    /// - INTERPRETING → CLASSIFIED, FAILED, CANCELLED
    /// - CLASSIFIED → GENERATING, FAILED, CANCELLED
    /// - GENERATING → UPLOADING, FAILED, CANCELLED
    /// - UPLOADING → PUBLISHING, FAILED, CANCELLED
    /// - PUBLISHING → COMPLETED, FAILED
    /// - COMPLETED → (none)
    /// - FAILED → (none)
    /// - CANCELLED → (none)
    pub fn can_transition(&self, to: MediaGenerationStatus) -> bool {
        use MediaGenerationStatus::*;
        match self {
            Queued => matches!(to, Interpreting | Failed | Cancelled),
            Interpreting => matches!(to, Classified | Failed | Cancelled),
            Classified => matches!(to, Generating | Failed | Cancelled),
            Generating => matches!(to, Uploading | Failed | Cancelled),
            Uploading => matches!(to, Publishing | Failed | Cancelled),
            Publishing => matches!(to, Completed | Failed),
            Completed => false,          // terminal
            Failed => false,             // terminal
            Cancelled => false,          // terminal
        }
    }

    /// Return the next valid transitions for this status.
    pub fn next_statuses(&self) -> &'static [MediaGenerationStatus] {
        use MediaGenerationStatus::*;
        match self {
            Queued => &[Interpreting, Failed, Cancelled],
            Interpreting => &[Classified, Failed, Cancelled],
            Classified => &[Generating, Failed, Cancelled],
            Generating => &[Uploading, Failed, Cancelled],
            Uploading => &[Publishing, Failed, Cancelled],
            Publishing => &[Completed, Failed],
            Completed => &[],
            Failed => &[],
            Cancelled => &[],
        }
    }

    /// Return the retry behavior for this status, matching Laravel EXACTLY.
    pub fn retry_behavior(&self) -> &'static str {
        use MediaGenerationStatus::*;
        match self {
            Queued => "requeue_pending_job",
            Interpreting => "resume_current_step",
            Classified => "continue_to_next_step",
            Generating => "resume_current_step",
            Uploading => "resume_current_step",
            Publishing => "resume_current_step",
            Completed => "forbidden",
            Failed => "restart_from_interpreting",
            Cancelled => "manual_requeue_only",
        }
    }

    /// Position in the ordering (0-based). Used for `StatusBefore` comparison.
    pub fn order_index(&self) -> usize {
        use MediaGenerationStatus::*;
        match self {
            Queued => 0,
            Interpreting => 1,
            Classified => 2,
            Generating => 3,
            Uploading => 4,
            Publishing => 5,
            Completed => 6,
            Failed => 7,
            Cancelled => 8,
        }
    }

    /// Convert from proto `GenerationStatus` integer value.
    pub fn from_proto_value(value: i32) -> Option<Self> {
        use MediaGenerationStatus::*;
        match value {
            1 => Some(Queued),
            2 => Some(Interpreting),
            3 => Some(Classified),
            4 => Some(Generating),
            5 => Some(Uploading),
            6 => Some(Publishing),
            7 => Some(Completed),
            8 => Some(Failed),
            9 => Some(Cancelled),
            _ => None,
        }
    }

    /// Convert to proto `GenerationStatus` integer value.
    pub fn to_proto_value(&self) -> i32 {
        *self as i32
    }

    /// Parse from a string (useful for DB columns and JSON deserialization).
    pub fn from_str(s: &str) -> Option<Self> {
        use MediaGenerationStatus::*;
        match s {
            "queued" => Some(Queued),
            "interpreting" => Some(Interpreting),
            "classified" => Some(Classified),
            "generating" => Some(Generating),
            "uploading" => Some(Uploading),
            "publishing" => Some(Publishing),
            "completed" => Some(Completed),
            "failed" => Some(Failed),
            "cancelled" => Some(Cancelled),
            _ => None,
        }
    }

    /// Convert to a string (inverse of `from_str`).
    pub fn as_str(&self) -> &'static str {
        use MediaGenerationStatus::*;
        match self {
            Queued => "queued",
            Interpreting => "interpreting",
            Classified => "classified",
            Generating => "generating",
            Uploading => "uploading",
            Publishing => "publishing",
            Completed => "completed",
            Failed => "failed",
            Cancelled => "cancelled",
        }
    }
}

impl fmt::Display for MediaGenerationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ─── StatusBefore invariant ─────────────────────────────────────────────────

/// Trait to check whether a current status occurs *before* a checkpoint in the
/// lifecycle ordering. The invariant `StatusBefore` prevents status regression:
/// a generation's status should never move to a lower-ordered state.
///
/// Example: `Queued.status_before(Interpreting)` → true (queued is before interpreting)
///          `Completed.status_before(Interpreting)` → false (completed is after)
pub trait StatusBefore {
    /// Returns true if `self` comes before `checkpoint` in the status order.
    /// Terminal states (COMPLETED, FAILED, CANCELLED) always return false for
    /// any checkpoint — they cannot move forward.
    fn status_before(&self, checkpoint: MediaGenerationStatus) -> bool;
}

impl StatusBefore for MediaGenerationStatus {
    fn status_before(&self, checkpoint: MediaGenerationStatus) -> bool {
        if self.is_terminal() {
            return false;
        }
        self.order_index() < checkpoint.order_index()
    }
}

impl StatusBefore for Option<MediaGenerationStatus> {
    fn status_before(&self, checkpoint: MediaGenerationStatus) -> bool {
        match self {
            Some(status) => status.status_before(checkpoint),
            None => true, // None (uninitialized) is before everything
        }
    }
}

// ─── TryFrom / IntoStr implementations ──────────────────────────────────────

impl TryFrom<i32> for MediaGenerationStatus {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::from_proto_value(value).ok_or_else(|| {
            format!("Unknown MediaGenerationStatus proto value: {}", value)
        })
    }
}

impl TryFrom<&str> for MediaGenerationStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_str(value).ok_or_else(|| {
            format!("Unknown MediaGenerationStatus string: {}", value)
        })
    }
}

impl From<MediaGenerationStatus> for i32 {
    fn from(status: MediaGenerationStatus) -> i32 {
        status.to_proto_value()
    }
}

// ─── Serde support (for JSON serialization) ────────────────────────────────

use serde::Deserialize as SerdeDeserialize;

impl serde::Serialize for MediaGenerationStatus {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for MediaGenerationStatus {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <String as SerdeDeserialize>::deserialize(deserializer)?;
        Self::from_str(&s).ok_or_else(|| serde::de::Error::custom(format!("Unknown status: {}", s)))
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Enum variants ──────────────────────────────────────────────────────

    #[test]
    fn test_all_statuses_count() {
        assert_eq!(MediaGenerationStatus::ALL.len(), 9);
    }

    #[test]
    fn test_minimum_statuses_count() {
        assert_eq!(MediaGenerationStatus::MINIMUM.len(), 8);
    }

    #[test]
    fn test_terminal_states_count() {
        assert_eq!(MediaGenerationStatus::TERMINAL.len(), 3);
    }

    #[test]
    fn test_minimum_excludes_cancelled() {
        assert!(!MediaGenerationStatus::MINIMUM.contains(&MediaGenerationStatus::Cancelled));
    }

    #[test]
    fn test_all_includes_cancelled() {
        assert!(MediaGenerationStatus::ALL.contains(&MediaGenerationStatus::Cancelled));
    }

    // ── Proto value conversion ─────────────────────────────────────────────

    #[test]
    fn test_proto_value_queued_is_1() {
        assert_eq!(MediaGenerationStatus::Queued.to_proto_value(), 1);
        assert_eq!(MediaGenerationStatus::from_proto_value(1), Some(MediaGenerationStatus::Queued));
    }

    #[test]
    fn test_proto_value_completed_is_7() {
        assert_eq!(MediaGenerationStatus::Completed.to_proto_value(), 7);
        assert_eq!(MediaGenerationStatus::from_proto_value(7), Some(MediaGenerationStatus::Completed));
    }

    #[test]
    fn test_proto_value_cancelled_is_9() {
        assert_eq!(MediaGenerationStatus::Cancelled.to_proto_value(), 9);
    }

    #[test]
    fn test_proto_value_unknown_returns_none() {
        assert!(MediaGenerationStatus::from_proto_value(0).is_none());
        assert!(MediaGenerationStatus::from_proto_value(10).is_none());
        assert!(MediaGenerationStatus::from_proto_value(-1).is_none());
    }

    #[test]
    fn test_try_from_i32_ok() {
        let status: MediaGenerationStatus = 1.try_into().unwrap();
        assert_eq!(status, MediaGenerationStatus::Queued);
    }

    #[test]
    fn test_try_from_i32_err() {
        let result: Result<MediaGenerationStatus, _> = 99.try_into();
        assert!(result.is_err());
    }

    #[test]
    fn test_from_status_to_i32() {
        let val: i32 = MediaGenerationStatus::Completed.into();
        assert_eq!(val, 7);
    }

    // ── String conversion ──────────────────────────────────────────────────

    #[test]
    fn test_as_str_matches_laravel() {
        assert_eq!(MediaGenerationStatus::Queued.as_str(), "queued");
        assert_eq!(MediaGenerationStatus::Failed.as_str(), "failed");
        assert_eq!(MediaGenerationStatus::Cancelled.as_str(), "cancelled");
    }

    #[test]
    fn test_from_str_valid() {
        assert_eq!(MediaGenerationStatus::from_str("interpreting"), Some(MediaGenerationStatus::Interpreting));
        assert_eq!(MediaGenerationStatus::from_str("generating"), Some(MediaGenerationStatus::Generating));
    }

    #[test]
    fn test_from_str_invalid() {
        assert!(MediaGenerationStatus::from_str("unknown").is_none());
        assert!(MediaGenerationStatus::from_str("").is_none());
    }

    #[test]
    fn test_display_matches_as_str() {
        assert_eq!(format!("{}", MediaGenerationStatus::Publishing), "publishing");
    }

    #[test]
    fn test_try_from_str_ok() {
        let status: MediaGenerationStatus = "uploading".try_into().unwrap();
        assert_eq!(status, MediaGenerationStatus::Uploading);
    }

    #[test]
    fn test_try_from_str_err() {
        let result: Result<MediaGenerationStatus, _> = "bogus".try_into();
        assert!(result.is_err());
    }

    // ── is_terminal ─────────────────────────────────────────────────────────

    #[test]
    fn test_is_terminal_completed() {
        assert!(MediaGenerationStatus::Completed.is_terminal());
    }

    #[test]
    fn test_is_terminal_failed() {
        assert!(MediaGenerationStatus::Failed.is_terminal());
    }

    #[test]
    fn test_is_terminal_cancelled() {
        assert!(MediaGenerationStatus::Cancelled.is_terminal());
    }

    #[test]
    fn test_is_not_terminal_queued() {
        assert!(!MediaGenerationStatus::Queued.is_terminal());
    }

    #[test]
    fn test_is_not_terminal_interpreting() {
        assert!(!MediaGenerationStatus::Interpreting.is_terminal());
    }

    // ── can_transition matrix ───────────────────────────────────────────────

    #[test]
    fn test_queued_transitions() {
        let s = MediaGenerationStatus::Queued;
        assert!(s.can_transition(MediaGenerationStatus::Interpreting));
        assert!(s.can_transition(MediaGenerationStatus::Failed));
        assert!(s.can_transition(MediaGenerationStatus::Cancelled));
        assert!(!s.can_transition(MediaGenerationStatus::Classified));
        assert!(!s.can_transition(MediaGenerationStatus::Completed));
    }

    #[test]
    fn test_interpreting_transitions() {
        let s = MediaGenerationStatus::Interpreting;
        assert!(s.can_transition(MediaGenerationStatus::Classified));
        assert!(s.can_transition(MediaGenerationStatus::Failed));
        assert!(s.can_transition(MediaGenerationStatus::Cancelled));
        assert!(!s.can_transition(MediaGenerationStatus::Queued));
        assert!(!s.can_transition(MediaGenerationStatus::Generating));
    }

    #[test]
    fn test_classified_transitions() {
        let s = MediaGenerationStatus::Classified;
        assert!(s.can_transition(MediaGenerationStatus::Generating));
        assert!(s.can_transition(MediaGenerationStatus::Failed));
        assert!(s.can_transition(MediaGenerationStatus::Cancelled));
        assert!(!s.can_transition(MediaGenerationStatus::Interpreting));
    }

    #[test]
    fn test_generating_transitions() {
        let s = MediaGenerationStatus::Generating;
        assert!(s.can_transition(MediaGenerationStatus::Uploading));
        assert!(s.can_transition(MediaGenerationStatus::Failed));
        assert!(s.can_transition(MediaGenerationStatus::Cancelled));
        assert!(!s.can_transition(MediaGenerationStatus::Classified));
    }

    #[test]
    fn test_uploading_transitions() {
        let s = MediaGenerationStatus::Uploading;
        assert!(s.can_transition(MediaGenerationStatus::Publishing));
        assert!(s.can_transition(MediaGenerationStatus::Failed));
        assert!(s.can_transition(MediaGenerationStatus::Cancelled));
        assert!(!s.can_transition(MediaGenerationStatus::Generating));
    }

    #[test]
    fn test_publishing_transitions() {
        let s = MediaGenerationStatus::Publishing;
        assert!(s.can_transition(MediaGenerationStatus::Completed));
        assert!(s.can_transition(MediaGenerationStatus::Failed));
        assert!(!s.can_transition(MediaGenerationStatus::Cancelled));
        assert!(!s.can_transition(MediaGenerationStatus::Uploading));
    }

    #[test]
    fn test_completed_no_transitions() {
        let s = MediaGenerationStatus::Completed;
        assert!(!s.can_transition(MediaGenerationStatus::Failed));
        assert!(!s.can_transition(MediaGenerationStatus::Cancelled));
        assert!(!s.can_transition(MediaGenerationStatus::Queued));
        assert!(!s.can_transition(MediaGenerationStatus::Interpreting));
    }

    #[test]
    fn test_failed_no_transitions() {
        let s = MediaGenerationStatus::Failed;
        assert!(!s.can_transition(MediaGenerationStatus::Queued));
        assert!(!s.can_transition(MediaGenerationStatus::Completed));
        assert!(!s.can_transition(MediaGenerationStatus::Cancelled));
    }

    #[test]
    fn test_cancelled_no_transitions() {
        let s = MediaGenerationStatus::Cancelled;
        assert!(!s.can_transition(MediaGenerationStatus::Queued));
        assert!(!s.can_transition(MediaGenerationStatus::Failed));
        assert!(!s.can_transition(MediaGenerationStatus::Completed));
    }

    #[test]
    fn test_transition_to_self_not_allowed() {
        for status in MediaGenerationStatus::ALL.iter() {
            assert!(!status.can_transition(*status), "self-transition should not be allowed for {:?}", status);
        }
    }

    // ── next_statuses ──────────────────────────────────────────────────────

    #[test]
    fn test_next_statuses_queued() {
        assert_eq!(MediaGenerationStatus::Queued.next_statuses(), &[
            MediaGenerationStatus::Interpreting,
            MediaGenerationStatus::Failed,
            MediaGenerationStatus::Cancelled,
        ]);
    }

    #[test]
    fn test_next_statuses_publishing() {
        assert_eq!(MediaGenerationStatus::Publishing.next_statuses(), &[
            MediaGenerationStatus::Completed,
            MediaGenerationStatus::Failed,
        ]);
    }

    #[test]
    fn test_next_statuses_terminal_returns_empty() {
        assert!(MediaGenerationStatus::Completed.next_statuses().is_empty());
        assert!(MediaGenerationStatus::Failed.next_statuses().is_empty());
        assert!(MediaGenerationStatus::Cancelled.next_statuses().is_empty());
    }

    // ── retry_behavior ──────────────────────────────────────────────────────

    #[test]
    fn test_retry_behavior_queued() {
        assert_eq!(MediaGenerationStatus::Queued.retry_behavior(), "requeue_pending_job");
    }

    #[test]
    fn test_retry_behavior_completed() {
        assert_eq!(MediaGenerationStatus::Completed.retry_behavior(), "forbidden");
    }

    #[test]
    fn test_retry_behavior_failed() {
        assert_eq!(MediaGenerationStatus::Failed.retry_behavior(), "restart_from_interpreting");
    }

    #[test]
    fn test_retry_behavior_cancelled() {
        assert_eq!(MediaGenerationStatus::Cancelled.retry_behavior(), "manual_requeue_only");
    }

    #[test]
    fn test_retry_behavior_classified() {
        assert_eq!(MediaGenerationStatus::Classified.retry_behavior(), "continue_to_next_step");
    }

    // ── order_index ─────────────────────────────────────────────────────────

    #[test]
    fn test_order_index_queued_is_0() {
        assert_eq!(MediaGenerationStatus::Queued.order_index(), 0);
    }

    #[test]
    fn test_order_index_completed_is_6() {
        assert_eq!(MediaGenerationStatus::Completed.order_index(), 6);
    }

    #[test]
    fn test_order_index_cancelled_is_8() {
        assert_eq!(MediaGenerationStatus::Cancelled.order_index(), 8);
    }

    #[test]
    fn test_order_increases_forward() {
        let order: Vec<_> = MediaGenerationStatus::ALL.iter().map(|s| s.order_index()).collect();
        for i in 1..order.len() {
            assert!(order[i] > order[i - 1], "order must be strictly increasing");
        }
    }

    // ── status_before (StatusBefore trait) ──────────────────────────────────

    #[test]
    fn test_queued_is_before_interpreting() {
        assert!(MediaGenerationStatus::Queued.status_before(MediaGenerationStatus::Interpreting));
    }

    #[test]
    fn test_interpreting_is_not_before_queued() {
        assert!(!MediaGenerationStatus::Interpreting.status_before(MediaGenerationStatus::Queued));
    }

    #[test]
    fn test_same_status_is_not_before() {
        assert!(!MediaGenerationStatus::Generating.status_before(MediaGenerationStatus::Generating));
    }

    #[test]
    fn test_terminal_never_before_anything() {
        assert!(!MediaGenerationStatus::Completed.status_before(MediaGenerationStatus::Queued));
        assert!(!MediaGenerationStatus::Completed.status_before(MediaGenerationStatus::Interpreting));
        assert!(!MediaGenerationStatus::Failed.status_before(MediaGenerationStatus::Queued));
        assert!(!MediaGenerationStatus::Cancelled.status_before(MediaGenerationStatus::Queued));
    }

    #[test]
    fn test_option_none_is_before_everything() {
        let none: Option<MediaGenerationStatus> = None;
        assert!(none.status_before(MediaGenerationStatus::Queued));
        assert!(none.status_before(MediaGenerationStatus::Completed));
    }

    #[test]
    fn test_option_some_delegates() {
        let status = Some(MediaGenerationStatus::Queued);
        assert!(status.status_before(MediaGenerationStatus::Interpreting));
        assert!(!status.status_before(MediaGenerationStatus::Queued));
    }

    // ── Edge cases ─────────────────────────────────────────────────────────

    #[test]
    fn test_equality() {
        assert_eq!(MediaGenerationStatus::Queued, MediaGenerationStatus::Queued);
        assert_ne!(MediaGenerationStatus::Queued, MediaGenerationStatus::Interpreting);
    }

    #[test]
    fn test_clone() {
        let a = MediaGenerationStatus::Failed;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn test_hash_map_key() {
        use std::collections::HashMap;
        let mut map: HashMap<MediaGenerationStatus, &str> = HashMap::new();
        map.insert(MediaGenerationStatus::Completed, "done");
        assert_eq!(map.get(&MediaGenerationStatus::Completed), Some(&"done"));
    }

    #[test]
    fn test_debug_format() {
        let s = format!("{:?}", MediaGenerationStatus::Interpreting);
        assert!(s.contains("Interpreting"));
    }

    #[test]
    fn test_transition_matrix_full_coverage() {
        // Verify all 9x9 possible transitions against Laravel's expected matrix
        let cases: Vec<(MediaGenerationStatus, MediaGenerationStatus, bool)> = vec![
            // QUEUED → *
            (MediaGenerationStatus::Queued, MediaGenerationStatus::Queued, false),
            (MediaGenerationStatus::Queued, MediaGenerationStatus::Interpreting, true),
            (MediaGenerationStatus::Queued, MediaGenerationStatus::Classified, false),
            (MediaGenerationStatus::Queued, MediaGenerationStatus::Generating, false),
            (MediaGenerationStatus::Queued, MediaGenerationStatus::Uploading, false),
            (MediaGenerationStatus::Queued, MediaGenerationStatus::Publishing, false),
            (MediaGenerationStatus::Queued, MediaGenerationStatus::Completed, false),
            (MediaGenerationStatus::Queued, MediaGenerationStatus::Failed, true),
            (MediaGenerationStatus::Queued, MediaGenerationStatus::Cancelled, true),
            // INTERPRETING → *
            (MediaGenerationStatus::Interpreting, MediaGenerationStatus::Interpreting, false),
            (MediaGenerationStatus::Interpreting, MediaGenerationStatus::Classified, true),
            (MediaGenerationStatus::Interpreting, MediaGenerationStatus::Generating, false),
            (MediaGenerationStatus::Interpreting, MediaGenerationStatus::Uploading, false),
            (MediaGenerationStatus::Interpreting, MediaGenerationStatus::Publishing, false),
            (MediaGenerationStatus::Interpreting, MediaGenerationStatus::Completed, false),
            (MediaGenerationStatus::Interpreting, MediaGenerationStatus::Failed, true),
            (MediaGenerationStatus::Interpreting, MediaGenerationStatus::Cancelled, true),
            // CLASSIFIED → *
            (MediaGenerationStatus::Classified, MediaGenerationStatus::Interpreting, false),
            (MediaGenerationStatus::Classified, MediaGenerationStatus::Classified, false),
            (MediaGenerationStatus::Classified, MediaGenerationStatus::Generating, true),
            (MediaGenerationStatus::Classified, MediaGenerationStatus::Uploading, false),
            (MediaGenerationStatus::Classified, MediaGenerationStatus::Publishing, false),
            (MediaGenerationStatus::Classified, MediaGenerationStatus::Completed, false),
            (MediaGenerationStatus::Classified, MediaGenerationStatus::Failed, true),
            (MediaGenerationStatus::Classified, MediaGenerationStatus::Cancelled, true),
            // GENERATING → *
            (MediaGenerationStatus::Generating, MediaGenerationStatus::Classified, false),
            (MediaGenerationStatus::Generating, MediaGenerationStatus::Generating, false),
            (MediaGenerationStatus::Generating, MediaGenerationStatus::Uploading, true),
            (MediaGenerationStatus::Generating, MediaGenerationStatus::Publishing, false),
            (MediaGenerationStatus::Generating, MediaGenerationStatus::Completed, false),
            (MediaGenerationStatus::Generating, MediaGenerationStatus::Failed, true),
            (MediaGenerationStatus::Generating, MediaGenerationStatus::Cancelled, true),
            // UPLOADING → *
            (MediaGenerationStatus::Uploading, MediaGenerationStatus::Generating, false),
            (MediaGenerationStatus::Uploading, MediaGenerationStatus::Uploading, false),
            (MediaGenerationStatus::Uploading, MediaGenerationStatus::Publishing, true),
            (MediaGenerationStatus::Uploading, MediaGenerationStatus::Completed, false),
            (MediaGenerationStatus::Uploading, MediaGenerationStatus::Failed, true),
            (MediaGenerationStatus::Uploading, MediaGenerationStatus::Cancelled, true),
            // PUBLISHING → *
            (MediaGenerationStatus::Publishing, MediaGenerationStatus::Uploading, false),
            (MediaGenerationStatus::Publishing, MediaGenerationStatus::Publishing, false),
            (MediaGenerationStatus::Publishing, MediaGenerationStatus::Completed, true),
            (MediaGenerationStatus::Publishing, MediaGenerationStatus::Failed, true),
            (MediaGenerationStatus::Publishing, MediaGenerationStatus::Cancelled, false),
            // COMPLETED → * (all false)
            (MediaGenerationStatus::Completed, MediaGenerationStatus::Queued, false),
            (MediaGenerationStatus::Completed, MediaGenerationStatus::Interpreting, false),
            (MediaGenerationStatus::Completed, MediaGenerationStatus::Classified, false),
            (MediaGenerationStatus::Completed, MediaGenerationStatus::Generating, false),
            (MediaGenerationStatus::Completed, MediaGenerationStatus::Uploading, false),
            (MediaGenerationStatus::Completed, MediaGenerationStatus::Publishing, false),
            (MediaGenerationStatus::Completed, MediaGenerationStatus::Completed, false),
            (MediaGenerationStatus::Completed, MediaGenerationStatus::Failed, false),
            (MediaGenerationStatus::Completed, MediaGenerationStatus::Cancelled, false),
            // FAILED → * (all false)
            (MediaGenerationStatus::Failed, MediaGenerationStatus::Queued, false),
            (MediaGenerationStatus::Failed, MediaGenerationStatus::Interpreting, false),
            (MediaGenerationStatus::Failed, MediaGenerationStatus::Classified, false),
            (MediaGenerationStatus::Failed, MediaGenerationStatus::Generating, false),
            (MediaGenerationStatus::Failed, MediaGenerationStatus::Uploading, false),
            (MediaGenerationStatus::Failed, MediaGenerationStatus::Publishing, false),
            (MediaGenerationStatus::Failed, MediaGenerationStatus::Completed, false),
            (MediaGenerationStatus::Failed, MediaGenerationStatus::Cancelled, false),
            // CANCELLED → * (all false)
            (MediaGenerationStatus::Cancelled, MediaGenerationStatus::Queued, false),
            (MediaGenerationStatus::Cancelled, MediaGenerationStatus::Interpreting, false),
            (MediaGenerationStatus::Cancelled, MediaGenerationStatus::Classified, false),
            (MediaGenerationStatus::Cancelled, MediaGenerationStatus::Generating, false),
            (MediaGenerationStatus::Cancelled, MediaGenerationStatus::Uploading, false),
            (MediaGenerationStatus::Cancelled, MediaGenerationStatus::Publishing, false),
            (MediaGenerationStatus::Cancelled, MediaGenerationStatus::Completed, false),
            (MediaGenerationStatus::Cancelled, MediaGenerationStatus::Failed, false),
        ];

        for (from, to, expected) in cases {
            assert_eq!(
                from.can_transition(to),
                expected,
                "can_transition({:?} → {:?}) expected {}",
                from, to, expected
            );
        }
    }
}

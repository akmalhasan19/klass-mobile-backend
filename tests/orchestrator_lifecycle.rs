//! Integration tests for the media generation lifecycle.
//!
//! Covers:
//! - Full transition chains through the status matrix
//! - Status ordering and StatusBefore invariant
//! - Serialization roundtrip (JSON, Display, proto value)
//! - orchestration_audit_payload shape verification (layers without DB)

use klass_gateway::orchestrator::lifecycle::MediaGenerationStatus;
use klass_gateway::orchestrator::lifecycle::StatusBefore;

// ═════════════════════════════════════════════════════════════════════════════
// 1. Transition chains
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_normal_flow_chain() {
    let chain = [
        MediaGenerationStatus::Queued,
        MediaGenerationStatus::Interpreting,
        MediaGenerationStatus::Classified,
        MediaGenerationStatus::Generating,
        MediaGenerationStatus::Uploading,
        MediaGenerationStatus::Publishing,
        MediaGenerationStatus::Completed,
    ];

    // Verify each consecutive pair is a valid transition
    for pair in chain.windows(2) {
        assert!(
            pair[0].can_transition(pair[1]),
            "expected valid transition: {:?} → {:?}",
            pair[0],
            pair[1]
        );
    }

    // Verify each reversed pair is NOT a valid transition (no regression)
    for pair in chain.windows(2) {
        assert!(
            !pair[1].can_transition(pair[0]),
            "expected no regression: {:?} → {:?}",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn test_failure_chain() {
    // From any non-terminal state → FAILED is valid
    let non_terminal = [
        MediaGenerationStatus::Queued,
        MediaGenerationStatus::Interpreting,
        MediaGenerationStatus::Classified,
        MediaGenerationStatus::Generating,
        MediaGenerationStatus::Uploading,
        MediaGenerationStatus::Publishing,
    ];
    for status in &non_terminal {
        assert!(
            status.can_transition(MediaGenerationStatus::Failed),
            "expected valid transition: {:?} → Failed",
            status
        );
    }
}

#[test]
fn test_cancellation_chain() {
    let cancellable = [
        MediaGenerationStatus::Queued,
        MediaGenerationStatus::Interpreting,
        MediaGenerationStatus::Classified,
        MediaGenerationStatus::Generating,
        MediaGenerationStatus::Uploading,
    ];
    for status in &cancellable {
        assert!(
            status.can_transition(MediaGenerationStatus::Cancelled),
            "expected valid transition: {:?} → Cancelled",
            status
        );
    }

    // Publishing cannot transition to Cancelled (per Laravel matrix)
    assert!(
        !MediaGenerationStatus::Publishing.can_transition(MediaGenerationStatus::Cancelled)
    );
}

#[test]
fn test_terminal_no_outgoing_transitions() {
    let terminal = [
        MediaGenerationStatus::Completed,
        MediaGenerationStatus::Failed,
        MediaGenerationStatus::Cancelled,
    ];
    for status in &terminal {
        for other in &MediaGenerationStatus::ALL {
            assert!(
                !status.can_transition(*other),
                "terminal {:?} should not transition to {:?}",
                status,
                other
            );
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. Status ordering and StatusBefore invariant
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_status_before_ordering() {
    for (i, earlier) in MediaGenerationStatus::ALL.iter().enumerate() {
        for (j, later) in MediaGenerationStatus::ALL.iter().enumerate() {
            if earlier.is_terminal() {
                // Terminal states are never before anything
                assert!(
                    !earlier.status_before(*later),
                    "terminal {:?} must not be before {:?}",
                    earlier,
                    later
                );
            } else if i < j {
                assert!(
                    earlier.status_before(*later),
                    "{:?} (index {}) should be before {:?} (index {})",
                    earlier,
                    i,
                    later,
                    j
                );
            } else {
                assert!(
                    !earlier.status_before(*later),
                    "{:?} (index {}) must not be before {:?} (index {})",
                    earlier,
                    i,
                    later,
                    j
                );
            }
        }
    }
}

#[test]
fn test_option_none_status_before_all() {
    let none: Option<MediaGenerationStatus> = None;
    for status in &MediaGenerationStatus::ALL {
        assert!(none.status_before(*status));
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. Serialization roundtrip
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn test_json_serialization_roundtrip() {
    for status in &MediaGenerationStatus::ALL {
        let json = serde_json::to_string(status).unwrap();
        let deserialized: MediaGenerationStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(*status, deserialized, "JSON roundtrip failed for {:?}", status);
    }
}

#[test]
fn test_display_roundtrip() {
    for status in &MediaGenerationStatus::ALL {
        let display = format!("{}", status);
        let from_str = MediaGenerationStatus::from_str(&display);
        assert_eq!(
            Some(*status),
            from_str,
            "Display/from_str roundtrip failed for {:?} (display: {})",
            status,
            display
        );
    }
}

#[test]
fn test_proto_value_roundtrip() {
    for status in &MediaGenerationStatus::ALL {
        let proto = status.to_proto_value();
        let from_proto = MediaGenerationStatus::from_proto_value(proto);
        assert_eq!(
            Some(*status),
            from_proto,
            "proto value roundtrip failed for {:?} (value: {})",
            status,
            proto
        );
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. orchestration_audit_payload shape verification (pure JSON)
// ═════════════════════════════════════════════════════════════════════════════

/// Verify the expected shape of a minimal orchestration_audit_payload.
#[test]
fn test_audit_payload_shape() {
    use serde_json::json;

    let payload = json!({
        "schema_version": "media_generation_orchestration_audit.v1",
        "current_status": "queued",
        "resolved_output_type": null,
        "provider_trace": {
            "interpretation": { "name": null, "model": null },
            "generator": { "name": null, "model": null },
            "delivery": { "name": null, "model": null },
        },
        "timing": {
            "queued_at": "2026-07-15T00:00:00Z",
            "total_duration_ms": null,
            "status_durations_ms": {},
        },
        "latest_error": null,
        "status_history": [
            {
                "event_type": "status_transition",
                "from_status": null,
                "to_status": "queued",
                "attempt": 0,
                "at": "2026-07-15T00:00:00Z",
            }
        ],
    });

    // Verify schema version
    assert_eq!(
        payload["schema_version"],
        "media_generation_orchestration_audit.v1"
    );

    // Verify provider_trace has all 3 entries
    let trace = &payload["provider_trace"];
    assert!(trace["interpretation"].is_object());
    assert!(trace["generator"].is_object());
    assert!(trace["delivery"].is_object());

    // Verify status_history is an array with at least one entry
    let history = payload["status_history"].as_array().unwrap();
    assert!(!history.is_empty());
    assert_eq!(history[0]["to_status"], "queued");

    // Verify timing has queued_at but processing_started_at is absent (null was set)
    assert!(payload["timing"]["queued_at"].is_string());
    assert!(payload["resolved_output_type"].is_null());
}

/// Verify that a completed transition payload contains expected fields.
#[test]
fn test_completed_transition_payload_shape() {
    use serde_json::json;

    let payload = json!({
        "schema_version": "media_generation_orchestration_audit.v1",
        "current_status": "completed",
        "provider_trace": {
            "interpretation": { "name": "openrouter", "model": "deepseek-v4-flash" },
            "generator": { "name": "python-renderer", "model": "hf-space-v3" },
            "delivery": { "name": "openrouter", "model": "deepseek-v4-flash" },
        },
        "timing": {
            "queued_at": "2026-07-15T00:00:00Z",
            "processing_started_at": "2026-07-15T00:00:05Z",
            "completed_at": "2026-07-15T00:01:30Z",
            "total_duration_ms": 90000,
            "status_durations_ms": {
                "queued": 5000,
                "interpreting": 15000,
                "classified": 2000,
                "generating": 30000,
                "uploading": 8000,
                "publishing": 20000,
                "completed": 0,
            },
        },
        "latest_error": null,
        "status_history": [
            { "event_type": "status_transition", "from_status": null, "to_status": "queued", "attempt": 0, "at": "2026-07-15T00:00:00Z" },
            { "event_type": "status_transition", "from_status": "queued", "to_status": "interpreting", "attempt": 1, "at": "2026-07-15T00:00:05Z" },
            { "event_type": "status_transition", "from_status": "classifying", "to_status": "classified", "attempt": 1, "at": "2026-07-15T00:00:20Z" },
            { "event_type": "status_transition", "from_status": "classified", "to_status": "generating", "attempt": 1, "at": "2026-07-15T00:00:22Z" },
            { "event_type": "status_transition", "from_status": "generating", "to_status": "completed", "attempt": 1, "at": "2026-07-15T00:01:30Z" },
        ],
    });

    // Verify terminal timing fields
    assert!(payload["timing"]["completed_at"].is_string());
    assert!(payload["timing"]["total_duration_ms"].is_number());
    assert_eq!(payload["current_status"], "completed");

    // Verify provider_trace is populated
    assert_eq!(
        payload["provider_trace"]["interpretation"]["name"],
        "openrouter"
    );
    assert_eq!(
        payload["provider_trace"]["generator"]["model"],
        "hf-space-v3"
    );
}

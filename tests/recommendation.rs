//! Integration tests for the recommendation engine.
//!
//! These tests exercise the pure transformation functions of the recommendation
//! modules — taxonomy inference, personalization, and feed aggregation — using
//! the public API. No database connection is required.
//!
//! Coverage:
//! - Taxonomy inference: snapshot matching, confidence thresholds, edge cases
//! - Personalization: guest vs authenticated contexts, signal sources, fallbacks
//! - Feed aggregation: admin + topic composition, sorting, source status
//! - Distribution summary: filtering, grouping, ordering

use klass_gateway::recommendation::aggregation::{
    build_feed_snapshot, build_system_distribution_summary, AggregationInput,
    AiSourceMetadata, CuratedItemInput, PersonalizationSummary, SubSubjectTaxonomy,
    SystemAssignmentInput, TopicItemInput, SOURCE_ADMIN_UPLOAD,
    SOURCE_AI_GENERATED, SOURCE_SYSTEM_TOPIC,
};
use klass_gateway::recommendation::personalization::{
    resolve, ActivityRow, PersonalizationInput,
    SubSubjectInfo, SubjectInfo, PERSONALIZED_MODE,
};
use klass_gateway::recommendation::taxonomy::SubjectsJsonTaxonomyCatalog;

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Taxonomy Inference
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_taxonomy_snapshot_handout_pecahan_kelas_5_sd() {
    // Plan: Input "handout pecahan kelas 5 SD" → expected subject (math) + sub_subject match
    let result = SubjectsJsonTaxonomyCatalog::infer("handout pecahan kelas 5 sd")
        .expect("Should infer taxonomy for 'handout pecahan kelas 5 sd'");

    assert_eq!(result.schema_version, "media_prompt_taxonomy_inference.v1");

    // Should match Mathematics SD subject
    assert!(
        result.best_match.subject_slug.contains("matematika")
            || result.best_match.subject_slug.contains("math"),
        "Expected math subject slug, got: {}",
        result.best_match.subject_slug
    );

    // Should detect SD jenjang
    assert_eq!(result.best_match.jenjang.as_deref(), Some("SD"));

    // Should detect kelas 5
    assert_eq!(result.best_match.kelas, Some(5));

    // Should have at least one candidate
    assert!(!result.candidate_matches.is_empty());

    // Best candidate should have "pecahan" in sub_subject name
    assert!(
        result
            .best_match
            .sub_subject_name
            .as_deref()
            .map(|n| n.to_lowercase().contains("pecahan"))
            .unwrap_or(false),
        "Expected 'pecahan' in sub_subject name, got: {:?}",
        result.best_match.sub_subject_name
    );

    // Confidence should be at least medium
    assert!(
        result.confidence.score >= 0.25,
        "Confidence score too low: {}",
        result.confidence.score
    );
}

#[test]
fn test_taxonomy_snapshot_modul_ipa_smp_kelas_7() {
    let result = SubjectsJsonTaxonomyCatalog::infer("modul ipa smp kelas 7")
        .expect("Should infer taxonomy for 'modul ipa smp kelas 7'");

    // Should match IPA/Science
    assert_eq!(result.best_match.jenjang.as_deref(), Some("SMP"));
    assert_eq!(result.best_match.kelas, Some(7));

    // Should have candidates
    assert!(!result.candidate_matches.is_empty());
}

#[test]
fn test_taxonomy_snapshot_pdf_matematika_sma_kelas_10() {
    let result = SubjectsJsonTaxonomyCatalog::infer("pdf matematika sma kelas 10")
        .expect("Should infer taxonomy for 'pdf matematika sma kelas 10'");

    assert_eq!(result.best_match.jenjang.as_deref(), Some("SMA"));
    assert_eq!(result.best_match.kelas, Some(10));
    assert!(
        result.best_match.subject_slug.contains("matematika"),
        "Expected matematika slug, got: {}",
        result.best_match.subject_slug
    );
}

#[test]
fn test_taxonomy_confidence_high_for_clear_match() {
    let result = SubjectsJsonTaxonomyCatalog::infer("buatkan handout pecahan kelas 5 sd")
        .expect("Should find match");

    // Clear prompt with grade, subject, and sub-subject should have high confidence
    assert!(
        result.confidence.score >= 0.45,
        "Expected at least medium confidence, got: {}",
        result.confidence.score
    );
}

#[test]
fn test_taxonomy_confidence_threshold_boundary() {
    let result = SubjectsJsonTaxonomyCatalog::infer("materi tentang gaya");
    // "gaya" should match content in IPAS/Physics sub_subjects
    // This prompt has no jenjang, no kelas → should still match via phrase/token overlap
    if let Some(result) = result {
        // Even if it matches, confidence should be tracked
        assert!(result.confidence.score > 0.0);
    }
    // The inference engine may return None for very short/generic prompts — that's OK
}

#[test]
fn test_taxonomy_rejects_gibberish() {
    let result = SubjectsJsonTaxonomyCatalog::infer("asdfghjkl qwerty zxcvbnm");
    assert!(result.is_none(), "Gibberish should not match");
}

#[test]
fn test_taxonomy_rejects_empty_prompt() {
    assert!(SubjectsJsonTaxonomyCatalog::infer("").is_none());
    assert!(SubjectsJsonTaxonomyCatalog::infer("   ").is_none());
}

#[test]
fn test_taxonomy_mixed_case_jenjang_detection() {
    let sd = SubjectsJsonTaxonomyCatalog::infer("matematika SD kelas 5");
    let smp = SubjectsJsonTaxonomyCatalog::infer("IPA SmP kelas 7");
    let sma = SubjectsJsonTaxonomyCatalog::infer("SMA kelas 11 bahasa inggris");
    let smk = SubjectsJsonTaxonomyCatalog::infer("SMK kelas 10 teknik");

    assert_eq!(sd.and_then(|r| r.best_match.jenjang).as_deref(), Some("SD"));
    assert_eq!(smp.and_then(|r| r.best_match.jenjang).as_deref(), Some("SMP"));
    assert_eq!(sma.and_then(|r| r.best_match.jenjang).as_deref(), Some("SMA"));
    assert_eq!(smk.and_then(|r| r.best_match.jenjang).as_deref(), Some("SMK"));
}

#[test]
fn test_taxonomy_exact_class_with_roman() {
    let xii = SubjectsJsonTaxonomyCatalog::infer("kelas XII sma matematika");
    let xi = SubjectsJsonTaxonomyCatalog::infer("KELAS XI sma");

    assert_eq!(xii.and_then(|r| r.best_match.kelas), Some(12));
    assert_eq!(xi.and_then(|r| r.best_match.kelas), Some(11));
}

#[test]
fn test_taxonomy_sub_subject_attached_on_phrase_match() {
    let result = SubjectsJsonTaxonomyCatalog::infer("gaya di sekitar kita kelas 4")
        .expect("Should match 'gaya di sekitar kita'");
    // This is a specific sub_subject slug — should match and attach sub_subject
    assert!(result.confidence.sub_subject_attached, "Should attach sub_subject for exact phrase match");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Personalization Contexts
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_personalization_guest_context() {
    let input = PersonalizationInput::guest();
    let ctx = resolve(&input);

    // Guest has no signals
    assert!(!ctx.public.signals_available);
    assert!(!ctx.public.has_primary_subject);
    assert!(!ctx.public.has_authored_topic_activity);
    assert_eq!(ctx.public.signal_source, "guest");
    assert_eq!(ctx.public.fallback_mode, Some("global_feed"));
    assert!(ctx.public.primary_subject.is_none());
    assert!(ctx.public.subject_anchor.is_none());
    assert!(ctx.public.candidate_sub_subject_ids.is_empty());
    assert!(ctx.public.candidate_sub_subjects.is_empty());

    // Internal state
    assert!(!ctx.internal.signals_available);
    assert!(ctx.internal.primary_subject_id.is_none());
    assert!(ctx.internal.preferred_activity_sub_subject_ids.is_empty());
    assert!(ctx.internal.secondary_activity_sub_subject_ids.is_empty());
    assert_eq!(ctx.internal.personalized_mode, PERSONALIZED_MODE);
}

#[test]
fn test_personalization_guest_serializes_correctly() {
    let input = PersonalizationInput::guest();
    let ctx = resolve(&input);

    // The public context should serialize correctly
    let json = serde_json::to_value(&ctx).expect("Should serialize PersonalizationContext");
    assert_eq!(json["public"]["signal_source"], "guest");
    assert_eq!(json["public"]["fallback_mode"], "global_feed");
    assert_eq!(json["public"]["signals_available"], false);
    assert_eq!(json["internal"]["personalized_mode"], PERSONALIZED_MODE);
}

#[test]
fn test_personalization_authenticated_no_signals() {
    let input = PersonalizationInput {
        user_id: Some(1),
        primary_subject: None,
        authored_topic_activity: vec![],
    };
    let ctx = resolve(&input);

    assert!(!ctx.public.signals_available);
    assert_eq!(ctx.public.signal_source, "insufficient_signals");
    assert_eq!(ctx.public.fallback_mode, Some("global_feed"));
    assert!(ctx.public.subject_anchor.is_none());
}

#[test]
fn test_personalization_authenticated_primary_only() {
    let input = PersonalizationInput {
        user_id: Some(1),
        primary_subject: Some(SubjectInfo {
            id: 10,
            name: "Mathematics".to_string(),
            slug: "mathematics".to_string(),
            source: None,
        }),
        authored_topic_activity: vec![],
    };
    let ctx = resolve(&input);

    assert!(ctx.public.signals_available);
    assert!(ctx.public.has_primary_subject);
    assert!(!ctx.public.has_authored_topic_activity);
    assert_eq!(ctx.public.signal_source, "profile_subject");
    assert_eq!(ctx.public.fallback_mode, Some("primary_subject_catalog"));

    // subject_anchor → from profile
    let anchor = ctx.public.subject_anchor.expect("Should have subject_anchor");
    assert_eq!(anchor.slug, "mathematics");
    assert_eq!(anchor.source.as_deref(), Some("profile"));
}

#[test]
fn test_personalization_authenticated_activity_only() {
    let input = PersonalizationInput {
        user_id: Some(1),
        primary_subject: None,
        authored_topic_activity: vec![
            ActivityRow {
                sub_subject_id: 100,
                subject_id: 20,
                topic_count: 3,
                latest_topic_activity_at: None,
                sub_subject: Some(SubSubjectInfo {
                    id: 100,
                    subject_id: 20,
                    name: "Algebra".to_string(),
                    slug: "algebra".to_string(),
                }),
                subject: Some(SubjectInfo {
                    id: 20,
                    name: "Mathematics".to_string(),
                    slug: "mathematics".to_string(),
                    source: None,
                }),
            },
        ],
    };
    let ctx = resolve(&input);

    assert!(ctx.public.signals_available);
    assert!(!ctx.public.has_primary_subject);
    assert!(ctx.public.has_authored_topic_activity);
    assert_eq!(ctx.public.signal_source, "authored_topic_activity");
    assert!(ctx.public.fallback_mode.is_none());

    // subject_anchor → from first activity
    let anchor = ctx.public.subject_anchor.expect("Should have subject_anchor");
    assert_eq!(anchor.slug, "mathematics");
    assert_eq!(anchor.source.as_deref(), Some("authored_topic_activity"));

    assert_eq!(ctx.public.candidate_sub_subject_ids, vec![100]);
}

#[test]
fn test_personalization_full_signals() {
    // Authenticated with primary subject + authored topic activity
    let input = PersonalizationInput {
        user_id: Some(1),
        primary_subject: Some(SubjectInfo {
            id: 20,
            name: "Science".to_string(),
            slug: "science".to_string(),
            source: None,
        }),
        authored_topic_activity: vec![
            ActivityRow {
                sub_subject_id: 100,
                subject_id: 20,
                topic_count: 5,
                latest_topic_activity_at: None,
                sub_subject: Some(SubSubjectInfo { id: 100, subject_id: 20, name: "Physics".to_string(), slug: "physics".to_string() }),
                subject: Some(SubjectInfo { id: 20, name: "Science".to_string(), slug: "science".to_string(), source: None }),
            },
            ActivityRow {
                sub_subject_id: 101,
                subject_id: 20,
                topic_count: 3,
                latest_topic_activity_at: None,
                sub_subject: Some(SubSubjectInfo { id: 101, subject_id: 20, name: "Chemistry".to_string(), slug: "chemistry".to_string() }),
                subject: Some(SubjectInfo { id: 20, name: "Science".to_string(), slug: "science".to_string(), source: None }),
            },
            ActivityRow {
                sub_subject_id: 200,
                subject_id: 30,
                topic_count: 8,
                latest_topic_activity_at: None,
                sub_subject: Some(SubSubjectInfo { id: 200, subject_id: 30, name: "History".to_string(), slug: "history".to_string() }),
                subject: Some(SubjectInfo { id: 30, name: "History".to_string(), slug: "history".to_string(), source: None }),
            },
        ],
    };
    let ctx = resolve(&input);

    assert_eq!(ctx.public.signal_source, "profile_subject_with_authored_activity");
    // Preferred first (science activity), then secondary (history)
    assert_eq!(ctx.public.candidate_sub_subject_ids, vec![100, 101, 200]);
    assert_eq!(ctx.internal.preferred_activity_sub_subject_ids, vec![100, 101]);
    assert_eq!(ctx.internal.secondary_activity_sub_subject_ids, vec![200]);
}

#[test]
fn test_personalization_fallback_modes() {
    // No signals → global_feed
    let guest = resolve(&PersonalizationInput::guest());
    assert_eq!(guest.public.fallback_mode, Some("global_feed"));

    // Has primary but no activity → primary_subject_catalog
    let primary_only = resolve(&PersonalizationInput {
        user_id: Some(1),
        primary_subject: Some(SubjectInfo { id: 10, name: "M".to_string(), slug: "m".to_string(), source: None }),
        authored_topic_activity: vec![],
    });
    assert_eq!(primary_only.public.fallback_mode, Some("primary_subject_catalog"));

    // Has activity → no fallback needed
    let has_activity = resolve(&PersonalizationInput {
        user_id: Some(1),
        primary_subject: None,
        authored_topic_activity: vec![
            ActivityRow { sub_subject_id: 1, subject_id: 1, topic_count: 1, latest_topic_activity_at: None, sub_subject: None, subject: None },
        ],
    });
    assert!(has_activity.public.fallback_mode.is_none());

    // Has primary + preferred activity → no fallback
    let full = resolve(&PersonalizationInput {
        user_id: Some(1),
        primary_subject: Some(SubjectInfo { id: 1, name: "M".to_string(), slug: "m".to_string(), source: None }),
        authored_topic_activity: vec![
            ActivityRow { sub_subject_id: 10, subject_id: 1, topic_count: 1, latest_topic_activity_at: None, sub_subject: None, subject: None },
        ],
    });
    assert!(full.public.fallback_mode.is_none());
}

#[test]
fn test_personalization_descriptions_vary_by_signals() {
    let empty = resolve(&PersonalizationInput {
        user_id: Some(1),
        primary_subject: None,
        authored_topic_activity: vec![],
    });
    // No signals → guest-like description
    assert!(!empty.internal.personalized_description.is_empty());

    let both = resolve(&PersonalizationInput {
        user_id: Some(1),
        primary_subject: Some(SubjectInfo { id: 1, name: "A".to_string(), slug: "a".to_string(), source: None }),
        authored_topic_activity: vec![
            ActivityRow { sub_subject_id: 10, subject_id: 1, topic_count: 1, latest_topic_activity_at: None, sub_subject: None, subject: None },
        ],
    });
    assert!(both.internal.personalized_description.contains("primary subject and authored-topic activity"));
}

#[test]
fn test_personalization_serialization_roundtrip() {
    let input = PersonalizationInput {
        user_id: Some(42),
        primary_subject: Some(SubjectInfo { id: 10, name: "Math".to_string(), slug: "math".to_string(), source: None }),
        authored_topic_activity: vec![],
    };
    let ctx = resolve(&input);

    // Serialize to JSON and verify key fields
    let json = serde_json::to_value(&ctx).unwrap();
    assert_eq!(json["public"]["signal_source"], "profile_subject");
    assert_eq!(json["public"]["has_primary_subject"], true);
    assert_eq!(json["public"]["primary_subject"]["id"], 10);
    assert_eq!(json["public"]["subject_anchor"]["source"], "profile");
    assert!(json["internal"]["personalized_description"].as_str().unwrap().len() > 10);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Feed Aggregation
// ═══════════════════════════════════════════════════════════════════════════════

fn make_curated(id: &str, source_type: &str, priority: i64, score: f64) -> CuratedItemInput {
    CuratedItemInput {
        id: id.to_string(),
        title: format!("Item {}", id),
        description: None,
        thumbnail_url: None,
        ratio: None,
        project_type: None,
        tags: vec![],
        modules: vec![],
        source_type: source_type.to_string(),
        source_reference: None,
        source_payload: serde_json::json!({ "score": score }),
        display_priority: priority,
        is_active: true,
        starts_at: None,
        ends_at: None,
        created_at: Some("2026-04-03T10:00:00Z".to_string()),
        updated_at: Some("2026-04-03T10:00:00Z".to_string()),
    }
}

fn make_topic(id: &str, sub_subject_id: i64, subject_id: i64) -> TopicItemInput {
    TopicItemInput {
        id: id.to_string(),
        title: format!("Topic {}", id),
        thumbnail_url: None,
        sub_subject_id: Some(sub_subject_id),
        subject_id: Some(subject_id),
        taxonomy: None,
        personalization_eligible: Some(true),
        personalization_mode: None,
        personalization_excluded_reason: None,
        has_normalized_ownership: true,
        modules: vec!["Module 1".to_string()],
        teacher_id: None,
        owner_user_id: None,
        ownership_status: None,
        order: None,
        created_at: Some("2026-04-03T10:00:00Z".to_string()),
        updated_at: Some("2026-04-03T10:00:00Z".to_string()),
    }
}

#[test]
fn test_feed_snapshot_empty() {
    let input = AggregationInput::empty();
    let snapshot = build_feed_snapshot(&input);
    assert!(snapshot.items.is_empty());
    assert!(!snapshot.personalization.applied);
    assert!(snapshot.source_status.contains_key(SOURCE_ADMIN_UPLOAD));
    assert!(snapshot.source_status.contains_key(SOURCE_SYSTEM_TOPIC));
    assert!(snapshot.source_status.contains_key(SOURCE_AI_GENERATED));
}

#[test]
fn test_feed_snapshot_admin_only() {
    let input = AggregationInput {
        curated_items: vec![make_curated("1", SOURCE_ADMIN_UPLOAD, 100, 1.5)],
        ..AggregationInput::empty()
    };
    let snapshot = build_feed_snapshot(&input);
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].source_type, SOURCE_ADMIN_UPLOAD);
    assert_eq!(snapshot.items[0].feed_origin, "admin_curated");
    assert_eq!(
        snapshot.source_status.get(SOURCE_ADMIN_UPLOAD).unwrap().state,
        "ok"
    );
    assert_eq!(
        snapshot.source_status.get(SOURCE_SYSTEM_TOPIC).unwrap().state,
        "empty"
    );
}

#[test]
fn test_feed_snapshot_admin_and_topics() {
    let input = AggregationInput {
        curated_items: vec![make_curated("1", SOURCE_ADMIN_UPLOAD, 100, 1.5)],
        topic_items: vec![make_topic("t1", 10, 20)],
        ..AggregationInput::empty()
    };
    let snapshot = build_feed_snapshot(&input);
    assert_eq!(snapshot.items.len(), 2);
    assert_eq!(snapshot.items[0].source_type, SOURCE_ADMIN_UPLOAD);
    assert_eq!(snapshot.items[1].source_type, SOURCE_SYSTEM_TOPIC);
}

#[test]
fn test_feed_snapshot_suppressed_topics_excluded() {
    let input = AggregationInput {
        topic_items: vec![
            make_topic("visible", 10, 20),
            make_topic("suppressed", 30, 40),
        ],
        suppressed_source_keys: vec![format!("{}:suppressed", SOURCE_SYSTEM_TOPIC)],
        ..AggregationInput::empty()
    };
    let snapshot = build_feed_snapshot(&input);
    assert_eq!(snapshot.items.len(), 1);
    assert!(snapshot.items[0].id.contains("visible"));
}

#[test]
fn test_feed_snapshot_sort_by_priority_then_score() {
    let input = AggregationInput {
        curated_items: vec![
            make_curated("a", SOURCE_ADMIN_UPLOAD, 50, 2.1),
            make_curated("b", SOURCE_ADMIN_UPLOAD, 50, 9.4),
        ],
        ..AggregationInput::empty()
    };
    // Override created_at for deterministic sort
    let mut input = input;
    input.curated_items[0].created_at = Some("2026-04-03T08:00:00Z".to_string());
    input.curated_items[1].created_at = Some("2026-04-03T09:00:00Z".to_string());

    let snapshot = build_feed_snapshot(&input);
    assert_eq!(snapshot.items.len(), 2);
    // Higher score first at same priority
    assert_eq!(snapshot.items[0].id, "b");
    assert_eq!(snapshot.items[1].id, "a");
}

#[test]
fn test_feed_snapshot_composition_with_personalization() {
    // Simulate user with primary subject (20) and preferred activity (sub 10)
    use klass_gateway::recommendation::personalization::*;
    let ctx = PersonalizationContext {
        public: PersonalizationPublic {
            signals_available: true,
            has_primary_subject: true,
            has_authored_topic_activity: true,
            signal_source: "profile_subject_with_authored_activity",
            fallback_mode: None,
            primary_subject: Some(SubjectInfo { id: 20, name: "Science".to_string(), slug: "science".to_string(), source: None }),
            subject_anchor: None,
            candidate_sub_subject_ids: vec![10],
            candidate_sub_subjects: vec![],
        },
        internal: PersonalizationInternal {
            signals_available: true,
            primary_subject_id: Some(20),
            preferred_activity_sub_subject_ids: vec![10],
            secondary_activity_sub_subject_ids: vec![],
            personalized_mode: PERSONALIZED_MODE,
            personalized_description: "Select system-generated recommendations using authenticated signals.",
        },
    };

    let input = AggregationInput {
        topic_items: vec![
            make_topic("preferred", 10, 20),    // preferred (group 0)
            make_topic("other_subject", 30, 40), // not preferred, different subject
        ],
        personalization_context: Some(ctx),
        ..AggregationInput::empty()
    };
    let snapshot = build_feed_snapshot(&input);

    // Only preferred topic should be selected
    assert_eq!(snapshot.items.len(), 1, "Only preferred item should be selected");
    assert!(snapshot.items[0].id.contains("preferred"));
    assert!(snapshot.personalization.applied);
    assert_eq!(snapshot.personalization.selected_system_candidate_count, Some(1));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Distribution Summary
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_distribution_summary_requires_eligible_types() {
    let input = AggregationInput::empty();
    let result = build_system_distribution_summary(&input);
    assert!(result.items.is_empty(), "Should be empty without eligible types");
}

#[test]
fn test_distribution_summary_filters_by_min_user_count() {
    let mut input = AggregationInput::empty();
    input.eligible_source_types = vec![SOURCE_AI_GENERATED.to_string()];
    input.minimum_distinct_user_count = 2;

    input.assignments = vec![
        SystemAssignmentInput {
            source_type: SOURCE_AI_GENERATED.to_string(),
            source_reference: "proj-1".to_string(),
            subject_id: Some(10),
            sub_subject_id: 100,
            distinct_user_count: 1, // Below threshold
            latest_distribution_at: None,
        },
        SystemAssignmentInput {
            source_type: SOURCE_AI_GENERATED.to_string(),
            source_reference: "proj-2".to_string(),
            subject_id: Some(10),
            sub_subject_id: 200,
            distinct_user_count: 3, // Meets threshold
            latest_distribution_at: None,
        },
    ];

    // Need subject lookup for the sub_subject
    input.sub_subject_lookup.insert(
        200,
        SubSubjectTaxonomy {
            id: 200,
            subject_id: 10,
            name: "Sub 200".to_string(),
            slug: "sub-200".to_string(),
            subject: None,
        },
    );
    input.ai_source_lookup.insert(
        "proj-2".to_string(),
        AiSourceMetadata {
            id: "proj-2".to_string(),
            title: "Project 2".to_string(),
            created_at: None,
        },
    );

    let result = build_system_distribution_summary(&input);
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].source_reference, "proj-2");
    assert_eq!(result.items[0].distinct_user_count, 3);
}

#[test]
fn test_distribution_summary_groups_by_sub_subject() {
    let mut input = AggregationInput::empty();
    input.eligible_source_types = vec![SOURCE_AI_GENERATED.to_string()];

    input.assignments = vec![
        SystemAssignmentInput {
            source_type: SOURCE_AI_GENERATED.to_string(),
            source_reference: "proj-a".to_string(),
            subject_id: Some(10),
            sub_subject_id: 100,
            distinct_user_count: 5,
            latest_distribution_at: None,
        },
        SystemAssignmentInput {
            source_type: SOURCE_AI_GENERATED.to_string(),
            source_reference: "proj-b".to_string(),
            subject_id: Some(10),
            sub_subject_id: 100, // Same sub_subject
            distinct_user_count: 3,
            latest_distribution_at: None,
        },
        SystemAssignmentInput {
            source_type: SOURCE_AI_GENERATED.to_string(),
            source_reference: "proj-c".to_string(),
            subject_id: Some(20),
            sub_subject_id: 200, // Different sub_subject
            distinct_user_count: 7,
            latest_distribution_at: None,
        },
    ];

    input.sub_subject_lookup.insert(100, SubSubjectTaxonomy {
        id: 100, subject_id: 10, name: "Sub 100".to_string(), slug: "sub-100".to_string(), subject: None,
    });
    input.sub_subject_lookup.insert(200, SubSubjectTaxonomy {
        id: 200, subject_id: 20, name: "Sub 200".to_string(), slug: "sub-200".to_string(), subject: None,
    });
    input.ai_source_lookup.insert("proj-a".to_string(), AiSourceMetadata { id: "proj-a".to_string(), title: "A".to_string(), created_at: None });
    input.ai_source_lookup.insert("proj-b".to_string(), AiSourceMetadata { id: "proj-b".to_string(), title: "B".to_string(), created_at: None });
    input.ai_source_lookup.insert("proj-c".to_string(), AiSourceMetadata { id: "proj-c".to_string(), title: "C".to_string(), created_at: None });

    input.maximum_items_per_sub_subject = 1; // Only top 1 per sub_subject

    let result = build_system_distribution_summary(&input);
    // Should have 2 items: one per sub_subject (proj-a has higher count for sub 100)
    assert_eq!(result.items.len(), 2, "Should have one item per sub_subject");
}

#[test]
fn test_distribution_summary_empty_with_no_assignments() {
    let mut input = AggregationInput::empty();
    input.eligible_source_types = vec![SOURCE_SYSTEM_TOPIC.to_string()];
    let result = build_system_distribution_summary(&input);
    assert!(result.items.is_empty());
}

#[test]
fn test_feed_snapshot_serialization() {
    let input = AggregationInput {
        curated_items: vec![make_curated("1", SOURCE_ADMIN_UPLOAD, 100, 1.5)],
        ..AggregationInput::empty()
    };
    let snapshot = build_feed_snapshot(&input);

    // The snapshot should serialize to JSON without errors
    let json = serde_json::to_value(&snapshot).expect("Should serialize FeedSnapshot");
    assert!(json["items"].is_array());
    assert_eq!(json["items"][0]["id"], "1");
    assert_eq!(json["items"][0]["source_type"], SOURCE_ADMIN_UPLOAD);
    assert!(json["source_status"].is_object());
    assert!(json["personalization"].is_object());

    // Personalization should not include fields with defaults
    assert!(json["personalization"].get("applied").is_none(), "applied=false should be skipped");
    assert!(json["personalization"].get("filter_applied").is_none(), "filter_applied=false should be skipped");
    assert!(json["personalization"].get("mode").is_none(), "mode=None should be skipped");
}

#[test]
fn test_personalization_summary_serialization_with_values() {
    let summary = PersonalizationSummary {
        applied: true,
        filter_applied: true,
        mode: Some("personalized_system_candidate_selection".to_string()),
        description: Some("Test".to_string()),
        selected_system_candidate_count: Some(3),
        filtered_out_system_candidate_count: Some(2),
        matched_system_topic_count: Some(1),
        selected_source_breakdown: None,
        matched_sub_subject_ids: Some(vec![10, 20, 30]),
    };

    let json = serde_json::to_value(&summary).expect("Should serialize");
    assert_eq!(json["applied"], true);
    assert_eq!(json["mode"], "personalized_system_candidate_selection");
    assert_eq!(json["matched_sub_subject_ids"], serde_json::json!([10, 20, 30]));
}

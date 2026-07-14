//! Recommendation personalization service.
//! Port of `App\\Services\\RecommendationPersonalizationService` from Laravel.
//!
//! This module is a **pure data transformation** layer — it takes pre-resolved
//! input (user profile + authored-topic activity) and produces a
//! `PersonalizationContext` with public-facing and internal sections.
//! The caller (e.g. aggregation service or homepage-recommendations handler)
//! is responsible for querying the DB and preparing the `PersonalizationInput`.

use serde::Serialize;

// ─── Input types ────────────────────────────────────────────────────────────

/// Input data needed to resolve a personalization context.
pub struct PersonalizationInput {
    /// Authenticated user ID, or `None` for guests.
    pub user_id: Option<i64>,
    /// The user's primary subject (from `users.primary_subject_id`), or `None`.
    pub primary_subject: Option<SubjectInfo>,
    /// Pre-resolved authored topic activity, grouped by sub_subject.
    /// An empty vec means no authored-topic activity.
    pub authored_topic_activity: Vec<ActivityRow>,
}

impl PersonalizationInput {
    /// Create an empty input representing a guest user.
    pub fn guest() -> Self {
        Self {
            user_id: None,
            primary_subject: None,
            authored_topic_activity: vec![],
        }
    }
}

/// Lightweight subject representation.
#[derive(Debug, Clone, Serialize)]
pub struct SubjectInfo {
    pub id: i64,
    pub name: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Lightweight sub-subject representation.
#[derive(Debug, Clone, Serialize)]
pub struct SubSubjectInfo {
    pub id: i64,
    pub subject_id: i64,
    pub name: String,
    pub slug: String,
}

/// A single row from the authored-topic activity aggregation query.
/// Pre-computed by the caller from `topics` + `sub_subjects` join.
#[derive(Debug, Clone)]
pub struct ActivityRow {
    pub sub_subject_id: i64,
    pub subject_id: i64,
    pub topic_count: i64,
    pub latest_topic_activity_at: Option<String>,
    pub sub_subject: Option<SubSubjectInfo>,
    pub subject: Option<SubjectInfo>,
}

// ─── Output types ───────────────────────────────────────────────────────────

/// Full personalization context returned by `resolve()`.
/// Mirrors the PHP `{public: ..., internal: ...}` structure.
#[derive(Debug, Clone, Serialize)]
pub struct PersonalizationContext {
    pub public: PersonalizationPublic,
    pub internal: PersonalizationInternal,
}

/// Public-facing personalization fields (sent in API responses).
#[derive(Debug, Clone, Serialize)]
pub struct PersonalizationPublic {
    pub signals_available: bool,
    pub has_primary_subject: bool,
    pub has_authored_topic_activity: bool,
    pub signal_source: &'static str,
    pub fallback_mode: Option<&'static str>,
    pub primary_subject: Option<SubjectInfo>,
    pub subject_anchor: Option<SubjectInfo>,
    pub candidate_sub_subject_ids: Vec<i64>,
    pub candidate_sub_subjects: Vec<CandidateSubject>,
}

/// Internal personalization fields (used by the aggregation service).
#[derive(Debug, Clone, Serialize)]
pub struct PersonalizationInternal {
    pub signals_available: bool,
    pub primary_subject_id: Option<i64>,
    pub preferred_activity_sub_subject_ids: Vec<i64>,
    pub secondary_activity_sub_subject_ids: Vec<i64>,
    pub personalized_mode: &'static str,
    pub personalized_description: &'static str,
}

/// A candidate sub-subject with its activity stats, serialised for API output.
#[derive(Debug, Clone, Serialize)]
pub struct CandidateSubject {
    pub sub_subject_id: i64,
    pub subject_id: i64,
    pub topic_count: i64,
    pub latest_topic_activity_at: Option<String>,
    pub subject: Option<SubjectInfo>,
    pub sub_subject: Option<SubSubjectInfo>,
    pub source: &'static str,
}

// ─── Constants ──────────────────────────────────────────────────────────────

pub const PERSONALIZED_MODE: &str = "personalized_system_candidate_selection";

// ─── Public API ─────────────────────────────────────────────────────────────

/// Resolve a personalization context from the given input.
///
/// * **Guest** (no user) → returns guest context with `global_feed` fallback.
/// * **Authenticated** → computes preferred/secondary sub_subject buckets
///   based on primary subject and authored-topic activity.
pub fn resolve(input: &PersonalizationInput) -> PersonalizationContext {
    match input.user_id {
        None => guest_context(),
        Some(_) => authenticated_context(input),
    }
}

// ─── Guest ──────────────────────────────────────────────────────────────────

fn guest_context() -> PersonalizationContext {
    PersonalizationContext {
        public: PersonalizationPublic {
            signals_available: false,
            has_primary_subject: false,
            has_authored_topic_activity: false,
            signal_source: "guest",
            fallback_mode: Some("global_feed"),
            primary_subject: None,
            subject_anchor: None,
            candidate_sub_subject_ids: vec![],
            candidate_sub_subjects: vec![],
        },
        internal: PersonalizationInternal {
            signals_available: false,
            primary_subject_id: None,
            preferred_activity_sub_subject_ids: vec![],
            secondary_activity_sub_subject_ids: vec![],
            personalized_mode: PERSONALIZED_MODE,
            personalized_description:
                "Select and order system-generated recommendations using authenticated user personalization signals.",
        },
    }
}

// ─── Authenticated ──────────────────────────────────────────────────────────

fn authenticated_context(input: &PersonalizationInput) -> PersonalizationContext {
    let has_primary_subject = input.primary_subject.is_some();
    let has_authored_activity = !input.authored_topic_activity.is_empty();
    let signals_available = has_primary_subject || has_authored_activity;

    // Split activity into preferred (matching primary subject) and secondary
    let primary_subject_id = input.primary_subject.as_ref().map(|s| s.id);

    let preferred: Vec<&ActivityRow> = match &input.primary_subject {
        Some(ps) => input
            .authored_topic_activity
            .iter()
            .filter(|r| r.subject_id == ps.id)
            .collect(),
        None => input.authored_topic_activity.iter().collect(),
    };

    let secondary: Vec<&ActivityRow> = match &input.primary_subject {
        Some(ps) => input
            .authored_topic_activity
            .iter()
            .filter(|r| r.subject_id != ps.id)
            .collect(),
        None => vec![],
    };

    // Ordered candidate list: preferred first, then secondary
    let candidate_rows: Vec<&ActivityRow> = preferred
        .iter()
        .chain(secondary.iter())
        .copied()
        .collect();

    // subject_anchor: primary subject (profile source), or fallback to first activity's subject
    let subject_anchor = match &input.primary_subject {
        Some(ps) => Some(SubjectInfo {
            id: ps.id,
            name: ps.name.clone(),
            slug: ps.slug.clone(),
            source: Some("profile".to_string()),
        }),
        None => {
            if has_authored_activity {
                candidate_rows.first().and_then(|r| r.subject.as_ref()).map(|s| SubjectInfo {
                    id: s.id,
                    name: s.name.clone(),
                    slug: s.slug.clone(),
                    source: Some("authored_topic_activity".to_string()),
                })
            } else {
                None
            }
        }
    };

    let signal_source = resolve_signal_source(has_primary_subject, has_authored_activity);
    let fallback_mode = resolve_fallback_mode(has_primary_subject, has_authored_activity, &preferred);
    let personalized_description = resolve_personalized_description(has_primary_subject, has_authored_activity);

    let candidate_sub_subjects: Vec<CandidateSubject> = candidate_rows
        .iter()
        .map(|r| CandidateSubject {
            sub_subject_id: r.sub_subject_id,
            subject_id: r.subject_id,
            topic_count: r.topic_count,
            latest_topic_activity_at: r.latest_topic_activity_at.clone(),
            subject: r.subject.clone(),
            sub_subject: r.sub_subject.clone(),
            source: "authored_topic_activity",
        })
        .collect();

    let candidate_sub_subject_ids: Vec<i64> = candidate_sub_subjects
        .iter()
        .map(|c| c.sub_subject_id)
        .collect();

    PersonalizationContext {
        public: PersonalizationPublic {
            signals_available,
            has_primary_subject,
            has_authored_topic_activity: has_authored_activity,
            signal_source,
            fallback_mode,
            primary_subject: input.primary_subject.clone(),
            subject_anchor,
            candidate_sub_subject_ids,
            candidate_sub_subjects,
        },
        internal: PersonalizationInternal {
            signals_available,
            primary_subject_id,
            preferred_activity_sub_subject_ids: preferred.iter().map(|r| r.sub_subject_id).collect(),
            secondary_activity_sub_subject_ids: secondary.iter().map(|r| r.sub_subject_id).collect(),
            personalized_mode: PERSONALIZED_MODE,
            personalized_description,
        },
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn resolve_signal_source(has_primary_subject: bool, has_authored_activity: bool) -> &'static str {
    match (has_primary_subject, has_authored_activity) {
        (true, true) => "profile_subject_with_authored_activity",
        (true, false) => "profile_subject",
        (false, true) => "authored_topic_activity",
        (false, false) => "insufficient_signals",
    }
}

fn resolve_fallback_mode(
    has_primary_subject: bool,
    has_authored_activity: bool,
    preferred: &[&ActivityRow],
) -> Option<&'static str> {
    if !has_primary_subject && !has_authored_activity {
        return Some("global_feed");
    }
    if has_primary_subject && preferred.is_empty() {
        return Some("primary_subject_catalog");
    }
    None
}

fn resolve_personalized_description(has_primary_subject: bool, has_authored_activity: bool) -> &'static str {
    match (has_primary_subject, has_authored_activity) {
        (true, true) => {
            "Select and order system-generated recommendations using the user primary subject and authored-topic activity."
        }
        (true, false) => {
            "Select and order system-generated recommendations using the user primary subject when authored-topic activity is still sparse."
        }
        _ => {
            "Select and order system-generated recommendations using authored-topic activity because no primary subject is set."
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_subject(id: i64, name: &str, slug: &str) -> SubjectInfo {
        SubjectInfo {
            id,
            name: name.to_string(),
            slug: slug.to_string(),
            source: None,
        }
    }

    fn make_activity(
        sub_subject_id: i64,
        subject_id: i64,
        topic_count: i64,
        subject_name: &str,
        subject_slug: &str,
    ) -> ActivityRow {
        ActivityRow {
            sub_subject_id,
            subject_id,
            topic_count,
            latest_topic_activity_at: None,
            sub_subject: Some(SubSubjectInfo {
                id: sub_subject_id,
                subject_id,
                name: format!("Subj {}", sub_subject_id),
                slug: format!("subj-{}", sub_subject_id),
            }),
            subject: Some(SubjectInfo {
                id: subject_id,
                name: subject_name.to_string(),
                slug: subject_slug.to_string(),
                source: None,
            }),
        }
    }

    // ── Guest ───────────────────────────────────────────────────────────────

    #[test]
    fn test_guest_context() {
        let ctx = resolve(&PersonalizationInput::guest());

        assert!(!ctx.public.signals_available);
        assert!(!ctx.public.has_primary_subject);
        assert!(!ctx.public.has_authored_topic_activity);
        assert_eq!(ctx.public.signal_source, "guest");
        assert_eq!(ctx.public.fallback_mode, Some("global_feed"));
        assert!(ctx.public.primary_subject.is_none());
        assert!(ctx.public.subject_anchor.is_none());
        assert!(ctx.public.candidate_sub_subject_ids.is_empty());
        assert!(ctx.public.candidate_sub_subjects.is_empty());

        assert!(!ctx.internal.signals_available);
        assert!(ctx.internal.primary_subject_id.is_none());
        assert!(ctx.internal.preferred_activity_sub_subject_ids.is_empty());
        assert!(ctx.internal.secondary_activity_sub_subject_ids.is_empty());
        assert_eq!(ctx.internal.personalized_mode, PERSONALIZED_MODE);
    }

    // ── Authenticated, no activity, no primary subject ──────────────────────

    #[test]
    fn test_authenticated_no_signals() {
        let input = PersonalizationInput {
            user_id: Some(1),
            primary_subject: None,
            authored_topic_activity: vec![],
        };
        let ctx = resolve(&input);

        assert!(!ctx.public.signals_available);
        assert!(!ctx.public.has_primary_subject);
        assert!(!ctx.public.has_authored_topic_activity);
        assert_eq!(ctx.public.signal_source, "insufficient_signals");
        assert_eq!(ctx.public.fallback_mode, Some("global_feed"));
        assert!(ctx.public.subject_anchor.is_none());
    }

    // ── Authenticated, primary subject only, no activity ────────────────────

    #[test]
    fn test_authenticated_primary_subject_only() {
        let input = PersonalizationInput {
            user_id: Some(1),
            primary_subject: Some(make_subject(10, "Mathematics", "mathematics")),
            authored_topic_activity: vec![],
        };
        let ctx = resolve(&input);

        assert!(ctx.public.signals_available);
        assert!(ctx.public.has_primary_subject);
        assert!(!ctx.public.has_authored_topic_activity);
        assert_eq!(ctx.public.signal_source, "profile_subject");
        assert_eq!(ctx.public.fallback_mode, Some("primary_subject_catalog"));

        // subject_anchor should be the primary subject
        let anchor = ctx.public.subject_anchor.unwrap();
        assert_eq!(anchor.slug, "mathematics");
        assert_eq!(anchor.source, Some("profile".to_string()));

        assert!(ctx.public.candidate_sub_subject_ids.is_empty());
    }

    // ── Authenticated, activity only, no primary subject ────────────────────

    #[test]
    fn test_authenticated_activity_only() {
        let input = PersonalizationInput {
            user_id: Some(1),
            primary_subject: None,
            authored_topic_activity: vec![
                make_activity(100, 20, 3, "Science", "science"),
                make_activity(101, 30, 2, "History", "history"),
            ],
        };
        let ctx = resolve(&input);

        assert!(ctx.public.signals_available);
        assert!(!ctx.public.has_primary_subject);
        assert!(ctx.public.has_authored_topic_activity);
        assert_eq!(ctx.public.signal_source, "authored_topic_activity");
        assert!(ctx.public.fallback_mode.is_none());

        // subject_anchor should come from first activity's subject
        let anchor = ctx.public.subject_anchor.unwrap();
        assert_eq!(anchor.slug, "science");
        assert_eq!(anchor.source, Some("authored_topic_activity".to_string()));

        assert_eq!(ctx.public.candidate_sub_subject_ids, vec![100, 101]);
    }

    // ── Authenticated with primary subject + activity ───────────────────────

    #[test]
    fn test_authenticated_full_signals() {
        let input = PersonalizationInput {
            user_id: Some(1),
            primary_subject: Some(make_subject(20, "Science", "science")),
            authored_topic_activity: vec![
                make_activity(100, 20, 5, "Science", "science"),   // preferred
                make_activity(101, 20, 3, "Science", "science"),   // preferred
                make_activity(200, 30, 8, "History", "history"),   // secondary
            ],
        };
        let ctx = resolve(&input);

        assert!(ctx.public.signals_available);
        assert!(ctx.public.has_primary_subject);
        assert!(ctx.public.has_authored_topic_activity);
        assert_eq!(ctx.public.signal_source, "profile_subject_with_authored_activity");

        // Order: preferred first (science: 100, 101), then secondary (history: 200)
        assert_eq!(ctx.public.candidate_sub_subject_ids, vec![100, 101, 200]);

        // Preferred internal
        assert_eq!(ctx.internal.preferred_activity_sub_subject_ids, vec![100, 101]);
        // Secondary internal
        assert_eq!(ctx.internal.secondary_activity_sub_subject_ids, vec![200]);
    }

    // ── Subject anchor from primary subject (with source = "profile") ───────

    #[test]
    fn test_subject_anchor_from_primary_subject() {
        let input = PersonalizationInput {
            user_id: Some(1),
            primary_subject: Some(make_subject(10, "Math", "mathematics")),
            authored_topic_activity: vec![
                make_activity(100, 10, 3, "Math", "mathematics"),
            ],
        };
        let ctx = resolve(&input);

        let anchor = ctx.public.subject_anchor.unwrap();
        assert_eq!(anchor.slug, "mathematics");
        assert_eq!(anchor.source, Some("profile".to_string()));
    }

    // ── Fallback mode: global_feed ──────────────────────────────────────────

    #[test]
    fn test_fallback_mode_global_feed() {
        let input = PersonalizationInput {
            user_id: Some(1),
            primary_subject: None,
            authored_topic_activity: vec![],
        };
        let ctx = resolve(&input);
        assert_eq!(ctx.public.fallback_mode, Some("global_feed"));
    }

    // ── Fallback mode: primary_subject_catalog ──────────────────────────────

    #[test]
    fn test_fallback_mode_primary_subject_catalog() {
        let input = PersonalizationInput {
            user_id: Some(1),
            primary_subject: Some(make_subject(10, "Math", "math")),
            authored_topic_activity: vec![],
        };
        let ctx = resolve(&input);
        assert_eq!(ctx.public.fallback_mode, Some("primary_subject_catalog"));
    }

    // ── No fallback when signals are present ────────────────────────────────

    #[test]
    fn test_no_fallback_when_signals_present() {
        let input = PersonalizationInput {
            user_id: Some(1),
            primary_subject: None,
            authored_topic_activity: vec![make_activity(100, 20, 1, "Science", "science")],
        };
        let ctx = resolve(&input);
        assert!(ctx.public.fallback_mode.is_none());
    }

    // ── Personalization descriptions ────────────────────────────────────────

    #[test]
    fn test_personalized_descriptions() {
        let both = resolve(&PersonalizationInput {
            user_id: Some(1),
            primary_subject: Some(make_subject(1, "A", "a")),
            authored_topic_activity: vec![make_activity(10, 1, 1, "A", "a")],
        });
        assert!(both.internal.personalized_description.contains("primary subject and authored-topic activity"));

        let primary_only = resolve(&PersonalizationInput {
            user_id: Some(1),
            primary_subject: Some(make_subject(1, "A", "a")),
            authored_topic_activity: vec![],
        });
        assert!(primary_only.internal.personalized_description.contains("authored-topic activity is still sparse"));

        let activity_only = resolve(&PersonalizationInput {
            user_id: Some(1),
            primary_subject: None,
            authored_topic_activity: vec![make_activity(10, 1, 1, "A", "a")],
        });
        assert!(activity_only.internal.personalized_description.contains("no primary subject is set"));
    }
}

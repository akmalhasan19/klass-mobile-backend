//! Recommendation aggregation service.
//! Port of `App\\Services\\RecommendationAggregationService` from Laravel.
//!
//! Pure data transformation layer — all DB data is pre-resolved and passed
//! via `AggregationInput`. No DB calls within this module.

use serde::{Deserialize, Serialize};

use crate::recommendation::personalization::PersonalizationContext;

// ─── Source type constants ─────────────────────────────────────────────────

pub const SOURCE_ADMIN_UPLOAD: &str = "admin_upload";
pub const SOURCE_SYSTEM_TOPIC: &str = "system_topic";
pub const SOURCE_AI_GENERATED: &str = "ai_generated";

const FEED_ORIGIN_ADMIN: &str = "admin_curated";
const FEED_ORIGIN_SYSTEM: &str = "system_generated";

// ─── Input types ────────────────────────────────────────────────────────────

/// All pre-resolved data needed for aggregation.
pub struct AggregationInput {
    /// Admin-curated and persisted system-generated RecommendedProjects.
    pub curated_items: Vec<CuratedItemInput>,
    /// All suppressed source keys (source_type:source_reference) from persisted
    /// non-admin items plus AI-generated items with linked topic_ids.
    pub suppressed_source_keys: Vec<String>,
    /// Published topics with their sub_subject/subject taxonomy and contents.
    pub topic_items: Vec<TopicItemInput>,
    /// System recommendation assignments for distribution summary.
    pub assignments: Vec<SystemAssignmentInput>,
    /// Lookup: subject + sub_subject metadata keyed by sub_subject_id.
    pub sub_subject_lookup: std::collections::HashMap<i64, SubSubjectTaxonomy>,
    /// Lookup: ai_generated RecommendedProject metadata keyed by id.
    pub ai_source_lookup: std::collections::HashMap<String, AiSourceMetadata>,
    /// Lookup: system_topic override RecommendedProjects keyed by source_reference.
    pub topic_override_lookup: std::collections::HashMap<String, TopicOverrideMetadata>,
    /// Optional personalization context from RecommendationPersonalizationService.
    pub personalization_context: Option<PersonalizationContext>,
    /// Minimum distinct user count for distribution summary (default 2).
    pub minimum_distinct_user_count: i64,
    /// Maximum items per sub_subject in distribution summary (default 1).
    pub maximum_items_per_sub_subject: usize,
    /// Eligible source types for distribution summary.
    pub eligible_source_types: Vec<String>,
}

impl AggregationInput {
    pub fn empty() -> Self {
        Self {
            curated_items: vec![],
            suppressed_source_keys: vec![],
            topic_items: vec![],
            assignments: vec![],
            sub_subject_lookup: std::collections::HashMap::new(),
            ai_source_lookup: std::collections::HashMap::new(),
            topic_override_lookup: std::collections::HashMap::new(),
            personalization_context: None,
            minimum_distinct_user_count: 2,
            maximum_items_per_sub_subject: 1,
            eligible_source_types: vec![],
        }
    }
}

/// A pre-resolved RecommendedProject row.
#[derive(Debug, Clone)]
pub struct CuratedItemInput {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub ratio: Option<String>,
    pub project_type: Option<String>,
    pub tags: Vec<String>,
    pub modules: Vec<String>,
    pub source_type: String,
    pub source_reference: Option<String>,
    pub source_payload: serde_json::Value,
    pub display_priority: i64,
    pub is_active: bool,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// A pre-resolved published Topic with its sub_subject/subject and contents.
#[derive(Debug, Clone)]
pub struct TopicItemInput {
    pub id: String,
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub sub_subject_id: Option<i64>,
    pub subject_id: Option<i64>,
    pub taxonomy: Option<TaxonomyInfo>,
    pub personalization_eligible: Option<bool>,
    pub personalization_mode: Option<String>,
    pub personalization_excluded_reason: Option<String>,
    pub has_normalized_ownership: bool,
    pub modules: Vec<String>,
    pub teacher_id: Option<String>,
    pub owner_user_id: Option<i64>,
    pub ownership_status: Option<String>,
    pub order: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Taxonomy info for a topic/subject.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TaxonomyInfo {
    pub subject: Option<SubjectTaxonomy>,
    pub sub_subject: SubSubjectTaxonomy,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SubjectTaxonomy {
    pub id: i64,
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SubSubjectTaxonomy {
    pub id: i64,
    pub subject_id: i64,
    pub name: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<SubjectTaxonomy>,
}

/// Metadata for an AI-generated source in distribution summary.
#[derive(Debug, Clone)]
pub struct AiSourceMetadata {
    pub id: String,
    pub title: String,
    pub created_at: Option<String>,
}

/// Metadata for a system topic override in distribution summary.
#[derive(Debug, Clone)]
pub struct TopicOverrideMetadata {
    pub id: String,
    pub title: String,
    pub created_at: Option<String>,
}

/// A system recommendation assignment row, pre-aggregated by source.
#[derive(Debug, Clone)]
pub struct SystemAssignmentInput {
    pub source_type: String,
    pub source_reference: String,
    pub subject_id: Option<i64>,
    pub sub_subject_id: i64,
    pub distinct_user_count: i64,
    pub latest_distribution_at: Option<String>,
}

// ─── Output types ───────────────────────────────────────────────────────────

/// Result of `build_feed_snapshot()`.
#[derive(Debug, Clone, Serialize)]
pub struct FeedSnapshot {
    pub items: Vec<FeedItem>,
    pub source_status: std::collections::HashMap<String, SourceStatusInfo>,
    pub personalization: PersonalizationSummary,
}

/// A single item in the feed, normalized for API output.
#[derive(Debug, Clone, Serialize)]
pub struct FeedItem {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub ratio: String,
    pub project_type: Option<String>,
    pub tags: Vec<String>,
    pub modules: Vec<String>,
    pub sub_subject_id: Option<i64>,
    pub subject_id: Option<i64>,
    pub taxonomy: Option<TaxonomyInfo>,
    pub personalization: Option<PersonalizationInfo>,
    pub source_type: String,
    pub source_reference: Option<String>,
    pub feed_origin: &'static str,
    pub display_priority: i64,
    pub score: f64,
    pub visibility: VisibilityInfo,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    #[serde(skip)]
    pub candidate_selection: CandidateSelection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalizationInfo {
    pub eligible: bool,
    pub mode: Option<String>,
    pub excluded_reason: Option<String>,
    pub has_normalized_ownership: bool,
    pub has_adequate_taxonomy: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct VisibilityInfo {
    pub is_active: bool,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
}

/// Internal candidate selection metadata (not serialised).
#[derive(Debug, Clone)]
pub struct CandidateSelection {
    pub eligible: bool,
    pub selected: bool,
    pub group: u8,
    pub rank: usize,
    pub reason: &'static str,
}

impl Default for CandidateSelection {
    fn default() -> Self {
        Self {
            eligible: false,
            selected: false,
            group: 4,
            rank: usize::MAX,
            reason: "normalization_required",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceStatusInfo {
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppressed_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct PersonalizationSummary {
    #[serde(skip_serializing_if = "is_false")]
    pub applied: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub filter_applied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_system_candidate_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filtered_out_system_candidate_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_system_topic_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_source_breakdown: Option<std::collections::HashMap<String, usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_sub_subject_ids: Option<Vec<i64>>,
}

/// A single item in the system distribution summary.
#[derive(Debug, Clone, Serialize)]
pub struct DistributionSummaryItem {
    pub recommendation_key: String,
    pub recommendation_item_id: String,
    pub title: String,
    pub source_type: String,
    pub source_reference: String,
    pub subject_id: Option<i64>,
    pub sub_subject_id: i64,
    pub subject: Option<SubjectTaxonomy>,
    pub sub_subject: Option<SubSubjectTaxonomy>,
    pub distinct_user_count: i64,
    pub latest_distribution_at: Option<String>,
    pub source_created_at: Option<String>,
}

/// Result of `build_system_distribution_summary()`.
#[derive(Debug, Clone, Serialize)]
pub struct DistributionSummaryResult {
    pub items: Vec<DistributionSummaryItem>,
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Build a feed snapshot from pre-resolved input.
pub fn build_feed_snapshot(input: &AggregationInput) -> FeedSnapshot {
    // 1. Separate curated items: admin vs persisted system-generated
    let admin_items: Vec<FeedItem> = input
        .curated_items
        .iter()
        .filter(|c| c.source_type == SOURCE_ADMIN_UPLOAD)
        .map(|c| normalize_curated_item(c, None, input))
        .collect();

    let persisted_system_items: Vec<FeedItem> = input
        .curated_items
        .iter()
        .filter(|c| c.source_type != SOURCE_ADMIN_UPLOAD)
        .map(|c| normalize_curated_item(c, None, input))
        .collect();

    // 2. Get normalized topic items (reject suppressed)
    let suppressed_lookup: std::collections::HashSet<String> =
        input.suppressed_source_keys.iter().cloned().collect();
    let topic_items: Vec<FeedItem> = input
        .topic_items
        .iter()
        .filter(|t| {
            let key = make_source_key(SOURCE_SYSTEM_TOPIC, &t.id);
            !suppressed_lookup.contains(&key)
        })
        .map(|t| normalize_topic_item(t))
        .collect();

    // 3. Merge persisted system items + topic items
    let system_items: Vec<FeedItem> = persisted_system_items
        .into_iter()
        .chain(topic_items)
        .collect();

    // 4. Apply personalization candidate selection
    let (selected_system_items, summary) = select_system_generated_candidates(&system_items, input);

    // 5. Combine admin + selected system items and sort
    let mut all_items: Vec<FeedItem> = admin_items
        .into_iter()
        .chain(selected_system_items)
        .collect();
    all_items.sort_by(compare_items);

    // 6. Brief source_status
    let curated_admin_count = input
        .curated_items
        .iter()
        .filter(|c| c.source_type == SOURCE_ADMIN_UPLOAD)
        .count();
    let curated_ai_count = input
        .curated_items
        .iter()
        .filter(|c| c.source_type == SOURCE_AI_GENERATED)
        .count();

    let mut source_status = std::collections::HashMap::new();
    source_status.insert(
        SOURCE_ADMIN_UPLOAD.to_string(),
        SourceStatusInfo {
            state: resolve_state_from_count(curated_admin_count),
            suppressed_count: None,
        },
    );
    source_status.insert(
        SOURCE_SYSTEM_TOPIC.to_string(),
        SourceStatusInfo {
            state: resolve_state_from_count(input.topic_items.len()),
            suppressed_count: Some(input.suppressed_source_keys.len()),
        },
    );
    source_status.insert(
        SOURCE_AI_GENERATED.to_string(),
        SourceStatusInfo {
            state: resolve_state_from_count(curated_ai_count),
            suppressed_count: None,
        },
    );

    FeedSnapshot {
        items: all_items,
        source_status,
        personalization: summary,
    }
}

/// Build system distribution summary from pre-resolved input.
pub fn build_system_distribution_summary(input: &AggregationInput) -> DistributionSummaryResult {
    if input.eligible_source_types.is_empty() {
        return DistributionSummaryResult { items: vec![] };
    }

    let eligible_set: std::collections::HashSet<&str> =
        input.eligible_source_types.iter().map(|s| s.as_str()).collect();
    let min_count = input.minimum_distinct_user_count.max(1);

    // Filter assignments
    let candidates: Vec<&SystemAssignmentInput> = input
        .assignments
        .iter()
        .filter(|a| {
            eligible_set.contains(a.source_type.as_str())
                && a.distinct_user_count >= min_count
        })
        .collect();

    if candidates.is_empty() {
        return DistributionSummaryResult { items: vec![] };
    }

    // Build distribution items with metadata
    let mut items: Vec<DistributionSummaryItem> = candidates
        .iter()
        .filter_map(|a| {
            let sub_subject = input.sub_subject_lookup.get(&a.sub_subject_id)?;
            let subject = sub_subject.subject.as_ref();
            let (item_id, title, source_created_at) =
                resolve_distribution_metadata(a, input);

            Some(DistributionSummaryItem {
                recommendation_key: make_source_key(&a.source_type, &a.source_reference),
                recommendation_item_id: item_id,
                title,
                source_type: a.source_type.clone(),
                source_reference: a.source_reference.clone(),
                subject_id: subject.as_ref().map(|s| s.id).or(a.subject_id),
                sub_subject_id: a.sub_subject_id,
                subject: subject.cloned(),
                sub_subject: Some(sub_subject.clone()),
                distinct_user_count: a.distinct_user_count,
                latest_distribution_at: a.latest_distribution_at.clone(),
                source_created_at,
            })
        })
        .collect();

    // Group by sub_subject, take top N per group, then sort globally
    items = group_and_sort_distribution(items, input.maximum_items_per_sub_subject);

    DistributionSummaryResult { items }
}

// ─── Normalization ──────────────────────────────────────────────────────────

fn normalize_curated_item(
    curated: &CuratedItemInput,
    _personalization_ctx: Option<&PersonalizationContext>,
    _input: &AggregationInput,
) -> FeedItem {
    let source_type = &curated.source_type;
    let feed_origin = if source_type == SOURCE_ADMIN_UPLOAD {
        FEED_ORIGIN_ADMIN
    } else {
        FEED_ORIGIN_SYSTEM
    };
    let score = curated
        .source_payload
        .get("score")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    FeedItem {
        id: curated.id.clone(),
        title: curated.title.clone(),
        description: curated.description.clone(),
        thumbnail_url: curated.thumbnail_url.clone(),
        ratio: curated.ratio.clone().unwrap_or_else(|| "16:9".to_string()),
        project_type: curated.project_type.clone(),
        tags: curated.tags.clone(),
        modules: curated.modules.clone(),
        sub_subject_id: curated
            .source_payload
            .get("sub_subject_id")
            .and_then(|v| v.as_i64()),
        subject_id: curated
            .source_payload
            .get("subject_id")
            .and_then(|v| v.as_i64()),
        taxonomy: curated
            .source_payload
            .get("taxonomy")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        personalization: curated
            .source_payload
            .get("personalization")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        source_type: source_type.clone(),
        source_reference: curated.source_reference.clone(),
        feed_origin,
        display_priority: curated.display_priority,
        score,
        visibility: VisibilityInfo {
            is_active: curated.is_active,
            starts_at: curated.starts_at.clone(),
            ends_at: curated.ends_at.clone(),
        },
        created_at: curated.created_at.clone(),
        updated_at: curated.updated_at.clone(),
        candidate_selection: CandidateSelection::default(),
    }
}

fn normalize_topic_item(topic: &TopicItemInput) -> FeedItem {
    let modules = topic.modules.clone();

    let personalization_info = PersonalizationInfo {
        eligible: topic.personalization_eligible.unwrap_or(true),
        mode: topic.personalization_mode.clone(),
        excluded_reason: topic.personalization_excluded_reason.clone(),
        has_normalized_ownership: topic.has_normalized_ownership,
        has_adequate_taxonomy: topic.sub_subject_id.is_some(),
    };

    FeedItem {
        id: format!("system_topic_{}", topic.id),
        title: topic.title.clone(),
        description: None,
        thumbnail_url: topic.thumbnail_url.clone(),
        ratio: "16:9".to_string(),
        project_type: None,
        tags: vec![],
        modules: modules.clone(),
        sub_subject_id: topic.sub_subject_id,
        subject_id: topic.subject_id,
        taxonomy: topic.taxonomy.clone(),
        personalization: Some(personalization_info),
        source_type: SOURCE_SYSTEM_TOPIC.to_string(),
        source_reference: Some(topic.id.clone()),
        feed_origin: FEED_ORIGIN_SYSTEM,
        display_priority: 0,
        score: 0.0,
        visibility: VisibilityInfo {
            is_active: true,
            starts_at: None,
            ends_at: None,
        },
        created_at: topic.created_at.clone(),
        updated_at: topic.updated_at.clone(),
        candidate_selection: CandidateSelection::default(),
    }
}

// ─── Candidate selection ────────────────────────────────────────────────────

fn select_system_generated_candidates(
    items: &[FeedItem],
    input: &AggregationInput,
) -> (Vec<FeedItem>, PersonalizationSummary) {
    if items.is_empty() {
        return (items.to_vec(), empty_personalization_summary());
    }

    let ctx = match &input.personalization_context {
        Some(c) if c.internal.signals_available => c,
        _ => return (items.to_vec(), empty_personalization_summary()),
    };

    let primary_subject_id = ctx.internal.primary_subject_id;
    let preferred_lookup = build_rank_lookup(&ctx.internal.preferred_activity_sub_subject_ids);
    let secondary_lookup = build_rank_lookup(&ctx.internal.secondary_activity_sub_subject_ids);
    let excluded_ids: Vec<i64> = ctx
        .internal
        .preferred_activity_sub_subject_ids
        .iter()
        .chain(ctx.internal.secondary_activity_sub_subject_ids.iter())
        .copied()
        .collect();
    let catalog_lookup = build_primary_subject_catalog_lookup(items, primary_subject_id, &excluded_ids);

    let annotated: Vec<FeedItem> = items
        .iter()
        .map(|item| {
            let mut annotated = item.clone();
            let is_eligible = is_eligible_system_generated_candidate(item);

            let mut selection = CandidateSelection {
                eligible: is_eligible,
                selected: false,
                group: if is_eligible { 3 } else { 4 },
                rank: usize::MAX,
                reason: if is_eligible {
                    "global_feed_fallback"
                } else {
                    "normalization_required"
                },
            };

            if is_eligible {
                let sub_subject_id = item.sub_subject_id;
                let subject_id = item.subject_id;

                if let Some(ss_id) = sub_subject_id {
                    if let Some(&rank) = preferred_lookup.get(&ss_id) {
                        selection = CandidateSelection {
                            eligible: true,
                            selected: true,
                            group: 0,
                            rank,
                            reason: "authored_topic_activity",
                        };
                    } else if let Some(ps_id) = primary_subject_id {
                        if subject_id == Some(ps_id) && catalog_lookup.contains_key(&ss_id) {
                            let rank = catalog_lookup[&ss_id];
                            selection = CandidateSelection {
                                eligible: true,
                                selected: true,
                                group: 1,
                                rank,
                                reason: "primary_subject_catalog",
                            };
                        }
                    }

                    // If still not selected by primary catalog, check secondary
                    if !selection.selected {
                        if let Some(&rank) = secondary_lookup.get(&ss_id) {
                            selection = CandidateSelection {
                                eligible: true,
                                selected: true,
                                group: 2,
                                rank,
                                reason: "secondary_authored_topic_activity",
                            };
                        }
                    }
                }
            }

            annotated.candidate_selection = selection;
            annotated
        })
        .collect();

    let selected: Vec<FeedItem> = annotated
        .iter()
        .filter(|item| item.candidate_selection.selected)
        .cloned()
        .collect();

    if selected.is_empty() {
        return (items.to_vec(), empty_personalization_summary());
    }

    let summary = build_personalization_summary(&annotated, &selected, ctx);

    (selected, summary)
}

fn is_eligible_system_generated_candidate(item: &FeedItem) -> bool {
    if item.source_type == SOURCE_ADMIN_UPLOAD {
        return false;
    }

    if item.sub_subject_id.is_none() || item.subject_id.is_none() {
        return false;
    }

    if item.source_type == SOURCE_SYSTEM_TOPIC {
        if let Some(ref p) = item.personalization {
            return p.eligible;
        }
        return true;
    }

    true
}

fn build_rank_lookup(ids: &[i64]) -> std::collections::HashMap<i64, usize> {
    let mut lookup = std::collections::HashMap::new();
    for (index, id) in ids.iter().enumerate() {
        lookup.insert(*id, index);
    }
    lookup
}

fn build_primary_subject_catalog_lookup(
    items: &[FeedItem],
    primary_subject_id: Option<i64>,
    excluded_ids: &[i64],
) -> std::collections::HashMap<i64, usize> {
    let ps_id = match primary_subject_id {
        Some(id) => id,
        None => return std::collections::HashMap::new(),
    };

    let excluded_set: std::collections::HashSet<i64> = excluded_ids.iter().copied().collect();

    // Filter: eligible, matching subject_id, not excluded
    let mut candidates: Vec<(i64, i64, Option<&str>)> = items
        .iter()
        .filter(|item| is_eligible_system_generated_candidate(item))
        .filter(|item| item.subject_id == Some(ps_id))
        .filter(|item| {
            item.sub_subject_id
                .map_or(true, |ss_id| !excluded_set.contains(&ss_id))
        })
        .filter_map(|item| item.sub_subject_id.map(|ss_id| (ss_id, item)))
        .map(|(ss_id, item)| {
            (
                ss_id,
                item.display_priority,
                item.updated_at.as_deref(),
            )
        })
        .collect();

    // Sort: by sub_subject_id ascending (stable default for catalog)
    candidates.sort_by_key(|&(ss_id, _, _)| ss_id);

    let lookup: std::collections::HashMap<i64, usize> = candidates
        .into_iter()
        .enumerate()
        .map(|(idx, (ss_id, _, _))| (ss_id, idx))
        .collect();

    lookup
}

// ─── Personalization summary ────────────────────────────────────────────────

fn build_personalization_summary(
    all: &[FeedItem],
    selected: &[FeedItem],
    ctx: &PersonalizationContext,
) -> PersonalizationSummary {
    let mut summary = PersonalizationSummary {
        applied: !selected.is_empty(),
        filter_applied: !selected.is_empty(),
        selected_system_candidate_count: Some(selected.len()),
        filtered_out_system_candidate_count: Some(all.len().saturating_sub(selected.len())),
        matched_system_topic_count: Some(
            selected
                .iter()
                .filter(|i| i.source_type == SOURCE_SYSTEM_TOPIC)
                .count(),
        ),
        mode: Some(ctx.internal.personalized_mode.to_string()),
        description: Some(ctx.internal.personalized_description.to_string()),
        selected_source_breakdown: None,
        matched_sub_subject_ids: None,
    };

    if !selected.is_empty() {
        // Source breakdown
        let mut breakdown = std::collections::HashMap::new();
        breakdown.insert(
            SOURCE_SYSTEM_TOPIC.to_string(),
            selected
                .iter()
                .filter(|i| i.source_type == SOURCE_SYSTEM_TOPIC)
                .count(),
        );
        breakdown.insert(
            SOURCE_AI_GENERATED.to_string(),
            selected
                .iter()
                .filter(|i| i.source_type == SOURCE_AI_GENERATED)
                .count(),
        );
        summary.selected_source_breakdown = Some(breakdown);

        // Matched sub_subject_ids (sorted by selection group+rank)
        let mut ordered: Vec<&FeedItem> = selected.iter().collect();
        ordered.sort_by(|a, b| {
            a.candidate_selection
                .group
                .cmp(&b.candidate_selection.group)
                .then_with(|| a.candidate_selection.rank.cmp(&b.candidate_selection.rank))
                .then_with(|| {
                    let a_ts = timestamp_value(a.updated_at.as_deref());
                    let b_ts = timestamp_value(b.updated_at.as_deref());
                    b_ts.cmp(&a_ts)
                })
        });
        // Preserve selection-priority order (PHP uses ->unique()->values())
        let mut seen = std::collections::HashSet::new();
        let ids: Vec<i64> = ordered
            .iter()
            .filter_map(|i| i.sub_subject_id)
            .filter(|id| seen.insert(*id))
            .collect();
        summary.matched_sub_subject_ids = Some(ids);
    }

    summary
}

fn empty_personalization_summary() -> PersonalizationSummary {
    PersonalizationSummary {
        applied: false,
        filter_applied: false,
        mode: None,
        description: None,
        selected_system_candidate_count: None,
        filtered_out_system_candidate_count: None,
        matched_system_topic_count: None,
        selected_source_breakdown: None,
        matched_sub_subject_ids: None,
    }
}

// ─── Comparison / sorting ───────────────────────────────────────────────────

fn compare_items(a: &FeedItem, b: &FeedItem) -> std::cmp::Ordering {
    // System-generated items with candidate_selection are sorted by group/rank first
    let a_is_system = a.source_type != SOURCE_ADMIN_UPLOAD;
    let b_is_system = b.source_type != SOURCE_ADMIN_UPLOAD;

    if a_is_system && b_is_system {
        let cmp = a
            .candidate_selection
            .group
            .cmp(&b.candidate_selection.group)
            .then_with(|| a.candidate_selection.rank.cmp(&b.candidate_selection.rank))
            .then_with(|| {
                let a_ts = timestamp_value(a.updated_at.as_deref());
                let b_ts = timestamp_value(b.updated_at.as_deref());
                b_ts.cmp(&a_ts)
            });
        if cmp != std::cmp::Ordering::Equal {
            return cmp;
        }
    }

    b.display_priority
        .cmp(&a.display_priority)
        .then_with(|| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| {
            let a_ts = timestamp_value(a.created_at.as_deref());
            let b_ts = timestamp_value(b.created_at.as_deref());
            b_ts.cmp(&a_ts)
        })
        .then_with(|| a.id.cmp(&b.id))
}

// ─── Distribution summary helpers ───────────────────────────────────────────

fn resolve_distribution_metadata(
    assignment: &SystemAssignmentInput,
    input: &AggregationInput,
) -> (String, String, Option<String>) {
    if assignment.source_type == SOURCE_AI_GENERATED {
        if let Some(meta) = input.ai_source_lookup.get(&assignment.source_reference) {
            return (meta.id.clone(), meta.title.clone(), meta.created_at.clone());
        }
        return (
            assignment.source_reference.clone(),
            format!("{}:{}", assignment.source_type, assignment.source_reference),
            None,
        );
    }

    if assignment.source_type == SOURCE_SYSTEM_TOPIC {
        // Check if there's an override first
        if let Some(override_meta) = input.topic_override_lookup.get(&assignment.source_reference) {
            return (
                override_meta.id.clone(),
                override_meta.title.clone(),
                override_meta.created_at.clone(),
            );
        }
        let item_id = format!("system_topic_{}", assignment.source_reference);
        return (item_id, assignment.source_reference.clone(), None);
    }

    (
        assignment.source_reference.clone(),
        format!("{}:{}", assignment.source_type, assignment.source_reference),
        None,
    )
}

fn group_and_sort_distribution(
    items: Vec<DistributionSummaryItem>,
    max_per_sub_subject: usize,
) -> Vec<DistributionSummaryItem> {
    use std::collections::HashMap;

    // Group by sub_subject_id
    let mut grouped: HashMap<i64, Vec<DistributionSummaryItem>> = HashMap::new();
    for item in items {
        grouped
            .entry(item.sub_subject_id)
            .or_default()
            .push(item);
    }

    // Sort within each group, take top N
    let mut flattened: Vec<DistributionSummaryItem> = grouped
        .into_values()
        .map(|mut group| {
            group.sort_by(compare_distribution_candidates);
            group.truncate(max_per_sub_subject);
            group
        })
        .flatten()
        .collect();

    // Global sort: by subject_id → sub_subject_id → distribution tie-breakers
    flattened.sort_by(|a, b| {
        let a_subject_id = a.subject.as_ref().map(|s| s.id).unwrap_or(0);
        let b_subject_id = b.subject.as_ref().map(|s| s.id).unwrap_or(0);
        a_subject_id
            .cmp(&b_subject_id)
            .then_with(|| a.sub_subject_id.cmp(&b.sub_subject_id))
            .then_with(|| compare_distribution_candidates(a, b))
    });

    flattened
}

fn compare_distribution_candidates(
    a: &DistributionSummaryItem,
    b: &DistributionSummaryItem,
) -> std::cmp::Ordering {
    // Tie-breakers: distinct_user_count DESC, latest_distribution_at DESC,
    // source_created_at DESC, source_reference ASC
    b.distinct_user_count
        .cmp(&a.distinct_user_count)
        .then_with(|| {
            let a_ts = timestamp_value(a.latest_distribution_at.as_deref());
            let b_ts = timestamp_value(b.latest_distribution_at.as_deref());
            b_ts.cmp(&a_ts)
        })
        .then_with(|| {
            let a_ts = timestamp_value(a.source_created_at.as_deref());
            let b_ts = timestamp_value(b.source_created_at.as_deref());
            b_ts.cmp(&a_ts)
        })
        .then_with(|| a.source_reference.cmp(&b.source_reference))
}

// ─── Utility functions ──────────────────────────────────────────────────────

fn make_source_key(source_type: &str, source_reference: &str) -> String {
    format!("{}:{}", source_type, source_reference)
}

fn timestamp_value(value: Option<&str>) -> i64 {
    value
        .and_then(|s| {
            if s.is_empty() {
                None
            } else {
                // Try to parse as RFC 3339 / ISO 8601
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.timestamp())
                    .or_else(|| {
                        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
                            .ok()
                            .map(|dt| dt.and_utc().timestamp())
                    })
            }
        })
        .unwrap_or(0)
}

fn resolve_state_from_count(count: usize) -> &'static str {
    if count > 0 {
        "ok"
    } else {
        "empty"
    }
}

fn is_false(v: &bool) -> bool {
    !*v
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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

    fn make_person_ctx(
        primary_subject_id: Option<i64>,
        preferred: Vec<i64>,
        secondary: Vec<i64>,
    ) -> PersonalizationContext {
        use crate::recommendation::personalization::*;
        let has_primary = primary_subject_id.is_some();
        let has_activity = !preferred.is_empty() || !secondary.is_empty();
        PersonalizationContext {
            public: PersonalizationPublic {
                signals_available: has_primary || has_activity,
                has_primary_subject: has_primary,
                has_authored_topic_activity: has_activity,
                signal_source: if has_primary && has_activity {
                    "profile_subject_with_authored_activity"
                } else if has_primary {
                    "profile_subject"
                } else if has_activity {
                    "authored_topic_activity"
                } else {
                    "insufficient_signals"
                },
                fallback_mode: None,
                primary_subject: primary_subject_id.map(|id| SubjectInfo {
                    id,
                    name: "Science".to_string(),
                    slug: "science".to_string(),
                    source: None,
                }),
                subject_anchor: None,
                candidate_sub_subject_ids: preferred.iter().chain(secondary.iter()).copied().collect(),
                candidate_sub_subjects: vec![],
            },
            internal: PersonalizationInternal {
                signals_available: has_primary || has_activity,
                primary_subject_id,
                preferred_activity_sub_subject_ids: preferred,
                secondary_activity_sub_subject_ids: secondary,
                personalized_mode: PERSONALIZED_MODE,
                personalized_description:
                    "Select and order system-generated recommendations using authenticated user personalization signals.",
            },
        }
    }

    // ── Basic feed snapshot ─────────────────────────────────────────────

    #[test]
    fn test_empty_feed() {
        let input = AggregationInput::empty();
        let snapshot = build_feed_snapshot(&input);
        assert!(snapshot.items.is_empty());
        assert!(!snapshot.personalization.applied);
    }

    #[test]
    fn test_only_admin_curated_items() {
        let input = AggregationInput {
            curated_items: vec![make_curated("1", SOURCE_ADMIN_UPLOAD, 100, 1.5)],
            ..AggregationInput::empty()
        };
        let snapshot = build_feed_snapshot(&input);
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].source_type, SOURCE_ADMIN_UPLOAD);
        assert_eq!(snapshot.items[0].feed_origin, "admin_curated");
    }

    // ── Admin + topic items ──────────────────────────────────────────────

    #[test]
    fn test_admin_and_topic_items() {
        let input = AggregationInput {
            curated_items: vec![make_curated("1", SOURCE_ADMIN_UPLOAD, 100, 1.5)],
            topic_items: vec![make_topic("t1", 10, 20)],
            ..AggregationInput::empty()
        };
        let snapshot = build_feed_snapshot(&input);
        assert_eq!(snapshot.items.len(), 2);
        // Admin items come first (higher display_priority)
        assert_eq!(snapshot.items[0].source_type, SOURCE_ADMIN_UPLOAD);
        assert_eq!(snapshot.items[1].source_type, SOURCE_SYSTEM_TOPIC);
    }

    // ── Suppressed topics ────────────────────────────────────────────────

    #[test]
    fn test_suppressed_topic_excluded() {
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
        assert_eq!(snapshot.items[0].id, "system_topic_visible");
    }

    // ── Personalization: preferred sub_subject selected ───────────────────

    #[test]
    fn test_preferred_sub_subject_selected() {
        let ctx = make_person_ctx(Some(20), vec![10], vec![]);
        let input = AggregationInput {
            topic_items: vec![
                make_topic("t1", 10, 20), // preferred (subject 20, sub 10)
                make_topic("t2", 30, 40), // other subject
            ],
            personalization_context: Some(ctx),
            ..AggregationInput::empty()
        };
        let snapshot = build_feed_snapshot(&input);
        // Only preferred item should be selected
        assert_eq!(snapshot.items.len(), 1);
        assert!(snapshot.items[0].id.contains("t1"));
        assert!(snapshot.personalization.applied);
        assert!(snapshot.personalization.filter_applied);
    }

    // ── No personalization context → all items pass through ───────────────

    #[test]
    fn test_no_personalization_all_items_pass() {
        let input = AggregationInput {
            topic_items: vec![make_topic("t1", 10, 20), make_topic("t2", 30, 40)],
            personalization_context: None,
            ..AggregationInput::empty()
        };
        let snapshot = build_feed_snapshot(&input);
        assert_eq!(snapshot.items.len(), 2);
        assert!(!snapshot.personalization.applied);
    }

    // ── Distribution summary ──────────────────────────────────────────────

    #[test]
    fn test_distribution_summary_empty() {
        let input = AggregationInput::empty();
        let result = build_system_distribution_summary(&input);
        assert!(result.items.is_empty());
    }

    #[test]
    fn test_distribution_summary_filters_by_min_count() {
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
                distinct_user_count: 2, // Meets threshold
                latest_distribution_at: None,
            },
        ];
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
    }

    // ── Sorting ──────────────────────────────────────────────────────────

    #[test]
    fn test_sort_by_priority_then_score_then_created_at() {
        let input = AggregationInput {
            curated_items: vec![
                make_curated("a", SOURCE_ADMIN_UPLOAD, 50, 2.1),
                make_curated("b", SOURCE_ADMIN_UPLOAD, 50, 9.4),
            ],
            ..AggregationInput::empty()
        };
        // Override created_at
        let mut input = input;
        input.curated_items[0].created_at = Some("2026-04-03T08:00:00Z".to_string());
        input.curated_items[1].created_at = Some("2026-04-03T09:00:00Z".to_string());

        let snapshot = build_feed_snapshot(&input);
        assert_eq!(snapshot.items.len(), 2);
        // Higher score first (9.4 > 2.1 at same priority)
        assert_eq!(snapshot.items[0].id, "b");
        assert_eq!(snapshot.items[1].id, "a");
    }

    // ── Source status ────────────────────────────────────────────────────

    #[test]
    fn test_source_status_reflects_counts() {
        let input = AggregationInput {
            curated_items: vec![make_curated("1", SOURCE_ADMIN_UPLOAD, 100, 1.0)],
            topic_items: vec![make_topic("t1", 10, 20)],
            suppressed_source_keys: vec!["system_topic:old".to_string()],
            ..AggregationInput::empty()
        };
        let snapshot = build_feed_snapshot(&input);
        assert_eq!(
            snapshot.source_status.get(SOURCE_ADMIN_UPLOAD).unwrap().state,
            "ok"
        );
        assert_eq!(
            snapshot.source_status.get(SOURCE_SYSTEM_TOPIC).unwrap().state,
            "ok"
        );
        assert_eq!(
            snapshot
                .source_status
                .get(SOURCE_SYSTEM_TOPIC)
                .unwrap()
                .suppressed_count,
            Some(1)
        );
    }

    // ── Candidate selection grouping ─────────────────────────────────────

    #[test]
    fn test_preferred_overrides_secondary() {
        let ctx = make_person_ctx(Some(20), vec![10], vec![30]);
        let input = AggregationInput {
            topic_items: vec![
                make_topic("pref", 10, 20),   // preferred sub 10
                make_topic("sec", 30, 20),    // secondary sub 30 (same subject, not preferred)
                make_topic("other", 50, 99),  // other subject, not selected
            ],
            personalization_context: Some(ctx),
            ..AggregationInput::empty()
        };
        let snapshot = build_feed_snapshot(&input);
        // Both preferred and secondary should be selected
        assert_eq!(snapshot.items.len(), 2);
        // Preferred (group 0) comes before secondary (group 2)
        assert_eq!(snapshot.items[0].candidate_selection.group, 0);
        assert_eq!(snapshot.items[1].candidate_selection.group, 2);
    }
}

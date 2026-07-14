//! Homepage Recommendations API handler.
//! `GET /api/v1/homepage-recommendations`
//!
//! Phase 4 implementation with full aggregation pipeline:
//! 1. Load HomepageSection by key `project_recommendations`
//! 2. Resolve optional user (Bearer header)
//! 3. If section disabled → return empty collection + context meta
//! 4. Otherwise build AggregationInput → build_feed_snapshot → track assignments

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::auth::middleware::OptionalPrincipal;
use crate::db::repositories::assignments::{
    default_trackable_source_types, PgSystemRecommendationAssignmentsRepo,
    SystemRecommendationAssignmentsRepo, TrackableFeedItem,
};
use crate::db::repositories::homepage_sections::{HomepageSectionsRepo, PgHomepageSectionsRepo};
use crate::db::repositories::recommended_projects::{
    RecommendedProjectsRepo, PgRecommendedProjectsRepo, SOURCE_ADMIN_UPLOAD,
    SOURCE_AI_GENERATED, SOURCE_SYSTEM_TOPIC,
};
use crate::db::repositories::users::{PgUsersRepo, UsersRepo};
use crate::error::{AppError, AppResult};
use crate::recommendation::aggregation::{
    build_feed_snapshot, AggregationInput, CuratedItemInput, FeedItem,
    PersonalizationSummary, SubjectTaxonomy, SubSubjectTaxonomy, SystemAssignmentInput,
    TaxonomyInfo, TopicItemInput,
};
use crate::recommendation::personalization::{
    resolve, ActivityRow, PersonalizationInput, SubjectInfo, SubSubjectInfo,
};
use crate::state::AppState;

// ─── Query params ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct HomepageRecommendationsQuery {
    limit: Option<usize>,
}

// ─── Output resources ────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct RecommendedProjectResource {
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
    pub personalization: Option<PersonalizationResource>,
    pub source_type: String,
    pub source_reference: Option<String>,
    pub feed_origin: String,
    pub display_priority: i64,
    pub visibility: VisibilityResource,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Serialize)]
pub struct PersonalizationResource {
    pub eligible: bool,
    pub mode: Option<String>,
    pub excluded_reason: Option<String>,
    pub has_normalized_ownership: bool,
    pub has_adequate_taxonomy: bool,
}

#[derive(Serialize)]
pub struct VisibilityResource {
    pub is_active: bool,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
}

#[derive(Serialize)]
pub struct SectionMeta {
    pub key: String,
    pub label: Option<String>,
    pub enabled: bool,
    pub position: i32,
    pub endpoint: String,
    pub admin_configurator_path: String,
}

#[derive(Serialize)]
pub struct LimitMeta {
    pub requested: Option<usize>,
    pub applied: usize,
}

#[derive(Serialize)]
pub struct PersonalizationMeta {
    pub policy_version: String,
    pub audience: String,
    pub mode: String,
    pub tracks_assignments: bool,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persona: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct Meta {
    pub total: usize,
    pub section: SectionMeta,
    pub limit: LimitMeta,
    pub personalization: PersonalizationMeta,
    pub source_status: std::collections::HashMap<String, SourceStatusResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personalization_detail: Option<PersonalizationSummary>,
}

#[derive(Serialize)]
pub struct SourceStatusResource {
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppressed_count: Option<usize>,
}

#[derive(Serialize)]
pub struct Response {
    pub data: Vec<RecommendedProjectResource>,
    pub meta: Meta,
}

// ─── Handler ─────────────────────────────────────────────────────────────────

/// `GET /api/v1/homepage-recommendations`
pub async fn index(
    State(state): State<AppState>,
    OptionalPrincipal(principal): OptionalPrincipal,
    Query(query): Query<HomepageRecommendationsQuery>,
) -> AppResult<(StatusCode, Json<Response>)> {
    let moment = Utc::now();

    // 1. Load HomepageSection
    let homepage_repo = PgHomepageSectionsRepo::new(state.db_pool.clone());
    let section = homepage_repo
        .find_by_key("project_recommendations")
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch homepage section: {e}")))?;

    let section_meta = SectionMeta {
        key: "project_recommendations".to_string(),
        label: section.as_ref().map(|s| s.label.clone()),
        enabled: section.as_ref().map(|s| s.is_enabled).unwrap_or(false),
        position: section.as_ref().map(|s| s.position).unwrap_or(0),
        endpoint: "/api/v1/homepage-recommendations".to_string(),
        admin_configurator_path: "/admin/homepage-sections".to_string(),
    };

    // 2. Build personalization context (optional auth)
    let is_authenticated = principal.is_some();
    let personalization_ctx = build_personalization_context(
        &state,
        principal.as_ref().map(|p| p.user_id),
    )
    .await?;

    // Determine audience label
    let audience = if is_authenticated { "authenticated" } else { "guest" };

    // 3. If section disabled → return empty
    let is_enabled = section.as_ref().map(|s| s.is_enabled).unwrap_or(false);
    if !is_enabled {
        return Ok(empty_response(query.limit, section_meta, audience));
    }

    // 4. Query DB data for AggregationInput
    let projects_repo = PgRecommendedProjectsRepo::new(state.db_pool.clone());
    let assignments_repo = PgSystemRecommendationAssignmentsRepo::new(state.db_pool.clone());

    // 4a. Visible recommended projects
    let visible_projects = projects_repo
        .find_visible()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch recommended projects: {e}")))?;

    // 4b. Published topics with taxonomy
    let topic_items = query_published_topics(&state, &moment).await?;

    // 4c. Sub_subject + subject lookup
    let (sub_subject_lookup, _subject_lookup) = query_subject_lookups(&state).await?;

    // 4d. Build curated items
    let curated_items: Vec<CuratedItemInput> = visible_projects
        .iter()
        .map(|p| map_curated_item(p))
        .collect();

    // 4e. Suppressed source keys from persisted non-admin items
    let suppressed_source_keys: Vec<String> = curated_items
        .iter()
        .filter(|c| {
            c.source_type != SOURCE_ADMIN_UPLOAD
                || (c.source_type == SOURCE_AI_GENERATED
                    && c.source_reference.is_some())
        })
        .filter_map(|c| {
            c.source_reference
                .as_ref()
                .map(|sr| format!("{}:{}", c.source_type, sr))
        })
        .collect();

    // 4f. Distribution summary assignments
    let eligible_source_types = vec![
        SOURCE_SYSTEM_TOPIC.to_string(),
        SOURCE_AI_GENERATED.to_string(),
    ];
    let distribution_rows = assignments_repo
        .find_distribution_summary(&eligible_source_types, 2)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch distribution summary: {e}")))?;

    let assignments: Vec<SystemAssignmentInput> = distribution_rows
        .into_iter()
        .map(|r| SystemAssignmentInput {
            source_type: r.source_type,
            source_reference: r.source_reference,
            subject_id: r.subject_id,
            sub_subject_id: r.sub_subject_id.unwrap_or(0),
            distinct_user_count: r.distinct_user_count,
            latest_distribution_at: r.latest_distribution_at.map(|dt| dt.to_rfc3339()),
        })
        .collect();

    // 4g. Build aggregation input
    let aggregation_input = AggregationInput {
        curated_items,
        suppressed_source_keys,
        topic_items,
        assignments,
        sub_subject_lookup,
        ai_source_lookup: std::collections::HashMap::new(), // populated when AI-generated items exist
        topic_override_lookup: std::collections::HashMap::new(), // populated when topic overrides exist
        personalization_context: personalization_ctx.clone(),
        minimum_distinct_user_count: 2,
        maximum_items_per_sub_subject: 1,
        eligible_source_types,
    };

    // 5. Build feed snapshot
    let snapshot = build_feed_snapshot(&aggregation_input);

    // 6. Track served assignments (only for authenticated users)
    if let Some(ref principal) = principal {
        let feed_items: Vec<TrackableFeedItem> = snapshot
            .items
            .iter()
            .map(|item| TrackableFeedItem {
                id: item.id.clone(),
                source_type: item.source_type.clone(),
                source_reference: item.source_reference.clone(),
                source_payload_topic_id: None,
                source_payload_source_reference: None,
                subject_id: item.subject_id,
                sub_subject_id: item.sub_subject_id,
            })
            .collect();

        let trackable_types = default_trackable_source_types();

        // Fire-and-forget tracking — log errors but don't fail the request
        if let Err(e) = assignments_repo
            .track_served(principal.user_id, &feed_items, &trackable_types, &moment)
            .await
        {
            tracing::warn!(
                user_id = principal.user_id,
                error = %e,
                "Failed to track served recommendations"
            );
        }
    }

    // 7. Build response
    let limit = query.limit.unwrap_or(50).min(50);
    let applied_items: Vec<&FeedItem> = snapshot.items.iter().take(limit).collect();

    let personalization_mode = snapshot.personalization.mode.clone()
        .unwrap_or_else(|| "default_global_feed".to_string());
    let personalization_description = snapshot.personalization.description.clone()
        .unwrap_or_else(|| {
            if is_authenticated {
                "Serve the current safe mixed homepage feed when subject profile or authored-topic signals are still insufficient."
            } else {
                "Guests remain on the current non-personalized homepage feed until an authenticated context exists."
            }.to_string()
        });

    let data: Vec<RecommendedProjectResource> = applied_items
        .iter()
        .map(|item| map_feed_item_to_resource(item))
        .collect();
    let data_len = data.len();

    // Build source_status from snapshot
    let mut source_status = std::collections::HashMap::new();
    for (key, info) in &snapshot.source_status {
        source_status.insert(
            key.clone(),
            SourceStatusResource {
                state: info.state.to_string(),
                suppressed_count: info.suppressed_count,
            },
        );
    }
    // Ensure all three source types are present
    for st in &[SOURCE_ADMIN_UPLOAD, SOURCE_SYSTEM_TOPIC, SOURCE_AI_GENERATED] {
        source_status.entry(st.to_string()).or_insert(SourceStatusResource {
            state: "not_evaluated".to_string(),
            suppressed_count: None,
        });
    }

    let response = Response {
        data,
        meta: Meta {
            total: snapshot.items.len(),
            section: section_meta,
            limit: LimitMeta {
                requested: query.limit,
                applied: data_len,
            },
            personalization: PersonalizationMeta {
                policy_version: "phase_4_2_assignment_tracking_deduplication".to_string(),
                audience: audience.to_string(),
                mode: personalization_mode,
                tracks_assignments: is_authenticated,
                description: personalization_description,
                persona: build_persona_json(&snapshot.personalization),
            },
            source_status,
            personalization_detail: if snapshot.personalization.applied {
                Some(snapshot.personalization)
            } else {
                None
            },
        },
    };

    Ok((StatusCode::OK, Json(response)))
}

// ─── Empty response helper ───────────────────────────────────────────────────

fn empty_response(
    requested_limit: Option<usize>,
    section_meta: SectionMeta,
    audience: &str,
) -> (StatusCode, Json<Response>) {
    let response = Response {
        data: vec![],
        meta: Meta {
            total: 0,
            section: section_meta,
            limit: LimitMeta {
                requested: requested_limit,
                applied: 0,
            },
            personalization: PersonalizationMeta {
                policy_version: "phase_4_2_assignment_tracking_deduplication".to_string(),
                audience: audience.to_string(),
                mode: "default_global_feed".to_string(),
                tracks_assignments: audience == "authenticated",
                description: "Section is disabled — no recommendations served.".to_string(),
                persona: None,
            },
            source_status: std::collections::HashMap::from([
                (SOURCE_ADMIN_UPLOAD.to_string(), SourceStatusResource { state: "disabled".to_string(), suppressed_count: None }),
                (SOURCE_SYSTEM_TOPIC.to_string(), SourceStatusResource { state: "disabled".to_string(), suppressed_count: None }),
                (SOURCE_AI_GENERATED.to_string(), SourceStatusResource { state: "disabled".to_string(), suppressed_count: None }),
            ]),
            personalization_detail: None,
        },
    };
    (StatusCode::OK, Json(response))
}

// ─── Personalization context builder ─────────────────────────────────────────

async fn build_personalization_context(
    state: &AppState,
    user_id: Option<i64>,
) -> AppResult<Option<crate::recommendation::personalization::PersonalizationContext>> {
    let user_id = match user_id {
        Some(id) => id,
        None => return Ok(None),
    };

    let users_repo = PgUsersRepo::new(state.db_pool.clone());
    let user = users_repo
        .find_by_id(user_id)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to find user: {e}")))?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    // Resolve primary subject info
    let primary_subject = if let Some(subject_id) = user.primary_subject_id {
        query_subject_by_id(state, subject_id).await?
    } else {
        None
    };

    // Query authored topic activity
    let authored_activity = query_authored_topic_activity(state, &user.email).await?;

    let input = PersonalizationInput {
        user_id: Some(user_id),
        primary_subject,
        authored_topic_activity: authored_activity,
    };

    Ok(Some(resolve(&input)))
}

// ─── DB query helpers ────────────────────────────────────────────────────────

/// Query published topics with sub_subject/subject taxonomy.
async fn query_published_topics(
    state: &AppState,
    _moment: &DateTime<Utc>,
) -> AppResult<Vec<TopicItemInput>> {
    // Fetch published topics with their sub_subject and subject info
    let rows = sqlx::query_as::<_, PublishedTopicRow>(
        r#"
        SELECT
            t.id::text,
            t.title,
            t.thumbnail_url,
            t.sub_subject_id,
            ss.subject_id,
            t.owner_user_id,
            t.ownership_status,
            t."order",
            t.created_at,
            t.updated_at,
            t.teacher_id,
            ss.name AS "ss_name",
            ss.slug AS "ss_slug",
            sb.name AS "sb_name",
            sb.slug AS "sb_slug"
        FROM topics t
        LEFT JOIN sub_subjects ss ON t.sub_subject_id = ss.id
        LEFT JOIN subjects sb ON ss.subject_id = sb.id
        WHERE t.is_published = true
          AND t.sub_subject_id IS NOT NULL
        ORDER BY t."order" ASC, t.created_at DESC
        "#,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to fetch published topics: {e}")))?;

    // For each topic, fetch its content types as "modules"
    let topic_ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let module_lookup = if topic_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        query_topic_modules(state, &topic_ids).await?
    };

    let items: Vec<TopicItemInput> = rows
        .into_iter()
        .map(|row| {
            let has_normalized = row.ownership_status == "normalized";
            let sub_subject_id = row.sub_subject_id;
            let subject_id_val = row.subject_id;

            let taxonomy = sub_subject_id.zip(subject_id_val).map(|(ss_id, s_id)| {
                TaxonomyInfo {
                    subject: Some(SubjectTaxonomy {
                        id: s_id,
                        name: row.sb_name.clone().unwrap_or_default(),
                        slug: row.sb_slug.clone().unwrap_or_default(),
                    }),
                    sub_subject: SubSubjectTaxonomy {
                        id: ss_id,
                        subject_id: s_id,
                        name: row.ss_name.clone().unwrap_or_default(),
                        slug: row.ss_slug.clone().unwrap_or_default(),
                        subject: None,
                    },
                }
            });

            TopicItemInput {
                id: row.id.clone(),
                title: row.title.clone(),
                thumbnail_url: row.thumbnail_url.clone(),
                sub_subject_id,
                subject_id: subject_id_val,
                taxonomy,
                personalization_eligible: Some(true),
                personalization_mode: None,
                personalization_excluded_reason: None,
                has_normalized_ownership: has_normalized,
                modules: module_lookup.get(&row.id).cloned().unwrap_or_default(),
                teacher_id: Some(row.teacher_id.clone()),
                owner_user_id: row.owner_user_id,
                ownership_status: Some(row.ownership_status.clone()),
                order: Some(row.order as i64),
                created_at: row.created_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
                updated_at: row.updated_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
            }
        })
        .collect();

    Ok(items)
}

/// Query distinct content types as "modules" for each topic.
async fn query_topic_modules(
    state: &AppState,
    topic_ids: &[String],
) -> AppResult<std::collections::HashMap<String, Vec<String>>> {
    // Convert text topic IDs to UUIDs
    let uuids: Vec<uuid::Uuid> = topic_ids
        .iter()
        .filter_map(|id| uuid::Uuid::parse_str(id).ok())
        .collect();

    if uuids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    #[derive(Debug, sqlx::FromRow)]
    struct ModuleRow {
        topic_id: uuid::Uuid,
        content_type: String,
    }

    let rows = sqlx::query_as::<_, ModuleRow>(
        r#"
        SELECT DISTINCT topic_id, type AS content_type
        FROM contents
        WHERE topic_id = ANY($1)
          AND is_published = true
        ORDER BY content_type
        "#,
    )
    .bind(&uuids)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to fetch topic modules: {e}")))?;

    let mut lookup: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for row in rows {
        lookup
            .entry(row.topic_id.to_string())
            .or_default()
            .push(row.content_type);
    }
    Ok(lookup)
}

/// Query sub_subjects with their subjects for the lookup.
async fn query_subject_lookups(
    state: &AppState,
) -> AppResult<(
    std::collections::HashMap<i64, SubSubjectTaxonomy>,
    std::collections::HashMap<i64, SubjectTaxonomy>,
)> {
    #[derive(Debug, sqlx::FromRow)]
    struct SubSubjectRow {
        id: i64,
        subject_id: i64,
        name: String,
        slug: String,
        sb_id: Option<i64>,
        sb_name: Option<String>,
        sb_slug: Option<String>,
    }

    let rows = sqlx::query_as::<_, SubSubjectRow>(
        r#"
        SELECT
            ss.id,
            ss.subject_id,
            ss.name,
            ss.slug,
            sb.id AS sb_id,
            sb.name AS sb_name,
            sb.slug AS sb_slug
        FROM sub_subjects ss
        LEFT JOIN subjects sb ON ss.subject_id = sb.id
        ORDER BY ss.id
        "#,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to fetch sub_subjects: {e}")))?;

    let mut sub_subject_lookup = std::collections::HashMap::new();
    let mut subject_lookup = std::collections::HashMap::new();

    for row in rows {
        let subject = row.sb_id.map(|id| SubjectTaxonomy {
            id,
            name: row.sb_name.clone().unwrap_or_default(),
            slug: row.sb_slug.clone().unwrap_or_default(),
        });

        if let Some(ref s) = subject {
            subject_lookup.entry(s.id).or_insert_with(|| s.clone());
        }

        sub_subject_lookup.insert(
            row.id,
            SubSubjectTaxonomy {
                id: row.id,
                subject_id: row.subject_id,
                name: row.name,
                slug: row.slug,
                subject,
            },
        );
    }

    Ok((sub_subject_lookup, subject_lookup))
}

/// Query a single subject by ID.
async fn query_subject_by_id(
    state: &AppState,
    subject_id: i64,
) -> AppResult<Option<SubjectInfo>> {
    #[derive(Debug, sqlx::FromRow)]
    struct SubjectRow {
        id: i64,
        name: String,
        slug: String,
    }

    let row = sqlx::query_as::<_, SubjectRow>(
        "SELECT id, name, slug FROM subjects WHERE id = $1",
    )
    .bind(subject_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to fetch subject: {e}")))?;

    Ok(row.map(|r| SubjectInfo {
        id: r.id,
        name: r.name,
        slug: r.slug,
        source: None,
    }))
}

/// Query authored topic activity for a teacher (group by sub_subject).
async fn query_authored_topic_activity(
    state: &AppState,
    teacher_id: &str,
) -> AppResult<Vec<ActivityRow>> {
    #[derive(Debug, sqlx::FromRow)]
    struct ActivityRowRaw {
        sub_subject_id: i64,
        subject_id: i64,
        topic_count: i64,
        latest_topic_activity_at: Option<chrono::NaiveDateTime>,
        ss_name: Option<String>,
        ss_slug: Option<String>,
        sb_name: Option<String>,
        sb_slug: Option<String>,
    }

    let rows = sqlx::query_as::<_, ActivityRowRaw>(
        r#"
        SELECT
            t.sub_subject_id,
            ss.subject_id,
            COUNT(t.id)::BIGINT AS topic_count,
            MAX(t.updated_at) AS latest_topic_activity_at,
            ss.name AS ss_name,
            ss.slug AS ss_slug,
            sb.name AS sb_name,
            sb.slug AS sb_slug
        FROM topics t
        JOIN sub_subjects ss ON t.sub_subject_id = ss.id
        LEFT JOIN subjects sb ON ss.subject_id = sb.id
        WHERE t.teacher_id = $1
          AND t.sub_subject_id IS NOT NULL
          AND t.is_published = true
        GROUP BY t.sub_subject_id, ss.subject_id, ss.name, ss.slug, sb.name, sb.slug
        ORDER BY topic_count DESC, latest_topic_activity_at DESC, t.sub_subject_id ASC
        "#,
    )
    .bind(teacher_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Internal(format!("Failed to fetch authored topic activity: {e}")))?;

    let activity: Vec<ActivityRow> = rows
        .into_iter()
        .map(|r| {
            let sub_subject = SubSubjectInfo {
                id: r.sub_subject_id,
                subject_id: r.subject_id,
                name: r.ss_name.clone().unwrap_or_default(),
                slug: r.ss_slug.clone().unwrap_or_default(),
            };
            let subject = SubjectInfo {
                id: r.subject_id,
                name: r.sb_name.clone().unwrap_or_default(),
                slug: r.sb_slug.clone().unwrap_or_default(),
                source: None,
            };

            ActivityRow {
                sub_subject_id: r.sub_subject_id,
                subject_id: r.subject_id,
                topic_count: r.topic_count,
                latest_topic_activity_at: r
                    .latest_topic_activity_at
                    .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
                sub_subject: Some(sub_subject),
                subject: Some(subject),
            }
        })
        .collect();

    Ok(activity)
}

/// A raw DB row for published topics query.
#[derive(Debug, sqlx::FromRow)]
struct PublishedTopicRow {
    id: String,
    title: String,
    thumbnail_url: Option<String>,
    sub_subject_id: Option<i64>,
    subject_id: Option<i64>,
    owner_user_id: Option<i64>,
    ownership_status: String,
    order: i32,
    created_at: Option<chrono::NaiveDateTime>,
    updated_at: Option<chrono::NaiveDateTime>,
    teacher_id: String,
    ss_name: Option<String>,
    ss_slug: Option<String>,
    sb_name: Option<String>,
    sb_slug: Option<String>,
}

// ─── Mapping helpers ─────────────────────────────────────────────────────────

fn map_curated_item(p: &crate::db::repositories::recommended_projects::RecommendedProject) -> CuratedItemInput {
    let tags: Vec<String> = p
        .tags
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let modules: Vec<String> = p
        .modules
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let mut source_payload = serde_json::json!({});
    if let Some(ref sp) = p.source_payload {
        source_payload = sp.clone();
    }

    CuratedItemInput {
        id: p.id.to_string(),
        title: p.title.clone(),
        description: p.description.clone(),
        thumbnail_url: p.thumbnail_url.clone(),
        ratio: Some(p.ratio.clone()),
        project_type: p.project_type.clone(),
        tags,
        modules,
        source_type: p.source_type.clone(),
        source_reference: p.source_reference.clone(),
        source_payload,
        display_priority: p.display_priority as i64,
        is_active: p.is_active,
        starts_at: p.starts_at.map(|dt| dt.to_rfc3339()),
        ends_at: p.ends_at.map(|dt| dt.to_rfc3339()),
        created_at: Some(p.created_at.to_rfc3339()),
        updated_at: Some(p.updated_at.to_rfc3339()),
    }
}

fn map_feed_item_to_resource(item: &FeedItem) -> RecommendedProjectResource {
    RecommendedProjectResource {
        id: item.id.clone(),
        title: item.title.clone(),
        description: item.description.clone(),
        thumbnail_url: item.thumbnail_url.clone(),
        ratio: item.ratio.clone(),
        project_type: item.project_type.clone(),
        tags: item.tags.clone(),
        modules: item.modules.clone(),
        sub_subject_id: item.sub_subject_id,
        subject_id: item.subject_id,
        taxonomy: item.taxonomy.clone(),
        personalization: item.personalization.as_ref().map(|p| PersonalizationResource {
            eligible: p.eligible,
            mode: p.mode.clone(),
            excluded_reason: p.excluded_reason.clone(),
            has_normalized_ownership: p.has_normalized_ownership,
            has_adequate_taxonomy: p.has_adequate_taxonomy,
        }),
        source_type: item.source_type.clone(),
        source_reference: item.source_reference.clone(),
        feed_origin: item.feed_origin.to_string(),
        display_priority: item.display_priority,
        visibility: VisibilityResource {
            is_active: item.visibility.is_active,
            starts_at: item.visibility.starts_at.clone(),
            ends_at: item.visibility.ends_at.clone(),
        },
        created_at: item.created_at.clone(),
        updated_at: item.updated_at.clone(),
    }
}

fn build_persona_json(summary: &PersonalizationSummary) -> Option<serde_json::Value> {
    if !summary.applied {
        return None;
    }
    let mut persona = serde_json::json!({});
    if let Some(ref mode) = summary.mode {
        persona["mode"] = serde_json::json!(mode);
    }
    if let Some(ref desc) = summary.description {
        persona["description"] = serde_json::json!(desc);
    }
    if let Some(ref ids) = summary.matched_sub_subject_ids {
        persona["matched_sub_subject_ids"] = serde_json::json!(ids);
    }
    if persona.is_object() && persona.as_object().map_or(true, |o| o.is_empty()) {
        return None;
    }
    Some(persona)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_response_resource() {
        let section_meta = SectionMeta {
            key: "project_recommendations".to_string(),
            label: None,
            enabled: false,
            position: 0,
            endpoint: "/api/v1/homepage-recommendations".to_string(),
            admin_configurator_path: "/admin/homepage-sections".to_string(),
        };

        let (status, resp) = empty_response(Some(10), section_meta, "guest");
        assert_eq!(status, StatusCode::OK);
        assert!(resp.data.is_empty());
        assert_eq!(resp.meta.total, 0);
        assert_eq!(resp.meta.personalization.audience, "guest");
        assert!(!resp.meta.personalization.tracks_assignments);
    }

    #[test]
    fn test_empty_response_authenticated() {
        let section_meta = SectionMeta {
            key: "project_recommendations".to_string(),
            label: Some("Recommendations".to_string()),
            enabled: false,
            position: 1,
            endpoint: "/api/v1/homepage-recommendations".to_string(),
            admin_configurator_path: "/admin/homepage-sections".to_string(),
        };

        let (_, resp) = empty_response(None, section_meta, "authenticated");
        assert_eq!(resp.meta.personalization.audience, "authenticated");
        assert!(resp.meta.personalization.tracks_assignments);
    }

    #[test]
    fn test_limit_meta_lifecycle() {
        let meta = LimitMeta {
            requested: Some(20),
            applied: 15,
        };
        assert_eq!(meta.requested, Some(20));
        assert_eq!(meta.applied, 15);
    }

    #[test]
    fn test_personalization_meta_default() {
        let meta = PersonalizationMeta {
            policy_version: "test".to_string(),
            audience: "guest".to_string(),
            mode: "default_global_feed".to_string(),
            tracks_assignments: false,
            description: "Test".to_string(),
            persona: None,
        };
        assert_eq!(meta.mode, "default_global_feed");
        assert!(!meta.tracks_assignments);
    }

    #[test]
    fn test_build_persona_json_not_applied() {
        let summary = PersonalizationSummary {
            applied: false,
            filter_applied: false,
            mode: None,
            description: None,
            selected_system_candidate_count: None,
            filtered_out_system_candidate_count: None,
            matched_system_topic_count: None,
            selected_source_breakdown: None,
            matched_sub_subject_ids: None,
        };
        assert!(build_persona_json(&summary).is_none());
    }

    #[test]
    fn test_build_persona_json_applied() {
        let summary = PersonalizationSummary {
            applied: true,
            filter_applied: true,
            mode: Some("personalized".to_string()),
            description: Some("Custom feed".to_string()),
            selected_system_candidate_count: Some(5),
            filtered_out_system_candidate_count: Some(3),
            matched_system_topic_count: Some(2),
            selected_source_breakdown: None,
            matched_sub_subject_ids: Some(vec![10, 20]),
        };
        let persona = build_persona_json(&summary).unwrap();
        assert_eq!(persona["mode"], "personalized");
        assert_eq!(persona["matched_sub_subject_ids"], serde_json::json!([10, 20]));
    }

    #[test]
    fn test_visibility_resource() {
        let v = VisibilityResource {
            is_active: true,
            starts_at: Some("2026-04-03T00:00:00Z".to_string()),
            ends_at: None,
        };
        assert!(v.is_active);
        assert_eq!(v.starts_at.as_deref(), Some("2026-04-03T00:00:00Z"));
        assert!(v.ends_at.is_none());
    }

    #[test]
    fn test_personalization_resource_defaults() {
        let p = PersonalizationResource {
            eligible: true,
            mode: None,
            excluded_reason: None,
            has_normalized_ownership: true,
            has_adequate_taxonomy: true,
        };
        assert!(p.eligible);
        assert!(p.has_normalized_ownership);
    }

    #[test]
    fn test_source_status_resource() {
        let s = SourceStatusResource {
            state: "ok".to_string(),
            suppressed_count: Some(3),
        };
        assert_eq!(s.state, "ok");
        assert_eq!(s.suppressed_count, Some(3));
    }

    #[test]
    fn test_section_meta_endpoint() {
        let sm = SectionMeta {
            key: "project_recommendations".to_string(),
            label: None,
            enabled: true,
            position: 0,
            endpoint: "/api/v1/homepage-recommendations".to_string(),
            admin_configurator_path: "/admin/homepage-sections".to_string(),
        };
        assert_eq!(sm.key, "project_recommendations");
        assert!(sm.enabled);
    }

    #[test]
    fn test_published_topic_row_structure() {
        let row = PublishedTopicRow {
            id: "123e4567-e89b-12d3-a456-426614174000".to_string(),
            title: "Test Topic".to_string(),
            thumbnail_url: None,
            sub_subject_id: Some(1),
            subject_id: Some(10),
            owner_user_id: Some(42),
            ownership_status: "normalized".to_string(),
            order: 0,
            created_at: None,
            updated_at: None,
            teacher_id: "teacher@test.com".to_string(),
            ss_name: Some("Algebra".to_string()),
            ss_slug: Some("algebra".to_string()),
            sb_name: Some("Mathematics".to_string()),
            sb_slug: Some("mathematics".to_string()),
        };
        assert_eq!(row.title, "Test Topic");
        assert_eq!(row.sub_subject_id, Some(1));
        assert_eq!(row.ownership_status, "normalized");
    }

    #[test]
    fn test_topic_item_input_from_published_topic_row() {
        let row = PublishedTopicRow {
            id: "topic-1".to_string(),
            title: "Fractions".to_string(),
            thumbnail_url: Some("https://example.com/thumb.jpg".to_string()),
            sub_subject_id: Some(5),
            subject_id: Some(1),
            owner_user_id: Some(100),
            ownership_status: "normalized".to_string(),
            order: 1,
            created_at: chrono::NaiveDateTime::parse_from_str("2026-04-01T10:00:00", "%Y-%m-%dT%H:%M:%S").ok(),
            updated_at: chrono::NaiveDateTime::parse_from_str("2026-04-02T12:00:00", "%Y-%m-%dT%H:%M:%S").ok(),
            teacher_id: "teacher1".to_string(),
            ss_name: Some("Fractions".to_string()),
            ss_slug: Some("fractions".to_string()),
            sb_name: Some("Mathematics".to_string()),
            sb_slug: Some("mathematics".to_string()),
        };
        assert_eq!(row.title, "Fractions");
    }
}

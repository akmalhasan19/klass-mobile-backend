use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::db::repositories::homepage_sections::{HomepageSectionsRepo, PgHomepageSectionsRepo};
use crate::db::repositories::recommended_projects::{
    PgRecommendedProjectsRepo, RecommendedProjectsRepo,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct HomepageRecommendationsQuery {
    limit: Option<i32>,
}

#[derive(Serialize)]
pub struct RecommendedProjectResource {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub ratio: String,
    pub project_type: Option<String>,
    pub tags: Option<serde_json::Value>,
    pub modules: Option<serde_json::Value>,
    pub source_type: String,
    pub display_priority: i32,
    pub visibility: VisibilityResource,
    pub created_at: String,
    pub updated_at: String,
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
    pub requested: Option<i32>,
    pub applied: i32,
}

#[derive(Serialize)]
pub struct PersonalizationMeta {
    pub policy_version: String,
    pub audience: String,
    pub mode: String,
    pub tracks_assignments: bool,
    pub description: String,
}

#[derive(Serialize)]
pub struct SourceStatusItem {
    pub state: String,
}

#[derive(Serialize)]
pub struct SourceStatus {
    pub admin_upload: SourceStatusItem,
    pub system_topic: SourceStatusItem,
    pub ai_generated: SourceStatusItem,
}

#[derive(Serialize)]
pub struct SourceBreakdown {
    pub admin_upload: i32,
    pub system_topic: i32,
    pub ai_generated: i32,
}

#[derive(Serialize)]
pub struct Meta {
    pub total: i32,
    pub source_breakdown: SourceBreakdown,
    pub section: SectionMeta,
    pub limit: LimitMeta,
    pub personalization: PersonalizationMeta,
    pub source_status: SourceStatus,
}

#[derive(Serialize)]
pub struct Response {
    pub data: Vec<RecommendedProjectResource>,
    pub meta: Meta,
}

pub async fn index(
    State(state): State<AppState>,
    Query(query): Query<HomepageRecommendationsQuery>,
) -> AppResult<(StatusCode, Json<Response>)> {
    let homepage_sections_repo = PgHomepageSectionsRepo::new(state.db_pool.clone());
    let recommended_projects_repo = PgRecommendedProjectsRepo::new(state.db_pool.clone());

    let section = homepage_sections_repo
        .find_by_key("project_recommendations")
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch homepage section: {}", e)))?;

    let section_meta = SectionMeta {
        key: "project_recommendations".to_string(),
        label: section.as_ref().map(|s| s.label.clone()),
        enabled: section.as_ref().map(|s| s.is_enabled).unwrap_or(false),
        position: section.as_ref().map(|s| s.position).unwrap_or(0),
        endpoint: "/api/v1/homepage-recommendations".to_string(),
        admin_configurator_path: "/admin/homepage-sections".to_string(),
    };

    let personalization = PersonalizationMeta {
        policy_version: "stub_phase_2".to_string(),
        audience: "guest".to_string(),
        mode: "admin_curated_only".to_string(),
        tracks_assignments: false,
        description: "Stub implementation returning admin-curated projects only".to_string(),
    };

    let source_status = SourceStatus {
        admin_upload: SourceStatusItem {
            state: "ok".to_string(),
        },
        system_topic: SourceStatusItem {
            state: "not_evaluated".to_string(),
        },
        ai_generated: SourceStatusItem {
            state: "not_evaluated".to_string(),
        },
    };

    if let Some(ref s) = section {
        if !s.is_enabled {
            return Ok((
                StatusCode::OK,
                Json(Response {
                    data: vec![],
                    meta: Meta {
                        total: 0,
                        source_breakdown: SourceBreakdown {
                            admin_upload: 0,
                            system_topic: 0,
                            ai_generated: 0,
                        },
                        section: section_meta,
                        limit: LimitMeta {
                            requested: query.limit,
                            applied: 0,
                        },
                        personalization,
                        source_status,
                    },
                }),
            ));
        }
    }

    let projects = recommended_projects_repo
        .find_visible()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch recommended projects: {}", e)))?;

    let limit = query.limit.unwrap_or(50).min(50);
    let applied_projects: Vec<_> = projects.into_iter().take(limit as usize).collect();

    let mut admin_upload_count = 0;
    let mut system_topic_count = 0;
    let mut ai_generated_count = 0;

    let data: Vec<RecommendedProjectResource> = applied_projects
        .iter()
        .map(|p| {
            match p.source_type.as_str() {
                "admin_upload" => admin_upload_count += 1,
                "system_topic" => system_topic_count += 1,
                "ai_generated" => ai_generated_count += 1,
                _ => {}
            }

            RecommendedProjectResource {
                id: p.id,
                title: p.title.clone(),
                description: p.description.clone(),
                thumbnail_url: p.thumbnail_url.clone(),
                ratio: p.ratio.clone(),
                project_type: p.project_type.clone(),
                tags: p.tags.clone(),
                modules: p.modules.clone(),
                source_type: p.source_type.clone(),
                display_priority: p.display_priority,
                visibility: VisibilityResource {
                    is_active: p.is_active,
                    starts_at: p.starts_at.map(|dt| dt.to_rfc3339()),
                    ends_at: p.ends_at.map(|dt| dt.to_rfc3339()),
                },
                created_at: p.created_at.to_rfc3339(),
                updated_at: p.updated_at.to_rfc3339(),
            }
        })
        .collect();

    let total = data.len() as i32;

    Ok((
        StatusCode::OK,
        Json(Response {
            data,
            meta: Meta {
                total,
                source_breakdown: SourceBreakdown {
                    admin_upload: admin_upload_count,
                    system_topic: system_topic_count,
                    ai_generated: ai_generated_count,
                },
                section: section_meta,
                limit: LimitMeta {
                    requested: query.limit,
                    applied: total,
                },
                personalization,
                source_status,
            },
        }),
    ))
}

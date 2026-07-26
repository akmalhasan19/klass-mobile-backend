use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::Principal;
use crate::db::repositories::freelancer_matches::{
    FreelancerMatchScores, FreelancerMatchesRepo, PgFreelancerMatchesRepo,
};
use crate::db::repositories::marketplace_tasks::{
    CreateMarketplaceTaskPayload, MarketplaceTasksRepo, PgMarketplaceTasksRepo,
};
use crate::db::repositories::media_generations::{MediaGenerationsRepo, PgMediaGenerationsRepo};
use crate::error::{AppError, AppResult};
use crate::matching::MatchingService;
use crate::orchestrator::submission::is_terminal_status;
use crate::state::AppState;

use super::response;

// ─── Guard helper ─────────────────────────────────────────────────────────────

fn require_teacher(principal: &Principal) -> Result<(), AppError> {
    if principal.role != "teacher" {
        return Err(AppError::Forbidden(format!(
            "requires role 'teacher', user has '{}'",
            principal.role
        )));
    }
    Ok(())
}

// ─── Request bodies ───────────────────────────────────────────────────────────

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SuggestFreelancersRequest {
    #[serde(default = "default_max_suggestions")]
    pub max_suggestions: usize,
}

fn default_max_suggestions() -> usize {
    5
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HireMode {
    AutoSuggest,
    ManualTask,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct HireFreelancerRequest {
    pub mode: HireMode,
    pub freelancer_id: Option<i64>,
}

// ─── Resources ────────────────────────────────────────────────────────────────

#[derive(Serialize, utoipa::ToSchema)]
pub struct FreelancerMatchResource {
    pub freelancer_id: i64,
    pub name: String,
    pub email: String,
    pub avatar_url: Option<String>,
    pub primary_subject_id: Option<i64>,
    pub portfolio_relevance_score: f64,
    pub success_rate: f64,
    pub availability_score: f64,
    pub match_score: f64,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct HiredFreelancerResource {
    pub task_id: Uuid,
    pub media_generation_id: Uuid,
    pub content_id: Uuid,
    pub status: String,
    pub task_type: String,
    pub suggested_freelancer_id: Option<i64>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct FreelancerUserResource {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub avatar_url: Option<String>,
    pub role: String,
    pub rating: f64,
    pub jobs: i32,
    pub job_count: i32,
    pub rate: String,
    pub price: String,
    pub tags: Vec<String>,
    pub skills: Vec<String>,
    pub verified: bool,
    #[serde(rename = "responseTime")]
    pub response_time: String,
}

#[derive(Debug, sqlx::FromRow)]
struct FreelancerDbRow {
    id: i64,
    name: String,
    email: String,
    avatar_url: Option<String>,
    role: String,
    rating: f64,
    job_count: i32,
    hourly_rate: String,
    skills: serde_json::Value,
    response_time: String,
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// GET /freelancers
#[utoipa::path(
    get,
    path = "/api/v1/freelancers",
    tag = "freelancers",
    responses(
        (status = 200, body = Vec<FreelancerUserResource>),
    ),
)]
pub async fn list_freelancers(
    State(state): State<AppState>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let rows = sqlx::query_as::<_, FreelancerDbRow>(
        r#"SELECT id, name, email, avatar_url, role,
                  COALESCE(rating, 4.8)::FLOAT8 AS rating,
                  COALESCE(job_count, 0)::INT AS job_count,
                  COALESCE(hourly_rate, '120K') AS hourly_rate,
                  COALESCE(skills, '[]'::jsonb) AS skills,
                  COALESCE(response_time, '< 30m') AS response_time
           FROM users WHERE role = 'freelancer' ORDER BY id ASC"#,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AppError::Internal(format!("failed to fetch freelancers: {e}")))?;

    let resources: Vec<FreelancerUserResource> = rows
        .into_iter()
        .map(|r| {
            let tags: Vec<String> = serde_json::from_value(r.skills.clone()).unwrap_or_default();
            let rate = r.hourly_rate;

            FreelancerUserResource {
                id: r.id,
                name: r.name,
                email: r.email,
                avatar_url: r.avatar_url,
                role: r.role,
                rating: r.rating,
                jobs: r.job_count,
                job_count: r.job_count,
                rate: rate.clone(),
                price: format!("Rp {rate}/topik"),
                tags: tags.clone(),
                skills: tags,
                verified: true,
                response_time: r.response_time,
            }
        })
        .collect();

    Ok(response::ok(resources))
}




/// POST /media-generations/{id}/suggest-freelancers
#[utoipa::path(
    post,
    path = "/api/v1/media-generations/{id}/suggest-freelancers",
    tag = "media-generations",
    params(
        ("id" = Uuid, Path, description = "Media generation ID"),
    ),
    request_body = SuggestFreelancersRequest,
    responses(
        (status = 200, body = Vec<FreelancerMatchResource>),
    ),
    security(
        ("bearer_auth" = [])
    ),
)]
pub async fn suggest_freelancers(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(payload): Json<SuggestFreelancersRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_teacher(&principal)?;

    let limit = payload.max_suggestions.clamp(1, 10);

    // Fetch the generation (scoped to teacher)
    let gen_repo = PgMediaGenerationsRepo::new(state.db_pool.clone());
    let generation = gen_repo
        .find_by_id_for_teacher(id, principal.user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("media generation not found".into()))?;

    // Run the matching service
    let matching = MatchingService::new(state.db_pool.clone());
    let matches = matching
        .find_best_matches(&generation.generation, limit)
        .await
        .map_err(|e| AppError::Internal(format!("failed to find freelancer matches: {e}")))?;

    // Upsert each match into freelancer_matches table
    let fm_repo = PgFreelancerMatchesRepo::new(state.db_pool.clone());
    for m in &matches {
        let scores = FreelancerMatchScores {
            match_score: m.scores.match_score,
            portfolio_relevance_score: m.scores.portfolio_relevance_score,
            success_rate: m.scores.success_rate,
        };
        fm_repo
            .upsert(id, m.freelancer.id, &scores)
            .await
            .map_err(|e| AppError::Internal(format!("failed to upsert freelancer match: {e}")))?;
    }

    let resources: Vec<FreelancerMatchResource> = matches
        .into_iter()
        .map(|m| FreelancerMatchResource {
            freelancer_id: m.freelancer.id,
            name: m.freelancer.name,
            email: m.freelancer.email,
            avatar_url: m.freelancer.avatar_url,
            primary_subject_id: m.freelancer.primary_subject_id,
            portfolio_relevance_score: m.scores.portfolio_relevance_score,
            success_rate: m.scores.success_rate,
            availability_score: m.scores.availability_score,
            match_score: m.scores.match_score,
        })
        .collect();

    Ok(response::ok(resources))
}

/// POST /media-generations/{id}/hire-freelancer
#[utoipa::path(
    post,
    path = "/api/v1/media-generations/{id}/hire-freelancer",
    tag = "media-generations",
    params(
        ("id" = Uuid, Path, description = "Media generation ID"),
    ),
    request_body = HireFreelancerRequest,
    responses(
        (status = 201, body = HiredFreelancerResource),
    ),
    security(
        ("bearer_auth" = [])
    ),
)]
pub async fn hire_freelancer(
    State(state): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(payload): Json<HireFreelancerRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    require_teacher(&principal)?;

    // Fetch the generation (scoped to teacher)
    let gen_repo = PgMediaGenerationsRepo::new(state.db_pool.clone());
    let generation = gen_repo
        .find_by_id_for_teacher(id, principal.user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("media generation not found".into()))?;

    // Validate terminal status
    if !is_terminal_status(&generation.generation.status) {
        return Err(AppError::Validation(format!(
            "generation status '{}' is not terminal — must be completed, failed, or cancelled",
            generation.generation.status
        )));
    }

    // Must have a content_id (published artifact)
    let content_id = generation
        .generation
        .content_id
        .ok_or_else(|| AppError::Validation("generation has no content (not yet published)".into()))?;

    let mt_repo = PgMarketplaceTasksRepo::new(state.db_pool.clone());

    let (task_type, status, suggested_freelancer_id) = match payload.mode {
        HireMode::AutoSuggest => {
            let freelancer_id = payload.freelancer_id.ok_or_else(|| {
                AppError::Validation(
                    "freelancer_id is required for auto_suggest mode".into(),
                )
            })?;
            ("suggestion".to_string(), "assigned".to_string(), Some(freelancer_id))
        }
        HireMode::ManualTask => {
            ("bid".to_string(), "open_for_bid".to_string(), None)
        }
    };

    let create_payload = CreateMarketplaceTaskPayload {
        content_id,
        status,
        task_type,
        description: None,
        creator_id: Some(principal.user_id.to_string()),
        suggested_freelancer_id,
        attachment_url: None,
        media_generation_id: Some(id),
    };

    let task = mt_repo
        .insert(&create_payload)
        .await
        .map_err(|e| AppError::Internal(format!("failed to create marketplace task: {e}")))?;

    let resource = HiredFreelancerResource {
        task_id: task.id,
        media_generation_id: id,
        content_id,
        status: task.status,
        task_type: task.task_type,
        suggested_freelancer_id: task.suggested_freelancer_id,
    };

    Ok(response::created(resource))
}

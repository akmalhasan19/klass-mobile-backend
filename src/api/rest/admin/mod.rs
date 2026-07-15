pub mod activity_logs;
pub mod contents;
pub mod homepage_sections;
pub mod marketplace_tasks;
pub mod media_generations;
pub mod media_generations_debug;
pub mod recommended_projects;
pub mod student_progress;
pub mod system_settings;
pub mod topics;
pub mod uploads;

use axum::routing::{get, patch, post, put};
use axum::Router;

use crate::auth::middleware::Principal;
use crate::error::AppError;
use crate::state::AppState;

/// Guard helper: returns `Err(AppError::Forbidden)` if the principal is not an admin.
///
/// Used by every admin write handler. Applied per-handler rather than as a
/// middleware layer because `Principal` extraction requires `AppState` which
/// is only available at request-handling time, not at route-build time.
/// A middleware-layer approach would require passing `AppState` into
/// `admin_router()` as a parameter — a larger refactor deferred to future work.
fn require_admin(principal: &Principal) -> Result<(), AppError> {
    if principal.role != "admin" {
        return Err(AppError::Forbidden(format!(
            "requires role 'admin', user has '{}'",
            principal.role
        )));
    }
    Ok(())
}

/// Build the admin REST router with all `/api/v1/admin/*` routes.
///
/// All routes are individually guarded by `require_admin()` calls inside each
/// handler function. This ensures every `/admin/*` endpoint is admin-protected.
pub fn admin_router() -> Router<AppState> {
    let topic_routes = Router::new()
        .route("/{id}", patch(topics::update).delete(topics::delete))
        .route("/{id}/reorder", patch(topics::reorder))
        .route("/{id}/publish", patch(topics::publish));

    let content_routes = Router::new()
        .route("/", post(contents::create))
        .route("/{id}", patch(contents::update).delete(contents::delete))
        .route("/{id}/reorder", patch(contents::reorder))
        .route("/{id}/publish", patch(contents::publish));

    let marketplace_task_routes = Router::new()
        .route("/", post(marketplace_tasks::create))
        .route(
            "/{id}",
            patch(marketplace_tasks::update).delete(marketplace_tasks::delete),
        )
        .route("/{id}/status", patch(marketplace_tasks::update_status));

    let student_progress_routes = Router::new()
        .route("/", post(student_progress::create))
        .route(
            "/{id}",
            patch(student_progress::update).delete(student_progress::delete),
        );

    let upload_routes = Router::new()
        .route("/{category}", post(uploads::upload).delete(uploads::delete));

    let activity_log_routes = Router::new().route("/", get(activity_logs::index));

    let homepage_section_routes = Router::new()
        .route("/", patch(homepage_sections::bulk_update));

    let system_setting_routes = Router::new()
        .route("/", get(system_settings::index).patch(system_settings::bulk_update));

    let media_generation_routes = Router::new()
        .route("/", get(media_generations::index))
        .route("/{id}/debug-taxonomy", get(media_generations_debug::debug_taxonomy));

    let recommended_project_routes = Router::new()
        .route("/", post(recommended_projects::create))
        .route(
            "/{id}",
            put(recommended_projects::update).delete(recommended_projects::delete),
        )
        .route("/{id}/toggle-active", patch(recommended_projects::toggle_active))
        .route("/{id}/show-now", patch(recommended_projects::show_now));

    Router::new()
        .nest("/topics", topic_routes)
        .nest("/contents", content_routes)
        .nest("/marketplace-tasks", marketplace_task_routes)
        .nest("/student-progress", student_progress_routes)
        .nest("/upload", upload_routes)
        .nest("/activity-logs", activity_log_routes)
        .nest("/homepage-sections", homepage_section_routes)
        .nest("/settings", system_setting_routes)
        .nest("/media-generations", media_generation_routes)
        .nest("/homepage-sections/recommended-projects", recommended_project_routes)
}

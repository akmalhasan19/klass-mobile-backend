pub mod admin;
pub mod auth;
pub mod avatar;
pub mod contents;
pub mod freelancer;
pub mod gallery;
pub mod homepage_recommendations;
pub mod homepage_sections;
pub mod marketplace_tasks;
pub mod media_generations;
pub mod response;
pub mod student_progress;
pub mod topics;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

/// Build the REST API router with all `/api/v1` routes.
pub fn api_router() -> Router<AppState> {
    let auth_routes = Router::new()
        .route("/register", post(auth::register))
        .route("/login", post(auth::login))
        .route("/logout", post(auth::logout))
        .route("/me", get(auth::me))
        .route("/refresh", post(auth::refresh))
        .route("/get-security-question", post(auth::get_security_question))
        .route(
            "/verify-and-reset-password",
            post(auth::verify_and_reset_password),
        );

    let user_routes = Router::new().route("/avatar", post(avatar::upload_avatar));

    let topic_routes = Router::new()
        .route("/", get(topics::index))
        .route("/{id}", get(topics::show));

    let content_routes = Router::new()
        .route("/", get(contents::index))
        .route("/{id}", get(contents::show));

    let marketplace_task_routes = Router::new()
        .route("/", get(marketplace_tasks::index))
        .route("/{id}", get(marketplace_tasks::show));

    let media_generation_routes = Router::new()
        .route("/", get(media_generations::index).post(media_generations::create))
        .route("/{id}", get(media_generations::show))
        .route("/{id}/regenerate", post(media_generations::regenerate))
        .route("/{id}/suggest-freelancers", post(freelancer::suggest_freelancers))
        .route("/{id}/hire-freelancer", post(freelancer::hire_freelancer));

    let student_progress_routes = Router::new()
        .route("/", get(student_progress::index))
        .route("/{id}", get(student_progress::show));

    let homepage_section_routes = Router::new().route("/", get(homepage_sections::index));

    let gallery_routes = Router::new().route("/", get(gallery::index));

    let admin_routes = admin::admin_router();

    Router::new()
        .nest("/auth", auth_routes)
        .nest("/user", user_routes)
        .nest("/topics", topic_routes)
        .nest("/contents", content_routes)
        .nest("/marketplace-tasks", marketplace_task_routes)
        .nest("/media-generations", media_generation_routes)
        .nest("/student-progress", student_progress_routes)
        .nest("/homepage-sections", homepage_section_routes)
        .nest("/gallery", gallery_routes)
        .nest("/admin", admin_routes)
        .route(
            "/homepage-recommendations",
            get(homepage_recommendations::index),
        )
}

pub mod auth;
pub mod avatar;
pub mod response;

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
        .route(
            "/get-security-question",
            post(auth::get_security_question),
        )
        .route(
            "/verify-and-reset-password",
            post(auth::verify_and_reset_password),
        );

    let user_routes = Router::new().route("/avatar", post(avatar::upload_avatar));

    Router::new()
        .nest("/auth", auth_routes)
        .nest("/user", user_routes)
}

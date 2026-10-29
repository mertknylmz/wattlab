mod auth;
mod dto;
mod error;
mod events;
mod health;
mod sessions;

use crate::{application::session_service::SessionService, auth::ApiToken};
use axum::{
    Router, middleware,
    response::IntoResponse,
    routing::{get, post},
};
use error::ApiError;

pub fn router(token: ApiToken, service: SessionService) -> Router {
    let api = Router::new()
        .route("/status", get(health::check))
        .route("/sessions", post(sessions::start))
        .route("/sessions/{id}", get(sessions::get))
        .route("/sessions/{id}/finish", post(sessions::finish))
        .route("/sessions/{id}/telemetry", post(events::telemetry))
        .route(
            "/sessions/{id}/connection-events",
            post(events::connections),
        )
        .fallback(not_found)
        .with_state(service)
        .layer(middleware::from_fn_with_state(token, auth::require_token));
    Router::new()
        .route("/health", get(health::check))
        .nest("/api/v1", api)
        .fallback(not_found)
}

async fn not_found() -> impl IntoResponse {
    ApiError::NotFound
}

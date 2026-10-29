//! Composition root: connects the application port to inbound adapters.
pub mod adapters;
pub mod application;
pub mod auth;
pub mod config;
pub mod domain;

use application::{ports::session_repository::SessionRepository, session_service::SessionService};
use std::sync::Arc;

pub fn build_router(token: auth::ApiToken, repository: Arc<dyn SessionRepository>) -> axum::Router {
    adapters::inbound::http::router(token, SessionService::new(repository))
}

//! Composition root: connects the application ports to inbound adapters.
pub mod adapters;
pub mod application;
pub mod auth;
pub mod config;
pub mod domain;

use adapters::{
    inbound::http::{Services, Shutdown},
    outbound::memory::InMemoryCommandQueue,
};
use application::{
    command_service::CommandService,
    history_service::HistoryService,
    ports::{
        history_repository::HistoryRepository, session_repository::SessionRepository,
        settings_repository::SettingsRepository,
    },
    session_service::SessionService,
    settings_service::SettingsService,
};
use std::sync::Arc;

pub struct App {
    pub router: axum::Router,
    /// Trigger before awaiting graceful shutdown so long polls and streams end.
    pub shutdown: Shutdown,
}

/// One storage adapter currently implements every persistence port; agent
/// commands are deliberately process-local and never persisted.
pub fn build_app<R>(token: auth::ApiToken, repository: Arc<R>) -> App
where
    R: SessionRepository + HistoryRepository + SettingsRepository + 'static,
{
    let shutdown = Shutdown::default();
    let router = adapters::inbound::http::router(
        token,
        Services {
            sessions: SessionService::new(repository.clone()),
            history: HistoryService::new(repository.clone(), repository.clone()),
            settings: SettingsService::new(repository),
            commands: CommandService::new(Arc::new(InMemoryCommandQueue::new())),
            shutdown: shutdown.clone(),
        },
    );
    App { router, shutdown }
}

pub fn build_router<R>(token: auth::ApiToken, repository: Arc<R>) -> axum::Router
where
    R: SessionRepository + HistoryRepository + SettingsRepository + 'static,
{
    build_app(token, repository).router
}

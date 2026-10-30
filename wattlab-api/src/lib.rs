//! Composition root: connects the application ports to inbound adapters.
pub mod adapters;
pub mod application;
pub mod auth;
pub mod config;
pub mod domain;

use adapters::{
    inbound::http::{Services, Shutdown},
    outbound::{argon2_hasher::Argon2Hasher, memory::InMemoryCommandQueue},
};
use application::{
    auth_service::AuthService,
    command_service::CommandService,
    history_service::HistoryService,
    ports::{
        history_repository::HistoryRepository, member_repository::MemberRepository,
        session_repository::SessionRepository, settings_repository::SettingsRepository,
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
/// `agent_token` authenticates the trainer agent; members log in with accounts.
pub fn build_app<R>(agent_token: auth::ApiToken, repository: Arc<R>) -> App
where
    R: SessionRepository + HistoryRepository + SettingsRepository + MemberRepository + 'static,
{
    let shutdown = Shutdown::default();
    let router = adapters::inbound::http::router(
        agent_token,
        Services {
            auth: AuthService::new(
                repository.clone(),
                repository.clone(),
                Arc::new(Argon2Hasher),
            ),
            sessions: SessionService::new(repository.clone()),
            history: HistoryService::new(repository.clone(), repository.clone()),
            settings: SettingsService::new(repository.clone()),
            commands: CommandService::new(
                Arc::new(InMemoryCommandQueue::new()),
                repository.clone(),
            ),
            shutdown: shutdown.clone(),
        },
    );
    App { router, shutdown }
}

pub fn build_router<R>(token: auth::ApiToken, repository: Arc<R>) -> axum::Router
where
    R: SessionRepository + HistoryRepository + SettingsRepository + MemberRepository + 'static,
{
    build_app(token, repository).router
}

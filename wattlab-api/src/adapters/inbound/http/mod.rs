mod agent;
mod auth;
mod dto;
mod error;
mod events;
mod health;
mod history;
mod live;
mod sessions;
mod settings;
mod shutdown;
mod stream;
pub use shutdown::Shutdown;

use crate::{
    application::{
        command_service::CommandService, history_service::HistoryService,
        session_service::SessionService, settings_service::SettingsService,
    },
    auth::ApiToken,
};
use axum::{
    Router,
    extract::FromRef,
    middleware,
    response::IntoResponse,
    routing::{get, post, put},
};
use error::ApiError;

/// Application services shared by handlers; each handler extracts only its own.
#[derive(Clone)]
pub struct Services {
    pub sessions: SessionService,
    pub history: HistoryService,
    pub settings: SettingsService,
    pub commands: CommandService,
    pub shutdown: Shutdown,
}

macro_rules! sub_state {
    ($($field:ident: $type:ty),* $(,)?) => {$(
        impl FromRef<Services> for $type {
            fn from_ref(services: &Services) -> Self {
                services.$field.clone()
            }
        }
    )*};
}
sub_state!(
    sessions: SessionService,
    history: HistoryService,
    settings: SettingsService,
    commands: CommandService,
    shutdown: Shutdown,
);

pub fn router(token: ApiToken, services: Services) -> Router {
    let api = Router::new()
        .route("/status", get(health::check))
        .route("/live", get(live::snapshot))
        .route("/live/stream", get(stream::live))
        .route("/settings", get(settings::get).put(settings::update))
        .route("/agent/status", get(agent::status).put(agent::heartbeat))
        .route("/agent/commands", post(agent::submit))
        .route("/agent/commands/next", get(agent::next))
        .route("/agent/commands/{id}", get(agent::get))
        .route("/agent/commands/{id}/result", put(agent::result))
        .route("/sessions/{id}/live", get(live::session_snapshot))
        .route("/sessions", post(sessions::start).get(history::list))
        .route("/sessions/{id}", get(sessions::get))
        .route("/sessions/{id}/summary", get(history::summary))
        .route("/sessions/{id}/series", get(history::series))
        .route("/sessions/{id}/finish", post(sessions::finish))
        .route("/sessions/{id}/telemetry", post(events::telemetry))
        .route(
            "/sessions/{id}/connection-events",
            post(events::connections),
        )
        .fallback(not_found)
        .with_state(services)
        .layer(middleware::from_fn_with_state(token, auth::require_token));
    Router::new()
        .route("/health", get(health::check))
        .nest("/api/v1", api)
        .fallback(not_found)
}

async fn not_found() -> impl IntoResponse {
    ApiError::NotFound
}

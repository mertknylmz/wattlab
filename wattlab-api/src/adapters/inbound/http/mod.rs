mod account;
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
        auth_service::AuthService, command_service::CommandService,
        history_service::HistoryService, session_service::SessionService,
        settings_service::SettingsService,
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
    pub auth: AuthService,
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
    auth: AuthService,
    sessions: SessionService,
    history: HistoryService,
    settings: SettingsService,
    commands: CommandService,
    shutdown: Shutdown,
);

/// `agent_token` is the trainer agent's service credential; members use login tokens.
pub fn router(agent_token: ApiToken, services: Services) -> Router {
    let gate = auth::Gate {
        agent: agent_token,
        auth: services.auth.clone(),
    };
    let public = Router::new()
        .route("/auth/register", post(account::register))
        .route("/auth/login", post(account::login))
        .with_state(services.clone());
    let api = Router::new()
        .route("/auth/logout", post(account::logout))
        .route("/auth/me", get(account::me))
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
        .route("/activity", get(history::activity))
        .route("/sessions/{id}/finish", post(sessions::finish))
        .route("/sessions/{id}/telemetry", post(events::telemetry))
        .route(
            "/sessions/{id}/connection-events",
            post(events::connections),
        )
        .fallback(not_found)
        .with_state(services)
        .layer(middleware::from_fn_with_state(
            gate,
            auth::require_principal,
        ));
    Router::new()
        .route("/health", get(health::check))
        .nest("/api/v1", public.merge(api))
        .fallback(not_found)
}

async fn not_found() -> impl IntoResponse {
    ApiError::NotFound
}

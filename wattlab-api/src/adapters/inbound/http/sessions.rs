use super::{
    auth::{AgentOnly, Scope},
    dto::{SessionResponse, StartSessionRequest},
    error::ApiError,
};
use crate::application::{command_service::CommandService, session_service::SessionService};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
};
use uuid::Uuid;

/// The agent starts a session while executing a member's start command;
/// that member owns the session.
pub(super) async fn start(
    _: AgentOnly,
    State(service): State<SessionService>,
    State(commands): State<CommandService>,
    payload: Result<Json<StartSessionRequest>, JsonRejection>,
) -> Result<Json<SessionResponse>, ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::InvalidRequest)?;
    let owner = commands.session_owner(request.command_id)?;
    Ok(Json(service.start(request.request_id, owner).await?.into()))
}

pub(super) async fn get(
    Scope(scope): Scope,
    State(service): State<SessionService>,
    Path(id): Path<String>,
) -> Result<Json<SessionResponse>, ApiError> {
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::InvalidRequest)?;
    Ok(Json(service.get(id, scope).await?.into()))
}

/// The agent finishes any session; a member only their own (e.g. after an agent crash).
pub(super) async fn finish(
    Scope(scope): Scope,
    State(service): State<SessionService>,
    Path(id): Path<String>,
) -> Result<Json<SessionResponse>, ApiError> {
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::InvalidRequest)?;
    Ok(Json(service.finish(id, scope).await?.into()))
}

use super::{
    auth::{AgentOnly, MemberId},
    dto::{
        AgentResponse, CommandRequest, CommandResponse, CommandResultRequest, HeartbeatRequest,
        NextQuery, ResultStatus,
    },
    error::ApiError,
    shutdown::Shutdown,
};
use crate::application::command_service::{CommandService, MAX_COMMAND_WAIT};
use axum::{
    Json,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use std::time::Duration;
use uuid::Uuid;

pub(super) async fn submit(
    MemberId(member): MemberId,
    State(service): State<CommandService>,
    payload: Result<Json<CommandRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CommandResponse>), ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::InvalidRequest)?;
    let kind = request.try_into().map_err(|_| ApiError::InvalidRequest)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(service.submit(kind, member).await?.into()),
    ))
}

pub(super) async fn get(
    MemberId(member): MemberId,
    State(service): State<CommandService>,
    Path(id): Path<String>,
) -> Result<Json<CommandResponse>, ApiError> {
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::InvalidRequest)?;
    Ok(Json(service.get(id, Some(member))?.into()))
}

/// Long poll for the agent. Returns 204 when nothing arrived in time or the
/// API is shutting down.
pub(super) async fn next(
    _: AgentOnly,
    State(service): State<CommandService>,
    State(shutdown): State<Shutdown>,
    query: Result<Query<NextQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::InvalidRequest)?;
    let wait = match query.wait_seconds {
        None => Duration::ZERO,
        Some(raw) => raw
            .parse::<u64>()
            .ok()
            .map(Duration::from_secs)
            .filter(|wait| *wait <= MAX_COMMAND_WAIT)
            .ok_or(ApiError::InvalidRequest)?,
    };
    let command = tokio::select! {
        command = service.next(wait) => command,
        () = shutdown.wait() => None,
    };
    Ok(match command {
        Some(command) => Json(CommandResponse::from(command)).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    })
}

pub(super) async fn result(
    _: AgentOnly,
    State(service): State<CommandService>,
    Path(id): Path<String>,
    payload: Result<Json<CommandResultRequest>, JsonRejection>,
) -> Result<Json<CommandResponse>, ApiError> {
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::InvalidRequest)?;
    let Json(request) = payload.map_err(|_| ApiError::InvalidRequest)?;
    let succeeded = matches!(request.status, ResultStatus::Succeeded);
    Ok(Json(
        service.complete(id, succeeded, request.message)?.into(),
    ))
}

pub(super) async fn heartbeat(
    _: AgentOnly,
    State(service): State<CommandService>,
    payload: Result<Json<HeartbeatRequest>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::InvalidRequest)?;
    let status = request
        .into_domain(Utc::now())
        .ok_or(ApiError::InvalidRequest)?;
    service.heartbeat(status);
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn status(State(service): State<CommandService>) -> Json<AgentResponse> {
    Json(service.agent(Utc::now()).into())
}

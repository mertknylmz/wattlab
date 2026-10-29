use super::{
    dto::{SessionResponse, StartSessionRequest},
    error::ApiError,
};
use crate::application::session_service::SessionService;
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
};
use uuid::Uuid;

pub(super) async fn start(
    State(service): State<SessionService>,
    payload: Result<Json<StartSessionRequest>, JsonRejection>,
) -> Result<Json<SessionResponse>, ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::InvalidRequest)?;
    Ok(Json(service.start(request.request_id).await?.into()))
}

pub(super) async fn get(
    State(service): State<SessionService>,
    Path(id): Path<String>,
) -> Result<Json<SessionResponse>, ApiError> {
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::InvalidRequest)?;
    Ok(Json(service.get(id).await?.into()))
}

pub(super) async fn finish(
    State(service): State<SessionService>,
    Path(id): Path<String>,
) -> Result<Json<SessionResponse>, ApiError> {
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::InvalidRequest)?;
    Ok(Json(service.finish(id).await?.into()))
}

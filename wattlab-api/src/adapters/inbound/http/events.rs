use super::{
    dto::{Accepted, Batch, ConnectionDto, MeasurementDto},
    error::ApiError,
};
use crate::application::session_service::SessionService;
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
};
use uuid::Uuid;

pub(super) async fn telemetry(
    State(service): State<SessionService>,
    Path(id): Path<String>,
    payload: Result<Json<Batch<MeasurementDto>>, JsonRejection>,
) -> Result<Json<Accepted>, ApiError> {
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::InvalidRequest)?;
    let Json(dto) = payload.map_err(|_| ApiError::InvalidRequest)?;
    let batch = dto.try_into().map_err(|_| ApiError::InvalidRequest)?;
    Ok(Json(Accepted {
        accepted: service.telemetry(id, batch).await?,
    }))
}
pub(super) async fn connections(
    State(service): State<SessionService>,
    Path(id): Path<String>,
    payload: Result<Json<Batch<ConnectionDto>>, JsonRejection>,
) -> Result<Json<Accepted>, ApiError> {
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::InvalidRequest)?;
    let Json(dto) = payload.map_err(|_| ApiError::InvalidRequest)?;
    let batch = dto.try_into().map_err(|_| ApiError::InvalidRequest)?;
    Ok(Json(Accepted {
        accepted: service.connections(id, batch).await?,
    }))
}

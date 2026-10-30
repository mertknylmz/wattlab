use super::{
    dto::{PageQuery, SeriesResponse, SessionPageResponse, SessionSummaryResponse},
    error::ApiError,
};
use crate::application::history_service::{HistoryService, MAX_PAGE_SIZE};
use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
};
use uuid::Uuid;

const DEFAULT_PAGE_SIZE: u32 = 20;

pub(super) async fn list(
    State(service): State<HistoryService>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> Result<Json<SessionPageResponse>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::InvalidRequest)?;
    let limit = match query.limit {
        None => DEFAULT_PAGE_SIZE,
        Some(raw) => raw
            .parse::<u32>()
            .ok()
            .filter(|limit| (1..=MAX_PAGE_SIZE).contains(limit))
            .ok_or(ApiError::InvalidRequest)?,
    };
    let before = query
        .before
        .map(|raw| Uuid::parse_str(&raw).map_err(|_| ApiError::InvalidRequest))
        .transpose()?;
    Ok(Json(service.page(before, limit).await?.into()))
}

pub(super) async fn summary(
    State(service): State<HistoryService>,
    Path(id): Path<String>,
) -> Result<Json<SessionSummaryResponse>, ApiError> {
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::InvalidRequest)?;
    Ok(Json(service.detail(id).await?.0.into()))
}

pub(super) async fn series(
    State(service): State<HistoryService>,
    Path(id): Path<String>,
) -> Result<Json<SeriesResponse>, ApiError> {
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::InvalidRequest)?;
    Ok(Json(service.detail(id).await?.into()))
}

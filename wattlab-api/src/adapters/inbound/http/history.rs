use super::{
    auth::MemberId,
    dto::{
        ActivityQuery, ActivityResponse, PageQuery, SeriesResponse, SessionPageResponse,
        SessionSummaryResponse,
    },
    error::ApiError,
};
use crate::application::history_service::{HistoryService, MAX_ACTIVITY_DAYS, MAX_PAGE_SIZE};
use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

const DEFAULT_PAGE_SIZE: u32 = 20;

pub(super) async fn list(
    MemberId(member): MemberId,
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
    Ok(Json(service.page(member, before, limit).await?.into()))
}

pub(super) async fn summary(
    MemberId(member): MemberId,
    State(service): State<HistoryService>,
    Path(id): Path<String>,
) -> Result<Json<SessionSummaryResponse>, ApiError> {
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::InvalidRequest)?;
    Ok(Json(service.detail(member, id).await?.0.into()))
}

pub(super) async fn activity(
    MemberId(member): MemberId,
    State(service): State<HistoryService>,
    query: Result<Query<ActivityQuery>, QueryRejection>,
) -> Result<Json<ActivityResponse>, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::InvalidRequest)?;
    let from = DateTime::parse_from_rfc3339(&query.from)
        .ok()
        .map(|value| value.with_timezone(&Utc))
        .ok_or(ApiError::InvalidRequest)?;
    let to = DateTime::parse_from_rfc3339(&query.to)
        .ok()
        .map(|value| value.with_timezone(&Utc))
        .ok_or(ApiError::InvalidRequest)?;
    if to <= from || (to - from).num_days() > MAX_ACTIVITY_DAYS {
        return Err(ApiError::InvalidRequest);
    }
    Ok(Json(service.activity(member, from, to).await?.into()))
}

pub(super) async fn series(
    MemberId(member): MemberId,
    State(service): State<HistoryService>,
    Path(id): Path<String>,
) -> Result<Json<SeriesResponse>, ApiError> {
    let id = Uuid::parse_str(&id).map_err(|_| ApiError::InvalidRequest)?;
    Ok(Json(service.detail(member, id).await?.into()))
}

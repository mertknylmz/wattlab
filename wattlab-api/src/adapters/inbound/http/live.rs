use super::{auth::MemberId, dto::LiveResponse, error::ApiError};
use crate::application::session_service::SessionService;
use axum::{Json, extract::State, http::header::CACHE_CONTROL};
pub(super) async fn snapshot(
    MemberId(member): MemberId,
    State(service): State<SessionService>,
) -> Result<
    (
        [(axum::http::HeaderName, &'static str); 1],
        Json<LiveResponse>,
    ),
    ApiError,
> {
    Ok((
        [(CACHE_CONTROL, "no-store")],
        Json(service.live(member).await?.into()),
    ))
}

pub(super) async fn session_snapshot(
    MemberId(member): MemberId,
    State(service): State<SessionService>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<
    (
        [(axum::http::HeaderName, &'static str); 1],
        Json<LiveResponse>,
    ),
    ApiError,
> {
    let id = uuid::Uuid::parse_str(&id).map_err(|_| ApiError::InvalidRequest)?;
    Ok((
        [(CACHE_CONTROL, "no-store")],
        Json(Some(service.live_session(member, id).await?).into()),
    ))
}

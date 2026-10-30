use super::{
    auth::MemberId,
    dto::{SettingsResponse, UpdateSettingsRequest},
    error::ApiError,
};
use crate::{application::settings_service::SettingsService, domain::settings::Ftp};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
};

pub(super) async fn get(
    MemberId(member): MemberId,
    State(service): State<SettingsService>,
) -> Result<Json<SettingsResponse>, ApiError> {
    Ok(Json(service.get(member).await?.into()))
}

pub(super) async fn update(
    MemberId(member): MemberId,
    State(service): State<SettingsService>,
    payload: Result<Json<UpdateSettingsRequest>, JsonRejection>,
) -> Result<Json<SettingsResponse>, ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::InvalidRequest)?;
    let ftp = request
        .ftp_w
        .map(Ftp::new)
        .transpose()
        .map_err(|_| ApiError::InvalidRequest)?;
    Ok(Json(service.set_ftp(member, ftp).await?.into()))
}

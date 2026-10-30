use super::{
    auth::Principal,
    dto::{AuthResponse, CredentialsRequest, MemberResponse},
    error::ApiError,
};
use crate::application::auth_service::AuthService;
use axum::{
    Extension, Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
};

pub(super) async fn register(
    State(service): State<AuthService>,
    payload: Result<Json<CredentialsRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<AuthResponse>), ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::InvalidRequest)?;
    let session = service
        .register(&request.username, request.password)
        .await?;
    Ok((StatusCode::CREATED, Json(session.into())))
}

pub(super) async fn login(
    State(service): State<AuthService>,
    payload: Result<Json<CredentialsRequest>, JsonRejection>,
) -> Result<Json<AuthResponse>, ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::InvalidRequest)?;
    Ok(Json(
        service
            .login(&request.username, request.password)
            .await?
            .into(),
    ))
}

pub(super) async fn logout(
    State(service): State<AuthService>,
    Extension(principal): Extension<Principal>,
) -> Result<StatusCode, ApiError> {
    let Principal::Member { member, token } = principal else {
        return Err(ApiError::Forbidden);
    };
    service.logout(member.id, token).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn me(
    Extension(principal): Extension<Principal>,
) -> Result<Json<MemberResponse>, ApiError> {
    match principal {
        Principal::Member { member, .. } => Ok(Json(member.into())),
        Principal::Agent => Err(ApiError::Forbidden),
    }
}

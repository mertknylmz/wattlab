use super::dto::ErrorResponse;
use crate::application::error::ApplicationError;
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub(super) enum ApiError {
    InvalidRequest,
    Conflict,
    NotFound,
    Unavailable,
    Internal,
}

impl From<ApplicationError> for ApiError {
    fn from(error: ApplicationError) -> Self {
        match error {
            ApplicationError::EventConflict => Self::Conflict,
            ApplicationError::SessionNotFound => Self::NotFound,
            ApplicationError::StorageUnavailable => Self::Unavailable,
            ApplicationError::StorageFailure => Self::Internal,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error) = match self {
            Self::Conflict => (StatusCode::CONFLICT, "conflict"),
            Self::InvalidRequest => (StatusCode::BAD_REQUEST, "invalid_request"),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "storage_unavailable"),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };
        (status, Json(ErrorResponse { error })).into_response()
    }
}

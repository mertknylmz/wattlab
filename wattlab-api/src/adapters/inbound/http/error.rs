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
    Forbidden,
    Unavailable,
    Internal,
    /// Account errors carry a specific machine-readable code for the screens.
    Account(StatusCode, &'static str),
}

impl From<ApplicationError> for ApiError {
    fn from(error: ApplicationError) -> Self {
        match error {
            ApplicationError::EventConflict => Self::Conflict,
            ApplicationError::SessionNotFound | ApplicationError::CommandNotFound => Self::NotFound,
            ApplicationError::CommandConflict => Self::Conflict,
            ApplicationError::StorageUnavailable => Self::Unavailable,
            ApplicationError::StorageFailure => Self::Internal,
            ApplicationError::InvalidUsername => {
                Self::Account(StatusCode::BAD_REQUEST, "invalid_username")
            }
            ApplicationError::InvalidPassword => {
                Self::Account(StatusCode::BAD_REQUEST, "invalid_password")
            }
            ApplicationError::UsernameTaken => {
                Self::Account(StatusCode::CONFLICT, "username_taken")
            }
            ApplicationError::InvalidCredentials => {
                Self::Account(StatusCode::UNAUTHORIZED, "invalid_credentials")
            }
            ApplicationError::TooManyAttempts => {
                Self::Account(StatusCode::TOO_MANY_REQUESTS, "too_many_attempts")
            }
            ApplicationError::TrainerInUse => Self::Account(StatusCode::CONFLICT, "trainer_in_use"),
            ApplicationError::ActiveSession => {
                Self::Account(StatusCode::CONFLICT, "active_session")
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error) = match self {
            Self::Conflict => (StatusCode::CONFLICT, "conflict"),
            Self::InvalidRequest => (StatusCode::BAD_REQUEST, "invalid_request"),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            Self::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "storage_unavailable"),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
            Self::Account(status, error) => (status, error),
        };
        (status, Json(ErrorResponse { error })).into_response()
    }
}

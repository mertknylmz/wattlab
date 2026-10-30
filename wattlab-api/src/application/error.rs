use super::ports::repository_error::RepositoryError;
use std::{error::Error, fmt};

#[derive(Debug, PartialEq, Eq)]
pub enum ApplicationError {
    SessionNotFound,
    EventConflict,
    StorageUnavailable,
    StorageFailure,
    CommandNotFound,
    CommandConflict,
}

impl From<RepositoryError> for ApplicationError {
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::NotFound => Self::SessionNotFound,
            RepositoryError::Conflict => Self::EventConflict,
            RepositoryError::Unavailable => Self::StorageUnavailable,
            RepositoryError::Unexpected | RepositoryError::InvalidData => Self::StorageFailure,
        }
    }
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EventConflict => "Event or session conflict",
            Self::SessionNotFound => "Session not found",
            Self::StorageUnavailable => "Session storage unavailable",
            Self::StorageFailure => "Session storage operation failed",
            Self::CommandNotFound => "Command not found",
            Self::CommandConflict => "Command is no longer awaiting this result",
        })
    }
}
impl Error for ApplicationError {}

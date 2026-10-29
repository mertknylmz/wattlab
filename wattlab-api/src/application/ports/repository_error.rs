use std::{error::Error, fmt};

/// Infrastructure-neutral failures exposed by the persistence port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryError {
    NotFound,
    Conflict,
    Unavailable,
    Unexpected,
    InvalidData,
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotFound => "Session not found",
            Self::Conflict => "Event or session conflict",
            Self::Unavailable => "Session repository unavailable",
            Self::Unexpected => "Session repository operation failed",
            Self::InvalidData => "Session repository returned invalid state",
        })
    }
}
impl Error for RepositoryError {}

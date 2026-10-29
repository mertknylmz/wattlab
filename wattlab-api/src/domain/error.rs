use std::{error::Error, fmt};

#[derive(Debug, PartialEq, Eq)]
pub enum DomainError {
    FinishBeforeStart,
    EventConflict,
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::FinishBeforeStart => "Session finish cannot precede its start",
            Self::EventConflict => "Event conflicts with session state or existing contents",
        })
    }
}
impl Error for DomainError {}

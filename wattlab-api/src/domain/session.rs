use super::error::DomainError;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// A session independent of transport DTOs and persistence rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    id: Uuid,
    request_id: Uuid,
    started_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

impl Session {
    /// Reconstruct persisted state while preserving the domain's timeline invariant.
    pub fn restore(
        id: Uuid,
        request_id: Uuid,
        started_at: DateTime<Utc>,
        finished_at: Option<DateTime<Utc>>,
    ) -> Result<Self, DomainError> {
        if finished_at.is_some_and(|finish| finish < started_at) {
            return Err(DomainError::FinishBeforeStart);
        }
        Ok(Self {
            id,
            request_id,
            started_at,
            finished_at,
        })
    }

    /// Exact retries remain valid after finish; new or altered events do not.
    /// Returns whether a new event should be persisted.
    pub fn accept_event(&self, existing_matches: Option<bool>) -> Result<bool, DomainError> {
        match existing_matches {
            Some(true) => Ok(false),
            Some(false) => Err(DomainError::EventConflict),
            None if self.finished_at.is_none() => Ok(true),
            None => Err(DomainError::EventConflict),
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn request_id(&self) -> Uuid {
        self.request_id
    }
    pub fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }
    pub fn finished_at(&self) -> Option<DateTime<Utc>> {
        self.finished_at
    }
}

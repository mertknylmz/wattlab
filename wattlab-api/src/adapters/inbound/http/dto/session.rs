use crate::domain::session::Session;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub(crate) struct StartSessionRequest {
    pub request_id: Uuid,
    /// The member's start-session command the agent is executing.
    pub command_id: Uuid,
}

#[derive(Serialize)]
pub(crate) struct SessionResponse {
    id: Uuid,
    request_id: Uuid,
    started_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

impl From<Session> for SessionResponse {
    fn from(session: Session) -> Self {
        Self {
            id: session.id(),
            request_id: session.request_id(),
            started_at: session.started_at(),
            finished_at: session.finished_at(),
        }
    }
}

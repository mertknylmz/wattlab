use crate::{application::ports::repository_error::RepositoryError, domain::session::Session};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub(super) struct SessionRow {
    id: Uuid,
    request_id: Uuid,
    started_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

impl SessionRow {
    pub fn into_domain(self) -> Result<Session, RepositoryError> {
        Session::restore(self.id, self.request_id, self.started_at, self.finished_at)
            .map_err(|_| RepositoryError::InvalidData)
    }
}

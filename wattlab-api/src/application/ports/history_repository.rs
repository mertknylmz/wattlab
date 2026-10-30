use super::session_repository::RepositoryFuture;
use crate::domain::history::{SeriesPoint, SessionRecord};
use std::collections::HashMap;
use uuid::Uuid;

/// Read-only access to recorded sessions; never changes lifecycle state.
pub trait HistoryRepository: Send + Sync {
    /// Newest first by start time. `before` is the last session of the previous
    /// page; an unknown cursor yields an empty page.
    fn sessions_page(
        &self,
        before: Option<Uuid>,
        limit: u32,
    ) -> RepositoryFuture<'_, Vec<SessionRecord>>;
    fn session_record(&self, id: Uuid) -> RepositoryFuture<'_, Option<SessionRecord>>;
    /// One-second buckets ordered by time. Sessions without telemetry are absent.
    fn series(&self, ids: Vec<Uuid>) -> RepositoryFuture<'_, HashMap<Uuid, Vec<SeriesPoint>>>;
}

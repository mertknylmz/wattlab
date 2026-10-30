use super::session_repository::RepositoryFuture;
use crate::domain::history::{ActivityEntry, SeriesPoint, SessionRecord};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

/// Read-only access to one member's recorded sessions; never changes lifecycle state.
pub trait HistoryRepository: Send + Sync {
    /// Newest first by start time. `before` is the last session of the previous
    /// page; an unknown cursor yields an empty page.
    fn sessions_page(
        &self,
        owner: Uuid,
        before: Option<Uuid>,
        limit: u32,
    ) -> RepositoryFuture<'_, Vec<SessionRecord>>;
    fn session_record(&self, owner: Uuid, id: Uuid) -> RepositoryFuture<'_, Option<SessionRecord>>;
    /// One-second buckets ordered by time. Sessions without telemetry are absent.
    fn series(&self, ids: Vec<Uuid>) -> RepositoryFuture<'_, HashMap<Uuid, Vec<SeriesPoint>>>;
    /// Rides that started in `[from, to)`. Duration is clock time, not work.
    fn activity(
        &self,
        owner: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> RepositoryFuture<'_, Vec<ActivityEntry>>;
}

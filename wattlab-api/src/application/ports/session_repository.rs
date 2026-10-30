use super::repository_error::RepositoryError;
use crate::domain::{
    events::{ConnectionEvent, EventBatch, Measurement},
    session::Session,
};
use std::{future::Future, pin::Pin};
use uuid::Uuid;

pub type RepositoryFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, RepositoryError>> + Send + 'a>>;

/// Atomic lifecycle operations, not separate read/check/write steps.
/// Adapters must satisfy these semantics even under concurrent calls.
pub trait SessionRepository: Send + Sync {
    /// Return the same identity and lifecycle for all retries of a request ID.
    /// A new session captures the owner's current FTP.
    fn start_or_get(&self, request_id: Uuid, owner: Uuid) -> RepositoryFuture<'_, Session>;
    /// `owner: Some` restricts the lookup to that member's sessions.
    fn find(&self, id: Uuid, owner: Option<Uuid>) -> RepositoryFuture<'_, Option<Session>>;
    /// Preserve the first finish time. Never reopen a finished session.
    fn finish_once(&self, id: Uuid, owner: Option<Uuid>) -> RepositoryFuture<'_, Option<Session>>;
    /// Owner of the most recently started unfinished session, if any.
    fn active_owner(&self) -> RepositoryFuture<'_, Option<Uuid>>;
    /// Lock the session against finish; accept exact retries even after finish.
    /// A new event on a finished session or a reused key with different contents
    /// rejects the entire batch. Missing session is distinct from conflict.
    fn ingest_measurements(
        &self,
        id: Uuid,
        batch: EventBatch<Measurement>,
    ) -> RepositoryFuture<'_, usize>;
    fn ingest_connections(
        &self,
        id: Uuid,
        batch: EventBatch<ConnectionEvent>,
    ) -> RepositoryFuture<'_, usize>;
    /// The member's latest (or given) session and its current stream/connection,
    /// from one consistent read.
    fn live_snapshot(
        &self,
        owner: Uuid,
        session_id: Option<Uuid>,
    ) -> RepositoryFuture<'_, Option<crate::domain::live::LiveSnapshot>>;
}

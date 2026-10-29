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
    fn start_or_get(&self, request_id: Uuid) -> RepositoryFuture<'_, Session>;
    fn find(&self, id: Uuid) -> RepositoryFuture<'_, Option<Session>>;
    /// Preserve the first finish time. Never reopen a finished session.
    fn finish_once(&self, id: Uuid) -> RepositoryFuture<'_, Option<Session>>;
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
}

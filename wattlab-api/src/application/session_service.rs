use super::{error::ApplicationError, ports::session_repository::SessionRepository};
use crate::domain::{
    events::{ConnectionEvent, EventBatch, Measurement},
    session::Session,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct SessionService {
    repository: Arc<dyn SessionRepository>,
}

impl SessionService {
    pub fn new(repository: Arc<dyn SessionRepository>) -> Self {
        Self { repository }
    }

    pub async fn start(&self, request_id: Uuid) -> Result<Session, ApplicationError> {
        Ok(self.repository.start_or_get(request_id).await?)
    }
    pub async fn get(&self, id: Uuid) -> Result<Session, ApplicationError> {
        self.repository
            .find(id)
            .await?
            .ok_or(ApplicationError::SessionNotFound)
    }
    pub async fn finish(&self, id: Uuid) -> Result<Session, ApplicationError> {
        self.repository
            .finish_once(id)
            .await?
            .ok_or(ApplicationError::SessionNotFound)
    }
    pub async fn telemetry(
        &self,
        id: Uuid,
        batch: EventBatch<Measurement>,
    ) -> Result<usize, ApplicationError> {
        Ok(self.repository.ingest_measurements(id, batch).await?)
    }
    pub async fn connections(
        &self,
        id: Uuid,
        batch: EventBatch<ConnectionEvent>,
    ) -> Result<usize, ApplicationError> {
        Ok(self.repository.ingest_connections(id, batch).await?)
    }
    pub async fn live(
        &self,
    ) -> Result<Option<crate::domain::live::LiveSnapshot>, ApplicationError> {
        Ok(self.repository.live_snapshot(None).await?)
    }
    pub async fn live_session(
        &self,
        id: Uuid,
    ) -> Result<crate::domain::live::LiveSnapshot, ApplicationError> {
        self.repository
            .live_snapshot(Some(id))
            .await?
            .ok_or(ApplicationError::SessionNotFound)
    }
}

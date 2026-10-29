use std::{future::ready, sync::Arc};
use uuid::Uuid;
use wattlab_api::{
    application::ports::{
        repository_error::RepositoryError,
        session_repository::{RepositoryFuture, SessionRepository},
    },
    domain::session::Session,
};

#[derive(Default)]
pub struct StubRepository {
    pub session: Option<Session>,
    pub failure: Option<RepositoryError>,
}

impl SessionRepository for StubRepository {
    fn start_or_get(&self, _: Uuid) -> RepositoryFuture<'_, Session> {
        let result = match self.failure {
            Some(error) => Err(error),
            None => self.session.clone().ok_or(RepositoryError::Unexpected),
        };
        Box::pin(ready(result))
    }
    fn find(&self, _: Uuid) -> RepositoryFuture<'_, Option<Session>> {
        Box::pin(ready(match self.failure {
            Some(error) => Err(error),
            None => Ok(self.session.clone()),
        }))
    }
    fn finish_once(&self, id: Uuid) -> RepositoryFuture<'_, Option<Session>> {
        self.find(id)
    }
    fn ingest_measurements(
        &self,
        _: Uuid,
        batch: wattlab_api::domain::events::EventBatch<wattlab_api::domain::events::Measurement>,
    ) -> RepositoryFuture<'_, usize> {
        Box::pin(ready(self.failure.map_or(Ok(batch.events().len()), Err)))
    }
    fn ingest_connections(
        &self,
        _: Uuid,
        batch: wattlab_api::domain::events::EventBatch<
            wattlab_api::domain::events::ConnectionEvent,
        >,
    ) -> RepositoryFuture<'_, usize> {
        Box::pin(ready(self.failure.map_or(Ok(batch.events().len()), Err)))
    }
}

pub fn unavailable_repository() -> Arc<dyn SessionRepository> {
    Arc::new(StubRepository {
        session: None,
        failure: Some(RepositoryError::Unavailable),
    })
}

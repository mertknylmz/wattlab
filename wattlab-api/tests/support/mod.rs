use std::{collections::HashMap, future::ready, sync::Arc};
use uuid::Uuid;
use wattlab_api::{
    application::ports::{
        history_repository::HistoryRepository,
        repository_error::RepositoryError,
        session_repository::{RepositoryFuture, SessionRepository},
        settings_repository::SettingsRepository,
    },
    domain::{
        history::{SeriesPoint, SessionRecord},
        session::Session,
        settings::{AthleteSettings, Ftp},
    },
};

#[derive(Default)]
pub struct StubRepository {
    pub session: Option<Session>,
    pub failure: Option<RepositoryError>,
    pub records: Vec<SessionRecord>,
    pub points: Vec<SeriesPoint>,
}

impl StubRepository {
    fn result<T>(&self, value: T) -> RepositoryFuture<'_, T>
    where
        T: Send + 'static,
    {
        Box::pin(ready(self.failure.map_or(Ok(value), Err)))
    }
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
        self.result(self.session.clone())
    }
    fn finish_once(&self, id: Uuid) -> RepositoryFuture<'_, Option<Session>> {
        self.find(id)
    }
    fn ingest_measurements(
        &self,
        _: Uuid,
        batch: wattlab_api::domain::events::EventBatch<wattlab_api::domain::events::Measurement>,
    ) -> RepositoryFuture<'_, usize> {
        self.result(batch.events().len())
    }
    fn ingest_connections(
        &self,
        _: Uuid,
        batch: wattlab_api::domain::events::EventBatch<
            wattlab_api::domain::events::ConnectionEvent,
        >,
    ) -> RepositoryFuture<'_, usize> {
        self.result(batch.events().len())
    }
    fn live_snapshot(
        &self,
        _session_id: Option<Uuid>,
    ) -> RepositoryFuture<'_, Option<wattlab_api::domain::live::LiveSnapshot>> {
        self.result(None)
    }
}

impl HistoryRepository for StubRepository {
    fn sessions_page(
        &self,
        _: Option<Uuid>,
        limit: u32,
    ) -> RepositoryFuture<'_, Vec<SessionRecord>> {
        self.result(self.records.iter().take(limit as usize).cloned().collect())
    }
    fn session_record(&self, id: Uuid) -> RepositoryFuture<'_, Option<SessionRecord>> {
        self.result(self.records.iter().find(|r| r.session.id() == id).cloned())
    }
    fn series(&self, ids: Vec<Uuid>) -> RepositoryFuture<'_, HashMap<Uuid, Vec<SeriesPoint>>> {
        // Every requested session receives the same fixture points.
        self.result(
            ids.into_iter()
                .map(|id| (id, self.points.clone()))
                .collect(),
        )
    }
}

impl SettingsRepository for StubRepository {
    fn settings(&self) -> RepositoryFuture<'_, AthleteSettings> {
        self.result(AthleteSettings {
            ftp: Ftp::default_reference(),
            ftp_is_default: true,
            updated_at: chrono::DateTime::UNIX_EPOCH,
        })
    }
    fn update_ftp(&self, ftp: Option<Ftp>) -> RepositoryFuture<'_, AthleteSettings> {
        self.result(AthleteSettings {
            ftp: ftp.unwrap_or(Ftp::default_reference()),
            ftp_is_default: ftp.is_none(),
            updated_at: chrono::DateTime::UNIX_EPOCH,
        })
    }
}

#[allow(dead_code)] // Not every test binary exercises storage failures.
pub fn unavailable_repository() -> Arc<StubRepository> {
    Arc::new(StubRepository {
        failure: Some(RepositoryError::Unavailable),
        ..StubRepository::default()
    })
}

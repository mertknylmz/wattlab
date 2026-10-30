use super::{error::repository_error, session_row::SessionRow};
use crate::{
    application::ports::session_repository::{RepositoryFuture, SessionRepository},
    domain::{session::Session, settings::DEFAULT_FTP_W},
};
use sqlx::PgPool;
use uuid::Uuid;

pub struct PgSessionRepository {
    pub(super) pool: PgPool,
}

impl PgSessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl SessionRepository for PgSessionRepository {
    fn start_or_get(&self, request_id: Uuid, owner: Uuid) -> RepositoryFuture<'_, Session> {
        Box::pin(async move {
            let row: SessionRow = sqlx::query_as(
                "INSERT INTO training_session (id, request_id, member_id, ftp_w) \
                VALUES ($1, $2, $3, COALESCE( \
                    (SELECT ftp_w FROM athlete_settings WHERE member_id = $3), $4)) \
                ON CONFLICT (request_id) DO UPDATE SET request_id = EXCLUDED.request_id \
                RETURNING id, request_id, started_at, finished_at",
            )
            .bind(Uuid::new_v4())
            .bind(request_id)
            .bind(owner)
            .bind(DEFAULT_FTP_W)
            .fetch_one(&self.pool)
            .await
            .map_err(repository_error)?;
            row.into_domain()
        })
    }

    fn find(&self, id: Uuid, owner: Option<Uuid>) -> RepositoryFuture<'_, Option<Session>> {
        Box::pin(async move {
            let row: Option<SessionRow> = sqlx::query_as(
                "SELECT id, request_id, started_at, finished_at FROM training_session \
                WHERE id = $1 AND ($2::uuid IS NULL OR member_id = $2)",
            )
            .bind(id)
            .bind(owner)
            .fetch_optional(&self.pool)
            .await
            .map_err(repository_error)?;
            row.map(SessionRow::into_domain).transpose()
        })
    }

    fn finish_once(&self, id: Uuid, owner: Option<Uuid>) -> RepositoryFuture<'_, Option<Session>> {
        Box::pin(async move {
            // This atomic operation retains the row-lock boundary used by future ingestion.
            let row: Option<SessionRow> = sqlx::query_as(
                "UPDATE training_session SET finished_at = \
                COALESCE(finished_at, GREATEST(started_at, clock_timestamp())) \
                WHERE id = $1 AND ($2::uuid IS NULL OR member_id = $2) \
                RETURNING id, request_id, started_at, finished_at",
            )
            .bind(id)
            .bind(owner)
            .fetch_optional(&self.pool)
            .await
            .map_err(repository_error)?;
            row.map(SessionRow::into_domain).transpose()
        })
    }
    fn active_owner(&self) -> RepositoryFuture<'_, Option<Uuid>> {
        Box::pin(async move {
            sqlx::query_scalar(
                "SELECT member_id FROM training_session WHERE finished_at IS NULL \
                ORDER BY started_at DESC LIMIT 1",
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(repository_error)
        })
    }
    fn ingest_measurements(
        &self,
        id: Uuid,
        batch: crate::domain::events::EventBatch<crate::domain::events::Measurement>,
    ) -> RepositoryFuture<'_, usize> {
        Box::pin(super::event_storage::measurements(&self.pool, id, batch))
    }
    fn ingest_connections(
        &self,
        id: Uuid,
        batch: crate::domain::events::EventBatch<crate::domain::events::ConnectionEvent>,
    ) -> RepositoryFuture<'_, usize> {
        Box::pin(super::event_storage::connections(&self.pool, id, batch))
    }
    fn live_snapshot(
        &self,
        owner: Uuid,
        session_id: Option<Uuid>,
    ) -> RepositoryFuture<'_, Option<crate::domain::live::LiveSnapshot>> {
        Box::pin(super::live_storage::snapshot(&self.pool, owner, session_id))
    }
}

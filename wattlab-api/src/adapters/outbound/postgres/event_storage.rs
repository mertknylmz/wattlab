//! A batch holds the session row lock until commit, serializing with finish.
use super::{error::repository_error, session_row::SessionRow};
use crate::{
    application::ports::repository_error::RepositoryError,
    domain::{
        events::{ConnectionEvent, EventBatch, Measurement},
        session::Session,
    },
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

async fn lock_session(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Session, RepositoryError> {
    let row: Option<SessionRow> = sqlx::query_as("SELECT id, request_id, started_at, finished_at FROM training_session WHERE id=$1 FOR UPDATE")
        .bind(id).fetch_optional(&mut **tx).await.map_err(repository_error)?;
    row.ok_or(RepositoryError::NotFound)?.into_domain()
}

pub(super) async fn measurements(
    pool: &PgPool,
    id: Uuid,
    batch: EventBatch<Measurement>,
) -> Result<usize, RepositoryError> {
    let mut tx = pool.begin().await.map_err(repository_error)?;
    let session = lock_session(&mut tx, id).await?;
    for event in batch.events() {
        let e = event.envelope();
        let same: Option<bool> = sqlx::query_scalar("SELECT schema_version = $4 AND connection_id = $5 AND observed_at = $6 AND source = $7 AND speed_kmh IS NOT DISTINCT FROM $8 AND cadence_rpm IS NOT DISTINCT FROM $9 AND power_w IS NOT DISTINCT FROM $10 FROM telemetry_event WHERE session_id=$1 AND stream_id=$2 AND sequence=$3")
            .bind(id)
            .bind(e.stream_id())
            .bind(e.sequence())
            .bind(1_i32)
            .bind(e.connection_id())
            .bind(e.observed_at())
            .bind(event.source().as_str())
            .bind(event.speed_kmh())
            .bind(event.cadence_rpm())
            .bind(event.power_w())
            .fetch_optional(&mut *tx).await.map_err(repository_error)?;
        // Domain owns the post-finish/replay rule; the adapter owns atomicity.
        if session
            .accept_event(same)
            .map_err(|_| RepositoryError::Conflict)?
        {
            sqlx::query("INSERT INTO telemetry_event (session_id, stream_id, sequence, schema_version, connection_id, observed_at, source, speed_kmh, cadence_rpm, power_w) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)")
            .bind(id)
            .bind(e.stream_id())
            .bind(e.sequence())
            .bind(1_i32)
            .bind(e.connection_id())
            .bind(e.observed_at())
            .bind(event.source().as_str())
            .bind(event.speed_kmh())
            .bind(event.cadence_rpm())
            .bind(event.power_w())
                .execute(&mut *tx).await.map_err(repository_error)?;
        }
    }
    tx.commit().await.map_err(repository_error)?;
    Ok(batch.events().len())
}

pub(super) async fn connections(
    pool: &PgPool,
    id: Uuid,
    batch: EventBatch<ConnectionEvent>,
) -> Result<usize, RepositoryError> {
    let mut tx = pool.begin().await.map_err(repository_error)?;
    let session = lock_session(&mut tx, id).await?;
    for event in batch.events() {
        let e = event.envelope();
        let same: Option<bool> = sqlx::query_scalar("SELECT schema_version = $4 AND connection_id = $5 AND observed_at = $6 AND state = $7 FROM connection_event WHERE session_id=$1 AND stream_id=$2 AND sequence=$3")
            .bind(id)
            .bind(e.stream_id())
            .bind(e.sequence())
            .bind(1_i32)
            .bind(e.connection_id())
            .bind(e.observed_at())
            .bind(event.state().as_str())
            .fetch_optional(&mut *tx).await.map_err(repository_error)?;
        // Domain owns the post-finish/replay rule; the adapter owns atomicity.
        if session
            .accept_event(same)
            .map_err(|_| RepositoryError::Conflict)?
        {
            sqlx::query("INSERT INTO connection_event (session_id, stream_id, sequence, schema_version, connection_id, observed_at, state) VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .bind(id)
            .bind(e.stream_id())
            .bind(e.sequence())
            .bind(1_i32)
            .bind(e.connection_id())
            .bind(e.observed_at())
            .bind(event.state().as_str())
                .execute(&mut *tx).await.map_err(repository_error)?;
        }
    }
    tx.commit().await.map_err(repository_error)?;
    Ok(batch.events().len())
}

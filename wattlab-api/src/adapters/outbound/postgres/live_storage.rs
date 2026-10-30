use super::error::repository_error;
use crate::{
    application::ports::repository_error::RepositoryError,
    domain::{
        live::{LiveMeasurement, LiveSnapshot},
        session::Session,
    },
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct LiveRow {
    id: Uuid,
    request_id: Uuid,
    started_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
    state: Option<String>,
    captured_at: DateTime<Utc>,
    ftms_received: Option<DateTime<Utc>>,
    speed_kmh: Option<f64>,
    cadence_rpm: Option<f64>,
    ftms_power: Option<i32>,
    cycling_received: Option<DateTime<Utc>>,
    cycling_power: Option<i32>,
}
pub(super) async fn snapshot(
    pool: &PgPool,
    owner: Uuid,
    session_id: Option<Uuid>,
) -> Result<Option<LiveSnapshot>, RepositoryError> {
    // One statement gives a consistent MVCC snapshot, including concurrent finish.
    // A stream is ranked by its first receipt, so delayed old-stream packets cannot
    // replace the newer stream. Sequence handles out-of-order packets within it.
    let row: Option<LiveRow> = sqlx::query_as(
        r#"
        SELECT s.*, c.state, clock_timestamp() AS captured_at,
               f.received_at AS ftms_received, f.speed_kmh, f.cadence_rpm, f.power_w AS ftms_power,
               p.received_at AS cycling_received, p.power_w AS cycling_power
        FROM (SELECT * FROM training_session WHERE member_id=$2 AND ($1::uuid IS NULL OR id=$1) ORDER BY started_at DESC,id DESC LIMIT 1) s
        LEFT JOIN LATERAL (
            SELECT stream_id FROM connection_event WHERE session_id=s.id AND received_at IS NOT NULL
            GROUP BY stream_id ORDER BY min(received_at) DESC, stream_id DESC LIMIT 1
        ) stream ON true
        LEFT JOIN LATERAL (
            SELECT state,stream_id,connection_id FROM connection_event
            WHERE session_id=s.id AND stream_id=stream.stream_id AND received_at IS NOT NULL
            ORDER BY sequence DESC LIMIT 1
        ) c ON true
        LEFT JOIN LATERAL (
            SELECT received_at,speed_kmh,cadence_rpm,power_w FROM telemetry_event
            WHERE session_id=s.id AND stream_id=c.stream_id AND connection_id=c.connection_id
                AND source='ftms' AND received_at IS NOT NULL
            ORDER BY sequence DESC LIMIT 1
        ) f ON true
        LEFT JOIN LATERAL (
            SELECT received_at,power_w FROM telemetry_event
            WHERE session_id=s.id AND stream_id=c.stream_id AND connection_id=c.connection_id
                AND source='cycling_power' AND received_at IS NOT NULL
            ORDER BY sequence DESC LIMIT 1
        ) p ON true
    "#,
    )
    .bind(session_id).bind(owner).fetch_optional(pool)
    .await
    .map_err(repository_error)?;
    row.map(|r| {
        Ok(LiveSnapshot {
            session: Session::restore(r.id, r.request_id, r.started_at, r.finished_at)
                .map_err(|_| RepositoryError::InvalidData)?,
            connection_state: r.state,
            captured_at: r.captured_at,
            ftms: r.ftms_received.map(|received_at| LiveMeasurement {
                received_at,
                speed_kmh: r.speed_kmh,
                cadence_rpm: r.cadence_rpm,
                power_w: r.ftms_power,
            }),
            cycling_power: r.cycling_received.map(|received_at| LiveMeasurement {
                received_at,
                speed_kmh: None,
                cadence_rpm: None,
                power_w: r.cycling_power,
            }),
        })
    })
    .transpose()
}

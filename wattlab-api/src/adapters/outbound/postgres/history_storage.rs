use super::{PgSessionRepository, error::repository_error, session_row::SessionRow};
use crate::{
    application::ports::{
        history_repository::HistoryRepository, repository_error::RepositoryError,
        session_repository::RepositoryFuture,
    },
    domain::history::{ActivityEntry, SeriesPoint, SessionRecord},
};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct RecordRow {
    #[sqlx(flatten)]
    session: SessionRow,
    ftp_w: Option<i32>,
}
impl RecordRow {
    fn into_domain(self) -> Result<SessionRecord, RepositoryError> {
        Ok(SessionRecord {
            session: self.session.into_domain()?,
            ftp_w: self.ftp_w,
        })
    }
}

#[derive(sqlx::FromRow)]
struct PointRow {
    session_id: Uuid,
    second: DateTime<Utc>,
    ftms_power_w: Option<i32>,
    cycling_power_w: Option<i32>,
    cadence_rpm: Option<f64>,
    speed_kmh: Option<f64>,
}

impl HistoryRepository for PgSessionRepository {
    fn sessions_page(
        &self,
        owner: Uuid,
        before: Option<Uuid>,
        limit: u32,
    ) -> RepositoryFuture<'_, Vec<SessionRecord>> {
        Box::pin(async move {
            // An unknown cursor makes the row comparison NULL, which yields no rows.
            let rows: Vec<RecordRow> = sqlx::query_as(
                "SELECT id, request_id, started_at, finished_at, ftp_w FROM training_session \
                WHERE member_id = $3 AND ($1::uuid IS NULL OR (started_at, id) < \
                    (SELECT started_at, id FROM training_session WHERE id = $1 AND member_id = $3)) \
                ORDER BY started_at DESC, id DESC LIMIT $2",
            )
            .bind(before)
            .bind(i64::from(limit))
            .bind(owner)
            .fetch_all(&self.pool)
            .await
            .map_err(repository_error)?;
            rows.into_iter().map(RecordRow::into_domain).collect()
        })
    }

    fn session_record(&self, owner: Uuid, id: Uuid) -> RepositoryFuture<'_, Option<SessionRecord>> {
        Box::pin(async move {
            let row: Option<RecordRow> = sqlx::query_as(
                "SELECT id, request_id, started_at, finished_at, ftp_w \
                FROM training_session WHERE id = $1 AND member_id = $2",
            )
            .bind(id)
            .bind(owner)
            .fetch_optional(&self.pool)
            .await
            .map_err(repository_error)?;
            row.map(RecordRow::into_domain).transpose()
        })
    }

    fn series(&self, ids: Vec<Uuid>) -> RepositoryFuture<'_, HashMap<Uuid, Vec<SeriesPoint>>> {
        Box::pin(async move {
            if ids.is_empty() {
                return Ok(HashMap::new());
            }
            // Buckets use the trainer-side observation clock; every stream of the
            // session contributes, and sources are averaged separately per second.
            let rows: Vec<PointRow> = sqlx::query_as(
                r#"
                SELECT session_id, date_trunc('second', observed_at) AS second,
                    round(avg(power_w) FILTER (WHERE source = 'ftms'))::integer AS ftms_power_w,
                    round(avg(power_w) FILTER (WHERE source = 'cycling_power'))::integer AS cycling_power_w,
                    avg(cadence_rpm) AS cadence_rpm,
                    avg(speed_kmh) AS speed_kmh
                FROM telemetry_event
                WHERE session_id = ANY($1)
                GROUP BY session_id, second
                ORDER BY session_id, second
                "#,
            )
            .bind(&ids)
            .fetch_all(&self.pool)
            .await
            .map_err(repository_error)?;
            let mut result: HashMap<Uuid, Vec<SeriesPoint>> = HashMap::new();
            for row in rows {
                result.entry(row.session_id).or_default().push(SeriesPoint {
                    second: row.second,
                    ftms_power_w: row.ftms_power_w,
                    cycling_power_w: row.cycling_power_w,
                    cadence_rpm: row.cadence_rpm,
                    speed_kmh: row.speed_kmh,
                });
            }
            Ok(result)
        })
    }

    fn activity(
        &self,
        owner: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> RepositoryFuture<'_, Vec<ActivityEntry>> {
        Box::pin(async move {
            let rows: Vec<(DateTime<Utc>, i64)> = sqlx::query_as(
                "SELECT started_at, \
                GREATEST(0, floor(extract(epoch from (coalesce(finished_at, clock_timestamp()) - started_at))))::bigint \
                FROM training_session \
                WHERE member_id = $1 AND started_at >= $2 AND started_at < $3 \
                ORDER BY started_at",
            )
            .bind(owner)
            .bind(from)
            .bind(to)
            .fetch_all(&self.pool)
            .await
            .map_err(repository_error)?;
            Ok(rows
                .into_iter()
                .map(|(started_at, duration_seconds)| ActivityEntry {
                    started_at,
                    duration_seconds,
                })
                .collect())
        })
    }
}

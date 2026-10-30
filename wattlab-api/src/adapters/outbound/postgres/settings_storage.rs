use super::{PgSessionRepository, error::repository_error};
use crate::{
    application::ports::{
        repository_error::RepositoryError, session_repository::RepositoryFuture,
        settings_repository::SettingsRepository,
    },
    domain::settings::{AthleteSettings, DEFAULT_FTP_W, Ftp},
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct SettingsRow {
    ftp_w: i32,
    ftp_is_default: bool,
    updated_at: DateTime<Utc>,
}
impl SettingsRow {
    fn into_domain(self) -> Result<AthleteSettings, RepositoryError> {
        Ok(AthleteSettings {
            ftp: Ftp::new(self.ftp_w.into()).map_err(|_| RepositoryError::InvalidData)?,
            ftp_is_default: self.ftp_is_default,
            updated_at: self.updated_at,
        })
    }
}

impl SettingsRepository for PgSessionRepository {
    fn settings(&self, member: Uuid) -> RepositoryFuture<'_, AthleteSettings> {
        Box::pin(async move {
            // Without a stored row the member sees the default since account creation.
            let row: Option<SettingsRow> = sqlx::query_as(
                "SELECT COALESCE(s.ftp_w, $2) AS ftp_w, \
                COALESCE(s.ftp_is_default, true) AS ftp_is_default, \
                COALESCE(s.updated_at, m.created_at) AS updated_at \
                FROM member m LEFT JOIN athlete_settings s ON s.member_id = m.id WHERE m.id = $1",
            )
            .bind(member)
            .bind(DEFAULT_FTP_W)
            .fetch_optional(&self.pool)
            .await
            .map_err(repository_error)?;
            row.ok_or(RepositoryError::InvalidData)?.into_domain()
        })
    }

    fn update_ftp(&self, member: Uuid, ftp: Option<Ftp>) -> RepositoryFuture<'_, AthleteSettings> {
        Box::pin(async move {
            let row: Option<SettingsRow> = sqlx::query_as(
                "INSERT INTO athlete_settings (member_id, ftp_w, ftp_is_default) VALUES ($3, $1, $2) \
                ON CONFLICT (member_id) DO UPDATE SET ftp_w = EXCLUDED.ftp_w, \
                ftp_is_default = EXCLUDED.ftp_is_default, updated_at = clock_timestamp() \
                RETURNING ftp_w, ftp_is_default, updated_at",
            )
            .bind(ftp.map_or(DEFAULT_FTP_W, Ftp::watts))
            .bind(ftp.is_none())
            .bind(member)
            .fetch_optional(&self.pool)
            .await
            .map_err(repository_error)?;
            row.ok_or(RepositoryError::InvalidData)?.into_domain()
        })
    }
}

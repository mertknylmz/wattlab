use super::{PgSessionRepository, error::repository_error};
use crate::{
    application::ports::{
        repository_error::RepositoryError, session_repository::RepositoryFuture,
        settings_repository::SettingsRepository,
    },
    domain::settings::{AthleteSettings, DEFAULT_FTP_W, Ftp},
};
use chrono::{DateTime, Utc};

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
    fn settings(&self) -> RepositoryFuture<'_, AthleteSettings> {
        Box::pin(async move {
            let row: Option<SettingsRow> = sqlx::query_as(
                "SELECT ftp_w, ftp_is_default, updated_at FROM athlete_settings WHERE id = 1",
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(repository_error)?;
            row.ok_or(RepositoryError::InvalidData)?.into_domain()
        })
    }

    fn update_ftp(&self, ftp: Option<Ftp>) -> RepositoryFuture<'_, AthleteSettings> {
        Box::pin(async move {
            let row: Option<SettingsRow> = sqlx::query_as(
                "UPDATE athlete_settings SET ftp_w = $1, ftp_is_default = $2, \
                updated_at = clock_timestamp() WHERE id = 1 \
                RETURNING ftp_w, ftp_is_default, updated_at",
            )
            .bind(ftp.map_or(DEFAULT_FTP_W, Ftp::watts))
            .bind(ftp.is_none())
            .fetch_optional(&self.pool)
            .await
            .map_err(repository_error)?;
            row.ok_or(RepositoryError::InvalidData)?.into_domain()
        })
    }
}

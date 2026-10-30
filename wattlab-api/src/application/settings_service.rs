use super::{error::ApplicationError, ports::settings_repository::SettingsRepository};
use crate::domain::settings::{AthleteSettings, Ftp};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct SettingsService {
    repository: Arc<dyn SettingsRepository>,
}

impl SettingsService {
    pub fn new(repository: Arc<dyn SettingsRepository>) -> Self {
        Self { repository }
    }
    pub async fn get(&self, member: Uuid) -> Result<AthleteSettings, ApplicationError> {
        Ok(self.repository.settings(member).await?)
    }
    pub async fn set_ftp(
        &self,
        member: Uuid,
        ftp: Option<Ftp>,
    ) -> Result<AthleteSettings, ApplicationError> {
        Ok(self.repository.update_ftp(member, ftp).await?)
    }
}

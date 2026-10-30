use super::{error::ApplicationError, ports::settings_repository::SettingsRepository};
use crate::domain::settings::{AthleteSettings, Ftp};
use std::sync::Arc;

#[derive(Clone)]
pub struct SettingsService {
    repository: Arc<dyn SettingsRepository>,
}

impl SettingsService {
    pub fn new(repository: Arc<dyn SettingsRepository>) -> Self {
        Self { repository }
    }
    pub async fn get(&self) -> Result<AthleteSettings, ApplicationError> {
        Ok(self.repository.settings().await?)
    }
    pub async fn set_ftp(&self, ftp: Option<Ftp>) -> Result<AthleteSettings, ApplicationError> {
        Ok(self.repository.update_ftp(ftp).await?)
    }
}

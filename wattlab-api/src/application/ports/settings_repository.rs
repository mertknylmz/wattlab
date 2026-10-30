use super::session_repository::RepositoryFuture;
use crate::domain::settings::{AthleteSettings, Ftp};

/// Settings of the single local athlete. A missing row is invalid storage state.
pub trait SettingsRepository: Send + Sync {
    fn settings(&self) -> RepositoryFuture<'_, AthleteSettings>;
    /// `None` restores the default display reference.
    fn update_ftp(&self, ftp: Option<Ftp>) -> RepositoryFuture<'_, AthleteSettings>;
}

use super::session_repository::RepositoryFuture;
use crate::domain::settings::{AthleteSettings, Ftp};

use uuid::Uuid;

/// Per-member settings. A member without stored settings gets the defaults.
pub trait SettingsRepository: Send + Sync {
    fn settings(&self, member: Uuid) -> RepositoryFuture<'_, AthleteSettings>;
    /// `None` restores the default display reference.
    fn update_ftp(&self, member: Uuid, ftp: Option<Ftp>) -> RepositoryFuture<'_, AthleteSettings>;
}

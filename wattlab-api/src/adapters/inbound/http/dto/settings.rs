use crate::domain::settings::AthleteSettings;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// `ftp_w: null` restores the default reference; the field itself is required.
#[derive(Deserialize)]
pub(crate) struct UpdateSettingsRequest {
    // An explicit deserializer disables serde's implicit "missing means None".
    #[serde(deserialize_with = "Option::deserialize")]
    pub ftp_w: Option<i64>,
}

#[derive(Serialize)]
pub(crate) struct SettingsResponse {
    ftp_w: i32,
    ftp_is_default: bool,
    updated_at: DateTime<Utc>,
}
impl From<AthleteSettings> for SettingsResponse {
    fn from(s: AthleteSettings) -> Self {
        Self {
            ftp_w: s.ftp.watts(),
            ftp_is_default: s.ftp_is_default,
            updated_at: s.updated_at,
        }
    }
}

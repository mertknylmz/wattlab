use super::SessionResponse;
use crate::domain::live::LiveSnapshot;
use chrono::{DateTime, Utc};
use serde::Serialize;
#[derive(Serialize)]
pub(crate) struct LiveResponse {
    session: Option<SessionResponse>,
    connection_state: Option<String>,
    fresh: bool,
    speed_kmh: Option<f64>,
    cadence_rpm: Option<f64>,
    power_w: Option<i32>,
    power_source: Option<&'static str>,
    ftms_received_at: Option<DateTime<Utc>>,
    cycling_power_received_at: Option<DateTime<Utc>>,
    captured_at: Option<DateTime<Utc>>,
}
impl From<Option<LiveSnapshot>> for LiveResponse {
    fn from(snapshot: Option<LiveSnapshot>) -> Self {
        match snapshot {
            Some(s) => {
                let m = s.metrics();
                Self {
                    session: Some(s.session.into()),
                    connection_state: s.connection_state,
                    fresh: m.fresh,
                    speed_kmh: m.speed_kmh,
                    cadence_rpm: m.cadence_rpm,
                    power_w: m.power_w,
                    power_source: m.power_source,
                    ftms_received_at: s.ftms.map(|m| m.received_at),
                    cycling_power_received_at: s.cycling_power.map(|m| m.received_at),
                    captured_at: Some(s.captured_at),
                }
            }
            None => Self {
                session: None,
                connection_state: None,
                fresh: false,
                speed_kmh: None,
                cadence_rpm: None,
                power_w: None,
                power_source: None,
                ftms_received_at: None,
                cycling_power_received_at: None,
                captured_at: None,
            },
        }
    }
}

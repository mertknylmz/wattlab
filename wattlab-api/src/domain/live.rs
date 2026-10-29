use super::session::Session;
use chrono::{DateTime, Utc};

/// Read-side projection. Receipt times belong to the local backend clock.
#[derive(Debug, Clone)]
pub struct LiveSnapshot {
    pub session: Session,
    pub connection_state: Option<String>,
    pub ftms: Option<LiveMeasurement>,
    pub cycling_power: Option<LiveMeasurement>,
    pub captured_at: DateTime<Utc>,
}
#[derive(Debug, Clone)]
pub struct LiveMeasurement {
    pub speed_kmh: Option<f64>,
    pub cadence_rpm: Option<f64>,
    pub power_w: Option<i32>,
    pub received_at: DateTime<Utc>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct LiveMetrics {
    pub speed_kmh: Option<f64>,
    pub cadence_rpm: Option<f64>,
    pub power_w: Option<i32>,
    pub power_source: Option<&'static str>,
    pub fresh: bool,
}
impl LiveSnapshot {
    /// Five seconds is a display freshness threshold, not a BLE disconnect test.
    pub fn metrics(&self) -> LiveMetrics {
        let mut result = LiveMetrics {
            speed_kmh: None,
            cadence_rpm: None,
            power_w: None,
            power_source: None,
            fresh: false,
        };
        if self.session.finished_at().is_some()
            || self.connection_state.as_deref() != Some("streaming")
        {
            return result;
        }
        let fresh = |m: &&LiveMeasurement| {
            let age = self
                .captured_at
                .signed_duration_since(m.received_at)
                .num_milliseconds();
            (0..=5000).contains(&age)
        };
        let ftms = self.ftms.as_ref().filter(fresh);
        let cycling = self.cycling_power.as_ref().filter(fresh);
        result.fresh = ftms.is_some() || cycling.is_some();
        if let Some(m) = ftms {
            result.speed_kmh = m.speed_kmh;
            result.cadence_rpm = m.cadence_rpm;
        }
        if let Some(m) = cycling.filter(|m| m.power_w.is_some()) {
            result.power_w = m.power_w;
            result.power_source = Some("cycling_power");
        } else if let Some(m) = ftms.filter(|m| m.power_w.is_some()) {
            result.power_w = m.power_w;
            result.power_source = Some("ftms");
        }
        result
    }
}

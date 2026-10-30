use super::{session::Session, settings::Ftp};
use chrono::{DateTime, Utc};

/// One observed-time second of a session. Sources stay separate until display
/// selection so that a missing Cycling Power second can fall back to FTMS.
#[derive(Debug, Clone, PartialEq)]
pub struct SeriesPoint {
    pub second: DateTime<Utc>,
    pub ftms_power_w: Option<i32>,
    pub cycling_power_w: Option<i32>,
    pub cadence_rpm: Option<f64>,
    pub speed_kmh: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerSource {
    CyclingPower,
    Ftms,
}

impl SeriesPoint {
    /// Same priority as the live view: Cycling Power first, FTMS as fallback.
    pub fn power(&self) -> Option<(i32, PowerSource)> {
        self.cycling_power_w
            .map(|watts| (watts, PowerSource::CyclingPower))
            .or(self.ftms_power_w.map(|watts| (watts, PowerSource::Ftms)))
    }
}

/// Aggregates over seconds that actually carried a value. Missing seconds are
/// excluded from averages; real zero readings are included.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionSummary {
    pub duration_seconds: Option<i64>,
    pub recorded_seconds: usize,
    pub power_seconds: usize,
    pub average_power_w: Option<f64>,
    pub max_power_w: Option<i32>,
    pub average_cadence_rpm: Option<f64>,
    pub average_speed_kmh: Option<f64>,
    pub energy_kj: f64,
    /// Seconds per zone Z0..Z6 against `zone_ftp`; negative power is excluded.
    pub zone_seconds: Option<[usize; 7]>,
    pub zone_ftp: Option<Ftp>,
}

/// A session with the display FTP captured when it started, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub session: Session,
    pub ftp_w: Option<i32>,
}

/// One ride in a calendar window. The screen groups these by the local day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityEntry {
    pub started_at: DateTime<Utc>,
    pub duration_seconds: i64,
}

pub fn summarize(
    session: &Session,
    points: &[SeriesPoint],
    zone_ftp: Option<Ftp>,
) -> SessionSummary {
    let powers: Vec<i32> = points
        .iter()
        .filter_map(|p| p.power().map(|(w, _)| w))
        .collect();
    let zone_seconds = zone_ftp.map(|ftp| {
        let mut seconds = [0; 7];
        for zone in powers.iter().filter_map(|&w| ftp.zone(w)) {
            seconds[zone] += 1;
        }
        seconds
    });
    SessionSummary {
        zone_seconds,
        zone_ftp,
        duration_seconds: session
            .finished_at()
            .map(|finish| (finish - session.started_at()).num_seconds()),
        recorded_seconds: points.len(),
        power_seconds: powers.len(),
        average_power_w: average(powers.iter().map(|&w| f64::from(w))),
        max_power_w: powers.iter().copied().max(),
        average_cadence_rpm: average(points.iter().filter_map(|p| p.cadence_rpm)),
        average_speed_kmh: average(points.iter().filter_map(|p| p.speed_kmh)),
        // One-second buckets: watts * 1 s = joules. Negative power is not work done.
        // Float `sum` of an empty iterator is -0.0; work is never negative.
        energy_kj: powers
            .iter()
            .fold(0.0, |total, &w| total + f64::from(w.max(0)))
            / 1000.0,
    }
}

fn average(values: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, count) = values.fold((0.0, 0usize), |(sum, count), v| (sum + v, count + 1));
    (count > 0).then(|| sum / count as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use uuid::Uuid;

    fn at(second: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 30, 10, 0, second).unwrap()
    }
    fn point(second: u32, ftms: Option<i32>, cycling: Option<i32>) -> SeriesPoint {
        SeriesPoint {
            second: at(second),
            ftms_power_w: ftms,
            cycling_power_w: cycling,
            cadence_rpm: None,
            speed_kmh: None,
        }
    }
    fn session(finished: Option<u32>) -> Session {
        Session::restore(Uuid::new_v4(), Uuid::new_v4(), at(0), finished.map(at)).unwrap()
    }

    #[test]
    fn cycling_power_wins_and_ftms_fills_missing_seconds() {
        assert_eq!(
            point(0, Some(100), Some(110)).power(),
            Some((110, PowerSource::CyclingPower))
        );
        assert_eq!(
            point(0, Some(100), None).power(),
            Some((100, PowerSource::Ftms))
        );
        assert_eq!(point(0, None, None).power(), None);
    }

    #[test]
    fn zero_counts_in_averages_but_missing_does_not() {
        let mut cadence = point(2, None, None);
        cadence.cadence_rpm = Some(90.0);
        let summary = summarize(
            &session(Some(10)),
            &[point(0, Some(200), None), point(1, None, Some(0)), cadence],
            None,
        );
        assert_eq!(summary.duration_seconds, Some(10));
        assert_eq!(summary.recorded_seconds, 3);
        assert_eq!(summary.power_seconds, 2);
        assert_eq!(summary.average_power_w, Some(100.0));
        assert_eq!(summary.max_power_w, Some(200));
        assert_eq!(summary.average_cadence_rpm, Some(90.0));
        assert_eq!(summary.average_speed_kmh, None);
        assert_eq!(summary.energy_kj, 0.2);
    }

    #[test]
    fn empty_or_active_sessions_have_no_invented_values() {
        let summary = summarize(&session(None), &[], None);
        assert_eq!(summary.duration_seconds, None);
        assert_eq!(summary.average_power_w, None);
        assert_eq!(summary.max_power_w, None);
        assert_eq!(summary.energy_kj, 0.0);
        assert!(summary.energy_kj.is_sign_positive());
    }

    #[test]
    fn negative_power_is_averaged_but_not_counted_as_work() {
        let summary = summarize(
            &session(Some(2)),
            &[point(0, Some(-20), None), point(1, Some(40), None)],
            None,
        );
        assert_eq!(summary.average_power_w, Some(10.0));
        assert_eq!(summary.energy_kj, 0.04);
    }

    #[test]
    fn zone_time_counts_power_seconds_and_skips_negative_values() {
        let summary = summarize(
            &session(Some(4)),
            &[
                point(0, Some(0), None),
                point(1, Some(150), None),
                point(2, None, Some(300)),
                point(3, Some(-5), None),
                point(4, None, None),
            ],
            Some(Ftp::new(250).unwrap()),
        );
        assert_eq!(summary.zone_seconds, Some([1, 0, 1, 0, 0, 0, 1]));
        assert_eq!(summarize(&session(None), &[], None).zone_seconds, None);
    }
}

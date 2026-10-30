use super::SessionResponse;
use crate::{
    application::history_service::{SessionPage, SummarizedSession, ZoneReference},
    domain::history::{PowerSource, SeriesPoint, SessionSummary},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub(crate) struct PageQuery {
    pub limit: Option<String>,
    pub before: Option<String>,
}

#[derive(Serialize)]
struct SummaryDto {
    duration_seconds: Option<i64>,
    recorded_seconds: usize,
    power_seconds: usize,
    average_power_w: Option<f64>,
    max_power_w: Option<i32>,
    average_cadence_rpm: Option<f64>,
    average_speed_kmh: Option<f64>,
    energy_kj: f64,
    zone_ftp_w: Option<i32>,
    zone_seconds: Option<[usize; 7]>,
}
impl From<SessionSummary> for SummaryDto {
    fn from(s: SessionSummary) -> Self {
        Self {
            zone_ftp_w: s.zone_ftp.map(|ftp| ftp.watts()),
            zone_seconds: s.zone_seconds,
            duration_seconds: s.duration_seconds,
            recorded_seconds: s.recorded_seconds,
            power_seconds: s.power_seconds,
            average_power_w: s.average_power_w,
            max_power_w: s.max_power_w,
            average_cadence_rpm: s.average_cadence_rpm,
            average_speed_kmh: s.average_speed_kmh,
            energy_kj: s.energy_kj,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct SessionSummaryResponse {
    session: SessionResponse,
    ftp_w: Option<i32>,
    zone_reference: &'static str,
    summary: SummaryDto,
}
impl From<SummarizedSession> for SessionSummaryResponse {
    fn from(s: SummarizedSession) -> Self {
        Self {
            session: s.record.session.into(),
            ftp_w: s.record.ftp_w,
            zone_reference: match s.zone_reference {
                ZoneReference::Session => "session",
                ZoneReference::Current => "current",
            },
            summary: s.summary.into(),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct SessionPageResponse {
    sessions: Vec<SessionSummaryResponse>,
    next_before: Option<Uuid>,
}
impl From<SessionPage> for SessionPageResponse {
    fn from(page: SessionPage) -> Self {
        Self {
            sessions: page.sessions.into_iter().map(Into::into).collect(),
            next_before: page.next_before,
        }
    }
}

#[derive(Serialize)]
struct PointDto {
    t: DateTime<Utc>,
    power_w: Option<i32>,
    power_source: Option<&'static str>,
    cadence_rpm: Option<f64>,
    speed_kmh: Option<f64>,
}
impl From<SeriesPoint> for PointDto {
    fn from(p: SeriesPoint) -> Self {
        let power = p.power();
        Self {
            t: p.second,
            power_w: power.map(|(watts, _)| watts),
            power_source: power.map(|(_, source)| match source {
                PowerSource::CyclingPower => "cycling_power",
                PowerSource::Ftms => "ftms",
            }),
            cadence_rpm: p.cadence_rpm,
            speed_kmh: p.speed_kmh,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct SeriesResponse {
    #[serde(flatten)]
    detail: SessionSummaryResponse,
    points: Vec<PointDto>,
}
impl From<(SummarizedSession, Vec<SeriesPoint>)> for SeriesResponse {
    fn from((session, points): (SummarizedSession, Vec<SeriesPoint>)) -> Self {
        Self {
            detail: session.into(),
            points: points.into_iter().map(Into::into).collect(),
        }
    }
}

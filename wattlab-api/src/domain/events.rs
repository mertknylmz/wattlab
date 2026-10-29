use super::{connection_state::ConnectionState, event_error::EventError, source::Source};
use chrono::{DateTime, Datelike, Utc};
use uuid::Uuid;

/// Immutable validated metadata. PostgreSQL timestamp precision is microseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    stream_id: Uuid,
    connection_id: Uuid,
    sequence: i64,
    observed_at: DateTime<Utc>,
}
impl Envelope {
    pub fn new(
        version: i32,
        stream_id: Uuid,
        connection_id: Uuid,
        sequence: i64,
        observed_at: DateTime<Utc>,
    ) -> Result<Self, EventError> {
        if version != 1 || sequence < 0 || !(1..=9999).contains(&observed_at.year()) {
            return Err(EventError::InvalidEnvelope);
        }
        let observed_at = DateTime::from_timestamp_micros(observed_at.timestamp_micros())
            .ok_or(EventError::InvalidEnvelope)?;
        Ok(Self {
            stream_id,
            connection_id,
            sequence,
            observed_at,
        })
    }
    pub fn stream_id(&self) -> Uuid {
        self.stream_id
    }
    pub fn connection_id(&self) -> Uuid {
        self.connection_id
    }
    pub fn sequence(&self) -> i64 {
        self.sequence
    }
    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Measurement {
    envelope: Envelope,
    source: Source,
    speed_kmh: Option<f64>,
    cadence_rpm: Option<f64>,
    power_w: Option<i32>,
}
impl Measurement {
    pub fn new(
        envelope: Envelope,
        source: Source,
        speed_kmh: Option<f64>,
        cadence_rpm: Option<f64>,
        power_w: Option<i32>,
    ) -> Result<Self, EventError> {
        if speed_kmh.is_none() && cadence_rpm.is_none() && power_w.is_none()
            || speed_kmh.is_some_and(|v| !v.is_finite() || !(0.0..=655.35).contains(&v))
            || cadence_rpm.is_some_and(|v| !v.is_finite() || !(0.0..=32767.5).contains(&v))
            || power_w.is_some_and(|v| !(-32768..=32767).contains(&v))
            || source == Source::CyclingPower && (speed_kmh.is_some() || cadence_rpm.is_some())
        {
            return Err(EventError::InvalidMeasurement);
        }
        Ok(Self {
            envelope,
            source,
            speed_kmh,
            cadence_rpm,
            power_w,
        })
    }
    pub fn envelope(&self) -> &Envelope {
        &self.envelope
    }
    pub fn source(&self) -> Source {
        self.source
    }
    pub fn speed_kmh(&self) -> Option<f64> {
        self.speed_kmh
    }
    pub fn cadence_rpm(&self) -> Option<f64> {
        self.cadence_rpm
    }
    pub fn power_w(&self) -> Option<i32> {
        self.power_w
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionEvent {
    envelope: Envelope,
    state: ConnectionState,
}
impl ConnectionEvent {
    pub fn new(envelope: Envelope, state: ConnectionState) -> Self {
        Self { envelope, state }
    }
    pub fn envelope(&self) -> &Envelope {
        &self.envelope
    }
    pub fn state(&self) -> ConnectionState {
        self.state
    }
}
/// A validated nonempty, bounded atomic submission.
#[derive(Debug, Clone)]
pub struct EventBatch<T>(Vec<T>);
impl<T> EventBatch<T> {
    pub fn new(events: Vec<T>) -> Result<Self, EventError> {
        if !(1..=100).contains(&events.len()) {
            return Err(EventError::InvalidBatch);
        }
        Ok(Self(events))
    }
    pub fn events(&self) -> &[T] {
        &self.0
    }
}

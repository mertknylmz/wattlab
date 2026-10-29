use crate::domain::{
    connection_state::ConnectionState,
    event_error::EventError,
    events::{ConnectionEvent, Envelope, EventBatch, Measurement},
    source::Source,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub(crate) struct Batch<T> {
    pub events: Vec<T>,
}
#[derive(Deserialize)]
pub(crate) struct EnvelopeDto {
    schema_version: i32,
    stream_id: Uuid,
    connection_id: Uuid,
    sequence: i64,
    observed_at: DateTime<Utc>,
}
impl EnvelopeDto {
    fn into_domain(self) -> Result<Envelope, EventError> {
        Envelope::new(
            self.schema_version,
            self.stream_id,
            self.connection_id,
            self.sequence,
            self.observed_at,
        )
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum SourceDto {
    Ftms,
    CyclingPower,
}
#[derive(Deserialize)]
pub(crate) struct MeasurementDto {
    #[serde(flatten)]
    envelope: EnvelopeDto,
    source: SourceDto,
    speed_kmh: Option<f64>,
    cadence_rpm: Option<f64>,
    power_w: Option<i32>,
}
impl TryFrom<Batch<MeasurementDto>> for EventBatch<Measurement> {
    type Error = EventError;
    fn try_from(batch: Batch<MeasurementDto>) -> Result<Self, Self::Error> {
        EventBatch::new(
            batch
                .events
                .into_iter()
                .map(|v| {
                    Measurement::new(
                        v.envelope.into_domain()?,
                        match v.source {
                            SourceDto::Ftms => Source::Ftms,
                            SourceDto::CyclingPower => Source::CyclingPower,
                        },
                        v.speed_kmh,
                        v.cadence_rpm,
                        v.power_w,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        )
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum StateDto {
    Connecting,
    Subscribing,
    Streaming,
    RetryWait,
    Stopped,
}
#[derive(Deserialize)]
pub(crate) struct ConnectionDto {
    #[serde(flatten)]
    envelope: EnvelopeDto,
    state: StateDto,
}
impl TryFrom<Batch<ConnectionDto>> for EventBatch<ConnectionEvent> {
    type Error = EventError;
    fn try_from(batch: Batch<ConnectionDto>) -> Result<Self, Self::Error> {
        EventBatch::new(
            batch
                .events
                .into_iter()
                .map(|v| {
                    Ok(ConnectionEvent::new(
                        v.envelope.into_domain()?,
                        match v.state {
                            StateDto::Connecting => ConnectionState::Connecting,
                            StateDto::Subscribing => ConnectionState::Subscribing,
                            StateDto::Streaming => ConnectionState::Streaming,
                            StateDto::RetryWait => ConnectionState::RetryWait,
                            StateDto::Stopped => ConnectionState::Stopped,
                        },
                    ))
                })
                .collect::<Result<Vec<_>, EventError>>()?,
        )
    }
}
#[derive(Serialize)]
pub(crate) struct Accepted {
    pub accepted: usize,
}

use std::{error::Error, fmt};
#[derive(Debug, PartialEq, Eq)]
pub enum EventError {
    InvalidEnvelope,
    InvalidMeasurement,
    InvalidBatch,
}
impl fmt::Display for EventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Invalid event or batch")
    }
}
impl Error for EventError {}

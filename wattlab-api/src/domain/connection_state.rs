#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Subscribing,
    Streaming,
    RetryWait,
    Stopped,
}
impl ConnectionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Subscribing => "subscribing",
            Self::Streaming => "streaming",
            Self::RetryWait => "retry_wait",
            Self::Stopped => "stopped",
        }
    }
}

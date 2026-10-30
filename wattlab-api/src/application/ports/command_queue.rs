use crate::domain::command::{AgentStatus, Command, CommandKind};
use std::{future::Future, pin::Pin, time::Duration};
use uuid::Uuid;

pub type QueueFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompleteError {
    NotFound,
    Conflict,
}

/// Hand-off between user intent and the trainer agent. Commands are delivered at
/// most once and expire rather than being replayed to a reconnected trainer.
pub trait CommandQueue: Send + Sync {
    fn submit(&self, kind: CommandKind) -> Command;
    fn get(&self, id: Uuid) -> Option<Command>;
    /// Wait up to `wait` for the oldest pending command and mark it delivered.
    fn next(&self, wait: Duration) -> QueueFuture<'_, Option<Command>>;
    fn complete(
        &self,
        id: Uuid,
        succeeded: bool,
        message: Option<String>,
    ) -> Result<Command, CompleteError>;
    fn record_status(&self, status: AgentStatus);
    fn status(&self) -> Option<AgentStatus>;
}

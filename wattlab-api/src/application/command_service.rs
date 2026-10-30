use super::{
    error::ApplicationError,
    ports::command_queue::{CommandQueue, CompleteError},
};
use crate::domain::command::{AgentStatus, Command, CommandKind};
use chrono::{DateTime, Utc};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

pub const MAX_COMMAND_WAIT: Duration = Duration::from_secs(20);

#[derive(Clone)]
pub struct CommandService {
    queue: Arc<dyn CommandQueue>,
}

#[derive(Debug, Clone)]
pub struct AgentView {
    pub status: Option<AgentStatus>,
    pub online: bool,
}

impl CommandService {
    pub fn new(queue: Arc<dyn CommandQueue>) -> Self {
        Self { queue }
    }
    pub fn submit(&self, kind: CommandKind) -> Command {
        self.queue.submit(kind)
    }
    pub fn get(&self, id: Uuid) -> Result<Command, ApplicationError> {
        self.queue.get(id).ok_or(ApplicationError::CommandNotFound)
    }
    pub async fn next(&self, wait: Duration) -> Option<Command> {
        self.queue.next(wait.min(MAX_COMMAND_WAIT)).await
    }
    pub fn complete(
        &self,
        id: Uuid,
        succeeded: bool,
        message: Option<String>,
    ) -> Result<Command, ApplicationError> {
        self.queue
            .complete(id, succeeded, message)
            .map_err(|error| match error {
                CompleteError::NotFound => ApplicationError::CommandNotFound,
                CompleteError::Conflict => ApplicationError::CommandConflict,
            })
    }
    pub fn heartbeat(&self, status: AgentStatus) {
        self.queue.record_status(status);
    }
    pub fn agent(&self, now: DateTime<Utc>) -> AgentView {
        let status = self.queue.status();
        AgentView {
            online: status.as_ref().is_some_and(|s| s.online(now)),
            status,
        }
    }
}

use super::{
    error::ApplicationError,
    ports::{
        command_queue::{CommandQueue, CompleteError},
        session_repository::SessionRepository,
    },
};
use crate::domain::command::{AgentStatus, Command, CommandKind, CommandStatus};
use chrono::{DateTime, Utc};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

pub const MAX_COMMAND_WAIT: Duration = Duration::from_secs(20);

#[derive(Clone)]
pub struct CommandService {
    queue: Arc<dyn CommandQueue>,
    sessions: Arc<dyn SessionRepository>,
}

#[derive(Debug, Clone)]
pub struct AgentView {
    pub status: Option<AgentStatus>,
    pub online: bool,
}

impl CommandService {
    pub fn new(queue: Arc<dyn CommandQueue>, sessions: Arc<dyn SessionRepository>) -> Self {
        Self { queue, sessions }
    }
    /// The trainer is one physical device: while another member's session is
    /// active, nobody else may steer it.
    pub async fn submit(
        &self,
        kind: CommandKind,
        member: Uuid,
    ) -> Result<Command, ApplicationError> {
        if self
            .sessions
            .active_owner()
            .await?
            .is_some_and(|owner| owner != member)
        {
            return Err(ApplicationError::TrainerInUse);
        }
        Ok(self.queue.submit(kind, member))
    }
    /// Members see only their own commands.
    pub fn get(&self, id: Uuid, member: Option<Uuid>) -> Result<Command, ApplicationError> {
        self.queue
            .get(id)
            .filter(|command| member.is_none_or(|member| command.issued_by() == member))
            .ok_or(ApplicationError::CommandNotFound)
    }
    /// Owner for a session the agent starts while executing `command_id`.
    pub fn session_owner(&self, command_id: Uuid) -> Result<Uuid, ApplicationError> {
        let command = self
            .queue
            .get(command_id)
            .ok_or(ApplicationError::CommandNotFound)?;
        let executing = matches!(
            command.status(),
            CommandStatus::Delivered | CommandStatus::Succeeded
        );
        if !matches!(command.kind(), CommandKind::StartSession) || !executing {
            return Err(ApplicationError::CommandConflict);
        }
        Ok(command.issued_by())
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

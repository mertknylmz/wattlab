use crate::{
    application::ports::command_queue::{CommandQueue, CompleteError, QueueFuture},
    domain::command::{AgentStatus, Command, CommandKind},
};
use chrono::Utc;
use std::{
    collections::VecDeque,
    sync::{Mutex, MutexGuard},
    time::Duration,
};
use tokio::{sync::Notify, time::Instant};
use uuid::Uuid;

/// Retained for status lookups; older entries are dropped first.
const RETAINED_COMMANDS: usize = 100;

/// Commands are ephemeral by design: a restarted API must not replay intent the
/// rider may no longer want. Locks are never held across an await.
#[derive(Default)]
pub struct InMemoryCommandQueue {
    state: Mutex<State>,
    submitted: Notify,
}

#[derive(Default)]
struct State {
    commands: VecDeque<Command>,
    agent: Option<AgentStatus>,
}

impl InMemoryCommandQueue {
    pub fn new() -> Self {
        Self::default()
    }

    fn state(&self) -> MutexGuard<'_, State> {
        // A panic while holding this lock cannot leave a command half-updated:
        // every mutation is a single domain method call.
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn take_pending(&self) -> Option<Command> {
        let now = Utc::now();
        let mut state = self.state();
        state
            .commands
            .iter_mut()
            .find_map(|command| command.deliver(now).then(|| command.clone()))
    }
}

impl CommandQueue for InMemoryCommandQueue {
    fn submit(&self, kind: CommandKind, issued_by: Uuid) -> Command {
        let command = Command::new(kind, issued_by, Utc::now());
        {
            let mut state = self.state();
            if state.commands.len() >= RETAINED_COMMANDS {
                state.commands.pop_front();
            }
            state.commands.push_back(command.clone());
        }
        self.submitted.notify_waiters();
        command
    }

    fn get(&self, id: Uuid) -> Option<Command> {
        let now = Utc::now();
        let mut state = self.state();
        state
            .commands
            .iter_mut()
            .find(|command| command.id() == id)
            .map(|command| {
                command.expire_if_due(now);
                command.clone()
            })
    }

    fn next(&self, wait: Duration) -> QueueFuture<'_, Option<Command>> {
        Box::pin(async move {
            let deadline = Instant::now() + wait;
            loop {
                // Register interest before checking so a concurrent submit is not missed.
                let notified = self.submitted.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                if let Some(command) = self.take_pending() {
                    return Some(command);
                }
                if tokio::time::timeout_at(deadline, notified).await.is_err() {
                    return None;
                }
            }
        })
    }

    fn complete(
        &self,
        id: Uuid,
        succeeded: bool,
        message: Option<String>,
    ) -> Result<Command, CompleteError> {
        let now = Utc::now();
        let mut state = self.state();
        let command = state
            .commands
            .iter_mut()
            .find(|command| command.id() == id)
            .ok_or(CompleteError::NotFound)?;
        command
            .complete(succeeded, message, now)
            .map_err(|_| CompleteError::Conflict)?;
        Ok(command.clone())
    }

    fn record_status(&self, status: AgentStatus) {
        self.state().agent = Some(status);
    }

    fn status(&self) -> Option<AgentStatus> {
        self.state().agent.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::command::CommandStatus;
    use std::sync::Arc;

    #[tokio::test]
    async fn waiting_agent_receives_a_later_command_exactly_once() {
        let queue = Arc::new(InMemoryCommandQueue::new());
        let waiter = tokio::spawn({
            let queue = queue.clone();
            async move { queue.next(Duration::from_secs(5)).await }
        });
        tokio::task::yield_now().await;
        let submitted = queue.submit(CommandKind::Connect, Uuid::nil());
        let delivered = waiter.await.unwrap().unwrap();
        assert_eq!(delivered.id(), submitted.id());
        assert_eq!(delivered.status(), CommandStatus::Delivered);
        assert!(queue.next(Duration::from_millis(10)).await.is_none());
    }

    #[tokio::test]
    async fn results_are_recorded_and_unknown_commands_are_reported() {
        let queue = InMemoryCommandQueue::new();
        let command = queue.submit(CommandKind::StartSession, Uuid::nil());
        assert_eq!(
            queue.complete(command.id(), true, None),
            Err(CompleteError::Conflict)
        );
        queue.next(Duration::ZERO).await.unwrap();
        let done = queue
            .complete(command.id(), false, Some("busy".into()))
            .unwrap();
        assert_eq!(done.status(), CommandStatus::Rejected);
        assert_eq!(queue.get(command.id()).unwrap().message(), Some("busy"));
        assert_eq!(
            queue.complete(Uuid::new_v4(), true, None),
            Err(CompleteError::NotFound)
        );
    }
}

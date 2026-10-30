use chrono::{DateTime, Duration, Utc};
use std::{error::Error, fmt};
use uuid::Uuid;

/// A request for the Python trainer agent. Rust records intent and outcome;
/// only the agent touches Bluetooth or decides whether the trainer can comply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Connect,
    Disconnect,
    StartSession,
    FinishSession,
    SetTargetPower { watts: i32 },
    ReleaseControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandStatus {
    Pending,
    Delivered,
    Succeeded,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    InvalidTargetPower,
    /// Result for an expired command or a different result for a finished one.
    Conflict,
}
impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidTargetPower => "Target power must be between 0 and 2000 W",
            Self::Conflict => "Command is no longer awaiting this result",
        })
    }
}
impl Error for CommandError {}

/// An undelivered command is stale quickly: a late connect or ERG change could
/// surprise a rider who already moved on. Delivered work gets longer to finish
/// because BLE connection and session flush can take several seconds.
pub const DELIVERY_TIMEOUT: Duration = Duration::seconds(10);
pub const RESULT_TIMEOUT: Duration = Duration::seconds(45);
const MAX_MESSAGE_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    id: Uuid,
    kind: CommandKind,
    status: CommandStatus,
    created_at: DateTime<Utc>,
    delivered_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    message: Option<String>,
}

impl CommandKind {
    pub fn set_target_power(watts: i64) -> Result<Self, CommandError> {
        i32::try_from(watts)
            .ok()
            .filter(|w| (0..=2000).contains(w))
            .map(|watts| Self::SetTargetPower { watts })
            .ok_or(CommandError::InvalidTargetPower)
    }
}

impl Command {
    pub fn new(kind: CommandKind, now: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            status: CommandStatus::Pending,
            created_at: now,
            delivered_at: None,
            completed_at: None,
            message: None,
        }
    }

    /// Moves overdue commands to Expired. Returns whether the status changed.
    pub fn expire_if_due(&mut self, now: DateTime<Utc>) -> bool {
        let due = match (self.status, self.delivered_at) {
            (CommandStatus::Pending, _) => now - self.created_at >= DELIVERY_TIMEOUT,
            (CommandStatus::Delivered, Some(delivered)) => now - delivered >= RESULT_TIMEOUT,
            _ => false,
        };
        if due {
            self.status = CommandStatus::Expired;
            self.completed_at = Some(now);
        }
        due
    }

    /// Hands a pending command to the agent exactly once; never redelivered.
    pub fn deliver(&mut self, now: DateTime<Utc>) -> bool {
        if self.expire_if_due(now) || self.status != CommandStatus::Pending {
            return false;
        }
        self.status = CommandStatus::Delivered;
        self.delivered_at = Some(now);
        true
    }

    /// An identical repeated result is accepted so agent retries are safe.
    pub fn complete(
        &mut self,
        succeeded: bool,
        message: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<(), CommandError> {
        let status = if succeeded {
            CommandStatus::Succeeded
        } else {
            CommandStatus::Rejected
        };
        let message = message.map(|m| m.chars().take(MAX_MESSAGE_CHARS).collect::<String>());
        if self.status == status && self.message == message {
            return Ok(());
        }
        self.expire_if_due(now);
        if self.status != CommandStatus::Delivered {
            return Err(CommandError::Conflict);
        }
        self.status = status;
        self.message = message;
        self.completed_at = Some(now);
        Ok(())
    }

    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn kind(&self) -> CommandKind {
        self.kind
    }
    pub fn status(&self) -> CommandStatus {
        self.status
    }
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub fn completed_at(&self) -> Option<DateTime<Utc>> {
        self.completed_at
    }
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

/// Last report from the trainer agent. Reported, not verified, by Rust.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentStatus {
    pub agent_id: Uuid,
    pub connection_state: Option<String>,
    pub session_id: Option<Uuid>,
    pub erg_supported: bool,
    pub power_range_w: Option<(i32, i32)>,
    pub target_power_w: Option<i32>,
    pub received_at: DateTime<Utc>,
}

/// Heartbeats arrive every second; three missed beats mark the agent offline.
pub const AGENT_ONLINE_WINDOW: Duration = Duration::seconds(3);

impl AgentStatus {
    pub fn online(&self, now: DateTime<Utc>) -> bool {
        let age = now - self.received_at;
        age >= Duration::zero() && age <= AGENT_ONLINE_WINDOW
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::from_timestamp(1_000_000, 0).unwrap()
    }

    #[test]
    fn commands_are_delivered_once_and_results_are_idempotent() {
        let mut command = Command::new(CommandKind::Connect, now());
        assert!(command.deliver(now()));
        assert!(!command.deliver(now()));
        command.complete(true, None, now()).unwrap();
        command.complete(true, None, now()).unwrap();
        assert_eq!(
            command.complete(false, Some("x".into()), now()),
            Err(CommandError::Conflict)
        );
        assert_eq!(command.status(), CommandStatus::Succeeded);
    }

    #[test]
    fn stale_commands_expire_instead_of_reaching_the_trainer_late() {
        let mut pending = Command::new(CommandKind::StartSession, now());
        assert!(!pending.deliver(now() + DELIVERY_TIMEOUT));
        assert_eq!(pending.status(), CommandStatus::Expired);

        let mut delivered = Command::new(CommandKind::FinishSession, now());
        assert!(delivered.deliver(now()));
        assert_eq!(
            delivered.complete(true, None, now() + RESULT_TIMEOUT),
            Err(CommandError::Conflict)
        );
        assert_eq!(delivered.status(), CommandStatus::Expired);
    }

    #[test]
    fn undelivered_commands_cannot_be_completed() {
        let mut command = Command::new(CommandKind::Disconnect, now());
        assert_eq!(
            command.complete(true, None, now()),
            Err(CommandError::Conflict)
        );
    }

    #[test]
    fn target_power_is_bounded_and_messages_are_truncated() {
        assert!(CommandKind::set_target_power(-1).is_err());
        assert!(CommandKind::set_target_power(2001).is_err());
        assert_eq!(
            CommandKind::set_target_power(250),
            Ok(CommandKind::SetTargetPower { watts: 250 })
        );
        let mut command = Command::new(CommandKind::ReleaseControl, now());
        command.deliver(now());
        command
            .complete(false, Some("é".repeat(500)), now())
            .unwrap();
        assert_eq!(command.message().unwrap().chars().count(), 200);
    }

    #[test]
    fn agent_is_online_only_within_the_heartbeat_window() {
        let status = AgentStatus {
            agent_id: Uuid::nil(),
            connection_state: None,
            session_id: None,
            erg_supported: false,
            power_range_w: None,
            target_power_w: None,
            received_at: now(),
        };
        assert!(status.online(now() + AGENT_ONLINE_WINDOW));
        assert!(!status.online(now() + AGENT_ONLINE_WINDOW + Duration::milliseconds(1)));
        assert!(!status.online(now() - Duration::seconds(1)));
    }
}

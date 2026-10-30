use crate::{
    application::command_service::AgentView,
    domain::command::{
        AgentStatus, Command, CommandError, CommandKind, CommandStatus, NearbyDevice,
        parse_ble_address,
    },
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum CommandRequest {
    Scan,
    Connect { address: String },
    Disconnect,
    StartSession,
    FinishSession,
    SetTargetPower { target_power_w: i64 },
    ReleaseControl,
}
impl TryFrom<CommandRequest> for CommandKind {
    type Error = CommandError;
    fn try_from(request: CommandRequest) -> Result<Self, Self::Error> {
        Ok(match request {
            CommandRequest::Scan => Self::Scan,
            CommandRequest::Connect { address } => Self::connect(&address)?,
            CommandRequest::Disconnect => Self::Disconnect,
            CommandRequest::StartSession => Self::StartSession,
            CommandRequest::FinishSession => Self::FinishSession,
            CommandRequest::SetTargetPower { target_power_w } => {
                Self::set_target_power(target_power_w)?
            }
            CommandRequest::ReleaseControl => Self::ReleaseControl,
        })
    }
}

#[derive(Serialize)]
pub(crate) struct CommandResponse {
    id: Uuid,
    kind: &'static str,
    target_power_w: Option<i32>,
    address: Option<String>,
    status: &'static str,
    created_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    message: Option<String>,
}
impl From<Command> for CommandResponse {
    fn from(command: Command) -> Self {
        let (kind, target_power_w, address) = match command.kind() {
            CommandKind::Scan => ("scan", None, None),
            CommandKind::Connect { address } => ("connect", None, Some(address.clone())),
            CommandKind::Disconnect => ("disconnect", None, None),
            CommandKind::StartSession => ("start_session", None, None),
            CommandKind::FinishSession => ("finish_session", None, None),
            CommandKind::SetTargetPower { watts } => ("set_target_power", Some(*watts), None),
            CommandKind::ReleaseControl => ("release_control", None, None),
        };
        Self {
            id: command.id(),
            kind,
            target_power_w,
            address,
            status: match command.status() {
                CommandStatus::Pending => "pending",
                CommandStatus::Delivered => "delivered",
                CommandStatus::Succeeded => "succeeded",
                CommandStatus::Rejected => "rejected",
                CommandStatus::Expired => "expired",
            },
            created_at: command.created_at(),
            completed_at: command.completed_at(),
            message: command.message().map(str::to_owned),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResultStatus {
    Succeeded,
    Rejected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommandResultRequest {
    pub status: ResultStatus,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct NextQuery {
    pub wait_seconds: Option<String>,
}

const AGENT_STATES: [&str; 7] = [
    "idle",
    "connecting",
    "subscribing",
    "streaming",
    "retry_wait",
    "stopped",
    "failed",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NearbyDeviceDto {
    address: String,
    name: Option<String>,
    rssi: Option<i16>,
}
impl NearbyDeviceDto {
    fn into_domain(self) -> Option<NearbyDevice> {
        let address = parse_ble_address(&self.address).ok()?;
        let rssi_ok = self.rssi.is_none_or(|rssi| (-127..=20).contains(&rssi));
        rssi_ok.then(|| NearbyDevice {
            address,
            name: self
                .name
                .map(|name| name.chars().take(48).collect::<String>())
                .filter(|name| !name.is_empty()),
            rssi: self.rssi,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HeartbeatRequest {
    agent_id: Uuid,
    connection_state: String,
    session_id: Option<Uuid>,
    erg_supported: bool,
    power_range_w: Option<[i32; 2]>,
    target_power_w: Option<i32>,
    #[serde(default)]
    trainer_address: Option<String>,
    #[serde(default)]
    devices: Vec<NearbyDeviceDto>,
}
impl HeartbeatRequest {
    pub fn into_domain(self, received_at: DateTime<Utc>) -> Option<AgentStatus> {
        let range_valid = self
            .power_range_w
            .is_none_or(|[min, max]| 0 <= min && min <= max && max <= 4000);
        let target_valid = self.target_power_w.is_none_or(|w| (0..=2000).contains(&w));
        let trainer_address = match self.trainer_address {
            Some(raw) if !raw.is_empty() => Some(parse_ble_address(&raw).ok()?),
            _ => None,
        };
        (AGENT_STATES.contains(&self.connection_state.as_str()) && range_valid && target_valid)
            .then(|| AgentStatus {
                agent_id: self.agent_id,
                connection_state: Some(self.connection_state),
                session_id: self.session_id,
                erg_supported: self.erg_supported,
                power_range_w: self.power_range_w.map(|[min, max]| (min, max)),
                target_power_w: self.target_power_w,
                trainer_address,
                devices: self
                    .devices
                    .into_iter()
                    .filter_map(NearbyDeviceDto::into_domain)
                    .take(24)
                    .collect(),
                received_at,
            })
    }
}

#[derive(Serialize)]
struct NearbyDeviceResponse {
    address: String,
    name: Option<String>,
    rssi: Option<i16>,
}

#[derive(Serialize)]
pub(crate) struct AgentResponse {
    online: bool,
    agent_id: Option<Uuid>,
    connection_state: Option<String>,
    session_id: Option<Uuid>,
    erg_supported: bool,
    power_range_w: Option<[i32; 2]>,
    target_power_w: Option<i32>,
    trainer_address: Option<String>,
    devices: Vec<NearbyDeviceResponse>,
    last_seen_at: Option<DateTime<Utc>>,
}
impl From<AgentView> for AgentResponse {
    fn from(view: AgentView) -> Self {
        // Offline agents report nothing actionable; only the last-seen time.
        let live = view.status.as_ref().filter(|_| view.online);
        Self {
            online: view.online,
            agent_id: live.map(|s| s.agent_id),
            connection_state: live.and_then(|s| s.connection_state.clone()),
            session_id: live.and_then(|s| s.session_id),
            erg_supported: live.is_some_and(|s| s.erg_supported),
            power_range_w: live.and_then(|s| s.power_range_w.map(|(min, max)| [min, max])),
            target_power_w: live.and_then(|s| s.target_power_w),
            trainer_address: live.and_then(|s| s.trainer_address.clone()),
            devices: live
                .map(|s| {
                    s.devices
                        .iter()
                        .map(|device| NearbyDeviceResponse {
                            address: device.address.clone(),
                            name: device.name.clone(),
                            rssi: device.rssi,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            last_seen_at: view.status.map(|s| s.received_at),
        }
    }
}

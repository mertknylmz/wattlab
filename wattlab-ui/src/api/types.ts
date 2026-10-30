// Wire contracts of wattlab-api. Missing measurements are null, never zero.

export interface Session {
  id: string;
  request_id: string;
  started_at: string;
  finished_at: string | null;
}

export interface LiveSnapshot {
  session: Session | null;
  connection_state: string | null;
  fresh: boolean;
  speed_kmh: number | null;
  cadence_rpm: number | null;
  power_w: number | null;
  power_source: 'cycling_power' | 'ftms' | null;
  ftms_received_at: string | null;
  cycling_power_received_at: string | null;
  captured_at: string | null;
}

export interface AgentState {
  online: boolean;
  agent_id: string | null;
  connection_state: string | null;
  session_id: string | null;
  erg_supported: boolean;
  power_range_w: [number, number] | null;
  target_power_w: number | null;
  last_seen_at: string | null;
}

export interface StreamSnapshot {
  live: LiveSnapshot;
  agent: AgentState;
}

export interface Summary {
  duration_seconds: number | null;
  recorded_seconds: number;
  power_seconds: number;
  average_power_w: number | null;
  max_power_w: number | null;
  average_cadence_rpm: number | null;
  average_speed_kmh: number | null;
  energy_kj: number;
  zone_ftp_w: number | null;
  zone_seconds: number[] | null;
}

export interface SessionSummary {
  session: Session;
  ftp_w: number | null;
  zone_reference: 'session' | 'current';
  summary: Summary;
}

export interface SessionPage {
  sessions: SessionSummary[];
  next_before: string | null;
}

export interface SeriesPoint {
  t: string;
  power_w: number | null;
  power_source: 'cycling_power' | 'ftms' | null;
  cadence_rpm: number | null;
  speed_kmh: number | null;
}

export interface SessionSeries extends SessionSummary {
  points: SeriesPoint[];
}

export interface Member {
  id: string;
  username: string;
}

export interface AuthSession {
  member: Member;
  expires_at: string;
}

export interface DesktopCapabilities {
  remember_available: boolean;
}

export interface ActivityEntry {
  started_at: string;
  duration_seconds: number;
}

export interface Activity {
  entries: ActivityEntry[];
}

export interface Settings {
  ftp_w: number;
  ftp_is_default: boolean;
  updated_at: string;
}

export type CommandKind =
  | 'connect'
  | 'disconnect'
  | 'start_session'
  | 'finish_session'
  | 'set_target_power'
  | 'release_control';

export type CommandStatus = 'pending' | 'delivered' | 'succeeded' | 'rejected' | 'expired';

export interface Command {
  id: string;
  kind: CommandKind;
  target_power_w: number | null;
  status: CommandStatus;
  created_at: string;
  completed_at: string | null;
  message: string | null;
}

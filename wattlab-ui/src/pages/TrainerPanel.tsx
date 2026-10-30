import type { ApiClient } from '../api/client';
import type { AgentState, CommandKind } from '../api/types';
import { useCommand, type CommandState } from '../hooks/useCommand';
import { useAppearance, type Translate } from '../i18n/appearance';
import { interpolate, MESSAGES, type MessageKey } from '../i18n/messages';

const ACTIONS: Record<CommandKind, MessageKey> = {
  scan: 'actionScan',
  connect: 'actionConnect',
  disconnect: 'actionDisconnect',
  start_session: 'actionStart',
  finish_session: 'actionFinish',
  set_target_power: 'actionTarget',
  release_control: 'actionRelease',
};

const REASONS: Record<string, MessageKey> = {
  session_already_active: 'reasonSessionActive',
  no_active_session: 'reasonNoSession',
  session_not_created: 'reasonSessionNotCreated',
  trainer_not_configured: 'reasonTrainerNotConfigured',
  trainer_not_connected: 'reasonTrainerNotConnected',
  erg_unavailable: 'reasonErgUnavailable',
  target_out_of_range: 'reasonTargetOutOfRange',
  control_not_permitted: 'reasonControlNotPermitted',
  op_code_not_supported: 'reasonOpUnsupported',
  invalid_parameter: 'reasonInvalidParameter',
  operation_failed: 'reasonOperationFailed',
  control_failed: 'reasonControlFailed',
  disconnect_timeout: 'reasonDisconnectTimeout',
  connect_failed: 'reasonConnectFailed',
  connect_timeout: 'reasonConnectTimeout',
  already_connected: 'reasonAlreadyConnected',
  scan_failed: 'reasonScanFailed',
};

const STATES: Record<string, MessageKey> = {
  idle: 'idle',
  connecting: 'connecting',
  subscribing: 'subscribing',
  streaming: 'streaming',
  retry_wait: 'retryWait',
  stopped: 'stopped',
  failed: 'failed',
};

const fallback: Translate = (key, values) => interpolate(MESSAGES.en[key], values);

export function trainerConnectionLabel(agent: AgentState | null, t: Translate = fallback): string {
  if (!agent?.online) return t('agentOffline');
  const key = STATES[agent.connection_state ?? 'idle'];
  return key ? t(key) : t('unknownState');
}

export function describeCommand(state: CommandState | null, t: Translate = fallback): string {
  if (!state) return t('commandIdle');
  const action = t(ACTIONS[state.kind]);
  switch (state.status) {
    case 'pending':
    case 'delivered':
      return t('commandPending', { action });
    case 'succeeded':
      return t('commandSucceeded', { action });
    case 'rejected':
      return t('commandRejected', {
        action,
        reason: state.message && REASONS[state.message] ? t(REASONS[state.message]) : t('reasonUnknown'),
      });
    case 'expired':
      return t('commandExpired', { action });
    default:
      return t('commandFailed', { action });
  }
}

interface Props {
  client: ApiClient;
  agent: AgentState | null;
  sessionActive: boolean;
  onUnauthorized(): void;
}

export function TrainerPanel({ client, agent, sessionActive, onUnauthorized }: Props) {
  const { t } = useAppearance();
  const { state, submit, busy } = useCommand(client, onUnauthorized);
  const online = agent?.online ?? false;
  const streaming = online && (agent?.connection_state ?? 'idle') === 'streaming';

  return (
    <section className="trainer-panel session-panel" aria-label={t('session')}>
      <div className="panel-row">
        <div>
          <h2>{t('session')}</h2>
          <p>
            {!online ? t('agentOfflineHint') : streaming ? t('sessionReadyHint') : t('startNeedsTrainer')}
          </p>
        </div>
        <div className="panel-actions">
          {sessionActive || agent?.session_id ? (
            <button type="button" className="secondary" disabled={busy || !online} onClick={() => void submit('finish_session')}>
              {t('finishSession')}
            </button>
          ) : (
            <button type="button" disabled={busy || !streaming} onClick={() => void submit('start_session')}>
              {t('startSession')}
            </button>
          )}
        </div>
      </div>
      <p className="command-status" role="status">
        {describeCommand(state, t)}
      </p>
    </section>
  );
}

import { useState, type FormEvent } from 'react';
import type { ApiClient } from '../api/client';
import type { AgentState, CommandKind } from '../api/types';
import { useCommand, type CommandState } from '../hooks/useCommand';
import { useAppearance, type Translate } from '../i18n/appearance';
import { interpolate, MESSAGES, type MessageKey } from '../i18n/messages';

const ACTIONS: Record<CommandKind, MessageKey> = {
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
  erg_unavailable: 'reasonErgUnavailable',
  target_out_of_range: 'reasonTargetOutOfRange',
  control_not_permitted: 'reasonControlNotPermitted',
  op_code_not_supported: 'reasonOpUnsupported',
  invalid_parameter: 'reasonInvalidParameter',
  operation_failed: 'reasonOperationFailed',
  control_failed: 'reasonControlFailed',
  disconnect_timeout: 'reasonDisconnectTimeout',
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
  const [target, setTarget] = useState('');
  const [targetError, setTargetError] = useState<string | null>(null);
  const online = agent?.online ?? false;
  const connected =
    online && agent?.connection_state != null && agent.connection_state !== 'idle' && agent.connection_state !== 'failed';
  const streaming = online && agent?.connection_state === 'streaming';
  const erg = streaming && agent?.erg_supported;
  const range = agent?.power_range_w;

  const applyTarget = (event: FormEvent) => {
    event.preventDefault();
    const watts = Number(target);
    const [min, max] = range ?? [0, 2000];
    if (!Number.isInteger(watts) || watts < min || watts > max) {
      setTargetError(t('targetOutOfRange', { min, max }));
      return;
    }
    setTargetError(null);
    void submit('set_target_power', watts);
  };

  return (
    <section className="trainer-panel" aria-label={t('trainerControl')}>
      <div className="panel-row">
        <div>
          <h2>{t('trainer')}</h2>
          <p role="status">
            {!online
              ? t('agentOfflineHint')
              : STATES[agent?.connection_state ?? 'idle']
                ? t(STATES[agent?.connection_state ?? 'idle'])
                : t('unknownState')}
          </p>
        </div>
        <div className="panel-actions">
          {connected ? (
            <button type="button" className="secondary" disabled={busy} onClick={() => void submit('disconnect')}>
              {t('disconnect')}
            </button>
          ) : (
            <button type="button" disabled={busy || !online} onClick={() => void submit('connect')}>
              {t('connectTrainer')}
            </button>
          )}
          {sessionActive || agent?.session_id ? (
            <button type="button" className="secondary" disabled={busy || !online} onClick={() => void submit('finish_session')}>
              {t('finishSession')}
            </button>
          ) : (
            <button type="button" disabled={busy || !online} onClick={() => void submit('start_session')}>
              {t('startSession')}
            </button>
          )}
        </div>
      </div>
      <div className="panel-row erg">
        <div>
          <h3>{t('targetPower')}</h3>
          <p>
            {erg
              ? t('ergReady', {
                  range: range ? t('ergRange', { min: range[0], max: range[1] }) : '',
                  target: agent?.target_power_w != null ? t('currentTarget', { watts: agent.target_power_w }) : t('noTarget'),
                })
              : t('ergUnavailableHint')}
          </p>
          {targetError && <p className="access-error">{targetError}</p>}
        </div>
        <form onSubmit={applyTarget} noValidate>
          <label htmlFor="target">
            {t('target')} <span>(W)</span>
          </label>
          <input
            id="target"
            type="number"
            step={1}
            min={range?.[0] ?? 0}
            max={range?.[1] ?? 2000}
            value={target}
            disabled={!erg}
            onChange={(event) => setTarget(event.target.value)}
          />
          <button type="submit" disabled={!erg || busy}>
            {t('apply')}
          </button>
          <button
            type="button"
            className="text-button"
            disabled={!erg || busy || agent?.target_power_w == null}
            onClick={() => void submit('release_control')}
          >
            {t('releaseControl')}
          </button>
        </form>
      </div>
      <p className="command-status" role="status">
        {describeCommand(state, t)}
      </p>
    </section>
  );
}

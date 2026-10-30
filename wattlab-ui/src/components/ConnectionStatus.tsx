import type { AgentState } from '../api/types';
import { useAppearance } from '../i18n/appearance';
import { trainerConnectionLabel } from '../pages/TrainerPanel';

export function connectionTone(agent: AgentState | null): 'offline' | 'idle' | 'busy' | 'live' | 'failed' {
  if (!agent?.online) return 'offline';
  const state = agent.connection_state ?? 'idle';
  if (state === 'streaming') return 'live';
  if (state === 'failed') return 'failed';
  if (state === 'connecting' || state === 'subscribing' || state === 'retry_wait') return 'busy';
  return 'idle';
}

function BluetoothMark({ slashed }: { slashed?: boolean }) {
  return (
    <svg className="connection-icon" viewBox="0 0 24 24" aria-hidden="true">
      <path
        fill="currentColor"
        d="M12.5 2v7.2L9.2 6.9 8 8.1l4.5 4-4.5 4 1.2 1.2 3.3-2.3V22l6.2-4.6-4.3-3.3 4.3-3.3zm1.5 9.1 1.8 1.4-1.8 1.4zm0-5.4 1.8 1.4-1.8 1.3z"
      />
      {slashed ? <path fill="currentColor" d="M4 5.2 5.2 4 20 18.8 18.8 20z" /> : null}
    </svg>
  );
}

interface Props {
  agent: AgentState | null;
}

export function ConnectionStatus({ agent }: Props) {
  const { t } = useAppearance();
  const tone = connectionTone(agent);
  const label = trainerConnectionLabel(agent, t);
  const chip =
    tone === 'live'
      ? t('connectionChipLive')
      : tone === 'busy'
        ? t('connectionChipBusy')
        : tone === 'failed'
          ? t('connectionChipFailed')
          : tone === 'offline'
            ? t('connectionChipOffline')
            : t('connectionChipIdle');

  return (
    <a
      className={`connection-chip ${tone}`}
      href="#/settings/connection"
      title={t('openConnectionSettings')}
      aria-label={`${t('openConnectionSettings')}: ${label}`}
      data-testid="connection-status"
    >
      <BluetoothMark slashed={tone === 'offline' || tone === 'failed'} />
      <span className={`status-dot${tone === 'live' ? ' live' : ''}`} />
      <span>{chip}</span>
    </a>
  );
}

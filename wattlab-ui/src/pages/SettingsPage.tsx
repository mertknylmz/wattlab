import { useEffect, useState, type FormEvent } from 'react';
import { UnauthorizedError, type ApiClient } from '../api/client';
import type { AgentState, Settings } from '../api/types';
import { ZoneLegend } from '../components/ZoneLegend';
import { useCommand } from '../hooks/useCommand';
import { useAppearance } from '../i18n/appearance';
import { formatDate } from '../model/format';
import { isValidFtp } from '../model/zones';
import { describeCommand, trainerConnectionLabel } from './TrainerPanel';

interface Props {
  client: ApiClient;
  settings: Settings | null;
  onSaved(settings: Settings): void;
  onUnauthorized(): void;
}

export function SettingsPage({ client, settings, onSaved, onUnauthorized }: Props) {
  const { t, locale } = useAppearance();
  const [value, setValue] = useState('');
  const [message, setMessage] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [agent, setAgent] = useState<AgentState | null>(null);
  const { state, submit: sendCommand, busy } = useCommand(client, onUnauthorized);
  const connected =
    Boolean(agent?.online) &&
    agent?.connection_state != null &&
    agent.connection_state !== 'idle' &&
    agent.connection_state !== 'failed';

  useEffect(() => {
    let active = true;
    const load = () =>
      client.agent().then(
        (value) => active && setAgent(value),
        (error) => {
          if (!active) return;
          if (error instanceof UnauthorizedError) onUnauthorized();
        },
      );
    void load();
    const timer = setInterval(() => void load(), 2000);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [client, onUnauthorized]);

  const save = async (ftp: number | null) => {
    setSaving(true);
    try {
      const saved = await client.saveFtp(ftp);
      onSaved(saved);
      setValue('');
      setMessage(ftp == null ? t('ftpReset') : t('ftpSaved', { watts: saved.ftp_w }));
    } catch (error) {
      if (error instanceof UnauthorizedError) onUnauthorized();
      else setMessage(t('ftpSaveFailed'));
    } finally {
      setSaving(false);
    }
  };

  const sendDevice = async (kind: 'connect' | 'disconnect') => {
    await sendCommand(kind);
    try {
      setAgent(await client.agent());
    } catch (error) {
      if (error instanceof UnauthorizedError) onUnauthorized();
    }
  };

  const submitFtp = (event: FormEvent) => {
    event.preventDefault();
    const ftp = Number(value);
    if (!isValidFtp(ftp)) {
      setMessage(t('ftpInvalid'));
      return;
    }
    void save(ftp);
  };

  return (
    <>
      <section className="page-heading">
        <div>
          <p className="eyebrow">{t('settingsEyebrow')}</p>
          <h1>{t('settingsTitle')}</h1>
          <p className="subtitle">{t('settingsSubtitle')}</p>
        </div>
      </section>
      <section className="list-card settings">
        <h2>{t('bluetooth')}</h2>
        <p>
          {trainerConnectionLabel(agent, t)}
          {agent?.online && agent.erg_supported ? t('ergSupported') : ''}
          {agent?.session_id ? t('activeRide') : ''}
        </p>
        <div className="panel-actions device-actions">
          {connected ? (
            <button type="button" className="secondary" disabled={busy} onClick={() => void sendDevice('disconnect')}>
              {t('disconnect')}
            </button>
          ) : (
            <button type="button" disabled={busy || !agent?.online} onClick={() => void sendDevice('connect')}>
              {t('connectTrainer')}
            </button>
          )}
        </div>
        <p className="command-status">{describeCommand(state, t)}</p>
      </section>
      <section className="list-card settings">
        <h2>FTP</h2>
        <p data-testid="ftp-current">
          {!settings
            ? t('loading')
            : settings.ftp_is_default
              ? t('ftpDefaultLabel', { watts: settings.ftp_w })
              : t('ftpSavedOn', { watts: settings.ftp_w, date: formatDate(settings.updated_at, locale) })}
        </p>
        <form className="ftp-form" onSubmit={submitFtp} noValidate>
          <label htmlFor="ftp">
            FTP <span>(W)</span>
          </label>
          <input
            id="ftp"
            type="number"
            min={1}
            max={2000}
            step={1}
            inputMode="numeric"
            value={value}
            onChange={(event) => setValue(event.target.value)}
          />
          <button type="submit" disabled={saving}>
            {t('save')}
          </button>
          <button
            type="button"
            className="text-button"
            disabled={saving || !settings || settings.ftp_is_default}
            onClick={() => void save(null)}
          >
            {t('resetDefault')}
          </button>
        </form>
        {message && <p role="status">{message}</p>}
        <p className="hint">{t('ftpHint')}</p>
        <ZoneLegend />
      </section>
    </>
  );
}

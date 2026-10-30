import { useEffect, useState, type FormEvent } from 'react';
import { UnauthorizedError, type ApiClient } from '../api/client';
import type { Settings } from '../api/types';
import { ZoneLegend } from '../components/ZoneLegend';
import { useAgentStatus } from '../hooks/useAgentStatus';
import { useCommand } from '../hooks/useCommand';
import { useAppearance } from '../i18n/appearance';
import { formatDate } from '../model/format';
import { readLastTrainer, trainerLabel, writeTrainerAddress } from '../model/trainerPrefs';
import { isValidFtp } from '../model/zones';
import { describeCommand, trainerConnectionLabel } from './TrainerPanel';

export type SettingsTab = 'general' | 'connection';

interface Props {
  client: ApiClient;
  tab: SettingsTab;
  settings: Settings | null;
  onSaved(settings: Settings): void;
  onUnauthorized(): void;
}

export function SettingsPage({ client, tab, settings, onSaved, onUnauthorized }: Props) {
  const { t, locale } = useAppearance();
  const [value, setValue] = useState('');
  const [message, setMessage] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [last, setLast] = useState(() => readLastTrainer());
  const selected = last?.address ?? null;
  const { state, submit: sendCommand, busy } = useCommand(client, onUnauthorized);
  const { agent, refresh } = useAgentStatus(client, onUnauthorized, tab === 'connection' ? 400 : 2000);
  const stateName = agent?.connection_state ?? 'idle';
  const streaming = Boolean(agent?.online) && stateName === 'streaming';
  const linking =
    Boolean(agent?.online) &&
    (stateName === 'connecting' || stateName === 'subscribing' || stateName === 'retry_wait');
  const inProgress = streaming || linking;
  const locked = linking || busy;
  const canScan = Boolean(agent?.online) && !streaming && !locked;
  const canConnect = canScan && Boolean(selected);
  const canDisconnect = streaming && !locked;
  const devices = agent?.devices ?? [];
  const erg = streaming && agent?.erg_supported;
  const range = agent?.power_range_w;
  const [target, setTarget] = useState('');
  const [targetError, setTargetError] = useState<string | null>(null);

  useEffect(() => {
    const match = devices.find((device) => device.address === selected && device.name);
    if (!match?.name || last?.name === match.name) return;
    writeTrainerAddress(match.address, match.name);
    setLast({ address: match.address, name: match.name });
  }, [devices, last?.name, selected]);

  const pick = (address: string, name: string | null) => {
    writeTrainerAddress(address, name);
    setLast({ address, name });
  };

  const sendDevice = async (kind: 'scan' | 'connect' | 'disconnect') => {
    await sendCommand(kind, kind === 'connect' && selected ? { address: selected } : undefined);
    await refresh();
  };

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

  const submitFtp = (event: FormEvent) => {
    event.preventDefault();
    const ftp = Number(value);
    if (!isValidFtp(ftp)) {
      setMessage(t('ftpInvalid'));
      return;
    }
    void save(ftp);
  };

  const applyTarget = (event: FormEvent) => {
    event.preventDefault();
    const watts = Number(target);
    const [min, max] = range ?? [0, 2000];
    if (!Number.isInteger(watts) || watts < min || watts > max) {
      setTargetError(t('targetOutOfRange', { min, max }));
      return;
    }
    setTargetError(null);
    void sendCommand('set_target_power', { targetPowerW: watts }).then(() => refresh());
  };

  return (
    <>
      <section className="page-heading">
        <div>
          <p className="eyebrow">{t('settingsEyebrow')}</p>
          <h1>{t('settingsTitle')}</h1>
          <p className="subtitle">{tab === 'connection' ? t('settingsSubtitleConnection') : t('settingsSubtitle')}</p>
        </div>
      </section>
      <nav className="settings-tabs" aria-label={t('settingsTabs')}>
        <a className={tab === 'general' ? 'active' : undefined} href="#/settings">
          {t('settingsTabGeneral')}
        </a>
        <a className={tab === 'connection' ? 'active' : undefined} href="#/settings/connection">
          {t('settingsTabConnection')}
        </a>
      </nav>
      {tab === 'general' ? (
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
      ) : (
        <>
          <section className="list-card settings">
            <h2>{t('bluetooth')}</h2>
            <p>
              {trainerConnectionLabel(agent, t)}
              {agent?.online && agent.erg_supported ? t('ergSupported') : ''}
              {agent?.session_id ? t('activeRide') : ''}
            </p>
            {inProgress ? null : <p>{last ? t('lastTrainerHint', { name: trainerLabel(last) }) : t('pickTrainer')}</p>}
            <div className="panel-actions device-actions">
              {streaming ? null : (
                <>
                  <button
                    type="button"
                    className="secondary"
                    disabled={!canScan}
                    aria-busy={busy && state?.kind === 'scan'}
                    onClick={() => void sendDevice('scan')}
                  >
                    {busy && state?.kind === 'scan' ? t('scanning') : t('scan')}
                  </button>
                  <button
                    type="button"
                    disabled={!canConnect}
                    aria-busy={busy && state?.kind === 'connect'}
                    onClick={() => void sendDevice('connect')}
                  >
                    {t('connectTrainer')}
                  </button>
                </>
              )}
              {streaming || linking ? (
                <button
                  type="button"
                  className="secondary"
                  disabled={!canDisconnect}
                  aria-busy={busy && state?.kind === 'disconnect'}
                  onClick={() => void sendDevice('disconnect')}
                >
                  {t('disconnect')}
                </button>
              ) : null}
            </div>
            {inProgress ? (
              <p className="hint">
                {t(streaming ? 'connectedTrainer' : 'connectingTrainer', {
                  name: last ? trainerLabel(last) : selected || '',
                })}
              </p>
            ) : devices.length ? (
              <ul className="device-list">
                {devices.map((device) => (
                  <li key={device.address}>
                    <button
                      type="button"
                      aria-pressed={selected === device.address}
                      disabled={locked || streaming}
                      onClick={() => pick(device.address, device.name)}
                    >
                      <strong>{device.name || device.address}</strong>
                      <span>
                        {device.address}
                        {device.rssi != null ? ` · ${device.rssi} dBm` : ''}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="hint">{t('noDevices')}</p>
            )}
            <p className="hint">{t('controllersLater')}</p>
          </section>
          <section className="list-card settings">
            <h2>{t('targetPower')}</h2>
            <p>
              {erg
                ? t('ergReady', {
                    range: range ? t('ergRange', { min: range[0], max: range[1] }) : '',
                    target: agent?.target_power_w != null ? t('currentTarget', { watts: agent.target_power_w }) : t('noTarget'),
                  })
                : t('ergUnavailableHint')}
            </p>
            {targetError && <p className="access-error">{targetError}</p>}
            <form className="ftp-form" onSubmit={applyTarget} noValidate>
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
                onClick={() => void sendCommand('release_control').then(() => refresh())}
              >
                {t('releaseControl')}
              </button>
            </form>
          </section>
          <p className="command-status">{describeCommand(state, t)}</p>
        </>
      )}
    </>
  );
}

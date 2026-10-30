import { useEffect, useRef, useState } from 'react';
import type { ApiClient } from '../api/client';
import { LiveStream } from '../api/liveStream';
import type { AgentState } from '../api/types';
import { PowerChart } from '../components/PowerChart';
import { useAppearance } from '../i18n/appearance';
import type { MessageKey } from '../i18n/messages';
import { formatDuration, formatNumber } from '../model/format';
import { PowerTrace, chartClock, currentMetric, liveChart, type Reception } from '../model/live';
import { ZONE_NAME_KEYS, powerZone } from '../model/zones';
import { TrainerPanel } from './TrainerPanel';

const STATES: Record<string, MessageKey> = {
  connecting: 'connecting',
  subscribing: 'subscribing',
  streaming: 'waitingData',
  retry_wait: 'retryWait',
  stopped: 'listenerStopped',
};

interface Props {
  client: ApiClient;
  ftp: number;
  ftpIsDefault: boolean;
  now?: () => number;
  onUnauthorized(): void;
  onOnline(online: boolean): void;
}

const monotonic = () => performance.now();

export function LivePage({ client, ftp, ftpIsDefault, now = monotonic, onUnauthorized, onOnline }: Props) {
  const { t, locale } = useAppearance();
  const [reception, setReception] = useState<Reception>({ snapshot: null, sampledAt: 0, online: false });
  const [agent, setAgent] = useState<AgentState | null>(null);
  const [, setTick] = useState(0);
  const trace = useRef(new PowerTrace());
  const receptionRef = useRef(reception);
  receptionRef.current = reception;

  useEffect(() => {
    const stream = new LiveStream(client, {
      onSnapshot({ live, agent }) {
        const time = now();
        const next: Reception = { snapshot: live, sampledAt: time, online: true };
        if (receptionRef.current.snapshot?.session?.id !== live.session?.id) trace.current.clear();
        if (!receptionRef.current.snapshot?.session?.finished_at) {
          trace.current.record(time, currentMetric(next, 'power_w', time));
        }
        receptionRef.current = next;
        setReception(next);
        setAgent(agent);
        onOnline(true);
      },
      onOffline() {
        const next = { ...receptionRef.current, online: false };
        if (receptionRef.current.online) trace.current.record(now(), null);
        receptionRef.current = next;
        setReception(next);
        setAgent(null);
        onOnline(false);
      },
      onUnauthorized,
    });
    stream.start();
    const timer = setInterval(() => {
      if (receptionRef.current.snapshot?.session?.finished_at) return;
      trace.current.markGapIfExpired(now(), currentMetric(receptionRef.current, 'power_w', now()));
      setTick((tick) => tick + 1);
    }, 1000);
    return () => {
      stream.stop();
      clearInterval(timer);
      onOnline(false);
    };
  }, [client, now, onUnauthorized, onOnline]);

  const time = now();
  const power = currentMetric(reception, 'power_w', time);
  const cadence = currentMetric(reception, 'cadence_rpm', time);
  const speed = currentMetric(reception, 'speed_kmh', time);
  const live = power != null || cadence != null || speed != null;
  const session = reception.snapshot?.session ?? null;
  const zone = powerZone(power, ftp);
  const chart = liveChart(trace.current, chartClock(trace.current, time, Boolean(session?.finished_at)), ftp);
  const hasSamples = trace.current.samples.some((sample) => sample.value != null);

  const trainerStatus = !reception.online
    ? t('noScreenLink')
    : !session
      ? t('waitingTrainerSession')
      : session.finished_at
        ? t('sessionFinished')
        : live
          ? t('liveReceiving')
          : STATES[reception.snapshot?.connection_state ?? '']
            ? t(STATES[reception.snapshot?.connection_state ?? ''])
            : t('waitingConnection');
  const elapsed =
    session && !session.finished_at ? (Date.now() - Date.parse(session.started_at)) / 1000 : null;
  const chartStatus =
    power === 0
      ? t('chartZero')
      : power != null
        ? t('chartLive')
        : session?.finished_at
          ? t('chartFinished')
          : hasSamples
            ? t('chartGap')
            : t('chartWaiting');

  return (
    <>
      <section className="page-heading">
        <div>
          <p className="eyebrow">{t('liveEyebrow')}</p>
          <h1>{t('liveTitle')}</h1>
          <p className="subtitle">{t('liveSubtitle')}</p>
        </div>
        <div className="time-block">
          <span>{t('sessionDuration')}</span>
          <strong data-testid="duration">{formatDuration(elapsed)}</strong>
        </div>
      </section>
      <TrainerPanel
        client={client}
        agent={agent}
        sessionActive={!!session && !session.finished_at}
        onUnauthorized={onUnauthorized}
      />
      <div className="session-strip">
        <div>
          <span className={`status-dot${live ? ' live' : ''}`} />
          <strong role="status" data-testid="trainer-status">
            {trainerStatus}
          </strong>
          <span className="separator">/</span>
          <span>{!session ? t('noSessionYet') : session.finished_at ? t('lastSession') : t('activeSession')}</span>
        </div>
      </div>
      <section className="metrics" aria-label={t('liveMetrics')}>
        <article className={`metric power${zone == null ? '' : ` zone-${zone}`}`} data-testid="power-card">
          <div className="metric-top">
            <h2>{t('power')}</h2>
            <span className="metric-icon">ϟ</span>
          </div>
          <div className="metric-number">
            <strong data-testid="power">{formatNumber(power, locale)}</strong>
            <span>W</span>
          </div>
          <div className="metric-foot">
            <span className="small-dot" />
            <span>
              {power == null
                ? t('noCurrentPower')
                : reception.snapshot?.power_source === 'cycling_power'
                  ? t('sourceCycling')
                  : t('sourceFtms')}
            </span>
          </div>
          <div className="zone-info">
            <strong data-testid="zone-label">
              {zone == null ? t('zoneNone') : `Z${zone} · ${t(ZONE_NAME_KEYS[zone])}`}
            </strong>
            <span>
              {power == null || power < 0
                ? t('noCurrentZone')
                : `%${formatNumber((power / ftp) * 100, locale)} FTP`}{' '}
              · {ftpIsDefault ? t('defaultFtp', { ftp }) : t('ftpWatts', { ftp })}
            </span>
          </div>
        </article>
        <article className="metric">
          <div className="metric-top">
            <h2>{t('cadence')}</h2>
            <span className="metric-icon">↻</span>
          </div>
          <div className="metric-number">
            <strong data-testid="cadence">{formatNumber(cadence, locale)}</strong>
            <span>RPM</span>
          </div>
          <div className="metric-foot">{cadence == null ? t('noCurrentReading') : t('cadenceFoot')}</div>
        </article>
        <article className="metric">
          <div className="metric-top">
            <h2>{t('speed')}</h2>
            <span className="metric-icon">↗</span>
          </div>
          <div className="metric-number">
            <strong data-testid="speed">{formatNumber(speed, locale)}</strong>
            <span>km/h</span>
          </div>
          <div className="metric-foot">{speed == null ? t('noCurrentReading') : t('speedFoot')}</div>
        </article>
      </section>
      <section className="chart-card">
        <div className="section-heading">
          <div>
            <p className="eyebrow">{t('chartEyebrow')}</p>
            <h2>{t('chartTitle')}</h2>
          </div>
          <span className="chart-caption">{t('chartCaption')}</span>
        </div>
        <PowerChart chart={chart} label={t('lastPower')} empty={t('chartEmpty')} />
        <div className="chart-axis">
          <span>{t('sixtyAgo')}</span>
          <span>{chart.drawable ? `${Math.floor(chart.min)}–${Math.ceil(chart.max)} W` : t('autoScale')}</span>
          <span>{t('now')}</span>
        </div>
        <p className="chart-status" role="status" data-testid="chart-status">
          {chartStatus}
        </p>
      </section>
    </>
  );
}

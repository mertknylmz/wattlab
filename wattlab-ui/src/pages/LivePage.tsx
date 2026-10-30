import { useEffect, useRef, useState } from 'react';
import type { ApiClient } from '../api/client';
import { LiveStream } from '../api/liveStream';
import type { AgentState } from '../api/types';
import { PowerChart } from '../components/PowerChart';
import { formatDuration, formatNumber } from '../model/format';
import { PowerTrace, currentMetric, liveChart, type Reception } from '../model/live';
import { ZONE_NAMES, powerZone } from '../model/zones';
import { TrainerPanel } from './TrainerPanel';

const STATES: Record<string, string> = {
  connecting: 'Bağlanıyor',
  subscribing: 'Veri kanalları hazırlanıyor',
  streaming: 'Veri bekleniyor',
  retry_wait: 'Yeniden bağlantı bekleniyor',
  stopped: 'Trainer dinleyicisi durdu',
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
        // Every received snapshot is sampled at once, including a zero just before finish.
        trace.current.record(time, currentMetric(next, 'power_w', time));
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
      // Ages values and marks expiry as a gap; never invents measurements.
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
  const chart = liveChart(trace.current, time, ftp);
  const hasSamples = trace.current.samples.some((sample) => sample.value != null);

  const trainerStatus = !reception.online
    ? 'Ekran bağlantısı yok'
    : !session
      ? 'Trainer oturumu bekleniyor'
      : session.finished_at
        ? 'Oturum tamamlandı'
        : live
          ? 'Canlı veri alınıyor'
          : STATES[reception.snapshot?.connection_state ?? ''] ?? 'Bağlantı bilgisi bekleniyor';
  const elapsed = session
    ? ((session.finished_at ? Date.parse(session.finished_at) : Date.now()) - Date.parse(session.started_at)) / 1000
    : null;
  const chartStatus =
    power === 0
      ? '0 W ölçülüyor · çizgi sıfır seviyesinde devam eder.'
      : power != null
        ? 'Canlı güç örnekleri · kesintiler birleştirilmez.'
        : session?.finished_at
          ? 'Oturum tamamlandı · son gerçek ölçümde biter.'
          : hasSamples
            ? 'Güncel güç ölçümü yok · boşluk sıfır watt anlamına gelmez.'
            : 'Güç ölçümü bekleniyor · yalnızca alınan değerler çizilir.';

  return (
    <>
      <section className="page-heading">
        <div>
          <p className="eyebrow">HER PEDALIN BİR KARŞILIĞI VAR</p>
          <h1>Ritmini bul.</h1>
          <p className="subtitle">Trainer verilerin, anlık ve tek bir yerde.</p>
        </div>
        <div className="time-block">
          <span>OTURUM SÜRESİ</span>
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
          <span>{!session ? 'Henüz oturum yok' : session.finished_at ? 'Son oturum' : 'Aktif oturum'}</span>
        </div>
      </div>
      <section className="metrics" aria-label="Canlı ölçümler">
        <article className={`metric power${zone == null ? '' : ` zone-${zone}`}`} data-testid="power-card">
          <div className="metric-top">
            <h2>GÜÇ</h2>
            <span className="metric-icon">ϟ</span>
          </div>
          <div className="metric-number">
            <strong data-testid="power">{formatNumber(power)}</strong>
            <span>W</span>
          </div>
          <div className="metric-foot">
            <span className="small-dot" />
            <span>
              {power == null
                ? 'Güncel güç ölçümü yok'
                : reception.snapshot?.power_source === 'cycling_power'
                  ? 'Cycling Power kaynağı'
                  : 'FTMS kaynağı'}
            </span>
          </div>
          <div className="zone-info">
            <strong data-testid="zone-label">{zone == null ? 'Zon —' : `Z${zone} · ${ZONE_NAMES[zone]}`}</strong>
            <span>
              {power == null || power < 0 ? 'Güncel zon yok' : `%${formatNumber((power / ftp) * 100)} FTP`} ·{' '}
              {ftpIsDefault ? `varsayılan ${ftp} W` : `FTP ${ftp} W`}
            </span>
          </div>
        </article>
        <article className="metric">
          <div className="metric-top">
            <h2>KADANS</h2>
            <span className="metric-icon">↻</span>
          </div>
          <div className="metric-number">
            <strong data-testid="cadence">{formatNumber(cadence)}</strong>
            <span>RPM</span>
          </div>
          <div className="metric-foot">{cadence == null ? 'Güncel ölçüm yok' : 'FTMS · devir / dakika'}</div>
        </article>
        <article className="metric">
          <div className="metric-top">
            <h2>HIZ</h2>
            <span className="metric-icon">↗</span>
          </div>
          <div className="metric-number">
            <strong data-testid="speed">{formatNumber(speed)}</strong>
            <span>km/h</span>
          </div>
          <div className="metric-foot">{speed == null ? 'Güncel ölçüm yok' : 'FTMS · kilometre / saat'}</div>
        </article>
      </section>
      <section className="chart-card">
        <div className="section-heading">
          <div>
            <p className="eyebrow">PEDALININ İZİ</p>
            <h2>Güç akışı</h2>
          </div>
          <span className="chart-caption">BU EKRANDAKİ SON 60 SANİYE</span>
        </div>
        <PowerChart chart={chart} label="Son güç ölçümleri" empty="Ölçümler geldiğinde güç akışın burada görünecek." />
        <div className="chart-axis">
          <span>60 sn önce</span>
          <span>{chart.drawable ? `${Math.floor(chart.min)}–${Math.ceil(chart.max)} W` : 'Ölçek otomatik'}</span>
          <span>Şimdi</span>
        </div>
        <p className="chart-status" role="status" data-testid="chart-status">
          {chartStatus}
        </p>
      </section>
    </>
  );
}

import { useEffect, useState } from 'react';
import { ApiError, UnauthorizedError, type ApiClient } from '../api/client';
import type { SessionSeries } from '../api/types';
import { PowerChart } from '../components/PowerChart';
import { ZoneLegend } from '../components/ZoneLegend';
import { formatDate, formatDuration, formatNumber } from '../model/format';
import { sessionChart } from '../model/history';
import { ZONE_NAMES } from '../model/zones';

interface Props {
  client: ApiClient;
  id: string;
  onUnauthorized(): void;
}

export function SessionPage({ client, id, onUnauthorized }: Props) {
  const [detail, setDetail] = useState<SessionSeries | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setDetail(null);
    setError(null);
    client.series(id).then(
      (value) => active && setDetail(value),
      (failure) => {
        if (!active) return;
        if (failure instanceof UnauthorizedError) onUnauthorized();
        else if (failure instanceof ApiError && (failure.status === 404 || failure.status === 400))
          setError('Oturum bulunamadı.');
        else setError('Oturum yüklenemedi · yerel API’ye ulaşılamıyor.');
      },
    );
    return () => {
      active = false;
    };
  }, [client, id, onUnauthorized]);

  const summary = detail?.summary;
  const zoneFtp = summary?.zone_ftp_w ?? null;
  const zones = summary?.zone_seconds ?? null;
  const zoneTotal = zones?.reduce((sum, value) => sum + value, 0) ?? 0;

  return (
    <>
      <section className="page-heading">
        <div>
          <p className="eyebrow">
            <a href="#/history">← GEÇMİŞ</a>
          </p>
          <h1>{detail ? formatDate(detail.session.started_at) : 'Oturum'}</h1>
          <p className="subtitle">
            {!detail
              ? (error ?? 'Yükleniyor…')
              : detail.session.finished_at
                ? 'Tamamlanmış oturum'
                : 'Oturum devam ediyor · özet şu ana kadarki kayıtlardan hesaplanır.'}
          </p>
        </div>
        {summary && (
          <div className="time-block">
            <span>SÜRE</span>
            <strong>{formatDuration(summary.duration_seconds)}</strong>
          </div>
        )}
      </section>
      {detail && summary && (
        <>
          <section className="metrics summary" aria-label="Oturum özeti">
            <SummaryCard label="ORT. GÜÇ" value={summary.average_power_w} unit="W" />
            <SummaryCard label="MAKS. GÜÇ" value={summary.max_power_w} unit="W" />
            <SummaryCard label="ORT. KADANS" value={summary.average_cadence_rpm} unit="RPM" />
            <SummaryCard label="ORT. HIZ" value={summary.average_speed_kmh} unit="km/h" />
            <SummaryCard label="İŞ" value={summary.energy_kj} unit="kJ" />
          </section>
          <section className="chart-card">
            <div className="section-heading">
              <div>
                <p className="eyebrow">SANİYELİK ORTALAMA</p>
                <h2>Güç</h2>
              </div>
              <span className="chart-caption">{summary.power_seconds} SN GÜÇ KAYDI</span>
            </div>
            <PowerChart
              chart={sessionChart(detail.points, zoneFtp ?? 200)}
              label="Oturum boyunca güç"
              empty="Bu oturumda çizilecek güç kaydı yok."
            />
            <p className="chart-status">Kayıt olmayan saniyeler boşluk olarak gösterilir; sıfır watt sayılmaz.</p>
          </section>
          <section className="chart-card">
            <div className="section-heading">
              <div>
                <p className="eyebrow">ZON DAĞILIMI</p>
                <h2>Zonlarda geçen süre</h2>
              </div>
              <span className="chart-caption">
                {zoneFtp == null
                  ? 'FTP YOK'
                  : detail.zone_reference === 'session'
                    ? `OTURUM FTP’Sİ ${zoneFtp} W`
                    : `GÜNCEL FTP ${zoneFtp} W · ESKİ KAYIT`}
              </span>
            </div>
            {zones && zoneTotal > 0 ? (
              <ul className="zone-bars">
                {zones.map((seconds, zone) => (
                  <li key={zone}>
                    <span>
                      Z{zone} · {ZONE_NAMES[zone]}
                    </span>
                    <span className="bar">
                      <span className={`zone-${zone}`} style={{ width: `${(seconds / zoneTotal) * 100}%` }} />
                    </span>
                    <span>{formatDuration(seconds)}</span>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="empty">Zon hesaplanacak güç kaydı yok.</p>
            )}
            <ZoneLegend />
          </section>
        </>
      )}
    </>
  );
}

function SummaryCard({ label, value, unit }: { label: string; value: number | null; unit: string }) {
  return (
    <article className="metric">
      <div className="metric-top">
        <h2>{label}</h2>
      </div>
      <div className="metric-number">
        <strong>{formatNumber(value)}</strong>
        <span>{unit}</span>
      </div>
    </article>
  );
}

import { useEffect, useState } from 'react';
import { ApiError, UnauthorizedError, type ApiClient } from '../api/client';
import type { SessionSeries } from '../api/types';
import { PowerChart } from '../components/PowerChart';
import { ZoneLegend } from '../components/ZoneLegend';
import { useAppearance } from '../i18n/appearance';
import { formatDate, formatDuration, formatNumber } from '../model/format';
import { sessionChart } from '../model/history';
import { ZONE_NAME_KEYS } from '../model/zones';

interface Props {
  client: ApiClient;
  id: string;
  onUnauthorized(): void;
}

export function SessionPage({ client, id, onUnauthorized }: Props) {
  const { t, locale } = useAppearance();
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
          setError(t('sessionNotFound'));
        else setError(t('sessionLoadFailed'));
      },
    );
    return () => {
      active = false;
    };
  }, [client, id, onUnauthorized, t]);

  const summary = detail?.summary;
  const zoneFtp = summary?.zone_ftp_w ?? null;
  const zones = summary?.zone_seconds ?? null;
  const zoneTotal = zones?.reduce((sum, value) => sum + value, 0) ?? 0;

  return (
    <>
      <section className="page-heading">
        <div>
          <p className="eyebrow">
            <a href="#/history">{t('backHistory')}</a>
          </p>
          <h1>{detail ? formatDate(detail.session.started_at, locale) : t('session')}</h1>
          <p className="subtitle">
            {!detail ? (error ?? t('loading')) : detail.session.finished_at ? t('sessionDone') : t('sessionOngoing')}
          </p>
        </div>
        {summary && (
          <div className="time-block">
            <span>{t('duration')}</span>
            <strong>{formatDuration(summary.duration_seconds)}</strong>
          </div>
        )}
      </section>
      {detail && summary && (
        <>
          <section className="metrics summary" aria-label={t('sessionSummary')}>
            <SummaryCard label={t('avgPower')} value={summary.average_power_w} unit="W" locale={locale} />
            <SummaryCard label={t('maxPower')} value={summary.max_power_w} unit="W" locale={locale} />
            <SummaryCard label={t('avgCadence')} value={summary.average_cadence_rpm} unit="RPM" locale={locale} />
            <SummaryCard label={t('avgSpeed')} value={summary.average_speed_kmh} unit="km/h" locale={locale} />
            <SummaryCard label={t('work')} value={summary.energy_kj} unit="kJ" locale={locale} />
          </section>
          <section className="chart-card">
            <div className="section-heading">
              <div>
                <p className="eyebrow">{t('perSecond')}</p>
                <h2>{t('powerNoun')}</h2>
              </div>
              <span className="chart-caption">{t('powerSeconds', { n: summary.power_seconds })}</span>
            </div>
            <PowerChart
              chart={sessionChart(detail.points, zoneFtp ?? 200)}
              label={t('sessionPower')}
              empty={t('noPowerTrace')}
            />
            <p className="chart-status">{t('missingSeconds')}</p>
          </section>
          <section className="chart-card">
            <div className="section-heading">
              <div>
                <p className="eyebrow">{t('zoneSplit')}</p>
                <h2>{t('zoneTime')}</h2>
              </div>
              <span className="chart-caption">
                {zoneFtp == null
                  ? t('noFtp')
                  : detail.zone_reference === 'session'
                    ? t('sessionFtp', { ftp: zoneFtp })
                    : t('currentFtpLegacy', { ftp: zoneFtp })}
              </span>
            </div>
            {zones && zoneTotal > 0 ? (
              <ul className="zone-bars">
                {zones.map((seconds, zone) => (
                  <li key={zone}>
                    <span>
                      Z{zone} · {t(ZONE_NAME_KEYS[zone])}
                    </span>
                    <span className="bar">
                      <span className={`zone-${zone}`} style={{ width: `${(seconds / zoneTotal) * 100}%` }} />
                    </span>
                    <span>{formatDuration(seconds)}</span>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="empty">{t('noZonePower')}</p>
            )}
            <ZoneLegend />
          </section>
        </>
      )}
    </>
  );
}

function SummaryCard({
  label,
  value,
  unit,
  locale,
}: {
  label: string;
  value: number | null;
  unit: string;
  locale: 'tr' | 'en';
}) {
  return (
    <article className="metric">
      <div className="metric-top">
        <h2>{label}</h2>
      </div>
      <div className="metric-number">
        <strong>{formatNumber(value, locale)}</strong>
        <span>{unit}</span>
      </div>
    </article>
  );
}

import { useCallback, useEffect, useState } from 'react';
import { UnauthorizedError, type ApiClient } from '../api/client';
import type { SessionSummary } from '../api/types';
import { useAppearance } from '../i18n/appearance';
import { formatDate, formatDuration, formatNumber } from '../model/format';

interface Props {
  client: ApiClient;
  onUnauthorized(): void;
}

export function HistoryPage({ client, onUnauthorized }: Props) {
  const { t, locale } = useAppearance();
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [next, setNext] = useState<string | null>(null);
  const [state, setState] = useState<'loading' | 'ready' | 'failed'>('loading');

  const load = useCallback(
    async (before: string | null) => {
      setState('loading');
      try {
        const page = await client.sessions(before);
        setSessions((current) => (before ? [...current, ...page.sessions] : page.sessions));
        setNext(page.next_before);
        setState('ready');
      } catch (error) {
        if (error instanceof UnauthorizedError) onUnauthorized();
        else setState('failed');
      }
    },
    [client, onUnauthorized],
  );

  useEffect(() => {
    void load(null);
  }, [load]);

  return (
    <>
      <section className="page-heading">
        <div>
          <p className="eyebrow">{t('historyEyebrow')}</p>
          <h1>{t('historyTitle')}</h1>
          <p className="subtitle">{t('historySubtitle')}</p>
        </div>
      </section>
      <section className="list-card">
        {sessions.length === 0 && state === 'ready' && <p className="empty">{t('noSessions')}</p>}
        <ul className="session-list">
          {sessions.map(({ session, summary }) => (
            <li key={session.id}>
              <a href={`#/history/${session.id}`}>
                <span className="session-date">{formatDate(session.started_at, locale)}</span>
                <span>{session.finished_at ? formatDuration(summary.duration_seconds) : t('inProgress')}</span>
                <span>{t('avgShort', { watts: formatNumber(summary.average_power_w, locale) })}</span>
                <span>{t('maxShort', { watts: formatNumber(summary.max_power_w, locale) })}</span>
                <span>{formatNumber(summary.energy_kj, locale)} kJ</span>
              </a>
            </li>
          ))}
        </ul>
        {state === 'failed' && (
          <p className="access-error" role="alert">
            {t('historyFailed')}{' '}
            <button type="button" className="text-button" onClick={() => void load(sessions.length ? next : null)}>
              {t('retry')}
            </button>
          </p>
        )}
        {state === 'loading' && <p className="empty">{t('loading')}</p>}
        {next && state === 'ready' && (
          <button type="button" className="secondary" onClick={() => void load(next)}>
            {t('olderSessions')}
          </button>
        )}
      </section>
    </>
  );
}

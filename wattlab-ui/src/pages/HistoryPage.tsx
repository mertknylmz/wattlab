import { useCallback, useEffect, useState } from 'react';
import { UnauthorizedError, type ApiClient } from '../api/client';
import type { SessionSummary } from '../api/types';
import { formatDate, formatDuration, formatNumber } from '../model/format';

interface Props {
  client: ApiClient;
  onUnauthorized(): void;
}

export function HistoryPage({ client, onUnauthorized }: Props) {
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
          <p className="eyebrow">GEÇMİŞ</p>
          <h1>Antrenmanların.</h1>
          <p className="subtitle">Kaydedilen oturumlar, en yenisi üstte.</p>
        </div>
      </section>
      <section className="list-card">
        {sessions.length === 0 && state === 'ready' && <p className="empty">Henüz kaydedilmiş oturum yok.</p>}
        <ul className="session-list">
          {sessions.map(({ session, summary }) => (
            <li key={session.id}>
              <a href={`#/history/${session.id}`}>
                <span className="session-date">{formatDate(session.started_at)}</span>
                <span>{session.finished_at ? formatDuration(summary.duration_seconds) : 'Devam ediyor'}</span>
                <span>{formatNumber(summary.average_power_w)} W ort.</span>
                <span>{formatNumber(summary.max_power_w)} W maks.</span>
                <span>{formatNumber(summary.energy_kj)} kJ</span>
              </a>
            </li>
          ))}
        </ul>
        {state === 'failed' && (
          <p className="access-error" role="alert">
            Geçmiş yüklenemedi · yerel API’ye ulaşılamıyor.{' '}
            <button type="button" className="text-button" onClick={() => void load(sessions.length ? next : null)}>
              Tekrar dene
            </button>
          </p>
        )}
        {state === 'loading' && <p className="empty">Yükleniyor…</p>}
        {next && state === 'ready' && (
          <button type="button" className="secondary" onClick={() => void load(next)}>
            Daha eski oturumlar
          </button>
        )}
      </section>
    </>
  );
}

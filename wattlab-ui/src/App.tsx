import { useCallback, useEffect, useMemo, useState } from 'react';
import { ApiClient, UnauthorizedError } from './api/client';
import type { Settings } from './api/types';
import { DEFAULT_FTP_W } from './model/zones';
import { HistoryPage } from './pages/HistoryPage';
import { LivePage } from './pages/LivePage';
import { SessionPage } from './pages/SessionPage';
import { SettingsPage } from './pages/SettingsPage';

type Route = { page: 'live' } | { page: 'history' } | { page: 'session'; id: string } | { page: 'settings' };

export function parseRoute(hash: string): Route {
  const parts = hash.replace(/^#\/?/, '').split('/');
  if (parts[0] === 'history' && parts[1]) return { page: 'session', id: parts[1] };
  if (parts[0] === 'history') return { page: 'history' };
  if (parts[0] === 'settings') return { page: 'settings' };
  return { page: 'live' };
}

function useRoute(): Route {
  const [route, setRoute] = useState(() => parseRoute(window.location.hash));
  useEffect(() => {
    const update = () => setRoute(parseRoute(window.location.hash));
    window.addEventListener('hashchange', update);
    return () => window.removeEventListener('hashchange', update);
  }, []);
  return route;
}

const TITLES: Record<Route['page'], string> = {
  live: 'CANLI ANTRENMAN',
  history: 'GEÇMİŞ',
  session: 'OTURUM',
  settings: 'AYARLAR',
};

export function App({ client: provided }: { client?: ApiClient }) {
  const route = useRoute();
  const client = useMemo(() => provided ?? new ApiClient(), [provided]);
  const [refused, setRefused] = useState(false);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [online, setOnline] = useState<boolean | null>(null);
  const refuse = useCallback(() => setRefused(true), []);

  useEffect(() => {
    let active = true;
    let retry: ReturnType<typeof setTimeout> | undefined;
    const load = () =>
      client.settings().then(
        (value) => active && setSettings(value),
        (error) => {
          if (!active) return;
          if (error instanceof UnauthorizedError) refuse();
          else retry = setTimeout(load, 3000);
        },
      );
    void load();
    return () => {
      active = false;
      clearTimeout(retry);
    };
  }, [client, refuse]);

  const nav = (page: Route['page'], href: string, label: string) => {
    const active = route.page === page || (page === 'history' && route.page === 'session');
    return (
      <a className={`nav-item${active ? ' active' : ''}`} href={href} aria-current={active ? 'page' : undefined}>
        <span className="nav-dot" /> {label} <span className="nav-arrow">↗</span>
      </a>
    );
  };

  const ftp = settings?.ftp_w ?? DEFAULT_FTP_W;
  let page = null;
  switch (route.page) {
    case 'live':
      page = (
        <LivePage
          client={client}
          ftp={ftp}
          ftpIsDefault={settings?.ftp_is_default ?? true}
          onUnauthorized={refuse}
          onOnline={setOnline}
        />
      );
      break;
    case 'history':
      page = <HistoryPage client={client} onUnauthorized={refuse} />;
      break;
    case 'session':
      page = <SessionPage key={route.id} client={client} id={route.id} onUnauthorized={refuse} />;
      break;
    case 'settings':
      page = <SettingsPage client={client} settings={settings} onSaved={setSettings} onUnauthorized={refuse} />;
      break;
  }

  const badge = refused
    ? ['Erişim reddedildi', 'error']
    : route.page !== 'live' || online == null
      ? ['Yerel API', 'good']
      : online
        ? ['Canlı · yerel', 'good']
        : ['API’ye ulaşılamıyor', 'error'];

  return (
    <div className="shell">
      <aside className="rail">
        <a className="brand" href="#/live" aria-label="WattLab ana sayfa">
          <span className="brand-icon">ϟ</span>Watt<span>Lab</span>
        </a>
        <p className="rail-caption">ANTRENMAN ALANI</p>
        <nav className="nav">
          {nav('live', '#/live', 'Canlı görünüm')}
          {nav('history', '#/history', 'Geçmiş')}
          {nav('settings', '#/settings', 'Ayarlar')}
        </nav>
        <div className="rail-bottom">
          <span className="local-dot" /> Bu bilgisayarda çalışıyor
          <p>
            Veriler yerelde kalır.
            <br />
            İnternet bağlantısı gerekmez.
          </p>
        </div>
      </aside>
      <main>
        <header>
          <div className="breadcrumb">
            WATTLAB <span>/</span> {TITLES[route.page]}
          </div>
          <span className={`badge ${badge[1]}`} data-testid="api-status">
            {badge[0]}
          </span>
        </header>
        {refused ? (
          <p className="access-error" role="alert">
            Yerel API erişimi reddetti. Uygulamayı kapatıp yeniden aç.
          </p>
        ) : (
          page
        )}
        <footer>
          <span>Trainer komutları yalnızca yerel trainer ajanı üzerinden gönderilir.</span>
        </footer>
      </main>
    </div>
  );
}

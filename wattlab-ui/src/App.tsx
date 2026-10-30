import { useCallback, useEffect, useMemo, useState } from 'react';
import { ApiClient, ApiError, UnauthorizedError } from './api/client';
import type { Member, Settings } from './api/types';
import { AppearanceBar } from './components/AppearanceBar';
import { ConnectionStatus } from './components/ConnectionStatus';
import { AppearanceProvider, useAppearance } from './i18n/appearance';
import type { MessageKey } from './i18n/messages';
import { DEFAULT_FTP_W } from './model/zones';
import { useAgentStatus } from './hooks/useAgentStatus';
import { useAutoConnect } from './hooks/useAutoConnect';
import { AuthPage } from './pages/AuthPage';
import { HistoryPage } from './pages/HistoryPage';
import { LivePage } from './pages/LivePage';
import { ProfilePage } from './pages/ProfilePage';
import { SessionPage } from './pages/SessionPage';
import { SettingsPage, type SettingsTab } from './pages/SettingsPage';

type Route =
  | { page: 'live' }
  | { page: 'history' }
  | { page: 'session'; id: string }
  | { page: 'settings'; tab: SettingsTab }
  | { page: 'profile' };

export function parseRoute(hash: string): Route {
  const parts = hash.replace(/^#\/?/, '').split('/');
  if (parts[0] === 'history' && parts[1]) return { page: 'session', id: parts[1] };
  if (parts[0] === 'history') return { page: 'history' };
  if (parts[0] === 'settings') {
    return { page: 'settings', tab: parts[1] === 'connection' ? 'connection' : 'general' };
  }
  if (parts[0] === 'profile') return { page: 'profile' };
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

const TITLES: Record<Route['page'], MessageKey> = {
  live: 'crumbLive',
  history: 'crumbHistory',
  session: 'crumbSession',
  settings: 'crumbSettings',
  profile: 'crumbProfile',
};

function Icon({ path }: { path: string }) {
  return (
    <svg className="rail-icon" viewBox="0 0 24 24" aria-hidden="true">
      <path fill="currentColor" d={path} />
    </svg>
  );
}

export function App({ client: provided }: { client?: ApiClient }) {
  return (
    <AppearanceProvider>
      <AppShell client={provided} />
    </AppearanceProvider>
  );
}

function AppShell({ client: provided }: { client?: ApiClient }) {
  const { t } = useAppearance();
  const route = useRoute();
  const client = useMemo(() => provided ?? new ApiClient(), [provided]);
  const [user, setUser] = useState<Member | null | undefined>(undefined);
  const [rememberAvailable, setRememberAvailable] = useState(false);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [online, setOnline] = useState<boolean | null>(null);
  const [logoutError, setLogoutError] = useState<string | null>(null);
  const signOut = useCallback(() => setUser(null), []);

  useEffect(() => {
    let active = true;
    client.capabilities().then(
      (value) => active && setRememberAvailable(value.remember_available),
      () => active && setRememberAvailable(false),
    );
    client.me().then(
      (member) => active && setUser(member),
      (error) => {
        if (!active) return;
        if (error instanceof UnauthorizedError) setUser(null);
        else setUser(null);
      },
    );
    return () => {
      active = false;
    };
  }, [client]);

  useEffect(() => {
    if (!user) {
      setSettings(null);
      return;
    }
    let active = true;
    let retry: ReturnType<typeof setTimeout> | undefined;
    const load = () =>
      client.settings().then(
        (value) => active && setSettings(value),
        (error) => {
          if (!active) return;
          if (error instanceof UnauthorizedError) signOut();
          else retry = setTimeout(load, 3000);
        },
      );
    void load();
    return () => {
      active = false;
      clearTimeout(retry);
    };
  }, [client, user, signOut]);

  const logout = async () => {
    setLogoutError(null);
    try {
      await client.logout();
      signOut();
    } catch (error) {
      if (error instanceof ApiError && error.code === 'active_session') {
        setLogoutError(t('logoutActiveRide'));
        return;
      }
      if (error instanceof UnauthorizedError) signOut();
      else setLogoutError(t('logoutFailed'));
    }
  };

  useAutoConnect(client, user != null, signOut);
  const { agent } = useAgentStatus(client, signOut, 1000, user != null);

  if (user === undefined) {
    return (
      <div className="auth-page">
        <p className="subtitle">{t('checkingAccount')}</p>
      </div>
    );
  }
  if (user === null) {
    return <AuthPage client={client} rememberAvailable={rememberAvailable} onSignedIn={setUser} />;
  }

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
          onUnauthorized={signOut}
          onOnline={setOnline}
        />
      );
      break;
    case 'history':
      page = <HistoryPage client={client} onUnauthorized={signOut} />;
      break;
    case 'session':
      page = <SessionPage key={route.id} client={client} id={route.id} onUnauthorized={signOut} />;
      break;
    case 'settings':
      page = (
        <SettingsPage
          client={client}
          tab={route.tab}
          settings={settings}
          onSaved={setSettings}
          onUnauthorized={signOut}
        />
      );
      break;
    case 'profile':
      page = <ProfilePage client={client} user={user} onUnauthorized={signOut} />;
      break;
  }

  const badge =
    route.page !== 'live' || online == null
      ? [t('badgeLocal'), 'good']
      : online
        ? [t('badgeLive'), 'good']
        : [t('badgeOffline'), 'error'];

  return (
    <div className="shell">
      <aside className="rail">
        <a className="brand" href="#/live" aria-label={t('brandHome')}>
          <span className="brand-icon">ϟ</span>Watt<span>Lab</span>
        </a>
        <p className="rail-caption">{t('railCaption')}</p>
        <nav className="nav">
          {nav('live', '#/live', t('navLive'))}
          {nav('history', '#/history', t('navHistory'))}
        </nav>
        <div className="rail-bottom">
          <span className="rail-user">{user.username}</span>
          <nav className="rail-account" aria-label={t('accountNav')}>
            <a className={route.page === 'profile' ? 'active' : undefined} href="#/profile">
              <Icon path="M12 12a4 4 0 1 0-4-4 4 4 0 0 0 4 4zm0 2c-3.3 0-8 1.7-8 4v2h16v-2c0-2.3-4.7-4-8-4z" />
              {t('profile')}
            </a>
            <a className={route.page === 'settings' ? 'active' : undefined} href="#/settings">
              <Icon path="M19.1 12.9a7.6 7.6 0 0 0 .1-1.8l2-1.6-2-3.4-2.4 1a7.4 7.4 0 0 0-1.6-.9L14.6 3h-5.2L8.8 6.2a7.4 7.4 0 0 0-1.6.9l-2.4-1-2 3.4 2 1.6a7.6 7.6 0 0 0-.1 1.8l-2 1.6 2 3.4 2.4-1a7.4 7.4 0 0 0 1.6.9L9.4 21h5.2l.6-3.2a7.4 7.4 0 0 0 1.6-.9l2.4 1 2-3.4zM12 15.5A3.5 3.5 0 1 1 15.5 12 3.5 3.5 0 0 1 12 15.5z" />
              {t('settings')}
            </a>
            <button type="button" onClick={() => void logout()}>
              <Icon path="M10 17l1.4-1.4L8.8 13H19v-2H8.8l2.6-2.6L10 7l-5 5zM4 3h8V1H4a2 2 0 0 0-2 2v18a2 2 0 0 0 2 2h8v-2H4z" />
              {t('logout')}
            </button>
          </nav>
          {logoutError ? (
            <p className="access-error" role="alert">
              {logoutError}
            </p>
          ) : null}
          <p>
            <span className="local-dot" /> {t('dataStaysLocal')}
          </p>
          <AppearanceBar />
        </div>
      </aside>
      <main>
        <header>
          <div className="breadcrumb">
            WATTLAB <span>/</span> {t(TITLES[route.page])}
          </div>
          <div className="header-tools">
            <ConnectionStatus agent={agent} />
            <span className={`badge ${badge[1]}`} data-testid="api-status">
              {badge[0]}
            </span>
          </div>
        </header>
        {page}
        <footer>
          <span>{t('footerCommands')}</span>
        </footer>
      </main>
    </div>
  );
}

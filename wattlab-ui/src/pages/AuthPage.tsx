import { useState } from 'react';
import { ApiClient, ApiError, UnauthorizedError } from '../api/client';
import type { Member } from '../api/types';
import { AppearanceBar } from '../components/AppearanceBar';
import { useAppearance } from '../i18n/appearance';
import { AUTH_ERRORS } from '../i18n/messages';

interface Props {
  client: ApiClient;
  rememberAvailable: boolean;
  onSignedIn(member: Member): void;
}

export function AuthPage({ client, rememberAvailable, onSignedIn }: Props) {
  const { t } = useAppearance();
  const [mode, setMode] = useState<'login' | 'register'>('login');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [repeat, setRepeat] = useState('');
  const [remember, setRemember] = useState(rememberAvailable);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (mode === 'register' && password !== repeat) {
      setError(t('passwordsMismatch'));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const keep = rememberAvailable && remember;
      const session =
        mode === 'register'
          ? await client.register(username, password, keep)
          : await client.login(username, password, keep);
      onSignedIn(session.member);
    } catch (cause) {
      if (cause instanceof UnauthorizedError) setError(t('errInvalidCredentials'));
      else if (cause instanceof ApiError && cause.code && AUTH_ERRORS[cause.code]) setError(t(AUTH_ERRORS[cause.code]));
      else setError(t('apiUnreachable'));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="auth-page">
      <div className="list-card auth-card">
        <p className="eyebrow">WATTLAB</p>
        <h1>{mode === 'login' ? t('login') : t('register')}</h1>
        <p className="subtitle">{t('authSubtitle')}</p>
        <form className="auth-form" onSubmit={(event) => void submit(event)}>
          <label>
            {t('username')}
            <input
              name="username"
              autoComplete="username"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              required
              minLength={3}
              maxLength={32}
            />
          </label>
          <label>
            {t('password')}
            <input
              name="password"
              type="password"
              autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              required
              minLength={8}
              maxLength={128}
            />
          </label>
          {mode === 'register' ? (
            <label>
              {t('passwordRepeat')}
              <input
                name="repeat"
                type="password"
                autoComplete="new-password"
                value={repeat}
                onChange={(event) => setRepeat(event.target.value)}
                required
                minLength={8}
                maxLength={128}
              />
            </label>
          ) : null}
          <label className="remember" htmlFor="remember">
            <input
              id="remember"
              type="checkbox"
              checked={rememberAvailable && remember}
              disabled={!rememberAvailable}
              onChange={(event) => setRemember(event.target.checked)}
            />
            <span>{t('rememberMe')}</span>
          </label>
          {!rememberAvailable ? <p className="hint">{t('rememberUnavailable')}</p> : null}
          {error ? (
            <p className="access-error" role="alert">
              {error}
            </p>
          ) : null}
          <button type="submit" disabled={busy}>
            {mode === 'login' ? t('login') : t('register')}
          </button>
        </form>
        <button
          type="button"
          className="text-button"
          onClick={() => {
            setMode(mode === 'login' ? 'register' : 'login');
            setError(null);
          }}
        >
          {mode === 'login' ? t('goRegister') : t('goLogin')}
        </button>
        <AppearanceBar />
      </div>
    </div>
  );
}

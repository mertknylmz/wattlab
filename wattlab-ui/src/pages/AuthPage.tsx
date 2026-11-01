import { useState } from 'react';
import { ApiClient, ApiError, UnauthorizedError } from '../api/client';
import type { Member } from '../api/types';
import { AppearanceBar } from '../components/AppearanceBar';
import { useAppearance } from '../i18n/appearance';
import { AUTH_ERRORS } from '../i18n/messages';
import { readRememberedUsername, writeRememberedUsername } from '../model/authPrefs';

interface Props {
  initialMode?: 'login' | 'register';
  onBack?(): void;
  client: ApiClient;
  rememberAvailable: boolean;
  onSignedIn(member: Member): void;
}

export function AuthPage({ client, rememberAvailable, onSignedIn, initialMode = 'login', onBack }: Props) {
  const { t } = useAppearance();
  const [mode, setMode] = useState<'login' | 'register'>(initialMode);
  const [rememberedUsername, setRememberedUsername] = useState(readRememberedUsername);
  const [username, setUsername] = useState(initialMode === 'login' ? rememberedUsername ?? '' : '');
  const passwordOnly = mode === 'login' && rememberedUsername !== null;
  const [password, setPassword] = useState('');
  const [repeat, setRepeat] = useState('');
  const [remember, setRemember] = useState<boolean | null>(null);
  const keep = rememberAvailable && (remember ?? true);
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
      const session =
        mode === 'register'
          ? await client.register(username, password, keep)
          : await client.login(passwordOnly ? rememberedUsername : username, password, keep);
      writeRememberedUsername(keep ? session.member.username : null);
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
        {onBack ? (
          <button type="button" className="text-button auth-back" onClick={onBack}>
            <span aria-hidden="true">← </span>{t('backToStart')}
          </button>
        ) : null}
        <p className="eyebrow">WATTLAB</p>
        <h1 className={passwordOnly ? 'auth-greeting' : undefined}>
          {passwordOnly ? t('welcomeBack', { username: rememberedUsername }) : mode === 'login' ? t('login') : t('register')}
        </h1>
        {mode === 'register' ? <p className="subtitle">{t('authSubtitle')}</p> : null}
        <form className="auth-form" onSubmit={(event) => void submit(event)}>
          {!passwordOnly ? (
            <label>
              {t('username')}
              <input
                name="username"
                autoComplete="username"
                autoFocus
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                required
                minLength={3}
                maxLength={32}
              />
            </label>
          ) : null}
          <label>
            {t('password')}
            <input
              autoFocus={passwordOnly}
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
              checked={keep}
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
        {passwordOnly ? (
          <button
            type="button"
            className="text-button auth-link switch-account"
            onClick={() => {
              writeRememberedUsername(null);
              setRememberedUsername(null);
              setUsername('');
              setPassword('');
              setRepeat('');
              setError(null);
            }}
          >
            {t('useDifferentAccount')}
          </button>
        ) : null}
        <p className="auth-navigation">
          <span>{mode === 'login' ? t('noAccount') : t('haveAccount')}</span>{' '}
          <button
            type="button"
            className="text-button auth-link"
            onClick={() => {
              setMode(mode === 'login' ? 'register' : 'login');
              setUsername(mode === 'login' ? '' : rememberedUsername ?? '');
              setPassword('');
              setRepeat('');
              setError(null);
            }}
          >
            {mode === 'login' ? t('createOne') : t('login')}
          </button>
        </p>
        <AppearanceBar />
      </div>
    </div>
  );
}

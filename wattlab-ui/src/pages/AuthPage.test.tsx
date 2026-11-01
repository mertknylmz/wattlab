import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { App } from '../App';
import { ApiClient } from '../api/client';
import { AppearanceProvider } from '../i18n/appearance';
import { AuthPage } from './AuthPage';

const KEY = 'wattlab.rememberedUsername';
const MEMBER = { id: '11111111-1111-1111-1111-111111111111', username: 'rider' };
const json = (body: unknown, status = 200) => Promise.resolve(new Response(JSON.stringify(body), { status }));
const input = (label: string) => screen.getByLabelText(label) as HTMLInputElement;

function form(fetcher: ReturnType<typeof vi.fn>, available = true) {
  const onSignedIn = vi.fn();
  render(<AppearanceProvider><AuthPage
    client={new ApiClient(fetcher as unknown as typeof fetch)}
    rememberAvailable={available}
    onSignedIn={onSignedIn}
  /></AppearanceProvider>);
  return onSignedIn;
}

function credentials(username = 'rider') {
  fireEvent.change(input('Username'), { target: { value: username } });
  fireEvent.change(input('Password'), { target: { value: 'correct horse' } });
}

afterEach(() => {
  cleanup();
  localStorage.clear();
  document.documentElement.removeAttribute('data-theme');
  window.history.replaceState(null, '', '#/welcome');
  vi.restoreAllMocks();
});

describe('remembered username', () => {
  it('retains only the username after remembered login and logout, requires a password and permits account switching', async () => {
    let authenticated = false;
    const fetcher = vi.fn((url: string, options?: RequestInit) => {
      if (url === '/api/v1/desktop/capabilities') return json({ remember_available: true });
      if (url === '/api/v1/auth/me') return json(authenticated ? MEMBER : {}, authenticated ? 200 : 401);
      if (url === '/api/v1/auth/login') {
        expect(options?.headers).toMatchObject({ 'X-WattLab-Remember': '1' });
        const body = JSON.parse(String(options?.body));
        authenticated = true;
        return json({ member: { ...MEMBER, username: body.username.toLowerCase() } });
      }
      if (url === '/api/v1/auth/logout') {
        expect(options?.method).toBe('POST');
        authenticated = false;
        return Promise.resolve(new Response(null, { status: 204 }));
      }
      return new Promise(() => {});
    });
    const mount = () => render(<App client={new ApiClient(fetcher as unknown as typeof fetch)} />);
    let view = mount();
    await screen.findByRole('heading', { name: 'Sign in' });
    await waitFor(() => expect(input('Remember me').disabled).toBe(false));
    credentials('Rider');
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    await screen.findByRole('link', { name: 'Get started' });
    expect(localStorage.getItem(KEY)).toBe('rider');
    expect(JSON.stringify(localStorage)).not.toContain('correct horse');

    // Restoring an authenticated session continues to open Welcome.
    view.unmount();
    view = mount();
    await screen.findByRole('link', { name: 'Get started' });
    expect(fetcher.mock.calls.filter(([url]) => url === '/api/v1/auth/login')).toHaveLength(1);
    fireEvent.click(screen.getByRole('button', { name: 'Log out' }));
    await screen.findByRole('heading', { name: 'Welcome back, rider' });
    expect(authenticated).toBe(false);
    expect(screen.queryByLabelText('Username')).toBeNull();
    expect(document.querySelector('input[name=username]')).toBeNull();
    expect(input('Password').value).toBe('');
    expect(document.activeElement).toBe(input('Password'));
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    expect(fetcher.mock.calls.filter(([url]) => url === '/api/v1/auth/login')).toHaveLength(1);

    // Restart after explicit logout must not restore authentication.
    view.unmount();
    mount();
    await screen.findByRole('heading', { name: 'Welcome back, rider' });
    expect(screen.queryByLabelText('Username')).toBeNull();
    expect(input('Password').value).toBe('');
    fireEvent.change(input('Password'), { target: { value: 'correct horse' } });
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    await screen.findByRole('link', { name: 'Get started' });
    const logins = fetcher.mock.calls.filter(([url]) => url === '/api/v1/auth/login');
    expect(JSON.parse(String(logins[1][1]?.body))).toEqual({ username: 'rider', password: 'correct horse' });
    fireEvent.click(screen.getByRole('button', { name: 'Log out' }));
    await screen.findByRole('heading', { name: 'Welcome back, rider' });
    fireEvent.click(screen.getByRole('button', { name: 'Use a different account' }));
    expect(input('Username').value).toBe('');
    expect(input('Password').value).toBe('');
    expect(localStorage.getItem(KEY)).toBeNull();
    credentials('another');
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    await screen.findByRole('link', { name: 'Get started' });
    expect(localStorage.getItem(KEY)).toBe('another');
  });

  it.each([true, false])('does not retain the username with remember unchecked or unavailable (available=%s)', async (available) => {
    localStorage.setItem(KEY, 'previous');
    const fetcher = vi.fn((_url: string, _options?: RequestInit) => json({ member: MEMBER }));
    const signedIn = form(fetcher, available);
    expect(screen.queryByLabelText('Username')).toBeNull();
    fireEvent.change(input('Password'), { target: { value: 'correct horse' } });
    if (available) fireEvent.click(input('Remember me'));
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    await waitFor(() => expect(signedIn).toHaveBeenCalledWith(MEMBER));
    expect(localStorage.getItem(KEY)).toBeNull();
    expect(fetcher.mock.calls[0][1]?.headers).toMatchObject({ 'X-WattLab-Remember': '0' });
  });

  it('keeps the previous preference after a rejected login', async () => {
    localStorage.setItem(KEY, 'previous');
    const signedIn = form(vi.fn(() => json({}, 401)));
    fireEvent.change(input('Password'), { target: { value: 'wrong password' } });
    fireEvent.click(input('Remember me'));
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    await screen.findByRole('alert');
    expect(signedIn).not.toHaveBeenCalled();
    expect(localStorage.getItem(KEY)).toBe('previous');
  });

  it('switches to another account with localized controls and clears unsent credentials', () => {
    localStorage.setItem(KEY, 'rider');
    const fetcher = vi.fn();
    form(fetcher);
    fireEvent.change(input('Password'), { target: { value: 'unsent password' } });
    fireEvent.click(screen.getByRole('button', { name: 'TR' }));
    expect(screen.getByRole('heading', { name: 'Tekrar hoş geldin, rider' })).toBeTruthy();
    expect(screen.queryByLabelText('Kullanıcı adı')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Başka bir hesapla giriş yap' }));
    expect(localStorage.getItem(KEY)).toBeNull();
    expect(input('Kullanıcı adı').value).toBe('');
    expect(input('Şifre').value).toBe('');
    expect(document.activeElement).toBe(input('Kullanıcı adı'));
    expect(fetcher).not.toHaveBeenCalled();
  });

  it('does not carry login credentials into registration and restores the saved username when returning', () => {
    localStorage.setItem(KEY, 'rider');
    form(vi.fn());
    fireEvent.change(input('Password'), { target: { value: 'unsent password' } });
    fireEvent.click(screen.getByRole('button', { name: /No account/ }));
    expect(input('Username').value).toBe('');
    expect(input('Password').value).toBe('');
    expect(input('Repeat password').value).toBe('');
    fireEvent.click(screen.getByRole('button', { name: /Already have/ }));
    expect(screen.getByRole('heading', { name: 'Welcome back, rider' })).toBeTruthy();
    expect(screen.queryByLabelText('Username')).toBeNull();
    expect(input('Password').value).toBe('');
  });

  it('still signs in if preference storage is unavailable', async () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => { throw new Error('unavailable'); });
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('unavailable'); });
    const signedIn = form(vi.fn(() => json({ member: MEMBER })));
    expect(input('Username').value).toBe('');
    credentials();
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    await waitFor(() => expect(signedIn).toHaveBeenCalledWith(MEMBER));
  });
});

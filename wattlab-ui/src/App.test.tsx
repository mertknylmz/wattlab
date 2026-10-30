import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiClient } from './api/client';
import { App, parseRoute } from './App';
import { describeCommand } from './pages/TrainerPanel';

function json(body: unknown, status = 200) {
  return Promise.resolve(
    new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } }),
  );
}

const SETTINGS = { ftp_w: 250, ftp_is_default: false, updated_at: '2026-10-30T10:00:00Z' };
const ME = { id: '11111111-1111-1111-1111-111111111111', username: 'holy' };
const AGENT_IDLE = {
  online: true,
  agent_id: 'agent',
  connection_state: 'idle',
  session_id: null,
  erg_supported: false,
  power_range_w: null,
  target_power_w: null,
  last_seen_at: '2026-10-30T10:00:00Z',
};

describe('App', () => {
  let fetcher: ReturnType<typeof vi.fn>;
  const renderApp = () => render(<App client={new ApiClient(fetcher as unknown as typeof fetch)} />);

  beforeEach(() => {
    window.location.hash = '#/settings';
    fetcher = vi.fn((url: string) => {
      if (url === '/api/v1/desktop/capabilities') return json({ remember_available: true });
      if (url === '/api/v1/auth/me') return json(ME);
      if (url === '/api/v1/settings') return json(SETTINGS);
      if (url === '/api/v1/agent/status') return json(AGENT_IDLE);
      return new Promise(() => {});
    });
  });

  afterEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute('data-theme');
    cleanup();
  });

  it('opens signed-in screens without exposing a token field', async () => {
    renderApp();
    expect(await screen.findByTestId('ftp-current')).toHaveProperty('textContent', expect.stringContaining('250 W'));
    expect(screen.getByText('holy')).toBeTruthy();
    expect(screen.queryByLabelText('Password')).toBeNull();
  });

  it('asks for login when the desktop shell has no member session', async () => {
    fetcher.mockImplementation((url: string) => {
      if (url === '/api/v1/desktop/capabilities') return json({ remember_available: false });
      if (url === '/api/v1/auth/me') return json({}, 401);
      return json({});
    });
    renderApp();
    expect(await screen.findByRole('heading', { name: 'Sign in' })).toBeTruthy();
    expect(screen.getByText(/no encrypted storage/)).toBeTruthy();
  });

  it('registers from the form and then shows the app', async () => {
    fetcher.mockImplementation((url: string, options?: RequestInit) => {
      if (url === '/api/v1/desktop/capabilities') return json({ remember_available: true });
      if (url === '/api/v1/auth/me') return json({}, 401);
      if (url === '/api/v1/auth/register') {
        expect(options?.headers).toMatchObject({ 'X-WattLab-Remember': '1' });
        expect(JSON.parse(String(options?.body))).toEqual({ username: 'holy', password: 'correct horse' });
        return json({ member: ME, expires_at: SETTINGS.updated_at }, 201);
      }
      if (url === '/api/v1/settings') return json(SETTINGS);
      return new Promise(() => {});
    });
    renderApp();
    fireEvent.click(await screen.findByRole('button', { name: /No account/ }));
    fireEvent.change(screen.getByLabelText('Username'), { target: { value: 'holy' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'correct horse' } });
    fireEvent.change(screen.getByLabelText('Repeat password'), { target: { value: 'correct horse' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create account' }));
    expect(await screen.findByTestId('ftp-current')).toBeTruthy();
  });

  it('blocks logout while a ride is active', async () => {
    fetcher.mockImplementation((url: string, options?: RequestInit) => {
      if (url === '/api/v1/desktop/capabilities') return json({ remember_available: true });
      if (url === '/api/v1/auth/me') return json(ME);
      if (url === '/api/v1/settings') return json(SETTINGS);
      if (url === '/api/v1/auth/logout' && options?.method === 'POST') {
        return json({ error: 'active_session' }, 409);
      }
      return new Promise(() => {});
    });
    renderApp();
    fireEvent.click(await screen.findByRole('button', { name: 'Log out' }));
    expect(await screen.findByRole('alert')).toHaveProperty('textContent', expect.stringContaining('active ride first'));
    expect(screen.getByText('holy')).toBeTruthy();
  });

  it('validates FTP before saving and can reset to the default', async () => {
    renderApp();
    await screen.findByText(/250 W ·/);
    const input = screen.getByRole('spinbutton');
    for (const invalid of ['0', '2010', '12.5']) {
      fireEvent.change(input, { target: { value: invalid } });
      fireEvent.click(screen.getByRole('button', { name: 'Save' }));
      expect(screen.getByRole('status').textContent).toMatch(/1 and 2000/);
    }
    fetcher.mockImplementation((url: string, options?: RequestInit) => {
      if (url === '/api/v1/desktop/capabilities') return json({ remember_available: true });
      if (url === '/api/v1/auth/me') return json(ME);
      if (url === '/api/v1/settings' && options?.method === 'PUT') {
        return json({ ftp_w: 200, ftp_is_default: true, updated_at: SETTINGS.updated_at });
      }
      return json(SETTINGS);
    });
    fireEvent.click(screen.getByRole('button', { name: 'Reset to default' }));
    await screen.findByText(/Default 200 W/);
    const put = fetcher.mock.calls.find(([, options]) => options?.method === 'PUT');
    expect(JSON.parse(put![1].body)).toEqual({ ftp_w: null });
  });

  it('connects the trainer from settings', async () => {
    let connected = false;
    fetcher.mockImplementation((url: string, options?: RequestInit) => {
      if (url === '/api/v1/desktop/capabilities') return json({ remember_available: true });
      if (url === '/api/v1/auth/me') return json(ME);
      if (url === '/api/v1/settings') return json(SETTINGS);
      if (url === '/api/v1/agent/status') {
        return json(
          connected ? { ...AGENT_IDLE, connection_state: 'streaming', erg_supported: true } : AGENT_IDLE,
        );
      }
      if (url === '/api/v1/agent/commands' && options?.method === 'POST') {
        expect(JSON.parse(String(options.body))).toEqual({ kind: 'connect' });
        connected = true;
        return json({
          id: 'cmd',
          kind: 'connect',
          target_power_w: null,
          status: 'succeeded',
          created_at: SETTINGS.updated_at,
          completed_at: SETTINGS.updated_at,
          message: null,
        });
      }
      return new Promise(() => {});
    });
    renderApp();
    fireEvent.click(await screen.findByRole('button', { name: 'Connect trainer' }));
    expect(await screen.findByRole('button', { name: 'Disconnect' })).toBeTruthy();
    expect(screen.getByText(/Connected · streaming/)).toBeTruthy();
  });

  it('shows the yearly activity grid on the profile screen', async () => {
    window.location.hash = '#/profile';
    fetcher.mockImplementation((url: string) => {
      if (url === '/api/v1/desktop/capabilities') return json({ remember_available: true });
      if (url === '/api/v1/auth/me') return json(ME);
      if (url === '/api/v1/settings') return json(SETTINGS);
      if (url.startsWith('/api/v1/activity')) {
        return json({
          entries: [{ started_at: '2026-10-30T08:00:00.000Z', duration_seconds: 3600 }],
        });
      }
      return new Promise(() => {});
    });
    renderApp();
    expect(await screen.findByLabelText('Yearly training calendar')).toBeTruthy();
    expect(screen.getByText('Sun')).toBeTruthy();
    expect(screen.getByText('Sat')).toBeTruthy();
    expect(screen.getByText(/1 h/)).toBeTruthy();
  });

  it('switches the whole UI language and theme from the left rail', async () => {
    renderApp();
    await screen.findByTestId('ftp-current');
    expect(screen.getByRole('heading', { name: 'Your references.' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'TR' }));
    expect(await screen.findByRole('button', { name: 'Çıkış' })).toBeTruthy();
    expect(localStorage.getItem('wattlab.locale')).toBe('tr');
    fireEvent.click(screen.getByRole('button', { name: 'EN' }));
    expect(await screen.findByRole('button', { name: 'Log out' })).toBeTruthy();
    expect(localStorage.getItem('wattlab.locale')).toBe('en');
    fireEvent.click(screen.getByRole('button', { name: 'Light theme' }));
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(localStorage.getItem('wattlab.theme')).toBe('light');
    fireEvent.click(screen.getByRole('button', { name: 'Dark theme' }));
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(localStorage.getItem('wattlab.theme')).toBe('dark');
  });
});

describe('routing and command text', () => {
  it('parses hash routes', () => {
    expect(parseRoute('')).toEqual({ page: 'live' });
    expect(parseRoute('#/history')).toEqual({ page: 'history' });
    expect(parseRoute('#/history/abc')).toEqual({ page: 'session', id: 'abc' });
    expect(parseRoute('#/settings')).toEqual({ page: 'settings' });
    expect(parseRoute('#/profile')).toEqual({ page: 'profile' });
  });

  it('reports the agent answer instead of assuming success', () => {
    expect(describeCommand({ kind: 'set_target_power', status: 'pending', message: null })).toMatch(/waiting/);
    expect(describeCommand({ kind: 'set_target_power', status: 'rejected', message: 'erg_unavailable' })).toMatch(
      /does not support/,
    );
    expect(describeCommand({ kind: 'start_session', status: 'expired', message: null })).toMatch(/timed out/);
  });
});

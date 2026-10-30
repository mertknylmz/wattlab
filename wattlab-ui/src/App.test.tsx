import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
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
  trainer_address: null,
  devices: [],
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
    window.location.hash = '#/settings';
    cleanup();
  });

  it('opens connection settings from the header status chip', async () => {
    window.location.hash = '#/live';
    renderApp();
    const chip = await screen.findByTestId('connection-status');
    expect(chip.getAttribute('href')).toBe('#/settings/connection');
    window.location.hash = '#/settings/connection';
    expect(await screen.findByRole('heading', { name: 'Bluetooth devices' })).toBeTruthy();
    expect(screen.getByRole('link', { name: 'Connection' })).toHaveProperty('className', expect.stringContaining('active'));
  });

  it('opens signed-in screens without exposing a token field', async () => {
    renderApp();
    expect(await screen.findByText(/250 W ·/)).toBeTruthy();
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

  it('reconnects the last trainer when the app opens', async () => {
    window.location.hash = '#/settings/connection';
    localStorage.setItem('wattlab.trainerAddress', 'AA:BB:CC:DD:EE:FF');
    localStorage.setItem('wattlab.trainerName', 'D500');
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
        expect(JSON.parse(String(options.body))).toEqual({
          kind: 'connect',
          address: 'AA:BB:CC:DD:EE:FF',
        });
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
    await waitFor(() => {
      expect((screen.getByRole('button', { name: 'Disconnect' }) as HTMLButtonElement).disabled).toBe(false);
    }, { timeout: 3000 });
    expect(screen.queryByRole('button', { name: 'Scan' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Connect trainer' })).toBeNull();
    expect((screen.getByRole('button', { name: 'Disconnect' }) as HTMLButtonElement).disabled).toBe(false);
    expect(screen.getByText(/Connected · streaming/)).toBeTruthy();
    expect(screen.getByText(/Connected to D500/)).toBeTruthy();
  });

  it('locks scan, connect and disconnect while the trainer is connecting', async () => {
    window.location.hash = '#/settings/connection';
    localStorage.setItem('wattlab.trainerAddress', 'AA:BB:CC:DD:EE:FF');
    fetcher.mockImplementation((url: string) => {
      if (url === '/api/v1/desktop/capabilities') return json({ remember_available: true });
      if (url === '/api/v1/auth/me') return json(ME);
      if (url === '/api/v1/settings') return json(SETTINGS);
      if (url === '/api/v1/agent/status') return json({ ...AGENT_IDLE, connection_state: 'connecting' });
      return new Promise(() => {});
    });
    renderApp();
    expect(await screen.findByText(/Connecting to/)).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Scan' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Connect trainer' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Disconnect' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('shows a remembered trainer as last used, not as connected', async () => {
    window.location.hash = '#/settings/connection';
    localStorage.setItem('wattlab.trainerAddress', 'F8:07:54:96:B7:43');
    localStorage.setItem('wattlab.trainerName', 'D500');
    fetcher.mockImplementation((url: string) => {
      if (url === '/api/v1/desktop/capabilities') return json({ remember_available: true });
      if (url === '/api/v1/auth/me') return json(ME);
      if (url === '/api/v1/settings') return json(SETTINGS);
      if (url === '/api/v1/agent/status') return json(AGENT_IDLE);
      return new Promise(() => {});
    });
    renderApp();
    expect(await screen.findByText(/Last trainer: D500/)).toBeTruthy();
    expect(screen.getAllByText(/Not connected/).length).toBeGreaterThan(0);
    expect(screen.queryByText(/Selected F8:07:54:96:B7:43/)).toBeNull();
    expect((screen.getByRole('button', { name: 'Scan' }) as HTMLButtonElement).disabled).toBe(false);
    expect((screen.getByRole('button', { name: 'Connect trainer' }) as HTMLButtonElement).disabled).toBe(false);
    expect(screen.queryByRole('button', { name: 'Disconnect' })).toBeNull();
  });

  it('does not connect on launch when no trainer was remembered', async () => {
    window.location.hash = '#/settings/connection';
    const posts: string[] = [];
    fetcher.mockImplementation((url: string, options?: RequestInit) => {
      if (url === '/api/v1/desktop/capabilities') return json({ remember_available: true });
      if (url === '/api/v1/auth/me') return json(ME);
      if (url === '/api/v1/settings') return json(SETTINGS);
      if (url === '/api/v1/agent/status') return json(AGENT_IDLE);
      if (url === '/api/v1/agent/commands' && options?.method === 'POST') {
        posts.push(String(options.body));
        return json({
          id: 'cmd',
          kind: 'scan',
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
    expect(await screen.findByText(/Pick a trainer/)).toBeTruthy();
    expect(posts).toEqual([]);
  });

  it('lists scanned devices so the user picks one before connecting', async () => {
    window.location.hash = '#/settings/connection';
    let scanned = false;
    fetcher.mockImplementation((url: string, options?: RequestInit) => {
      if (url === '/api/v1/desktop/capabilities') return json({ remember_available: true });
      if (url === '/api/v1/auth/me') return json(ME);
      if (url === '/api/v1/settings') return json(SETTINGS);
      if (url === '/api/v1/agent/status') {
        return json(
          scanned
            ? {
                ...AGENT_IDLE,
                devices: [{ address: 'AA:BB:CC:DD:EE:FF', name: 'D500', rssi: -42 }],
              }
            : AGENT_IDLE,
        );
      }
      if (url === '/api/v1/agent/commands' && options?.method === 'POST') {
        expect(JSON.parse(String(options.body))).toEqual({ kind: 'scan' });
        scanned = true;
        return json({
          id: 'scan',
          kind: 'scan',
          target_power_w: null,
          address: null,
          status: 'succeeded',
          created_at: SETTINGS.updated_at,
          completed_at: SETTINGS.updated_at,
          message: 'found:1',
        });
      }
      return new Promise(() => {});
    });
    renderApp();
    const scan = await screen.findByRole('button', { name: 'Scan' });
    expect((scan as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(scan);
    expect(await screen.findByText('D500')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /D500/ }));
    expect(localStorage.getItem('wattlab.trainerAddress')).toBe('AA:BB:CC:DD:EE:FF');
    expect(localStorage.getItem('wattlab.trainerName')).toBe('D500');
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
    expect(screen.getByLabelText('Activity year').textContent).toMatch(/2026/);
    expect(screen.getByText('Sun')).toBeTruthy();
    expect(screen.getByText('Sat')).toBeTruthy();
    expect(screen.getByText(/1 h/)).toBeTruthy();
    expect(screen.getByTestId('current-streak').textContent).toMatch(/1/);
    expect(screen.getByTestId('longest-streak').textContent).toMatch(/1/);
    expect(screen.getByRole('button', { name: /at least 30 minutes/ })).toBeTruthy();
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
    expect(parseRoute('#/settings')).toEqual({ page: 'settings', tab: 'general' });
    expect(parseRoute('#/settings/connection')).toEqual({ page: 'settings', tab: 'connection' });
    expect(parseRoute('#/profile')).toEqual({ page: 'profile' });
  });

  it('reports the agent answer instead of assuming success', () => {
    expect(describeCommand({ kind: 'set_target_power', status: 'pending', message: null })).toMatch(/waiting/);
    expect(describeCommand({ kind: 'set_target_power', status: 'rejected', message: 'erg_unavailable' })).toMatch(
      /does not support/,
    );
    expect(describeCommand({ kind: 'start_session', status: 'expired', message: null })).toMatch(/timed out/);
    expect(describeCommand({ kind: 'start_session', status: 'rejected', message: 'trainer_not_connected' })).toMatch(
      /connect the trainer first/,
    );
  });
});

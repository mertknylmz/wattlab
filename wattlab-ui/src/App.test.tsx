import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiClient } from './api/client';
import { App, parseRoute } from './App';
import { describeCommand } from './pages/TrainerPanel';

function json(body: unknown, status = 200) {
  return Promise.resolve(new Response(JSON.stringify(body), { status }));
}

const SETTINGS = { ftp_w: 250, ftp_is_default: false, updated_at: '2026-10-30T10:00:00Z' };

describe('App', () => {
  let fetcher: ReturnType<typeof vi.fn>;
  const renderApp = () => render(<App client={new ApiClient(fetcher as unknown as typeof fetch)} />);

  beforeEach(() => {
    window.location.hash = '#/settings';
    fetcher = vi.fn((url: string) => (url === '/api/v1/settings' ? json(SETTINGS) : new Promise(() => {})));
  });

  afterEach(cleanup);

  it('opens directly without asking for an API key and loads the persistent FTP', async () => {
    renderApp();
    expect(await screen.findByTestId('ftp-current')).toHaveProperty('textContent', expect.stringContaining('250 W'));
    expect(screen.queryByRole('textbox')).toBeNull();
  });

  it('explains a refused API instead of retrying forever', async () => {
    fetcher.mockImplementation(() => json({}, 401));
    renderApp();
    expect(await screen.findByRole('alert')).toHaveProperty('textContent', expect.stringContaining('yeniden aç'));
    expect(screen.getByTestId('api-status').textContent).toBe('Erişim reddedildi');
  });

  it('validates FTP before saving and can reset to the default', async () => {
    renderApp();
    await screen.findByText(/250 W ·/);
    const input = screen.getByRole('spinbutton');
    for (const invalid of ['0', '2010', '12.5']) {
      fireEvent.change(input, { target: { value: invalid } });
      fireEvent.click(screen.getByRole('button', { name: 'Kaydet' }));
      expect(screen.getByRole('status').textContent).toMatch(/1–2000/);
    }
    fetcher.mockImplementation((url: string, options?: RequestInit) =>
      url === '/api/v1/settings' && options?.method === 'PUT'
        ? json({ ftp_w: 200, ftp_is_default: true, updated_at: SETTINGS.updated_at })
        : json(SETTINGS),
    );
    fireEvent.click(screen.getByRole('button', { name: 'Varsayılana dön' }));
    await screen.findByText(/Varsayılan 200 W/);
    const put = fetcher.mock.calls.find(([, options]) => options?.method === 'PUT');
    expect(JSON.parse(put![1].body)).toEqual({ ftp_w: null });
  });
});

describe('routing and command text', () => {
  it('parses hash routes', () => {
    expect(parseRoute('')).toEqual({ page: 'live' });
    expect(parseRoute('#/history')).toEqual({ page: 'history' });
    expect(parseRoute('#/history/abc')).toEqual({ page: 'session', id: 'abc' });
    expect(parseRoute('#/settings')).toEqual({ page: 'settings' });
  });

  it('reports the agent answer instead of assuming success', () => {
    expect(describeCommand({ kind: 'set_target_power', status: 'pending', message: null })).toMatch(/bekleniyor/);
    expect(describeCommand({ kind: 'set_target_power', status: 'rejected', message: 'erg_unavailable' })).toMatch(
      /desteklemiyor/,
    );
    expect(describeCommand({ kind: 'start_session', status: 'expired', message: null })).toMatch(/zaman aşımı/);
  });
});

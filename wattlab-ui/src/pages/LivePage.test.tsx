import { act, cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { ApiClient } from '../api/client';
import { AppearanceProvider } from '../i18n/appearance';
import { LivePage } from './LivePage';

afterEach(() => {
  localStorage.clear();
  document.documentElement.removeAttribute('data-theme');
  cleanup();
});

function setup() {
  let push: (text: string) => void = () => {};
  const body = new ReadableStream<Uint8Array>({
    start(controller) {
      push = (text) => controller.enqueue(new TextEncoder().encode(text));
    },
  });
  const fetcher = vi.fn().mockResolvedValue(new Response(body, { status: 200 }));
  let clock = 0;
  const onOnline = vi.fn();
  render(
    <AppearanceProvider>
      <LivePage
        client={new ApiClient(fetcher)}
        ftp={200}
        ftpIsDefault
        now={() => clock}
        onUnauthorized={() => {}}
        onOnline={onOnline}
      />
    </AppearanceProvider>,
  );
  const send = async (power: number | null, finished = false) => {
    const at = new Date().toISOString();
    const live = {
      session: { id: 's', request_id: 'r', started_at: at, finished_at: finished ? at : null },
      connection_state: 'streaming',
      fresh: true,
      speed_kmh: 20,
      cadence_rpm: 80,
      power_w: power,
      power_source: 'ftms',
      ftms_received_at: at,
      cycling_power_received_at: null,
      captured_at: at,
    };
    const agent = { online: false, agent_id: null, connection_state: null, session_id: null, erg_supported: false, power_range_w: null, target_power_w: null, trainer_address: null, devices: [], last_seen_at: null };
    await act(async () => {
      push(`event: snapshot\ndata: ${JSON.stringify({ live, agent })}\n\n`);
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  };
  return { send, push, onOnline, tick: (ms: number) => (clock = ms) };
}

it('shows live values with zone and keeps a real zero before finish', async () => {
  const live = setup();
  await live.send(180);
  expect(screen.getByTestId('power').textContent).toBe('180');
  expect(screen.getByTestId('zone-label').textContent).toMatch(/Z4/);
  expect(live.onOnline).toHaveBeenCalledWith(true);

  live.tick(1000);
  await live.send(0);
  expect(screen.getByTestId('power').textContent).toBe('0');
  expect(screen.getByTestId('chart-status').textContent).toMatch(/0 W/);

  live.tick(1100);
  await live.send(0, true);
  expect(screen.getByTestId('power').textContent).toBe('—');
  expect(screen.getByTestId('trainer-status').textContent).toBe('Session finished');
  expect(screen.getByTestId('power-chart').querySelectorAll('polyline')).toHaveLength(1);
});

it('says the agent is not running and disables commands', async () => {
  const live = setup();
  await live.send(100);
  expect(screen.getByText(/Trainer agent is not running/)).toBeTruthy();
  expect(screen.queryByRole('button', { name: 'Connect trainer' })).toBeNull();
  expect(screen.queryByRole('button', { name: 'Disconnect' })).toBeNull();
  expect((screen.getByRole('button', { name: 'Finish session' }) as HTMLButtonElement).disabled).toBe(true);
  expect(screen.queryByText('Target power (ERG)')).toBeNull();
});

it('keeps session time empty until a ride is running', async () => {
  const live = setup();
  expect(screen.getByTestId('duration').textContent).toBe('—:—');
  const started = new Date(Date.now() - 53 * 60 * 1000).toISOString();
  const finished = new Date().toISOString();
  await act(async () => {
    live.push(
      `event: snapshot\ndata: ${JSON.stringify({
        live: {
          session: { id: 's', request_id: 'r', started_at: started, finished_at: finished },
          connection_state: 'streaming',
          fresh: false,
          speed_kmh: null,
          cadence_rpm: null,
          power_w: null,
          power_source: null,
          ftms_received_at: null,
          cycling_power_received_at: null,
          captured_at: finished,
        },
        agent: {
          online: true,
          agent_id: 'agent',
          connection_state: 'streaming',
          session_id: null,
          erg_supported: false,
          power_range_w: null,
          target_power_w: null,
          trainer_address: null,
          devices: [],
          last_seen_at: finished,
        },
      })}\n\n`,
    );
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  expect(screen.getByTestId('trainer-status').textContent).toBe('Session finished');
  expect(screen.getByTestId('duration').textContent).toBe('—:—');
});

it('keeps start session disabled until the trainer is streaming', async () => {
  const live = setup();
  const at = new Date().toISOString();
  const empty = {
    session: null,
    connection_state: 'idle',
    fresh: false,
    speed_kmh: null,
    cadence_rpm: null,
    power_w: null,
    power_source: null,
    ftms_received_at: null,
    cycling_power_received_at: null,
    captured_at: null,
  };
  const idleAgent = {
    online: true,
    agent_id: 'agent',
    connection_state: 'idle',
    session_id: null,
    erg_supported: false,
    power_range_w: null,
    target_power_w: null,
    trainer_address: null,
    devices: [],
    last_seen_at: at,
  };
  await act(async () => {
    live.push(`event: snapshot\ndata: ${JSON.stringify({ live: empty, agent: idleAgent })}\n\n`);
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  expect((screen.getByRole('button', { name: 'Start session' }) as HTMLButtonElement).disabled).toBe(true);

  await act(async () => {
    live.push(
      `event: snapshot\ndata: ${JSON.stringify({
        live: { ...empty, connection_state: 'streaming' },
        agent: { ...idleAgent, connection_state: 'streaming' },
      })}\n\n`,
    );
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  expect((screen.getByRole('button', { name: 'Start session' }) as HTMLButtonElement).disabled).toBe(false);
});

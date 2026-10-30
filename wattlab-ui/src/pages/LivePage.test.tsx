import { act, cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { ApiClient } from '../api/client';
import { LivePage } from './LivePage';

afterEach(cleanup);

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
    <LivePage
      client={new ApiClient(fetcher)}
      ftp={200}
      ftpIsDefault
      now={() => clock}
      onUnauthorized={() => {}}
      onOnline={onOnline}
    />,
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
    const agent = { online: false, agent_id: null, connection_state: null, session_id: null, erg_supported: false, power_range_w: null, target_power_w: null, last_seen_at: null };
    await act(async () => {
      push(`event: snapshot\ndata: ${JSON.stringify({ live, agent })}\n\n`);
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  };
  return { send, onOnline, tick: (ms: number) => (clock = ms) };
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
  expect(screen.getByTestId('trainer-status').textContent).toBe('Oturum tamamlandı');
  expect(screen.getByTestId('power-chart').querySelectorAll('polyline')).toHaveLength(1);
});

it('says the agent is not running and disables commands', async () => {
  const live = setup();
  await live.send(100);
  expect(screen.getByText(/Trainer ajanı çalışmıyor/)).toBeTruthy();
  expect((screen.getByRole('button', { name: 'Trainer’a bağlan' }) as HTMLButtonElement).disabled).toBe(true);
});

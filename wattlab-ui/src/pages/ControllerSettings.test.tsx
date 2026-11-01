import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AgentState } from '../api/types';
import { AppearanceProvider } from '../i18n/appearance';
import { SettingsPage } from './SettingsPage';

const SETTINGS = { ftp_w: 200, ftp_is_default: true, updated_at: '2026-11-01T00:00:00Z' };

function agent(overrides: Partial<AgentState> = {}): AgentState {
  return {
    online: true,
    agent_id: 'agent',
    connection_state: 'idle',
    session_id: null,
    erg_supported: false,
    power_range_w: null,
    target_power_w: null,
    trainer_address: null,
    devices: [],
    controller_devices: [],
    controllers: [],
    last_seen_at: '2026-11-01T00:00:00Z',
    ...overrides,
  };
}

function renderSettings(status: AgentState) {
  const posts: unknown[] = [];
  const client = {
    agent: vi.fn(async () => status),
    submit: vi.fn(async (kind: string) => {
      posts.push(kind);
      return {
        id: 'cmd',
        kind,
        target_power_w: null,
        address: null,
        status: 'succeeded',
        created_at: SETTINGS.updated_at,
        completed_at: SETTINGS.updated_at,
        message: null,
      };
    }),
    command: vi.fn(),
    saveFtp: vi.fn(),
  };
  render(
    <AppearanceProvider>
      <SettingsPage
        client={client as never}
        tab="connection"
        settings={SETTINGS}
        onSaved={() => undefined}
        onUnauthorized={() => undefined}
      />
    </AppearanceProvider>,
  );
  return { posts, client };
}

describe('Zwift controller settings', () => {
  afterEach(() => {
    cleanup();
    localStorage.removeItem('wattlab.locale');
  });

  it('scans and connects while the trainer is already streaming', async () => {
    const { posts } = renderSettings(
      agent({
        connection_state: 'streaming',
        trainer_address: 'AA:BB:CC:DD:EE:FF',
        controller_devices: [
          { address: 'AA:BB:CC:DD:EE:02', name: 'Zwift Play', rssi: -60, kind: 'play', side: 'left', selected: true },
          { address: 'AA:BB:CC:DD:EE:03', name: 'Zwift Play', rssi: -70, kind: 'play', side: 'right', selected: true },
          { address: 'AA:BB:CC:DD:EE:01', name: 'Zwift Click', rssi: -20, kind: 'click', side: null, selected: false },
        ],
      }),
    );
    expect((await screen.findByTestId('controller-status')).textContent).toMatch(/3 found/);
    expect(screen.queryByRole('button', { name: 'Scan' })).toBeNull();
    const scan = screen.getByRole('button', { name: 'Scan controllers' });
    expect((scan as HTMLButtonElement).disabled).toBe(false);
    expect((screen.getByRole('button', { name: 'Disconnect controllers' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByLabelText('Discovered controllers').textContent).toMatch(/Zwift Play · Play · left/);
    expect(screen.getByLabelText('Discovered controllers').textContent).toMatch(/Zwift Play · Play · right/);
    expect(screen.getByLabelText('Discovered controllers').textContent).toMatch(/will connect/);
    fireEvent.click(scan);
    await waitFor(() => expect(posts).toEqual(['scan_controllers']));
    fireEvent.click(screen.getByRole('button', { name: 'Connect controllers' }));
    await waitFor(() => expect(posts).toEqual(['scan_controllers', 'connect_controller']));
  });

  it('shows a controller that is still connecting', async () => {
    renderSettings(
      agent({
        controllers: [
          {
            address: 'AA:BB:CC:DD:EE:02',
            name: 'Zwift Play',
            kind: 'play',
            side: 'left',
            state: 'connecting',
            message: null,
          },
        ],
      }),
    );
    expect((await screen.findByTestId('controller-status')).textContent).toBe('Connecting: Zwift Play · Play · left');
    expect((screen.getByRole('button', { name: 'Connect controllers' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('shows connecting, connected, and failed controller states', async () => {
    const { posts } = renderSettings(
      agent({
        controllers: [
          {
            address: 'AA:BB:CC:DD:EE:02',
            name: 'Zwift Play',
            kind: 'play',
            side: 'left',
            state: 'connected',
            message: null,
          },
          {
            address: 'AA:BB:CC:DD:EE:03',
            name: 'Zwift Play',
            kind: 'play',
            side: 'right',
            state: 'failed',
            message: 'controller_connect_timeout',
          },
        ],
      }),
    );
    expect((await screen.findByTestId('controller-status')).textContent).toBe(
      'Connected: Zwift Play · Play · left. Failed: Zwift Play · Play · right (the controller did not answer in time).',
    );
    fireEvent.click(screen.getByRole('button', { name: 'Disconnect controllers' }));
    await waitFor(() => expect(posts).toEqual(['disconnect_controller']));
  });

  it('keeps controller controls off when the agent is offline and translates them', async () => {
    localStorage.setItem('wattlab.locale', 'tr');
    renderSettings(agent({ online: false, connection_state: null }));
    const scan = await screen.findByRole('button', { name: 'Kumandaları tara' });
    expect((scan as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Kumandaları bağla' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId('controller-status').textContent).toBe('Kumanda bağlı değil.');
  });
});

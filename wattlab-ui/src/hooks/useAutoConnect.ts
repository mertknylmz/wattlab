import { useEffect } from 'react';
import { UnauthorizedError, type ApiClient } from '../api/client';
import { readLastTrainer } from '../model/trainerPrefs';
import { useCommand } from './useCommand';

const IN_PROGRESS = new Set(['connecting', 'subscribing', 'streaming', 'retry_wait']);

/**
 * One reconnect per signed-in mount to the last picked trainer. The agent
 * still waits for a screen command; this is that command. A later Disconnect
 * does not reconnect until the app is opened again.
 */
export function useAutoConnect(client: ApiClient, enabled: boolean, onUnauthorized: () => void) {
  const { submit } = useCommand(client, onUnauthorized);

  useEffect(() => {
    if (!enabled) return;
    const last = readLastTrainer();
    if (!last) return;

    let active = true;
    let finished = false;
    const tick = async () => {
      if (!active || finished) return;
      try {
        const agent = await client.agent();
        if (!active || finished) return;
        if (!agent.online) return;
        const state = agent.connection_state ?? 'idle';
        if (IN_PROGRESS.has(state)) {
          finished = true;
          return;
        }
        finished = true;
        await submit('connect', { address: last.address });
      } catch (error) {
        if (error instanceof UnauthorizedError) onUnauthorized();
      }
    };

    void tick();
    const timer = setInterval(() => void tick(), 2000);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [client, enabled, onUnauthorized, submit]);
}

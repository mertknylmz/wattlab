import { useCallback, useEffect, useRef, useState } from 'react';
import { ApiClient, UnauthorizedError } from '../api/client';
import type { Command, CommandKind } from '../api/types';

const POLL_MS = 500;
const TERMINAL = new Set(['succeeded', 'rejected', 'expired']);

export interface CommandState {
  kind: CommandKind;
  status: Command['status'] | 'failed';
  message: string | null;
}

/**
 * Sends one command at a time and follows it to a terminal status. The screen
 * reports what the agent answered; it never assumes the trainer complied.
 */
export function useCommand(client: ApiClient, onUnauthorized: () => void) {
  const [state, setState] = useState<CommandState | null>(null);
  const generation = useRef(0);
  useEffect(() => () => void generation.current++, []);

  const submit = useCallback(
    async (kind: CommandKind, targetPowerW?: number) => {
      const run = ++generation.current;
      setState({ kind, status: 'pending', message: null });
      try {
        let command = await client.submit(kind, targetPowerW);
        while (!TERMINAL.has(command.status)) {
          await new Promise((resolve) => setTimeout(resolve, POLL_MS));
          if (run !== generation.current) return;
          command = await client.command(command.id);
        }
        if (run === generation.current) setState({ kind, status: command.status, message: command.message });
      } catch (error) {
        if (run !== generation.current) return;
        if (error instanceof UnauthorizedError) onUnauthorized();
        else setState({ kind, status: 'failed', message: null });
      }
    },
    [client, onUnauthorized],
  );

  const busy = state != null && !TERMINAL.has(state.status) && state.status !== 'failed';
  return { state, submit, busy };
}

import { useCallback, useEffect, useState } from 'react';
import { UnauthorizedError, type ApiClient } from '../api/client';
import type { AgentState } from '../api/types';

export function useAgentStatus(
  client: ApiClient,
  onUnauthorized: () => void,
  intervalMs = 2000,
  enabled = true,
) {
  const [agent, setAgent] = useState<AgentState | null>(null);

  const refresh = useCallback(async () => {
    if (!enabled) return;
    try {
      setAgent(await client.agent());
    } catch (error) {
      if (error instanceof UnauthorizedError) onUnauthorized();
      else setAgent(null);
    }
  }, [client, enabled, onUnauthorized]);

  useEffect(() => {
    if (!enabled) {
      setAgent(null);
      return;
    }
    void refresh();
    const timer = setInterval(() => void refresh(), intervalMs);
    return () => clearInterval(timer);
  }, [enabled, intervalMs, refresh]);

  return { agent, setAgent, refresh };
}

import type {
  Activity,
  AgentState,
  AuthSession,
  Command,
  CommandKind,
  CommandPayload,
  DesktopCapabilities,
  Member,
  SessionPage,
  SessionSeries,
  SessionSummary,
  Settings,
} from './types';

const REQUEST_TIMEOUT_MS = 3000;

/** The desktop shell's API access was refused; retrying cannot fix it. */
export class UnauthorizedError extends Error {}
/** The local API could not be reached or answered with an unexpected status. */
export class ApiError extends Error {
  constructor(
    readonly status: number | null,
    readonly code?: string,
    message = 'API unavailable',
  ) {
    super(message);
  }
}

/**
 * Same-origin client for the desktop renderer. The Electron main process
 * forwards /api requests to the local API and adds the credentials, so the
 * renderer never holds a token.
 */
export class ApiClient {
  constructor(private readonly fetcher: typeof fetch = fetch.bind(globalThis)) {}

  private headers(json = false): Record<string, string> {
    return json ? { 'Content-Type': 'application/json' } : {};
  }

  async request<T>(
    method: string,
    path: string,
    body?: unknown,
    signal?: AbortSignal,
    extraHeaders?: Record<string, string>,
  ): Promise<T> {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
    const abort = () => controller.abort();
    signal?.addEventListener('abort', abort);
    try {
      let response: Response;
      try {
        response = await this.fetcher(path, {
          method,
          headers: { ...this.headers(body !== undefined), ...extraHeaders },
          body: body === undefined ? undefined : JSON.stringify(body),
          cache: 'no-store',
          credentials: 'omit',
          signal: controller.signal,
        });
      } catch {
        throw new ApiError(null);
      }
      if (response.status === 401) throw new UnauthorizedError('Unauthorized');
      if (response.status === 204) return undefined as T;
      if (!response.ok) {
        const parsed = (await response.json().catch(() => ({}))) as { error?: string };
        throw new ApiError(response.status, parsed.error, `HTTP ${response.status}`);
      }
      return (await response.json()) as T;
    } finally {
      clearTimeout(timeout);
      signal?.removeEventListener('abort', abort);
    }
  }

  /** Opens the live SSE stream; fetch gives control over aborts and stalls. */
  async openStream(signal: AbortSignal): Promise<ReadableStreamDefaultReader<Uint8Array>> {
    let response: Response;
    try {
      response = await this.fetcher('/api/v1/live/stream', {
        headers: { ...this.headers(), Accept: 'text/event-stream' },
        cache: 'no-store',
        credentials: 'omit',
        signal,
      });
    } catch {
      throw new ApiError(null);
    }
    if (response.status === 401) throw new UnauthorizedError('Unauthorized');
    if (!response.ok || !response.body) throw new ApiError(response.status);
    return response.body.getReader();
  }

  capabilities() {
    return this.request<DesktopCapabilities>('GET', '/api/v1/desktop/capabilities');
  }
  me() {
    return this.request<Member>('GET', '/api/v1/auth/me');
  }
  register(username: string, password: string, remember: boolean) {
    return this.request<AuthSession>(
      'POST',
      '/api/v1/auth/register',
      { username, password },
      undefined,
      { 'X-WattLab-Remember': remember ? '1' : '0' },
    );
  }
  login(username: string, password: string, remember: boolean) {
    return this.request<AuthSession>(
      'POST',
      '/api/v1/auth/login',
      { username, password },
      undefined,
      { 'X-WattLab-Remember': remember ? '1' : '0' },
    );
  }
  logout() {
    return this.request<void>('POST', '/api/v1/auth/logout');
  }
  sessions(before?: string | null, limit = 20) {
    const query = new URLSearchParams({ limit: String(limit) });
    if (before) query.set('before', before);
    return this.request<SessionPage>('GET', `/api/v1/sessions?${query}`);
  }
  summary(id: string) {
    return this.request<SessionSummary>('GET', `/api/v1/sessions/${encodeURIComponent(id)}/summary`);
  }
  series(id: string) {
    return this.request<SessionSeries>('GET', `/api/v1/sessions/${encodeURIComponent(id)}/series`);
  }
  activity(from: string, to: string) {
    const query = new URLSearchParams({ from, to });
    return this.request<Activity>('GET', `/api/v1/activity?${query}`);
  }
  agent() {
    return this.request<AgentState>('GET', '/api/v1/agent/status');
  }
  settings() {
    return this.request<Settings>('GET', '/api/v1/settings');
  }
  saveFtp(ftp: number | null) {
    return this.request<Settings>('PUT', '/api/v1/settings', { ftp_w: ftp });
  }
  submit(kind: CommandKind, payload?: CommandPayload) {
    const body: Record<string, unknown> = { kind };
    if (kind === 'set_target_power') body.target_power_w = payload?.targetPowerW;
    if (kind === 'connect') body.address = payload?.address;
    return this.request<Command>('POST', '/api/v1/agent/commands', body);
  }
  command(id: string) {
    return this.request<Command>('GET', `/api/v1/agent/commands/${encodeURIComponent(id)}`);
  }
}

export interface SseEvent {
  event: string;
  data: string;
}

/** Incremental text/event-stream parser; returns complete events and the remainder. */
export function parseSse(buffer: string): { events: SseEvent[]; rest: string } {
  const normalized = buffer.replace(/\r\n?/g, '\n');
  const blocks = normalized.split('\n\n');
  const rest = blocks.pop() ?? '';
  const events: SseEvent[] = [];
  for (const block of blocks) {
    let event = 'message';
    const data: string[] = [];
    for (const line of block.split('\n')) {
      if (line.startsWith(':') || line === '') continue;
      const colon = line.indexOf(':');
      const field = colon === -1 ? line : line.slice(0, colon);
      const value = colon === -1 ? '' : line.slice(colon + 1).replace(/^ /, '');
      if (field === 'event') event = value;
      else if (field === 'data') data.push(value);
    }
    if (data.length) events.push({ event, data: data.join('\n') });
  }
  return { events, rest };
}

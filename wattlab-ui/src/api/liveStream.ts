import { ApiClient, UnauthorizedError, parseSse } from './client';
import type { StreamSnapshot } from './types';

/** No snapshot for this long means the stream is stalled; reconnect. */
export const STALL_MS = 5000;
export const RECONNECT_MS = 1000;

export interface LiveHandlers {
  onSnapshot(snapshot: StreamSnapshot): void;
  /** The API or its storage is unreachable; shown values must be cleared. */
  onOffline(): void;
  onUnauthorized(): void;
}

/**
 * Consumes the server-sent live view and reconnects with a fixed delay. A
 * stopped stream ignores late data so an old connection cannot overwrite a
 * newer view.
 */
export class LiveStream {
  private generation = 0;
  private controller: AbortController | null = null;
  private retry: ReturnType<typeof setTimeout> | null = null;

  constructor(private readonly client: ApiClient, private readonly handlers: LiveHandlers) {}

  start() {
    this.stop();
    const run = ++this.generation;
    void this.connect(run);
  }

  stop() {
    this.generation++;
    this.controller?.abort();
    this.controller = null;
    if (this.retry) clearTimeout(this.retry);
    this.retry = null;
  }

  private async connect(run: number) {
    const controller = new AbortController();
    this.controller = controller;
    let stall: ReturnType<typeof setTimeout> | null = null;
    const armStall = () => {
      if (stall) clearTimeout(stall);
      stall = setTimeout(() => controller.abort(), STALL_MS);
    };
    try {
      armStall();
      const reader = await this.client.openStream(controller.signal);
      const decoder = new TextDecoder();
      let buffer = '';
      for (;;) {
        const { value, done } = await reader.read();
        if (run !== this.generation) return;
        if (done) break;
        buffer += decoder.decode(value, { stream: true });
        const parsed = parseSse(buffer);
        buffer = parsed.rest;
        for (const event of parsed.events) {
          if (event.event === 'snapshot') {
            armStall();
            this.handlers.onSnapshot(JSON.parse(event.data) as StreamSnapshot);
          } else if (event.event === 'unavailable') {
            this.handlers.onOffline();
          }
        }
      }
    } catch (error) {
      if (run !== this.generation) return;
      if (error instanceof UnauthorizedError) {
        this.generation++;
        this.handlers.onUnauthorized();
        return;
      }
    } finally {
      if (stall) clearTimeout(stall);
    }
    if (run !== this.generation) return;
    this.handlers.onOffline();
    this.retry = setTimeout(() => {
      if (run === this.generation) void this.connect(run);
    }, RECONNECT_MS);
  }
}

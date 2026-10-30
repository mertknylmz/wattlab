import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiClient, ApiError, UnauthorizedError, parseSse } from './client';
import { LiveStream, RECONNECT_MS, STALL_MS } from './liveStream';

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });
}

describe('ApiClient', () => {
  it('sends relative API requests without credentials of its own', async () => {
    const fetcher = vi.fn().mockResolvedValue(json({ sessions: [], next_before: null }));
    await new ApiClient(fetcher).sessions('abc', 5);
    const [url, options] = fetcher.mock.calls[0];
    expect(url).toBe('/api/v1/sessions?limit=5&before=abc');
    expect(options.headers.Authorization).toBeUndefined();
    expect(options.credentials).toBe('omit');
    expect(options.cache).toBe('no-store');
  });

  it('distinguishes refused tokens from unreachable APIs', async () => {
    await expect(new ApiClient(vi.fn().mockResolvedValue(json({}, 401))).settings()).rejects.toBeInstanceOf(
      UnauthorizedError,
    );
    await expect(new ApiClient(vi.fn().mockRejectedValue(new TypeError('net'))).settings()).rejects.toBeInstanceOf(
      ApiError,
    );
    const failure = await new ApiClient(vi.fn().mockResolvedValue(json({}, 404))).summary('x').catch((e) => e);
    expect(failure).toMatchObject({ status: 404 });
  });

  it('asks for activity in a calendar window', async () => {
    const fetcher = vi.fn().mockResolvedValue(json({ entries: [] }));
    await new ApiClient(fetcher).activity('2026-01-01T00:00:00.000Z', '2026-12-31T00:00:00.000Z');
    expect(fetcher.mock.calls[0][0]).toBe(
      '/api/v1/activity?from=2026-01-01T00%3A00%3A00.000Z&to=2026-12-31T00%3A00%3A00.000Z',
    );
  });

  it('sends target power only with the ERG command', async () => {
    const fetcher = vi.fn().mockImplementation(() => Promise.resolve(json({})));
    const client = new ApiClient(fetcher);
    await client.submit('set_target_power', { targetPowerW: 180 });
    await client.submit('connect', { address: 'AA:BB:CC:DD:EE:FF' });
    await client.submit('start_session');
    expect(JSON.parse(fetcher.mock.calls[0][1].body)).toEqual({ kind: 'set_target_power', target_power_w: 180 });
    expect(JSON.parse(fetcher.mock.calls[1][1].body)).toEqual({
      kind: 'connect',
      address: 'AA:BB:CC:DD:EE:FF',
    });
    expect(JSON.parse(fetcher.mock.calls[2][1].body)).toEqual({ kind: 'start_session' });
  });
});

describe('parseSse', () => {
  it('returns complete events and keeps a partial remainder', () => {
    const { events, rest } = parseSse(': keep-alive\n\nevent: snapshot\ndata: {"a":1}\n\nevent: snap');
    expect(events).toEqual([{ event: 'snapshot', data: '{"a":1}' }]);
    expect(rest).toBe('event: snap');
  });

  it('accepts CRLF line endings and multi-line data', () => {
    expect(parseSse('data: one\r\ndata: two\r\n\r\n').events).toEqual([{ event: 'message', data: 'one\ntwo' }]);
  });
});

describe('LiveStream', () => {
  afterEach(() => vi.useRealTimers());

  function streamResponse() {
    let push: (text: string) => void = () => {};
    let close: () => void = () => {};
    const body = new ReadableStream<Uint8Array>({
      start(controller) {
        push = (text) => controller.enqueue(new TextEncoder().encode(text));
        close = () => controller.close();
      },
    });
    return { response: new Response(body, { status: 200 }), push: (t: string) => push(t), close: () => close() };
  }

  const handlers = () => ({ onSnapshot: vi.fn(), onOffline: vi.fn(), onUnauthorized: vi.fn() });
  const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

  it('delivers snapshots and reconnects after the stream ends', async () => {
    const first = streamResponse();
    const fetcher = vi.fn().mockResolvedValueOnce(first.response).mockReturnValue(new Promise(() => {}));
    const h = handlers();
    const stream = new LiveStream(new ApiClient(fetcher), h);
    stream.start();
    await flush();
    first.push('event: snapshot\ndata: {"live":{},"agent":{}}\n\n');
    await flush();
    expect(h.onSnapshot).toHaveBeenCalledTimes(1);
    first.close();
    await flush();
    await flush();
    expect(h.onOffline).toHaveBeenCalledTimes(1);
    await new Promise((resolve) => setTimeout(resolve, RECONNECT_MS + 50));
    expect(fetcher).toHaveBeenCalledTimes(2);
    stream.stop();
  });

  it('stops on 401 instead of retrying forever', async () => {
    const fetcher = vi.fn().mockResolvedValue(json({}, 401));
    const h = handlers();
    new LiveStream(new ApiClient(fetcher), h).start();
    await flush();
    await flush();
    expect(h.onUnauthorized).toHaveBeenCalledTimes(1);
    await new Promise((resolve) => setTimeout(resolve, RECONNECT_MS + 50));
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it('treats a silent stream as stalled and reports the outage', async () => {
    vi.useFakeTimers();
    const silent = streamResponse();
    const fetcher = vi.fn((_url: string, options: RequestInit) => {
      options.signal?.addEventListener('abort', () => silent.close());
      return Promise.resolve(silent.response);
    });
    const h = handlers();
    const stream = new LiveStream(new ApiClient(fetcher as unknown as typeof fetch), h);
    stream.start();
    await vi.advanceTimersByTimeAsync(STALL_MS + 10);
    expect(h.onOffline).toHaveBeenCalled();
    stream.stop();
  });

  it('ignores data from a stopped stream', async () => {
    const first = streamResponse();
    const fetcher = vi.fn().mockResolvedValue(first.response);
    const h = handlers();
    const stream = new LiveStream(new ApiClient(fetcher), h);
    stream.start();
    await flush();
    stream.stop();
    first.push('event: snapshot\ndata: {}\n\n');
    await flush();
    expect(h.onSnapshot).not.toHaveBeenCalled();
    expect(h.onOffline).not.toHaveBeenCalled();
  });
});

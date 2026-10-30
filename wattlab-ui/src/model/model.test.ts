import { describe, expect, it } from 'vitest';
import type { LiveSnapshot, SeriesPoint } from '../api/types';
import { PowerTrace, buildChart, currentMetric, liveChart, type Reception } from './live';
import { sessionChart } from './history';
import { formatDuration } from './format';
import { isValidFtp, powerZone } from './zones';

const NOW = '2026-10-30T10:00:00.000Z';

function snapshot(overrides: Partial<LiveSnapshot> = {}): LiveSnapshot {
  return {
    session: { id: 'one', request_id: 'r', started_at: NOW, finished_at: null },
    connection_state: 'streaming',
    fresh: true,
    speed_kmh: 0,
    cadence_rpm: null,
    power_w: -12,
    power_source: 'cycling_power',
    ftms_received_at: NOW,
    cycling_power_received_at: NOW,
    captured_at: NOW,
    ...overrides,
  };
}

const received = (live: LiveSnapshot, online = true): Reception => ({ snapshot: live, sampledAt: 0, online });

describe('currentMetric', () => {
  it('keeps real zeros and signed power, and missing values stay null', () => {
    const reception = received(snapshot());
    expect(currentMetric(reception, 'power_w', 0)).toBe(-12);
    expect(currentMetric(reception, 'speed_kmh', 0)).toBe(0);
    expect(currentMetric(reception, 'cadence_rpm', 0)).toBeNull();
  });

  it('expires values five seconds after receipt without waiting for the next reply', () => {
    const reception = received(snapshot());
    expect(currentMetric(reception, 'speed_kmh', 5000)).toBe(0);
    expect(currentMetric(reception, 'speed_kmh', 5001)).toBeNull();
  });

  it('uses the receipt of the reported power source', () => {
    const live = snapshot({ cycling_power_received_at: '2026-10-30T09:59:50.000Z' });
    expect(currentMetric(received(live), 'power_w', 0)).toBeNull();
    expect(currentMetric(received(live), 'speed_kmh', 0)).toBe(0);
  });

  it('hides metrics for finished sessions, non-streaming trainers and offline screens', () => {
    const finished = snapshot();
    finished.session = { ...finished.session!, finished_at: NOW };
    for (const reception of [
      received(finished),
      received(snapshot({ connection_state: 'retry_wait' })),
      received(snapshot(), false),
      received(snapshot({ session: null })),
    ]) {
      expect(currentMetric(reception, 'power_w', 0)).toBeNull();
    }
  });
});

describe('zones', () => {
  it('uses the agreed boundaries for the default FTP', () => {
    const cases: [number, number][] = [
      [0, 0], [1, 1], [119, 1], [120, 2], [151, 2], [152, 3], [179, 3],
      [180, 4], [209, 4], [210, 5], [237, 5], [238, 6],
    ];
    for (const [watts, zone] of cases) expect(powerZone(watts, 200)).toBe(zone);
    expect(powerZone(null, 200)).toBeNull();
    expect(powerZone(-1, 200)).toBeNull();
  });

  it('accepts only whole watts between 1 and 2000', () => {
    expect([1, 200, 2000].every(isValidFtp)).toBe(true);
    expect([0, -2, 2010, 12.5, NaN].some(isValidFtp)).toBe(false);
  });
});

describe('live chart', () => {
  it('records a gap once on expiry and never draws missing data as zero', () => {
    const trace = new PowerTrace();
    trace.record(0, 150);
    trace.markGapIfExpired(1000, null);
    trace.markGapIfExpired(2000, null);
    expect(trace.samples.map((s) => s.value)).toEqual([150, null]);
  });

  it('keeps a real zero before a finish and ends at the zero line', () => {
    const trace = new PowerTrace();
    trace.record(0, 150);
    trace.record(1000, 0);
    trace.record(1100, null);
    const chart = liveChart(trace, 1100, 200);
    expect(chart.segments).toHaveLength(1);
    expect(Number(chart.segments[0].points.split(' ').at(-1)!.split(',')[1])).toBe(142);
  });

  it('drops samples older than sixty seconds', () => {
    const trace = new PowerTrace();
    trace.record(0, 100);
    trace.record(60_001, 120);
    expect(trace.samples).toHaveLength(1);
  });

  it('colors segments by zone and still breaks at missing data', () => {
    const chart = buildChart(
      [80, 150, null, 230, 250].map((value, index) => ({ x: index / 4, value })),
      200,
    );
    expect(chart.segments.map((s) => s.color)).toEqual(['#55b8f3', '#f17474']);
  });

  it('recolors with a new FTP without changing the drawn watts', () => {
    const samples = [{ x: 0, value: 180 }, { x: 1, value: 180 }];
    const before = buildChart(samples, 200);
    const after = buildChart(samples, 300);
    expect(before.segments[0].color).toBe('#ebd35e');
    expect(after.segments[0].color).toBe('#55b8f3');
    expect(after.segments[0].points).toBe(before.segments[0].points);
  });

  it('needs two real samples before drawing', () => {
    expect(buildChart([{ x: 0, value: 100 }, { x: 1, value: null }], 200).drawable).toBe(false);
  });
});

describe('session chart', () => {
  const point = (second: number, power_w: number | null): SeriesPoint => ({
    t: new Date(Date.parse(NOW) + second * 1000).toISOString(),
    power_w,
    power_source: power_w == null ? null : 'ftms',
    cadence_rpm: null,
    speed_kmh: null,
  });

  it('breaks the line at missing seconds instead of interpolating', () => {
    const chart = sessionChart([point(0, 100), point(1, 110), point(5, 120), point(6, 130)], 200);
    expect(chart.segments).toHaveLength(2);
  });

  it('breaks the line at seconds without power', () => {
    const chart = sessionChart([point(0, 100), point(1, 110), point(2, null), point(3, 120), point(4, 130)], 200);
    expect(chart.segments).toHaveLength(2);
  });
});

it('formats durations with hours only when needed', () => {
  expect(formatDuration(null)).toBe('—:—');
  expect(formatDuration(65)).toBe('01:05');
  expect(formatDuration(3725)).toBe('1:02:05');
});

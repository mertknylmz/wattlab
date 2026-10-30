import type { LiveSnapshot } from '../api/types';
import { zoneColor } from './zones';

export const FRESH_MS = 5000;
export const LIVE_WINDOW_MS = 60_000;

export type MetricKey = 'power_w' | 'cadence_rpm' | 'speed_kmh';

export interface Reception {
  snapshot: LiveSnapshot | null;
  /** Local monotonic time (performance.now) when the snapshot arrived. */
  sampledAt: number;
  online: boolean;
}

/**
 * A value is shown only while the session is active, the trainer is streaming
 * and the server receipt is at most five seconds old, counting the time since
 * this screen received the snapshot. Stale or missing values are null, real
 * zeros stay zero. This is a display rule, not a Bluetooth disconnect test.
 */
export function currentMetric(reception: Reception, key: MetricKey, now: number): number | null {
  const { snapshot, sampledAt, online } = reception;
  if (!online || !snapshot?.session || snapshot.session.finished_at || snapshot.connection_state !== 'streaming') {
    return null;
  }
  const receipt =
    key === 'power_w' && snapshot.power_source === 'cycling_power'
      ? snapshot.cycling_power_received_at
      : snapshot.ftms_received_at;
  if (!receipt || !snapshot.captured_at) return null;
  const age = Date.parse(snapshot.captured_at) - Date.parse(receipt) + now - sampledAt;
  return age >= 0 && age <= FRESH_MS ? snapshot[key] : null;
}

export interface Sample {
  time: number;
  value: number | null;
}

/**
 * Power samples actually seen by this screen. A null sample is a gap marker:
 * the line breaks there and is never drawn down to zero.
 */
export class PowerTrace {
  samples: Sample[] = [];

  record(time: number, value: number | null) {
    this.samples.push({ time, value });
    this.prune(time);
  }

  /** On a timer tick: mark expiry as a gap once, never repeat old values. */
  markGapIfExpired(time: number, current: number | null) {
    if (current == null && this.samples.at(-1)?.value != null) this.record(time, null);
    this.prune(time);
  }

  clear() {
    this.samples = [];
  }

  private prune(now: number) {
    this.samples = this.samples.filter((sample) => now - sample.time <= LIVE_WINDOW_MS);
  }
}

export interface Segment {
  points: string;
  color: string;
}

export interface Chart {
  segments: Segment[];
  min: number;
  max: number;
  drawable: boolean;
}

/**
 * Build zone-colored polylines in a 900x150 box. Each segment is colored by its
 * ending sample; nulls split the line. Recoloring after an FTP change never
 * alters the raw watts.
 */
export function buildChart(
  samples: { x: number; value: number | null }[],
  ftp: number,
  width = 900,
  height = 150,
): Chart {
  const valid = samples.filter((s) => s.value != null) as { x: number; value: number }[];
  const max = Math.max(50, ...valid.map((s) => s.value));
  const min = Math.min(0, ...valid.map((s) => s.value));
  const segments: Segment[] = [];
  if (valid.length < 2) return { segments, min, max, drawable: false };
  let points: string[] = [];
  let color: string | null = null;
  let previous: string | null = null;
  const flush = () => {
    if (points.length > 1 && color) segments.push({ points: points.join(' '), color });
    points = [];
  };
  for (const sample of samples) {
    if (sample.value == null) {
      flush();
      previous = null;
      color = null;
      continue;
    }
    const position = `${round(sample.x * width)},${round(height - 8 - ((sample.value - min) / (max - min)) * (height - 16))}`;
    const next = zoneColor(sample.value, ftp);
    if (next !== color) {
      flush();
      if (previous) points.push(previous);
      color = next;
    }
    points.push(position);
    previous = position;
  }
  flush();
  return { segments, min, max, drawable: true };
}

function round(value: number) {
  return Math.round(value * 100) / 100;
}

export function liveChart(trace: PowerTrace, now: number, ftp: number): Chart {
  return buildChart(
    trace.samples.map((s) => ({ x: 1 - (now - s.time) / LIVE_WINDOW_MS, value: s.value })),
    ftp,
  );
}

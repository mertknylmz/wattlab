import type { SeriesPoint } from '../api/types';
import { buildChart, type Chart } from './live';

/**
 * Whole-session chart. Seconds without a power value, or a jump of more than
 * one second between buckets, break the line instead of being interpolated.
 */
export function sessionChart(points: SeriesPoint[], ftp: number): Chart {
  if (points.length === 0) return buildChart([], ftp);
  const first = Date.parse(points[0].t);
  const span = Math.max(1000, Date.parse(points[points.length - 1].t) - first);
  const samples: { x: number; value: number | null }[] = [];
  let previous: number | null = null;
  for (const point of points) {
    const time = Date.parse(point.t);
    if (previous != null && time - previous > 1000) samples.push({ x: (previous - first) / span, value: null });
    samples.push({ x: (time - first) / span, value: point.power_w });
    previous = time;
  }
  return buildChart(samples, ftp);
}

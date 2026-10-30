import type { ActivityEntry } from '../api/types';
import type { Locale } from '../i18n/messages';
import { interpolate, MESSAGES } from '../i18n/messages';

export interface DayActivity {
  date: string;
  sessions: number;
  durationSeconds: number;
}

export function localDateKey(value: Date | string): string {
  const date = typeof value === 'string' ? new Date(value) : value;
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

export function startOfDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

export function startOfSunday(date: Date): Date {
  const start = startOfDay(date);
  start.setDate(start.getDate() - start.getDay());
  return start;
}

/** Last 53 weeks ending today, Sunday-aligned like GitHub's graph. */
export function heatmapRange(now = new Date()): { from: Date; to: Date } {
  const to = startOfDay(now);
  to.setDate(to.getDate() + 1);
  const from = startOfSunday(now);
  from.setDate(from.getDate() - 52 * 7);
  return { from, to };
}

export function groupActivity(entries: ActivityEntry[]): Map<string, DayActivity> {
  const days = new Map<string, DayActivity>();
  for (const entry of entries) {
    const date = localDateKey(entry.started_at);
    const current = days.get(date) ?? { date, sessions: 0, durationSeconds: 0 };
    current.sessions += 1;
    current.durationSeconds += Math.max(0, entry.duration_seconds);
    days.set(date, current);
  }
  return days;
}

export function activityLevel(sessions: number): 0 | 1 | 2 | 3 | 4 {
  if (sessions <= 0) return 0;
  if (sessions === 1) return 1;
  if (sessions === 2) return 2;
  if (sessions === 3) return 3;
  return 4;
}

export function formatHours(seconds: number, locale: Locale = 'en'): string {
  const total = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const t = (key: 'hoursMinutes' | 'hoursOnly' | 'minutesOnly' | 'zeroMinutes', values?: Record<string, number>) =>
    interpolate(MESSAGES[locale][key], values);
  if (hours && minutes) return t('hoursMinutes', { h: hours, m: minutes });
  if (hours) return t('hoursOnly', { h: hours });
  if (minutes) return t('minutesOnly', { m: minutes });
  return t('zeroMinutes');
}

export function heatmapWeeks(range: { from: Date; to: Date }, days: Map<string, DayActivity>): DayActivity[][] {
  const weeks: DayActivity[][] = [];
  const cursor = new Date(range.from);
  for (let weekIndex = 0; weekIndex < 53; weekIndex += 1) {
    const week: DayActivity[] = [];
    for (let weekday = 0; weekday < 7; weekday += 1) {
      const date = localDateKey(cursor);
      week.push(days.get(date) ?? { date, sessions: 0, durationSeconds: 0 });
      cursor.setDate(cursor.getDate() + 1);
    }
    weeks.push(week);
  }
  return weeks;
}

export function monthLabels(weeks: DayActivity[][], locale: Locale = 'en'): { index: number; label: string }[] {
  const labels: { index: number; label: string }[] = [];
  let last = '';
  let lastIndex = -3;
  weeks.forEach((week, index) => {
    const month = new Date(`${week[0].date}T00:00:00`).toLocaleString(locale === 'en' ? 'en-GB' : 'tr-TR', {
      month: 'short',
    });
    if (month === last) return;
    last = month;
    if (index - lastIndex < 3) return;
    labels.push({ index, label: month });
    lastIndex = index;
  });
  return labels;
}

import type { Locale } from '../i18n/messages';

const TAG: Record<Locale, string> = { tr: 'tr-TR', en: 'en-GB' };

export function formatNumber(value: number | null | undefined, locale: Locale = 'en'): string {
  return value == null ? '—' : new Intl.NumberFormat(TAG[locale], { maximumFractionDigits: 1 }).format(value);
}

export function formatDuration(seconds: number | null | undefined): string {
  if (seconds == null) return '—:—';
  const total = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60).toString().padStart(2, '0');
  const rest = (total % 60).toString().padStart(2, '0');
  return hours ? `${hours}:${minutes}:${rest}` : `${minutes}:${rest}`;
}

export function formatDate(iso: string, locale: Locale = 'en'): string {
  return new Intl.DateTimeFormat(TAG[locale], { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(iso));
}

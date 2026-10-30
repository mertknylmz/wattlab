const number = new Intl.NumberFormat('tr-TR', { maximumFractionDigits: 1 });
const date = new Intl.DateTimeFormat('tr-TR', { dateStyle: 'medium', timeStyle: 'short' });

export function formatNumber(value: number | null | undefined): string {
  return value == null ? '—' : number.format(value);
}

export function formatDuration(seconds: number | null | undefined): string {
  if (seconds == null) return '—:—';
  const total = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60).toString().padStart(2, '0');
  const rest = (total % 60).toString().padStart(2, '0');
  return hours ? `${hours}:${minutes}:${rest}` : `${minutes}:${rest}`;
}

export function formatDate(iso: string): string {
  return date.format(new Date(iso));
}

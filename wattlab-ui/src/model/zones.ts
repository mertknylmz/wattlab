export const DEFAULT_FTP_W = 200;
export const ZONE_COLORS = ['#b3bcb2', '#9aa6b2', '#55b8f3', '#79d68b', '#ebd35e', '#f5a15e', '#f17474'];
export const ZONE_NAMES = ['Boşta', 'Toparlanma', 'Dayanıklılık', 'Tempo', 'Eşik', 'Yüksek yoğunluk', 'Anaerobik'];
export const ZONE_RANGES = ['0 W', '<%60', '%60–<76', '%76–<90', '%90–<105', '%105–<119', '≥%119'];
const LIMITS = [60, 76, 90, 105, 119];

/**
 * Z0 is a real zero reading; missing or negative power has no zone. Integer
 * comparison keeps boundaries identical to the Rust summary calculation.
 */
export function powerZone(watts: number | null | undefined, ftp: number): number | null {
  if (watts == null || !Number.isFinite(watts) || watts < 0) return null;
  if (watts === 0) return 0;
  const index = LIMITS.findIndex((limit) => watts * 100 < limit * ftp);
  return index === -1 ? 6 : index + 1;
}

export function zoneColor(watts: number | null, ftp: number): string {
  const zone = powerZone(watts, ftp);
  return zone == null ? ZONE_COLORS[0] : ZONE_COLORS[zone];
}

export function isValidFtp(value: number): boolean {
  return Number.isInteger(value) && value >= 1 && value <= 2000;
}

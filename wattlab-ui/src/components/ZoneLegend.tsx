import { useAppearance } from '../i18n/appearance';
import { ZONE_NAME_KEYS, ZONE_RANGES } from '../model/zones';

export function ZoneLegend() {
  const { t } = useAppearance();
  return (
    <div className="zone-legend" aria-label={t('zoneLegend')}>
      {ZONE_RANGES.map((range, zone) => (
        <span key={zone} className={`zone-${zone}`} title={t(ZONE_NAME_KEYS[zone])}>
          Z{zone} · {range}
        </span>
      ))}
    </div>
  );
}

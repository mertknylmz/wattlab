import { ZONE_NAMES, ZONE_RANGES } from '../model/zones';

export function ZoneLegend() {
  return (
    <div className="zone-legend" aria-label="FTP yüzdesine göre zonlar">
      {ZONE_RANGES.map((range, zone) => (
        <span key={zone} className={`zone-${zone}`} title={ZONE_NAMES[zone]}>
          Z{zone} · {range}
        </span>
      ))}
    </div>
  );
}

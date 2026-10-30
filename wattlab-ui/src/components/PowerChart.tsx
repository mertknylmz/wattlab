import type { Chart } from '../model/live';

interface Props {
  chart: Chart;
  label: string;
  empty: string;
}

export function PowerChart({ chart, label, empty }: Props) {
  return (
    <div className="chart">
      <div className="chart-grid" />
      <svg viewBox="0 0 900 150" preserveAspectRatio="none" role="img" aria-label={label} data-testid="power-chart">
        {chart.segments.map((segment, index) => (
          <polyline
            key={index}
            points={segment.points}
            fill="none"
            stroke={segment.color}
            strokeWidth={2}
            vectorEffect="non-scaling-stroke"
          />
        ))}
      </svg>
      {!chart.drawable && <p>{empty}</p>}
    </div>
  );
}

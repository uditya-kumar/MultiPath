import { ArrowUp } from "lucide-react";
import type { LinkStatus } from "../api/types";
import { formatRate, networkColor, ringSegments, scaleRate } from "../lib/format";
import { useSmoothed } from "../lib/useSmoothed";

const SIZE = 120;
const STROKE = 10.8;
const RADIUS = (SIZE - STROKE) / 2;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;
const GAP = 4;

interface Props {
  links: LinkStatus[];
  rxRate: number;
  txRate: number;
}

/** Total download speed in a ring split by how much each network carries. */
export function ShareRing({ links, rxRate, txRate }: Props) {
  const total = scaleRate(useSmoothed(rxRate));
  const segments = ringSegments(
    links.map((l) => l.share),
    CIRCUMFERENCE,
    GAP,
  );

  return (
    <div className="share-ring">
      <div className="ring">
        <svg width={SIZE} height={SIZE} viewBox={`0 0 ${SIZE} ${SIZE}`} aria-hidden>
          <circle
            cx={SIZE / 2}
            cy={SIZE / 2}
            r={RADIUS}
            fill="none"
            stroke="var(--track)"
            strokeWidth={STROKE}
          />
          {links.map((link, i) => (
            <circle
              key={link.id}
              cx={SIZE / 2}
              cy={SIZE / 2}
              r={RADIUS}
              fill="none"
              stroke={networkColor(link.kind)}
              strokeWidth={STROKE}
              strokeDasharray={`${segments[i].length} ${CIRCUMFERENCE}`}
              strokeDashoffset={-segments[i].offset}
            />
          ))}
        </svg>
        <div className="ring-label">
          <span className="ring-total">{total.value}</span>
          <span className="ring-unit">{total.unit}</span>
        </div>
      </div>

      <div className="legend">
        {links.map((link) => (
          <div key={link.id} className="legend-item">
            <span className="legend-label">
              <span className="dot" style={{ background: networkColor(link.kind) }} />
              {link.name}
            </span>
            <span className="legend-value">
              <span className="legend-rate">{formatRate(link.rxRate)}</span>
              <span className="muted">{link.share}%</span>
            </span>
          </div>
        ))}
        <span className="upload muted">
          <ArrowUp size={12} />
          {formatRate(txRate)} upload
        </span>
      </div>
    </div>
  );
}

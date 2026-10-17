import type { CSSProperties } from "react";
import type { LinkStatus } from "../api/types";
import { networkColor, plural } from "../lib/format";

interface Props {
  /** The networks new connections can go to. */
  links: LinkStatus[];
}

/**
 * Where connections are going: one lane per network, as long as its share of
 * the open connections, with a pulse running along it while it gets traffic.
 */
export function ConnectionSpread({ links }: Props) {
  const total = links.reduce((sum, l) => sum + l.connections, 0);
  return (
    <div className="spread">
      <span className="section-title">Where connections go</span>
      <ul className="lanes" aria-label="Open connections per network">
        {links.map((link) => {
          const color = networkColor(link.kind);
          const share = total > 0 ? Math.round((link.connections / total) * 100) : 0;
          return (
            <li key={link.id} className="lane">
              <span className="lane-name">
                <span className="dot" style={{ background: color }} />
                <span className="lane-label">{link.name}</span>
              </span>
              <span className="lane-track" aria-hidden>
                <span className="lane-fill" style={{ width: `${share}%`, background: color }} />
                {link.connections > 0 && (
                  <span className="lane-pulse" style={{ "--lane-color": color } as CSSProperties} />
                )}
              </span>
              <span className="lane-count">{plural(link.connections, "connection")}</span>
            </li>
          );
        })}
      </ul>
      <span className="muted spread-note">
        Each new connection goes out on one network. A single download stays on the network it
        started on, so apps that open several connections gain the most.
      </span>
    </div>
  );
}

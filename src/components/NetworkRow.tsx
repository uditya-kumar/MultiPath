import { ArrowDown, ArrowUp } from "lucide-react";
import type { LinkStatus } from "../api/types";
import { formatLinkSpeed, formatRate, healthText, networkColor, plural } from "../lib/format";
import { NetIcon } from "./NetIcon";
import { Toggle } from "./Toggle";

interface Props {
  link: LinkStatus;
  /** MultiPath is on, so show live traffic. */
  live: boolean;
  onEnabled: (enabled: boolean) => void;
}

/** One network: what it is, whether it works, what it carries, and a switch. */
export function NetworkRow({ link, live, onEnabled }: Props) {
  const showTraffic = live && link.present && link.enabled;
  const health = healthText(link, live);
  const color = networkColor(link.kind);

  return (
    <div
      className={link.present ? "network-row" : "network-row dim"}
      style={{ borderLeftColor: link.present && link.enabled ? color : "transparent" }}
      role="group"
      aria-label={`${link.name}: ${health.text}`}
    >
      <div className="network-top">
        <NetIcon kind={link.kind} />
        <div className="network-info">
          <span className="network-name" title={link.description || link.name}>
            {link.name}
          </span>
          <span className={health.problem ? "muted problem" : "muted"}>
            {health.problem && <span className="dot" style={{ background: "var(--warn)" }} />}
            {health.text}
          </span>
        </div>
        {showTraffic && (
          <div className="network-rate" aria-label={`Download ${formatRate(link.rxRate)}, upload ${formatRate(link.txRate)}`}>
            <span className="value">
              <ArrowDown size={11} aria-hidden />
              {formatRate(link.rxRate)}
            </span>
            <span className="muted">
              <ArrowUp size={10} aria-hidden />
              {formatRate(link.txRate)}
            </span>
          </div>
        )}
        {/* A network that isn't there can't be used, whatever the setting says. */}
        <Toggle
          on={link.enabled && link.present}
          label={`Use ${link.name}`}
          disabled={!link.present}
          onChange={onEnabled}
        />
      </div>
      {showTraffic && (
        <div className="network-share">
          <div className="share-track" aria-hidden>
            <div className="share-fill" style={{ width: `${link.share}%`, background: color }} />
          </div>
          <span className="muted">
            {link.share}% of traffic · {plural(link.connections, "connection")}
          </span>
        </div>
      )}
      {link.present && (
        <details className="advanced">
          <summary>Advanced</summary>
          <dl>
            <dt>Interface</dt>
            <dd>{link.id}</dd>
            {link.description && (
              <>
                <dt>Adapter</dt>
                <dd>{link.description}</dd>
              </>
            )}
            <dt>Address</dt>
            <dd>{link.address ?? "—"}</dd>
            <dt>Gateway</dt>
            <dd>{link.gateway ?? "—"}</dd>
            <dt>Round trip</dt>
            <dd>{link.latencyMs != null ? `${link.latencyMs} ms` : "—"}</dd>
            {link.speedBps != null && (
              <>
                <dt>Link speed</dt>
                <dd>{formatLinkSpeed(link.speedBps)}</dd>
              </>
            )}
            <dt>Share weight</dt>
            <dd>{link.weight} / 10</dd>
            {link.isDefault && (
              <>
                <dt>System default</dt>
                <dd>Yes, when MultiPath is off</dd>
              </>
            )}
          </dl>
        </details>
      )}
    </div>
  );
}

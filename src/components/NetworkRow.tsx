import type { LinkStatus } from "../api/types";
import { formatRate, networkColor, networkMeta } from "../lib/format";
import { NetIcon } from "./NetIcon";
import { Toggle } from "./Toggle";

interface Props {
  link: LinkStatus;
  /** MultiPath is on, so show live traffic. */
  live: boolean;
  onEnabled: (enabled: boolean) => void;
}

export function NetworkRow({ link, live, onEnabled }: Props) {
  const showTraffic = live && link.present && link.enabled;
  let value: string;
  let detail: string;
  if (!link.present) {
    value = "—";
    detail = "Not available";
  } else if (showTraffic) {
    value = formatRate(link.rxRate);
    detail = `${link.share}% · ${link.connections} conn`;
  } else if (live) {
    value = "—";
    detail = "Not used";
  } else {
    value = link.latencyMs != null ? `${link.latencyMs} ms` : "—";
    detail = link.isDefault ? "Default route" : "Ready";
  }

  return (
    <div className={link.present ? "network-row" : "network-row dim"}>
      <div className="network-top">
        <NetIcon kind={link.kind} />
        <div className="network-info">
          <span className="network-name" title={link.description}>
            {link.name}
          </span>
          <span className="muted">{networkMeta(link)}</span>
        </div>
        <div className="network-rate">
          <span className="value">{value}</span>
          <span className="muted">{detail}</span>
        </div>
        {/* A network that isn't there can't be used, whatever the setting says. */}
        <Toggle
          on={link.enabled && link.present}
          label={`Use ${link.name}`}
          disabled={!link.present}
          onChange={onEnabled}
        />
      </div>
      {showTraffic && (
        <div className="share-track">
          <div
            className="share-fill"
            style={{ width: `${link.share}%`, background: networkColor(link.kind) }}
          />
        </div>
      )}
    </div>
  );
}

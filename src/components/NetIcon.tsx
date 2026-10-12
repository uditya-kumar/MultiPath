import { EthernetPort, Network, Smartphone, Wifi } from "lucide-react";
import type { LinkKind } from "../api/types";
import { networkColor } from "../lib/format";

const ICONS = {
  wifi: Wifi,
  ethernet: EthernetPort,
  phone: Smartphone,
  cellular: Smartphone,
  other: Network,
};

/** The network's icon in a tinted square of its color. */
export function NetIcon({ kind }: { kind: LinkKind }) {
  const Icon = ICONS[kind] ?? Network;
  const color = networkColor(kind);
  return (
    <span
      className="icon-box"
      style={{ color, background: `color-mix(in srgb, ${color} 15%, transparent)` }}
    >
      <Icon size={16} />
    </span>
  );
}

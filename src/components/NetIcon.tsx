import { EthernetPort, Network, Smartphone, Wifi } from "lucide-react";
import type { LinkKind } from "../api/types";

const ICONS = {
  wifi: Wifi,
  ethernet: EthernetPort,
  phone: Smartphone,
  cellular: Smartphone,
  other: Network,
};

/** The network's icon in a tinted square of its color. */
export function NetIcon({ kind, color }: { kind: LinkKind; color: string }) {
  const Icon = ICONS[kind] ?? Network;
  return (
    <span
      className="icon-box"
      style={{ color, background: `color-mix(in srgb, ${color} 15%, transparent)` }}
    >
      <Icon size={16} />
    </span>
  );
}

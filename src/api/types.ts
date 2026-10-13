// Mirrors the structs in src-tauri/src/app/status.rs and commands.rs.

export type LinkKind = "wifi" | "ethernet" | "phone" | "cellular" | "other";
export type Health = "up" | "down" | "unknown";

export interface LinkStatus {
  id: string;
  name: string;
  description: string;
  kind: LinkKind;
  address: string | null;
  gateway: string | null;
  speedBps: number | null;
  latencyMs: number | null;
  health: Health;
  present: boolean;
  isDefault: boolean;
  enabled: boolean;
  weight: number;
  /** Bytes per second. */
  rxRate: number;
  txRate: number;
  /** Percent of the current traffic. */
  share: number;
  connections: number;
}

export interface SettingsStatus {
  adaptive: boolean;
  sticky: boolean;
  captureIpv6: boolean;
  dns: string[];
  startOnLogin: boolean;
}

export interface Status {
  platform: "windows" | "linux";
  on: boolean;
  busy: boolean;
  elevated: boolean;
  canElevate: boolean;
  error: string | null;
  ipv6: boolean;
  rxRate: number;
  txRate: number;
  connections: number;
  todayBytes: number;
  links: LinkStatus[];
  settings: SettingsStatus;
}

export interface Connection {
  id: number;
  proto: "tcp" | "udp";
  app: string | null;
  host: string | null;
  destination: string;
  linkId: string | null;
  linkName: string | null;
  rx: number;
  tx: number;
  rxRate: number;
  txRate: number;
  seconds: number;
}

/** Settings a change can carry; fields left out stay as they are. */
export interface OptionChange {
  adaptive?: boolean;
  sticky?: boolean;
  captureIpv6?: boolean;
  dns?: string[];
  startOnLogin?: boolean;
}

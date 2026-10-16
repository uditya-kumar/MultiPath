import type { LinkKind, LinkStatus } from "../api/types";

const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** Scales a byte count to decimal units, like Task Manager: 3.4 GB. */
export function scale(bytes: number): { value: string; unit: string } {
  let n = Math.max(0, bytes);
  let i = 0;
  while (n >= 1000 && i < UNITS.length - 1) {
    n /= 1000;
    i++;
  }
  // Keep about three significant digits, and none for plain bytes.
  const digits = i === 0 || n >= 100 ? 0 : 1;
  let value = n.toFixed(digits);
  // 999.96 KB rounds up to "1000.0"; show it as 1.0 MB instead.
  if (Number(value) >= 1000 && i < UNITS.length - 1) {
    value = "1.0";
    i++;
  }
  return { value, unit: UNITS[i] };
}

export function formatBytes(bytes: number): string {
  const { value, unit } = scale(bytes);
  return `${value} ${unit}`;
}

/** A rate in bytes per second, split so the ring can style the unit. */
export function scaleRate(bytesPerSecond: number): { value: string; unit: string } {
  const { value, unit } = scale(bytesPerSecond);
  return { value, unit: `${unit}/s` };
}

export function formatRate(bytesPerSecond: number): string {
  const { value, unit } = scaleRate(bytesPerSecond);
  return `${value} ${unit}`;
}

/** The rate scaled to a fixed unit, so ring and legend agree: "47.8". */
export function rateIn(bytesPerSecond: number, unit: string): string {
  const power = UNITS.indexOf(unit.replace("/s", ""));
  const n = Math.max(0, bytesPerSecond) / 1000 ** Math.max(0, power);
  return n.toFixed(power <= 0 || n >= 100 ? 0 : 1);
}

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n.toLocaleString("en-US")} ${n === 1 ? one : many}`;
}

/** The CSS color for a network, by what kind of adapter it is. */
export function networkColor(kind: LinkKind): string {
  switch (kind) {
    case "wifi":
      return "var(--net-wifi)";
    case "phone":
    case "cellular":
      return "var(--net-phone)";
    case "ethernet":
      return "var(--net-eth)";
    default:
      return "var(--text-3)";
  }
}

/** A network's health in plain words, and whether it's a problem. */
export function healthText(link: LinkStatus, live: boolean): { text: string; problem: boolean } {
  if (!link.present) {
    return { text: link.kind === "ethernet" ? "Cable unplugged" : "Not connected", problem: false };
  }
  if (!link.enabled) return { text: "Switched off", problem: false };
  if (link.health === "down") return { text: "No internet", problem: true };
  if (link.health === "unknown") return { text: "Checking…", problem: false };
  const latency = link.latencyMs != null ? ` · ${link.latencyMs} ms` : "";
  return { text: `${live ? "In use" : "Working"}${latency}`, problem: false };
}

/** A link speed the adapter reports, in bits per second: "866 Mbit/s". */
export function formatLinkSpeed(bitsPerSecond: number): string {
  if (bitsPerSecond >= 1e9) return `${(bitsPerSecond / 1e9).toFixed(1)} Gbit/s`;
  return `${Math.round(bitsPerSecond / 1e6)} Mbit/s`;
}

/** How long a connection has been open: "45 s", "3 min", "2 h". */
export function formatAge(seconds: number): string {
  if (seconds < 60) return `${seconds} s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)} min`;
  return `${Math.floor(seconds / 3600)} h`;
}

/** Arc lengths for the share ring: one per share, with a gap between them. */
export function ringSegments(
  shares: number[],
  circumference: number,
  gap: number,
): { length: number; offset: number }[] {
  const visible = shares.filter((s) => s > 0).length;
  const room = circumference - (visible > 1 ? visible * gap : 0);
  let at = 0;
  return shares.map((share) => {
    const length = share > 0 ? (room * share) / 100 : 0;
    const segment = { length, offset: at };
    if (share > 0) at += length + (visible > 1 ? gap : 0);
    return segment;
  });
}

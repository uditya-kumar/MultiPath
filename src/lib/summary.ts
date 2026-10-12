import type { LinkStatus, Status } from "../api/types";
import { formatBytes, plural } from "./format";

/** What's switching, if anything, from the user's click or the backend. */
export type Pending = "on" | "off" | null;

/** Networks MultiPath spreads connections over while it's on. */
export function inUse(links: LinkStatus[]): LinkStatus[] {
  return links.filter((l) => l.present && l.enabled);
}

/** The header's status line and dot color. */
export function headerStatus(status: Status, pending: Pending): { text: string; dot: string } {
  const switching = pending ?? (status.busy ? (status.on ? "off" : "on") : null);
  if (switching) {
    return { text: switching === "on" ? "Turning on…" : "Turning off…", dot: "var(--text-3)" };
  }
  if (status.on) {
    const n = inUse(status.links).length;
    return {
      text: n === 1 ? "On · using 1 network" : `Combining ${n} networks`,
      dot: status.error ? "var(--warn)" : "var(--ok)",
    };
  }
  const fallback = status.links.find((l) => l.isDefault);
  return {
    text: fallback ? `Off · Windows is using ${fallback.name} only` : "Off",
    dot: status.error ? "var(--warn)" : "var(--text-3)",
  };
}

/** The footer's one-line summary. */
export function footerInfo(status: Status): string {
  if (status.on) {
    return `${plural(status.connections, "connection")} · ${formatBytes(status.todayBytes)} today`;
  }
  const ready = status.links.filter((l) => l.present).length;
  return `${plural(ready, "network")} ready`;
}

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
    text: fallback ? `Off · ${osName(status)} is using ${fallback.name} only` : "Off",
    dot: status.error ? "var(--warn)" : "var(--text-3)",
  };
}

/** The system's own name, for the lines that talk about it. */
function osName(status: Status): string {
  switch (status.platform) {
    case "linux":
      return "Linux";
    case "macos":
      return "macOS";
    default:
      return "Windows";
  }
}

/** What turning on will ask for, shown while MultiPath lacks the rights. */
export function elevationNote(status: Status): string {
  if (!status.canElevate) {
    return "The AppImage needs to run with sudo. Install the .deb to turn on without it.";
  }
  switch (status.platform) {
    case "linux":
      return "Linux will ask for your password once, to give MultiPath network access.";
    case "macos":
      return "macOS will ask for an administrator password each time you turn it on.";
    default:
      return "Windows will ask for administrator permission the first time.";
  }
}

/** The settings switch for starting with the system, in each system's words. */
export function startAtLoginLabel(status: Status): string {
  switch (status.platform) {
    case "linux":
      return "Start on login";
    case "macos":
      return "Open at login";
    default:
      return "Start with Windows";
  }
}

/** The footer's one-line summary. */
export function footerInfo(status: Status): string {
  if (status.on) {
    return `${plural(status.connections, "connection")} · ${formatBytes(status.todayBytes)} today`;
  }
  const ready = status.links.filter((l) => l.present).length;
  return `${plural(ready, "network")} ready`;
}

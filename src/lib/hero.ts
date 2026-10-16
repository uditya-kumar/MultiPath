import type { LinkStatus, Status } from "../api/types";
import { errorText } from "../api/commands";
import type { Pending } from "./summary";

/** The five states the top of the popup can be in. */
export type HeroKind = "off" | "starting" | "active" | "limited" | "attention";

export interface Hero {
  kind: HeroKind;
  title: string;
  /** One plain sentence: what's happening, or what to do. */
  sentence: string;
}

/** Present and switched on: MultiPath may send new connections there. */
export function usable(link: LinkStatus): boolean {
  return link.present && link.enabled && link.health !== "down";
}

/** "Wi-Fi", "Wi-Fi and iPhone USB", "Wi-Fi, Ethernet and iPhone USB". */
export function listNames(names: string[]): string {
  if (names.length <= 1) return names[0] ?? "";
  return `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}

/** What the user is sitting at, for the sentences that mention it. */
export function deviceName(status: Status): string {
  return status.platform === "macos" ? "this Mac" : "this PC";
}

function systemName(status: Status): string {
  switch (status.platform) {
    case "macos":
      return "macOS";
    case "linux":
      return "Linux";
    default:
      return "Windows";
  }
}

/** The state shown at the top of the popup, in words anyone can follow. */
export function heroState(status: Status, pending: Pending): Hero {
  const switching = pending ?? (status.busy ? (status.on ? "off" : "on") : null);
  if (switching === "on") {
    const asking = !status.elevated && status.platform === "macos";
    return {
      kind: "starting",
      title: "Starting…",
      sentence: asking
        ? "Enter an administrator password in the macOS dialog to continue."
        : "Setting up MultiPath's network adapter.",
    };
  }
  if (switching === "off") {
    return {
      kind: "starting",
      title: "Turning off…",
      sentence: `Handing your traffic back to ${systemName(status)}.`,
    };
  }

  const present = status.links.filter((l) => l.present);
  if (status.error) {
    return { kind: "attention", title: "Needs attention", sentence: errorText(status.error) };
  }
  if (present.length === 0) {
    return {
      kind: "attention",
      title: "No network connection",
      sentence: "Join a Wi-Fi network, plug in a cable, or connect a phone with USB.",
    };
  }

  if (!status.on) {
    const fallback = status.links.find((l) => l.isDefault) ?? present[0];
    return {
      kind: "off",
      title: "Off",
      sentence:
        present.length > 1
          ? `${systemName(status)} is using ${fallback.name} only. Turn on to use all ${present.length} networks together.`
          : `${systemName(status)} is using ${fallback.name}. Connect a second network to combine them.`,
    };
  }

  const working = present.filter(usable);
  const names = working.map((l) => l.name);
  if (working.length === 0) {
    if (present.every((l) => !l.enabled)) {
      return {
        kind: "attention",
        title: "All networks are switched off",
        sentence: "Switch on at least one network below, or turn MultiPath off.",
      };
    }
    return {
      kind: "attention",
      title: "No internet",
      sentence: "None of your networks can reach the internet. MultiPath checks again every 5 seconds.",
    };
  }
  if (working.length === 1) {
    const failing = present.find((l) => l.enabled && l.health === "down");
    return {
      kind: "limited",
      title: `Only using ${names[0]}`,
      sentence: failing
        ? `${failing.name} has no internet, so new connections go to ${names[0]}.`
        : `New connections all go to ${names[0]}. Connect another network to add its speed.`,
    };
  }
  return {
    kind: "active",
    title: "Active",
    sentence: `New connections are shared between ${listNames(names)}.`,
  };
}

/**
 * What changed between two statuses that the user should hear about: a
 * network that dropped out (and where its traffic went), or one that came
 * back. `null` when nothing did.
 */
export function linkChange(before: Status, after: Status): string | null {
  if (!before.on || !after.on) return null;
  const wasUsable = new Set(before.links.filter(usable).map((l) => l.id));
  const working = after.links.filter(usable);
  const lost = after.links.find((l) => wasUsable.has(l.id) && !usable(l) && l.enabled);
  if (lost) {
    const what = lost.present ? "lost its internet connection" : "disconnected";
    const where =
      working.length > 0
        ? `new connections moved to ${listNames(working.map((l) => l.name))}`
        : "no other network is available";
    return `${lost.name} ${what} — ${where}.`;
  }
  const back = working.find((l) => !wasUsable.has(l.id) && before.links.some((b) => b.id === l.id));
  if (back) {
    return `${back.name} is back — new connections use it again.`;
  }
  return null;
}

import type { LinkStatus, Status } from "../api/types";

/** Made-up statuses for the view tests and the screenshot page. */

export const wifi: LinkStatus = {
  id: "en0",
  name: "Wi-Fi",
  description: "",
  kind: "wifi",
  address: "192.168.178.157",
  gateway: "192.168.178.1",
  speedBps: 866_000_000,
  latencyMs: 14,
  health: "up",
  present: true,
  isDefault: true,
  enabled: true,
  weight: 5,
  rxRate: 0,
  txRate: 0,
  share: 0,
  connections: 0,
};

export const iphone: LinkStatus = {
  ...wifi,
  id: "en8",
  name: "iPhone USB",
  kind: "phone",
  address: "172.20.10.2",
  gateway: "172.20.10.1",
  speedBps: null,
  latencyMs: 38,
  isDefault: false,
};

export const ethernet: LinkStatus = {
  ...wifi,
  id: "en7",
  name: "USB 10/100/1000 LAN",
  kind: "ethernet",
  address: "10.0.0.23",
  gateway: "10.0.0.1",
  speedBps: 1_000_000_000,
  latencyMs: 6,
  isDefault: false,
};

export const off: Status = {
  platform: "macos",
  on: false,
  busy: false,
  elevated: false,
  canElevate: true,
  error: null,
  ipv6: false,
  rxRate: 0,
  txRate: 0,
  connections: 0,
  todayBytes: 0,
  links: [wifi, iphone],
  settings: {
    adaptive: true,
    sticky: false,
    captureIpv6: true,
    dns: ["8.8.8.8", "1.1.1.1"],
    startOnLogin: false,
  },
};

export const active: Status = {
  ...off,
  on: true,
  rxRate: 4_830_000,
  txRate: 212_000,
  connections: 37,
  todayBytes: 3_400_000_000,
  links: [
    { ...wifi, rxRate: 2_210_000, txRate: 96_000, share: 46, connections: 17 },
    { ...ethernet, rxRate: 1_630_000, txRate: 71_000, share: 34, connections: 12 },
    { ...iphone, rxRate: 990_000, txRate: 45_000, share: 20, connections: 8 },
  ],
};

/** On, but the phone has lost its internet. */
export const limited: Status = {
  ...active,
  rxRate: 2_900_000,
  txRate: 120_000,
  connections: 21,
  links: [
    { ...wifi, rxRate: 2_900_000, txRate: 120_000, share: 100, connections: 21 },
    { ...iphone, health: "down", latencyMs: null, connections: 0 },
  ],
};

/** On, and no network reaches the internet. */
export const attention: Status = {
  ...active,
  rxRate: 0,
  txRate: 0,
  connections: 0,
  links: [
    { ...wifi, health: "down", latencyMs: null },
    { ...iphone, present: false, health: "unknown", latencyMs: null },
  ],
};

export const failedToStart: Status = {
  ...off,
  error: "Couldn't turn on: the administrator prompt was cancelled",
};

export const fixtures = { off, active, limited, attention, failedToStart } as const;
export type FixtureName = keyof typeof fixtures;

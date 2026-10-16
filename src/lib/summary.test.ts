import { describe, expect, it } from "vitest";
import type { LinkStatus, Status } from "../api/types";
import { elevationNote, footerInfo, headerStatus, startAtLoginLabel } from "./summary";

const wifi: LinkStatus = {
  id: "1",
  name: "Wi-Fi",
  description: "",
  kind: "wifi",
  address: "192.168.1.40",
  gateway: "192.168.1.1",
  speedBps: null,
  latencyMs: 18,
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
const phone: LinkStatus = { ...wifi, id: "2", name: "Phone", kind: "phone", isDefault: false };
const cable: LinkStatus = { ...wifi, id: "3", name: "Ethernet", kind: "ethernet", isDefault: false, present: false };

const off: Status = {
  platform: "windows",
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
  links: [wifi, phone, cable],
  settings: { adaptive: true, sticky: false, captureIpv6: false, dns: [], startOnLogin: false },
};
const on: Status = { ...off, on: true, connections: 142, todayBytes: 3_400_000_000 };

describe("summary", () => {
  it("says what MultiPath is doing", () => {
    expect(headerStatus(off, null).text).toBe("Off · Windows is using Wi-Fi only");
    expect(headerStatus({ ...off, platform: "linux" }, null).text).toBe("Off · Linux is using Wi-Fi only");
    expect(headerStatus({ ...off, platform: "macos" }, null).text).toBe("Off · macOS is using Wi-Fi only");
    expect(headerStatus(on, null)).toEqual({ text: "Combining 2 networks", dot: "var(--ok)" });
    expect(headerStatus({ ...on, links: [wifi] }, null).text).toBe("On · using 1 network");
    expect(headerStatus(off, "on").text).toBe("Turning on…");
    expect(headerStatus({ ...on, busy: true }, null).text).toBe("Turning off…");
  });

  it("warns after an error", () => {
    expect(headerStatus({ ...off, error: "Couldn't turn on" }, null).dot).toBe("var(--warn)");
  });

  it("sums up in the footer", () => {
    expect(footerInfo(on)).toBe("142 connections · 3.4 GB today");
    expect(footerInfo(off)).toBe("2 networks ready");
  });

  it("words the permission note and login switch per system", () => {
    expect(elevationNote({ ...off, platform: "macos" })).toBe(
      "macOS will ask for an administrator password each time you turn it on.",
    );
    expect(elevationNote(off)).toBe("Windows will ask for administrator permission the first time.");
    expect(elevationNote({ ...off, platform: "linux", canElevate: false })).toMatch(/AppImage/);
    expect(startAtLoginLabel({ ...off, platform: "macos" })).toBe("Open at login");
    expect(startAtLoginLabel({ ...off, platform: "linux" })).toBe("Start on login");
    expect(startAtLoginLabel(off)).toBe("Start with Windows");
  });
});

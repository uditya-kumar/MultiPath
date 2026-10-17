import { describe, expect, it } from "vitest";
import type { LinkStatus } from "../api/types";
import {
  formatAge,
  formatBytes,
  formatLinkSpeed,
  formatRate,
  healthText,
  plural,
  rateIn,
  ringSegments,
  scaleRate,
} from "./format";

const link: LinkStatus = {
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

describe("format", () => {
  it("scales bytes in decimal units", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(999)).toBe("999 B");
    expect(formatBytes(3_400_000_000)).toBe("3.4 GB");
    expect(formatBytes(123_456_000)).toBe("123 MB");
    expect(formatBytes(999_960)).toBe("1.0 MB");
  });

  it("splits rates for the ring", () => {
    expect(scaleRate(47_800_000)).toEqual({ value: "47.8", unit: "MB/s" });
    expect(formatRate(3_100_000)).toBe("3.1 MB/s");
    expect(rateIn(29_640_000, "MB/s")).toBe("29.6");
    expect(rateIn(500, "B/s")).toBe("500");
  });

  it("counts things", () => {
    expect(plural(1, "network")).toBe("1 network");
    expect(plural(1420, "connection")).toBe("1,420 connections");
  });

  it("describes a network's health in plain words", () => {
    expect(healthText(link, false)).toEqual({ text: "Working · 18 ms", problem: false });
    expect(healthText(link, true).text).toBe("In use · 18 ms");
    expect(healthText({ ...link, health: "down" }, true)).toEqual({ text: "No internet", problem: true });
    expect(healthText({ ...link, health: "unknown" }, true).text).toBe("Checking…");
    expect(healthText({ ...link, enabled: false }, true).text).toBe("Switched off");
    expect(healthText({ ...link, kind: "ethernet", present: false }, true).text).toBe("Cable unplugged");
    expect(healthText({ ...link, kind: "phone", present: false }, true).text).toBe("Not connected");
  });

  it("formats link speeds", () => {
    expect(formatLinkSpeed(866_000_000)).toBe("866 Mbit/s");
    expect(formatLinkSpeed(2_500_000_000)).toBe("2.5 Gbit/s");
  });

  it("tells a connection's age", () => {
    expect(formatAge(45)).toBe("45 s");
    expect(formatAge(185)).toBe("3 min");
    expect(formatAge(7300)).toBe("2 h");
  });

  it("lays out the ring with gaps between arcs", () => {
    const segments = ringSegments([60, 40, 0], 100, 2);
    expect(segments[0]).toEqual({ length: 57.6, offset: 0 });
    expect(segments[1].offset).toBeCloseTo(59.6);
    expect(segments[1].length).toBeCloseTo(38.4);
    expect(segments[2].length).toBe(0);
    // One network fills the ring with no gap.
    expect(ringSegments([100], 100, 2)).toEqual([{ length: 100, offset: 0 }]);
  });
});

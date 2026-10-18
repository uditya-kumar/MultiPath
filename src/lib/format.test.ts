import { describe, expect, it } from "vitest";
import type { LinkStatus } from "../api/types";
import {
  formatAge,
  formatBytes,
  formatRate,
  networkColors,
  networkMeta,
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

describe("networkColors", () => {
  const net = (id: string, name: string, kind: LinkStatus["kind"], present = true): LinkStatus => ({
    ...link,
    id,
    name,
    kind,
    present,
  });

  it("gives every network its own colour, the first of a kind keeping the kind's", () => {
    const colors = networkColors([
      net("a", "Ethernet 3", "ethernet"),
      net("b", "Wi-Fi", "wifi"),
      net("c", "Ethernet 4", "ethernet"),
      net("d", "Ethernet 2", "ethernet", false),
      net("e", "Wi-Fi 2", "wifi", false),
      net("f", "Pixel 8 (USB)", "phone"),
      net("g", "iPhone", "cellular"),
    ]);
    expect(colors.get("d")).toBe("var(--net-eth)");
    expect(colors.get("a")).toBe("var(--net-alt-1)");
    expect(colors.get("c")).toBe("var(--net-alt-2)");
    expect(colors.get("b")).toBe("var(--net-wifi)");
    expect(colors.get("e")).toBe("var(--net-alt-4)");
    expect(colors.get("g")).toBe("var(--net-phone)");
    expect(colors.get("f")).toBe("var(--net-alt-3)");
    expect(new Set(colors.values()).size).toBe(7);
  });

  it("stays the same whatever order the networks come in", () => {
    const links = [net("a", "Ethernet 10", "ethernet"), net("b", "Ethernet 9", "ethernet")];
    const one = networkColors(links);
    const other = networkColors([...links].reverse());
    expect(one).toEqual(other);
    expect(one.get("b")).toBe("var(--net-eth)");
  });
});

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

  it("describes a network", () => {
    expect(networkMeta(link)).toBe("192.168.1.40 · 18 ms");
    expect(networkMeta({ ...link, health: "down" })).toBe("192.168.1.40 · No internet");
    expect(networkMeta({ ...link, kind: "ethernet", present: false })).toBe("Cable unplugged");
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

import { describe, expect, it } from "vitest";
import { active, attention, failedToStart, iphone, limited, off, wifi } from "../demo/fixtures";
import { heroState, linkChange, listNames } from "./hero";

describe("hero", () => {
  it("is off, in plain words", () => {
    expect(heroState(off, null)).toEqual({
      kind: "off",
      title: "Off",
      sentence: "macOS is using Wi-Fi only. Turn on to use all 2 networks together.",
    });
    const alone = { ...off, links: [wifi] };
    expect(heroState(alone, null).sentence).toBe(
      "macOS is using Wi-Fi. Connect a second network to combine them.",
    );
  });

  it("explains the password dialog while starting on macOS", () => {
    expect(heroState(off, "on")).toEqual({
      kind: "starting",
      title: "Starting…",
      sentence: "Enter an administrator password in the macOS dialog to continue.",
    });
    expect(heroState({ ...off, platform: "windows" }, "on").sentence).toBe(
      "Setting up MultiPath's network adapter.",
    );
    expect(heroState({ ...active, busy: true }, null).title).toBe("Turning off…");
  });

  it("is active with two or more working networks", () => {
    expect(heroState(active, null)).toEqual({
      kind: "active",
      title: "Active",
      sentence: "New connections are shared between Wi-Fi, USB 10/100/1000 LAN and iPhone USB.",
    });
  });

  it("is limited with one", () => {
    expect(heroState(limited, null)).toEqual({
      kind: "limited",
      title: "Only using Wi-Fi",
      sentence: "iPhone USB has no internet, so new connections go to Wi-Fi.",
    });
    const single = { ...active, links: [wifi] };
    expect(heroState(single, null).sentence).toBe(
      "New connections all go to Wi-Fi. Connect another network to add its speed.",
    );
  });

  it("needs attention when nothing works", () => {
    expect(heroState(attention, null).kind).toBe("attention");
    expect(heroState(attention, null).title).toBe("No internet");
    expect(heroState(failedToStart, null)).toEqual({
      kind: "attention",
      title: "Needs attention",
      sentence: "Couldn't turn on: the administrator prompt was cancelled",
    });
    const allOff = { ...active, links: active.links.map((l) => ({ ...l, enabled: false })) };
    expect(heroState(allOff, null).title).toBe("All networks are switched off");
    expect(heroState({ ...off, links: [] }, null).title).toBe("No network connection");
  });

  it("says what happened when a network drops out, and when it's back", () => {
    const both = { ...active, links: [wifi, iphone] };
    const unplugged = { ...active, links: [wifi, { ...iphone, present: false }] };
    expect(linkChange(both, unplugged)).toBe(
      "iPhone USB disconnected — new connections moved to Wi-Fi.",
    );
    const noInternet = { ...active, links: [wifi, { ...iphone, health: "down" as const }] };
    expect(linkChange(both, noInternet)).toBe(
      "iPhone USB lost its internet connection — new connections moved to Wi-Fi.",
    );
    expect(linkChange(unplugged, both)).toBe("iPhone USB is back — new connections use it again.");
    // Switching a network off yourself isn't news.
    const switchedOff = { ...active, links: [wifi, { ...iphone, enabled: false }] };
    expect(linkChange(both, switchedOff)).toBeNull();
    expect(linkChange(both, both)).toBeNull();
    expect(linkChange(off, active)).toBeNull();
  });

  it("lists names like a sentence", () => {
    expect(listNames([])).toBe("");
    expect(listNames(["Wi-Fi"])).toBe("Wi-Fi");
    expect(listNames(["Wi-Fi", "iPhone"])).toBe("Wi-Fi and iPhone");
    expect(listNames(["A", "B", "C"])).toBe("A, B and C");
  });
});

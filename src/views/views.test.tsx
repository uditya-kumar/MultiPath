import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Status } from "../api/types";
import { active, attention, failedToStart, limited, off } from "../demo/fixtures";
import { Main } from "./Main";
import { Settings } from "./Settings";

const noop = () => {};

function main(status: Status): string {
  return renderToStaticMarkup(
    <Main status={status} onStatus={noop} onSettings={noop} onConnections={noop} />,
  );
}

/** The visible text, without markup, for readable assertions. */
function text(html: string): string {
  return html
    .replace(/<[^>]+>/g, " ")
    .replace(/&#x27;/g, "'")
    .replace(/\s+/g, " ");
}

describe("Main", () => {
  it("off: explains, offers to turn on, and reassures", () => {
    const page = text(main(off));
    expect(page).toContain("macOS is using Wi-Fi only.");
    expect(page).toContain("Turn on");
    expect(page).toContain("macOS will ask for an administrator password each time you turn it on.");
    expect(page).toContain("Runs only on this Mac: no relay server, no account.");
    expect(page).not.toContain("Where connections go");
  });

  it("active: shows the speed, where connections go, and each network's traffic", () => {
    const html = main(active);
    const page = text(html);
    expect(page).toContain("Active");
    expect(page).toContain("Where connections go");
    expect(page).toContain("17 connections");
    expect(page).toContain("46% of traffic · 17 connections");
    expect(page).toContain("A single download stays on the network it started on");
    // One row per network, each switchable by name for VoiceOver.
    expect(html).toContain('aria-label="Use iPhone USB"');
    expect(html).toContain('aria-label="Wi-Fi: In use · 14 ms"');
    expect(html).toContain('role="status"');
  });

  it("keeps technical details behind Advanced", () => {
    const html = main(active);
    expect(html).toMatch(/<details class="advanced"><summary>Advanced<\/summary>.*en0/);
    expect(html).toContain("192.168.178.1");
  });

  it("limited: says which network is left and why", () => {
    const page = text(main(limited));
    expect(page).toContain("Only using Wi-Fi");
    expect(page).toContain("iPhone USB has no internet, so new connections go to Wi-Fi.");
    expect(page).toContain("No internet");
  });

  it("needs attention: no internet anywhere, or a failed start", () => {
    expect(text(main(attention))).toContain("None of your networks can reach the internet.");
    expect(main(attention)).toContain('class="hero attention"');
    expect(text(main(failedToStart))).toContain("the administrator prompt was cancelled");
  });

  it("suggests a phone when there's nothing to combine", () => {
    const wifiOnly = { ...off, links: [off.links[0]] };
    expect(text(main(wifiOnly))).toContain("No phone found.");
    expect(text(main(off))).not.toContain("No phone found.");
  });

  it("warns when every network is switched off", () => {
    const allOff = { ...active, links: active.links.map((l) => ({ ...l, enabled: false })) };
    expect(text(main(allOff))).toContain("Every network is switched off.");
  });
});

describe("Settings", () => {
  it("explains sticky sites, DNS and privacy in plain words", () => {
    const page = text(renderToStaticMarkup(<Settings status={off} onStatus={noop} onBack={noop} />));
    expect(page).toContain("Fixes sites that sign you out");
    expect(page).toContain("Look up website names while MultiPath is on: 8.8.8.8, 1.1.1.1");
    expect(page).toContain("Everything stays on this Mac");
    expect(page).toContain("Open at login");
  });
});

import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Status } from "../api/types";
import { active, attention, failedToStart, limited, off, sameRouter } from "../demo/fixtures";
import { UpdatePanel } from "../components/UpdatePanel";
import type { UpdateState } from "../lib/update";
import type { UpdateControls } from "../lib/useUpdater";
import { Main } from "./Main";
import { Settings } from "./Settings";

const noop = () => {};

function controls(state: UpdateState = { step: "idle" }): UpdateControls {
  return { state, check: noop, install: noop, dismiss: noop, restart: noop };
}

function panel(state: UpdateState): string {
  return renderToStaticMarkup(<UpdatePanel current="1.2.0" update={controls(state)} />);
}

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

  const SAME_ROUTER = "These connections use the same router, so your maximum internet speed may not increase.";

  it("says so, without blocking, when networks share a router", () => {
    for (const status of [sameRouter, { ...sameRouter, on: true }]) {
      const html = main(status);
      expect(text(html)).toContain(SAME_ROUTER);
      expect(html).toContain('role="note"');
      // Informational: not styled as a warning, and every switch stays usable.
      expect(html).not.toContain('class="tip warn"');
      expect(html).not.toMatch(/<button[^>]*disabled/);
    }
  });

  it("stays quiet when every network has its own router", () => {
    expect(text(main(off))).not.toContain(SAME_ROUTER);
    expect(text(main(active))).not.toContain(SAME_ROUTER);
    const oneSwitchedOff = {
      ...sameRouter,
      links: sameRouter.links.map((l, i) => ({ ...l, enabled: i === 0 })),
    };
    expect(text(main(oneSwitchedOff))).not.toContain(SAME_ROUTER);
  });
});

describe("Settings", () => {
  it("explains sticky sites, DNS and privacy in plain words", () => {
    const page = text(renderToStaticMarkup(<Settings status={off} onStatus={noop} onBack={noop} update={controls()} />));
    expect(page).toContain("Fixes sites that sign you out");
    expect(page).toContain("Look up website names while MultiPath is on: 8.8.8.8, 1.1.1.1");
    expect(page).toContain("Everything stays on this Mac");
    expect(page).toContain("Open at login");
  });
});

describe("Updates", () => {
  const available: UpdateState = {
    step: "available",
    version: "1.2.1",
    notes: "Faster failover.",
    date: null,
  };

  it("Settings on macOS: the version, a Check for updates button, and the opt-in check at start", () => {
    const html = renderToStaticMarkup(
      <Settings status={off} onStatus={noop} onBack={noop} update={controls()} />,
    );
    const page = text(html);
    expect(page).toContain("MultiPath 1.2.0");
    expect(page).toContain("Nothing is installed until you say so.");
    expect(html).toMatch(/<button[^>]*>Check for updates<\/button>/);
    // Off unless chosen.
    expect(html).toContain('aria-checked="false" aria-label="Check for updates at start"');
  });

  it("not on Windows or Linux, where no update is published", () => {
    for (const platform of ["windows", "linux"] as const) {
      const page = text(
        renderToStaticMarkup(
          <Settings status={{ ...off, platform }} onStatus={noop} onBack={noop} update={controls()} />,
        ),
      );
      expect(page).not.toContain("Check for updates");
    }
  });

  it("checking: busy, announced, the button can't be pressed twice", () => {
    const html = panel({ step: "checking", quiet: false });
    expect(html).toContain('aria-busy="true"');
    expect(html).toMatch(/role="status"[^>]*>.*Checking for updates…/);
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Checking…<\/button>/);
  });

  it("up to date", () => {
    const page = text(panel({ step: "upToDate" }));
    expect(page).toContain("You're up to date");
    expect(page).toContain("MultiPath 1.2.0 is the newest version.");
    expect(page).toContain("Check again");
  });

  it("available: what's new, what installing does, and an explicit choice", () => {
    const html = panel(available);
    const page = text(html);
    expect(page).toContain("MultiPath 1.2.1 is available");
    expect(page).toContain("checks its signature");
    expect(page).toContain("Faster failover.");
    expect(html).toMatch(/<button[^>]*class="primary-button"[^>]*>Install and restart<\/button>/);
    expect(html).toMatch(/<button[^>]*>Not now<\/button>/);
  });

  it("downloading: a labelled progress bar with the percentage", () => {
    const html = panel({ step: "downloading", version: "1.2.1", downloaded: 3_200_000, total: 6_400_000 });
    expect(html).toContain('role="progressbar"');
    expect(html).toContain('aria-valuenow="50"');
    expect(html).toContain('aria-valuetext="50%, 3.2 MB of 6.4 MB"');
    expect(text(html)).toContain("Downloading MultiPath 1.2.1…");
    expect(html).not.toContain("<button");
  });

  it("downloading without a known size: an indeterminate bar", () => {
    const html = panel({ step: "downloading", version: "1.2.1", downloaded: 1_000_000, total: null });
    expect(html).toContain("progress indeterminate");
    expect(html).not.toContain("aria-valuenow");
  });

  it("installing and restarting", () => {
    expect(text(panel({ step: "installing", version: "1.2.1" }))).toContain(
      "Checking the signature and installing…",
    );
    const done = text(panel({ step: "restarting", version: "1.2.1" }));
    expect(done).toContain("MultiPath 1.2.1 is installed");
    expect(done).toContain("Restarting MultiPath…");
  });

  it("errors: an alert, a way forward, and the manual download", () => {
    const html = panel({ step: "error", during: "check", message: "Couldn't reach the update server." });
    expect(html).toMatch(/role="alert"[^>]*>.*Couldn&#x27;t check for updates/);
    expect(html).toMatch(/<button[^>]*>Try again<\/button>/);
    expect(text(html)).toContain("github.com/timowse/MultiPath/releases");
    const restart = panel({ step: "error", during: "restart", message: "Quit and open it again." });
    expect(restart).toMatch(/<button[^>]*>Restart now<\/button>/);
    expect(text(restart)).not.toContain("download MultiPath yourself");
  });

  it("Main: a found update is pointed out, never installed from there", () => {
    const html = renderToStaticMarkup(
      <Main status={off} onStatus={noop} onSettings={noop} onConnections={noop} update={available} />,
    );
    expect(text(html)).toContain("MultiPath 1.2.1 is available.");
    expect(html).toMatch(/<button[^>]*>Review update<\/button>/);
    expect(html).not.toContain("Install and restart");
    expect(text(main(off))).not.toContain("is available");
  });
});

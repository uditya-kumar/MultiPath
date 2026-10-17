import { describe, expect, it, vi } from "vitest";
import type { FoundUpdate, Progress, Updater } from "../api/updater";
import { failureOf, installNote, isBusy, percent, UpdateFlow, type UpdateState, updateErrorText } from "./update";

/** A found update whose install reports the given progress, then succeeds or throws. */
function found(steps: Progress[] = [], error?: Error): FoundUpdate {
  return {
    version: "1.2.1",
    notes: "Fixes",
    date: "2026-11-02T10:00:00Z",
    install: vi.fn(async (onProgress: (p: Progress) => void) => {
      steps.forEach(onProgress);
      if (error) throw error;
    }),
    dismiss: vi.fn(async () => {}),
  };
}

function flowWith(updater: Partial<Updater>) {
  const seen: UpdateState[] = [];
  const full: Updater = { check: async () => null, restart: async () => {}, ...updater };
  const flow = new UpdateFlow(full, (s) => seen.push(s));
  return { flow, seen, steps: () => seen.map((s) => s.step) };
}

describe("UpdateFlow", () => {
  it("up to date: checking, then says so", async () => {
    const { flow, steps } = flowWith({ check: async () => null });
    await flow.check();
    expect(steps()).toEqual(["checking", "upToDate"]);
  });

  it("found: waits for the user, never installs on its own", async () => {
    const update = found();
    const { flow, seen } = flowWith({ check: async () => update });
    await flow.check();
    expect(seen.at(-1)).toEqual({
      step: "available",
      version: "1.2.1",
      notes: "Fixes",
      date: "2026-11-02T10:00:00Z",
    });
    expect(update.install).not.toHaveBeenCalled();
  });

  it("confirmed: downloads with progress, installs, then restarts", async () => {
    const update = found([
      { downloaded: 0, total: 1000, finished: false },
      { downloaded: 600, total: 1000, finished: false },
      { downloaded: 1000, total: 1000, finished: true },
    ]);
    const restart = vi.fn(async () => {});
    const { flow, seen, steps } = flowWith({ check: async () => update, restart });
    await flow.check();
    await flow.install();
    expect(steps()).toEqual([
      "checking",
      "available",
      "downloading",
      "downloading",
      "downloading",
      "installing",
      "restarting",
    ]);
    expect(seen[4]).toMatchObject({ downloaded: 600, total: 1000 });
    expect(restart).toHaveBeenCalledOnce();
  });

  it("a bad signature: nothing installed, no restart, a clear error", async () => {
    const update = found([], new Error("minisign: signature verification failed"));
    const restart = vi.fn(async () => {});
    const { flow, seen } = flowWith({ check: async () => update, restart });
    await flow.check();
    await flow.install();
    expect(seen.at(-1)).toEqual({
      step: "error",
      during: "install",
      failure: "signature",
      detail: "minisign: signature verification failed",
    });
    expect(restart).not.toHaveBeenCalled();
    // A second click can't install the rejected download.
    await flow.install();
    expect(update.install).toHaveBeenCalledOnce();
    expect(update.dismiss).toHaveBeenCalled();
  });

  it("offline: an error with what to do", async () => {
    const { flow, seen } = flowWith({
      check: async () => {
        throw "error sending request for url (https://github.com/uditya-kumar/MultiPath/releases/latest/download/latest.json)";
      },
    });
    await flow.check();
    const last = seen.at(-1) as Extract<UpdateState, { step: "error" }>;
    expect(last).toMatchObject({ step: "error", during: "check", failure: "network" });
    expect(updateErrorText(last, "windows")).toContain("Check that you're online");
  });

  it("the check at start stays quiet unless it finds something", async () => {
    const offline = flowWith({ check: async () => Promise.reject(new Error("offline")) });
    await offline.flow.check(true);
    expect(offline.steps()).toEqual(["checking", "idle"]);
    const latest = flowWith({ check: async () => null });
    await latest.flow.check(true);
    expect(latest.steps()).toEqual(["checking", "idle"]);
    const newer = flowWith({ check: async () => found() });
    await newer.flow.check(true);
    expect(newer.steps()).toEqual(["checking", "available"]);
  });

  it("not now: forgets the update and frees it", async () => {
    const update = found();
    const { flow, steps } = flowWith({ check: async () => update });
    await flow.check();
    await flow.dismiss();
    expect(steps().at(-1)).toBe("idle");
    expect(update.dismiss).toHaveBeenCalledOnce();
    await flow.install();
    expect(update.install).not.toHaveBeenCalled();
  });

  it("a failed restart says how to finish, and can be retried", async () => {
    const restart = vi.fn().mockRejectedValueOnce(new Error("failed to restart app")).mockResolvedValue(undefined);
    const { flow, seen, steps } = flowWith({ check: async () => found(), restart });
    await flow.check();
    await flow.install();
    expect(seen.at(-1)).toMatchObject({ step: "error", during: "restart" });
    await flow.restart();
    expect(steps().slice(-3)).toEqual(["restarting", "error", "restarting"]);
    expect(restart).toHaveBeenCalledTimes(2);
  });

  it("ignores a second check while one is running", async () => {
    let finish: (u: FoundUpdate | null) => void = () => {};
    const check = vi.fn(() => new Promise<FoundUpdate | null>((resolve) => (finish = resolve)));
    const { flow } = flowWith({ check });
    const first = flow.check();
    await flow.check();
    finish(null);
    await first;
    expect(check).toHaveBeenCalledOnce();
  });
});

describe("update helpers", () => {
  it("percent: whole numbers, unknown size is null", () => {
    expect(percent(0, 1000)).toBe(0);
    expect(percent(999, 1000)).toBe(99);
    expect(percent(1200, 1000)).toBe(100);
    expect(percent(500, null)).toBeNull();
    expect(percent(500, 0)).toBeNull();
  });

  it("busy only while something runs", () => {
    expect(isBusy({ step: "checking", quiet: false })).toBe(true);
    expect(isBusy({ step: "installing", version: "1.2.1" })).toBe(true);
    expect(isBusy({ step: "available", version: "1.2.1", notes: null, date: null })).toBe(false);
    expect(isBusy({ step: "error", during: "check", failure: "other", detail: "" })).toBe(false);
  });

  it("errors in plain words", () => {
    type During = "check" | "install" | "restart";
    const text = (err: unknown, during: During, platform: "windows" | "linux" | "macos" = "windows") =>
      updateErrorText({ step: "error", during, ...failureOf(err) }, platform);
    expect(text("Could not fetch a valid release JSON from the remote", "check")).toContain(
      "Couldn't reach the update server",
    );
    expect(text(new Error("operation timed out"), "install")).toContain("nothing was installed");
    expect(
      text("The update was signed for version 1.2.0 but the update endpoint announced version 1.3.0.", "install"),
    ).toContain("security check");
    expect(text("disk full", "install")).toBe("Something went wrong: Disk full.");
  });

  it("says what to do in each system's words", () => {
    const denied = { step: "error" as const, during: "install" as const, ...failureOf("Permission denied (os error 13)") };
    expect(updateErrorText(denied, "macos")).toContain("Applications folder");
    expect(updateErrorText(denied, "windows")).toContain("administrator prompt");
    expect(updateErrorText(denied, "linux")).toContain("AppImage");
    const missing = { step: "error" as const, during: "check" as const, ...failureOf("the platform `darwin-x86_64` was not found in the response") };
    expect(updateErrorText(missing, "macos")).toContain("no update for this Mac");
    expect(updateErrorText(missing, "linux")).toContain("no update for this PC");
    const restart = { step: "error" as const, during: "restart" as const, ...failureOf("failed to restart app") };
    expect(updateErrorText(restart, "macos")).toContain("menu bar");
    expect(updateErrorText(restart, "windows")).toContain("tray icon");
    expect(installNote("windows")).toContain("administrator permission");
    expect(installNote("linux")).toContain("password");
    expect(installNote("macos")).toContain("macOS");
  });
});

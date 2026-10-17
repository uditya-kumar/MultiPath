import type { Platform } from "../api/types";
import type { FoundUpdate, Updater } from "../api/updater";

/** Where the "Check for updates" flow is. Nothing is installed before the user confirms. */
export type UpdateState =
  | { step: "idle" }
  /** `quiet` is the check at start: it only speaks up when there's an update. */
  | { step: "checking"; quiet: boolean }
  | { step: "upToDate" }
  | { step: "available"; version: string; notes: string | null; date: string | null }
  | { step: "downloading"; version: string; downloaded: number; total: number | null }
  /** Downloaded; the signature check and the install are running. */
  | { step: "installing"; version: string }
  | { step: "restarting"; version: string }
  | { step: "error"; during: "check" | "install" | "restart"; failure: UpdateFailure; detail: string };

export const IDLE: UpdateState = { step: "idle" };

/** While these run, buttons that would start another step are disabled. */
export function isBusy(state: UpdateState): boolean {
  return ["checking", "downloading", "installing", "restarting"].includes(state.step);
}

/** Whole percent downloaded, or null while the size is unknown. */
export function percent(downloaded: number, total: number | null): number | null {
  if (!total || total <= 0) return null;
  return Math.min(100, Math.floor((downloaded / total) * 100));
}

/** Why an update step failed, sorted so the popup can say what to do in each system's words. */
export type UpdateFailure = "signature" | "noRelease" | "permission" | "network" | "other";

/** Sorts the updater's technical errors; `detail` keeps the original text for the "other" case. */
export function failureOf(err: unknown): { failure: UpdateFailure; detail: string } {
  const detail = typeof err === "string" ? err : err instanceof Error ? err.message : String(err);
  if (/signature|minisign|signed for version|base64/i.test(detail)) return { failure: "signature", detail };
  if (/platform .* was not found|fallback platforms/i.test(detail)) return { failure: "noRelease", detail };
  if (/permission denied|operation not permitted|access is denied|read-only|authentication failed|cancel/i.test(detail)) {
    return { failure: "permission", detail };
  }
  if (/request|connect|dns|timed? ?out|network|offline|fetch|status code|tls|certificate/i.test(detail)) {
    return { failure: "network", detail };
  }
  return { failure: "other", detail };
}

/** An update error as a sentence and what to do about it, in the running system's words. */
export function updateErrorText(error: Extract<UpdateState, { step: "error" }>, platform: Platform): string {
  if (error.during === "restart") {
    const where = platform === "macos" ? "the menu bar" : "its tray icon";
    return `The update is installed. Quit MultiPath from ${where} and open it again to finish.`;
  }
  switch (error.failure) {
    case "signature":
      return "The download didn't pass the security check, so nothing was installed. Try again later.";
    case "noRelease":
      return `There's no update for this ${platform === "macos" ? "Mac" : "PC"} yet. Try again later.`;
    case "permission":
      switch (platform) {
        case "macos":
          return "MultiPath couldn't replace itself. Move it to the Applications folder, then try again.";
        case "linux":
          return "MultiPath couldn't replace itself. Enter your password when Linux asks, or move the AppImage to a folder you can write to, then try again.";
        default:
          return "MultiPath couldn't replace itself. Allow the administrator prompt, then try again.";
      }
    case "network":
      return error.during === "check"
        ? "Couldn't reach the update server. Check that you're online, then try again."
        : "The download stopped before it finished, so nothing was installed. Try again.";
    case "other": {
      const text = error.detail.charAt(0).toUpperCase() + error.detail.slice(1);
      return `Something went wrong: ${text.replace(/\.?$/, ".")}`;
    }
  }
}

/** What installing will ask for, added to the "available" line. */
export function installNote(platform: Platform): string {
  switch (platform) {
    case "macos":
      return "If MultiPath is on, macOS asks for your password again.";
    case "linux":
      return "Linux may ask for your password.";
    default:
      return "Windows may ask for administrator permission.";
  }
}

/**
 * Runs check → confirm → download, verify and install → restart, one step at a time, and reports
 * every state change. Plain TypeScript so tests can drive it with a fake updater.
 */
export class UpdateFlow {
  state: UpdateState = IDLE;
  private found: FoundUpdate | null = null;
  private installed: string | null = null;

  constructor(
    private readonly updater: Updater,
    private readonly onChange: (state: UpdateState) => void,
  ) {}

  private set(state: UpdateState) {
    this.state = state;
    this.onChange(state);
  }

  /** Asks the update server. A quiet check (at start) stays silent unless it finds one. */
  async check(quiet = false): Promise<void> {
    if (isBusy(this.state)) return;
    this.set({ step: "checking", quiet });
    let found: FoundUpdate | null;
    try {
      found = await this.updater.check();
    } catch (err) {
      this.set(quiet ? IDLE : { step: "error", during: "check", ...failureOf(err) });
      return;
    }
    await this.release();
    this.found = found;
    if (found) {
      this.set({ step: "available", version: found.version, notes: found.notes, date: found.date });
    } else {
      this.set(quiet ? IDLE : { step: "upToDate" });
    }
  }

  /** The user said yes: download, verify, install, then restart. */
  async install(): Promise<void> {
    const found = this.found;
    if (!found || this.state.step !== "available") return;
    const version = found.version;
    this.set({ step: "downloading", version, downloaded: 0, total: null });
    try {
      await found.install(({ downloaded, total, finished }) => {
        this.set(
          finished ? { step: "installing", version } : { step: "downloading", version, downloaded, total },
        );
      });
    } catch (err) {
      // Start over from a fresh check rather than retrying a half-used update.
      await this.release();
      this.set({ step: "error", during: "install", ...failureOf(err) });
      return;
    }
    this.found = null;
    this.installed = version;
    await this.restart();
  }

  /** Restarts into the installed version; also the retry when a restart failed. */
  async restart(): Promise<void> {
    if (!this.installed) return;
    this.set({ step: "restarting", version: this.installed });
    try {
      await this.updater.restart();
    } catch (err) {
      this.set({ step: "error", during: "restart", ...failureOf(err) });
    }
  }

  /** "Not now": forget the update until the next check. */
  async dismiss(): Promise<void> {
    if (this.state.step !== "available") return;
    await this.release();
    this.set(IDLE);
  }

  private async release() {
    const found = this.found;
    this.found = null;
    // Only frees memory on the Rust side; failing to is harmless.
    await found?.dismiss().catch(() => {});
  }
}

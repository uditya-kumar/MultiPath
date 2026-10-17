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
  | { step: "error"; during: "check" | "install" | "restart"; message: string };

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

/** Turns the updater's technical errors into a sentence and what to do about it. */
export function updateErrorText(err: unknown, during: "check" | "install" | "restart"): string {
  const raw = typeof err === "string" ? err : err instanceof Error ? err.message : String(err);
  if (during === "restart") {
    return "The update is installed. Quit MultiPath from the menu bar and open it again to finish.";
  }
  if (/signature|minisign|signed for version|base64/i.test(raw)) {
    return "The download didn't pass the security check, so nothing was installed. Try again later.";
  }
  if (/platform .* was not found|fallback platforms/i.test(raw)) {
    return "There's no update for this Mac yet. Try again later.";
  }
  if (/permission denied|operation not permitted|read-only|authentication failed|cancel/i.test(raw)) {
    return "MultiPath couldn't replace itself. Move it to the Applications folder, then try again.";
  }
  if (/request|connect|dns|timed? ?out|network|offline|fetch|status code|tls|certificate/i.test(raw)) {
    return during === "check"
      ? "Couldn't reach the update server. Check that you're online, then try again."
      : "The download stopped before it finished, so nothing was installed. Try again.";
  }
  const text = raw.charAt(0).toUpperCase() + raw.slice(1);
  return `Something went wrong: ${text.replace(/\.?$/, ".")}`;
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
      this.set(quiet ? IDLE : { step: "error", during: "check", message: updateErrorText(err, "check") });
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
      this.set({ step: "error", during: "install", message: updateErrorText(err, "install") });
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
      this.set({ step: "error", during: "restart", message: updateErrorText(err, "restart") });
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

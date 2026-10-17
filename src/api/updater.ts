import { relaunch } from "@tauri-apps/plugin-process";
import { check, type DownloadEvent } from "@tauri-apps/plugin-updater";

/** Where people can always get MultiPath by hand. */
export const RELEASES_PAGE = "github.com/timowse/MultiPath/releases";

/** How far a download has got. `total` is null when the server doesn't say. */
export interface Progress {
  downloaded: number;
  total: number | null;
  /** All bytes are in; the signature check and the install come next. */
  finished: boolean;
}

/** A newer release the update server announced. Nothing is downloaded yet. */
export interface FoundUpdate {
  version: string;
  notes: string | null;
  date: string | null;
  /**
   * Downloads the update, verifies its signature against the public key built into the app, then
   * installs it. A download whose signature doesn't verify is rejected and never installed.
   */
  install(onProgress: (progress: Progress) => void): Promise<void>;
  /** Releases the update without installing it. */
  dismiss(): Promise<void>;
}

/** What the popup needs from the updater; the demo page and tests pass fakes. */
export interface Updater {
  /** The newer release, or null when this is the latest. */
  check(): Promise<FoundUpdate | null>;
  /** Restarts into the version just installed. */
  restart(): Promise<void>;
}

const CHECK_TIMEOUT_MS = 30_000;
// Long enough for a slow phone tether, short enough that a stalled download ends in an error.
const DOWNLOAD_TIMEOUT_MS = 10 * 60_000;

/** The Tauri updater: the endpoint, public key and HTTPS-only rule come from tauri.conf.json. */
export const appUpdater: Updater = {
  async check() {
    const update = await check({ timeout: CHECK_TIMEOUT_MS });
    if (!update) return null;
    return {
      version: update.version,
      notes: update.body?.trim() || null,
      date: update.date ?? null,
      install(onProgress) {
        let downloaded = 0;
        let total: number | null = null;
        return update.downloadAndInstall(
          (event: DownloadEvent) => {
            if (event.event === "Started") total = event.data.contentLength ?? null;
            if (event.event === "Progress") downloaded += event.data.chunkLength;
            onProgress({ downloaded, total, finished: event.event === "Finished" });
          },
          { timeout: DOWNLOAD_TIMEOUT_MS },
        );
      },
      dismiss: () => update.close(),
    };
  },
  restart: relaunch,
};

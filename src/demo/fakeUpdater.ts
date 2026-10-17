import type { FoundUpdate, Updater } from "../api/updater";

/**
 * What the made-up update server answers on the demo page (&update=…). Nothing here touches the
 * network: the e2e tests drive the whole flow against it.
 */
export type UpdateScenario = "latest" | "available" | "offline" | "badSignature" | "restartFails";

export const scenarios: UpdateScenario[] = ["latest", "available", "offline", "badSignature", "restartFails"];

const SIZE = 6_400_000;
const CHUNKS = 8;
const STEP_MS = 120;

const wait = () => new Promise((resolve) => setTimeout(resolve, STEP_MS));

export function fakeUpdater(scenario: UpdateScenario): Updater {
  const found: FoundUpdate = {
    version: "1.2.1",
    notes: "Faster failover when a phone is unplugged.\nClearer error messages.",
    date: "2026-11-02T10:00:00Z",
    async install(onProgress) {
      onProgress({ downloaded: 0, total: SIZE, finished: false });
      for (let i = 1; i <= CHUNKS; i++) {
        await wait();
        onProgress({ downloaded: (SIZE / CHUNKS) * i, total: SIZE, finished: false });
      }
      onProgress({ downloaded: SIZE, total: SIZE, finished: true });
      // Signature check and install: long enough to be seen.
      await new Promise((resolve) => setTimeout(resolve, 8 * STEP_MS));
      if (scenario === "badSignature") {
        throw new Error("The signature verification failed: signature mismatch");
      }
    },
    dismiss: async () => {},
  };

  return {
    async check() {
      await wait();
      if (scenario === "offline") throw new Error("error sending request for url (https://github.com/…)");
      return scenario === "latest" ? null : found;
    },
    async restart() {
      await wait();
      if (scenario === "restartFails") throw new Error("failed to restart app");
      // The real app would be gone now; the demo page stays on "Restarting…".
      await new Promise(() => {});
    },
  };
}

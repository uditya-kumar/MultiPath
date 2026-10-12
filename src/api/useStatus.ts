import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getStatus } from "./commands";
import type { Status } from "./types";

/**
 * The latest status: fetched when the popup opens, then pushed by the
 * backend's ticker once a second while it stays open.
 */
export function useStatus(): [Status | null, (status: Status) => void] {
  const [status, setStatus] = useState<Status | null>(null);

  useEffect(() => {
    let alive = true;
    const refresh = () =>
      getStatus()
        .then((s) => alive && setStatus(s))
        .catch(() => {});
    refresh();
    const unlisten = listen<Status>("status", (e) => alive && setStatus(e.payload));
    // The popup is hidden, not closed, so catch up whenever it's shown again.
    window.addEventListener("focus", refresh);
    return () => {
      alive = false;
      void unlisten.then((stop) => stop());
      window.removeEventListener("focus", refresh);
    };
  }, []);

  return [status, setStatus];
}

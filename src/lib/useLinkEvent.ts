import { useEffect, useRef, useState } from "react";
import type { Status } from "../api/types";
import { linkChange } from "./hero";

/** How long a "network dropped out" message stays up. */
const HOLD_MS = 20_000;

/**
 * The latest change worth telling the user about ("iPhone USB disconnected —
 * new connections moved to Wi-Fi."), for a while after it happened.
 */
export function useLinkEvent(status: Status): string | null {
  const previous = useRef<Status | null>(null);
  const [event, setEvent] = useState<string | null>(null);

  useEffect(() => {
    const before = previous.current;
    previous.current = status;
    if (!status.on) {
      setEvent(null);
      return;
    }
    const change = before && linkChange(before, status);
    if (change) setEvent(change);
  }, [status]);

  useEffect(() => {
    if (!event) return;
    const timer = window.setTimeout(() => setEvent(null), HOLD_MS);
    return () => window.clearTimeout(timer);
  }, [event]);

  return event;
}

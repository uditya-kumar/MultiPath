import { useEffect, useMemo, useRef, useState } from "react";
import type { Updater } from "../api/updater";
import { IDLE, UpdateFlow, type UpdateState } from "./update";

export interface UpdateControls {
  state: UpdateState;
  check: () => void;
  install: () => void;
  dismiss: () => void;
  restart: () => void;
}

/**
 * The update flow for the whole popup, so it survives switching pages. With `checkAtStart` it
 * looks once, quietly, as soon as the first status says so; it never installs on its own.
 */
export function useUpdater(updater: Updater, checkAtStart: boolean | null): UpdateControls {
  const [state, setState] = useState<UpdateState>(IDLE);
  const flow = useMemo(() => new UpdateFlow(updater, setState), [updater]);
  const decided = useRef(false);

  useEffect(() => {
    // null: the first status hasn't arrived yet.
    if (checkAtStart === null || decided.current) return;
    decided.current = true;
    if (checkAtStart) void flow.check(true);
  }, [checkAtStart, flow]);

  return useMemo(
    () => ({
      state,
      check: () => void flow.check(),
      install: () => void flow.install(),
      dismiss: () => void flow.dismiss(),
      restart: () => void flow.restart(),
    }),
    [state, flow],
  );
}

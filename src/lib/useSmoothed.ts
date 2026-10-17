import { useEffect, useRef, useState } from "react";

/**
 * Eases a number toward its latest value, so once-a-second rates
 * count up and down instead of jumping.
 */
export function useSmoothed(target: number, ms = 400): number {
  const [value, setValue] = useState(target);
  const from = useRef(target);

  useEffect(() => {
    const start = performance.now();
    const begin = from.current;
    let frame = requestAnimationFrame(function step(now) {
      const t = Math.min(1, (now - start) / ms);
      const eased = 1 - (1 - t) ** 3;
      const next = begin + (target - begin) * eased;
      from.current = next;
      setValue(next);
      if (t < 1) frame = requestAnimationFrame(step);
    });
    return () => cancelAnimationFrame(frame);
  }, [target, ms]);

  return value;
}

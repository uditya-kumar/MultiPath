import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

/** Keeps the popup window exactly as tall as its content, anchored above the tray. */
export function useFitWindow<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let last = 0;
    const observer = new ResizeObserver(() => {
      const height = Math.ceil(el.getBoundingClientRect().height);
      if (height !== last) {
        last = height;
        void invoke("fit_popup", { height });
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  return ref;
}

/** Hides the popup back into the tray. */
export function hidePopup() {
  return invoke("hide_popup");
}

import { useCallback, useState } from "react";

const KEY = "sheafly.inspector.open";

function readOpen(): boolean {
  try {
    return window.localStorage.getItem(KEY) !== "false";
  } catch {
    return true;
  }
}

/** Whether the inspector is expanded; a per-window preference kept across launches. */
export function useInspectorOpen(): [boolean, () => void] {
  const [open, setOpen] = useState(readOpen);
  const toggle = useCallback(() => {
    setOpen((current) => {
      const next = !current;
      try {
        window.localStorage.setItem(KEY, String(next));
      } catch {
        // Storage can be unavailable; the panel still toggles for this session.
      }
      return next;
    });
  }, []);
  return [open, toggle];
}

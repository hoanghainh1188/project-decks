import { useEffect, useRef, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";

import { useMessages } from "../../i18n";
import { paneSplitterMessages } from "./PaneSplitter.i18n";
import { clampShare, DEFAULT_SHARE, MAX_SHARE } from "./sheetShare";

const STEP = 0.02;
const BIG_STEP = 0.1;

/**
 * The bar between the task sheet and the Gantt. Dragging sets the sheet's share of the
 * parent's width; arrows move it by 2% (10% with Shift), End to the widest sheet, Home or a
 * double click back to the default.
 * Follows the WAI-ARIA window splitter pattern.
 */
export function PaneSplitter({ share, onChange }: { share: number; onChange: (share: number) => void }) {
  const m = useMessages(paneSplitterMessages);
  const ref = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);
  const latest = useRef(onChange);
  latest.current = onChange;

  useEffect(() => {
    const move = (event: PointerEvent) => {
      const parent = ref.current?.parentElement;
      if (!dragging.current || !parent) return;
      // The button was let go where this window could not see it (another app, the Dock).
      if (event.buttons === 0) return stop();
      const box = parent.getBoundingClientRect();
      if (box.width <= 0) return;
      latest.current(clampShare(round((event.clientX - box.left) / box.width)));
    };
    const stop = () => {
      dragging.current = false;
      document.body.classList.remove("pane-resizing");
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
    window.addEventListener("pointercancel", stop);
    window.addEventListener("blur", stop);
    return () => {
      window.removeEventListener("blur", stop);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
      stop();
    };
  }, []);

  const start = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    dragging.current = true;
    document.body.classList.add("pane-resizing");
  };

  const key = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = event.shiftKey ? BIG_STEP : STEP;
    const next =
      event.key === "ArrowLeft"
        ? share - step
        : event.key === "ArrowRight"
          ? share + step
          : event.key === "Home"
            ? DEFAULT_SHARE
            : event.key === "End"
              ? MAX_SHARE
              : null;
    if (next === null) return;
    event.preventDefault();
    onChange(clampShare(round(next)));
  };

  return (
    <div
      ref={ref}
      className="pane-splitter"
      role="separator"
      aria-orientation="vertical"
      aria-label={m.label}
      aria-valuemin={15}
      aria-valuemax={85}
      aria-valuenow={Math.round(share * 100)}
      title={m.hint}
      tabIndex={0}
      onPointerDown={start}
      onKeyDown={key}
      onDoubleClick={() => onChange(DEFAULT_SHARE)}
    />
  );
}

/** Whole tenths of a percent, so stored values stay short and steps add up exactly. */
function round(share: number): number {
  return Math.round(share * 1000) / 1000;
}

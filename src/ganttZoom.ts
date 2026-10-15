import { pixelsPerDay } from "./ganttLayout";
import type { Scale } from "./types";

/** The narrowest and widest a day column can be, whatever the scale. */
const MIN_DAY_PX = 2;
const MAX_DAY_PX = 80;
/** One zoom step (menu, keys): a quarter wider, or back. */
const STEP = 1.25;
/** How far one unit of wheel delta zooms; a trackpad pinch sends small deltas. */
const WHEEL_RATE = 0.01;

/** Width of a day column: the scale's own width times the zoom. */
export function zoomedPx(scale: Scale, zoom: number): number {
  return pixelsPerDay(scale) * zoom;
}

export function clampZoom(scale: Scale, zoom: number): number {
  if (!Number.isFinite(zoom)) return 1;
  const base = pixelsPerDay(scale);
  return Math.min(MAX_DAY_PX / base, Math.max(MIN_DAY_PX / base, zoom));
}

/** One step in (1) or out (-1); steps are powers of 1.25, so they come back to 1. */
export function stepZoom(scale: Scale, zoom: number, direction: 1 | -1): number {
  const steps = Math.round(Math.log(zoom) / Math.log(STEP)) + direction;
  return clampZoom(scale, Number((STEP ** steps).toPrecision(6)));
}

/** The most one wheel event zooms by, in pixels of delta: a mouse notch sends about 100. */
const MAX_WHEEL_DELTA = 25;

/**
 * Wheel delta in pixels, whatever unit the event uses (lines, pages), clamped so a mouse
 * notch is one modest step while a pinch's small deltas stay smooth.
 */
export function wheelPixels(event: Pick<WheelEvent, "deltaY" | "deltaMode">): number {
  const scale = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? 400 : 1;
  return Math.max(-MAX_WHEEL_DELTA, Math.min(MAX_WHEEL_DELTA, event.deltaY * scale));
}

/** Zoom after a Cmd/Ctrl wheel or a pinch: a negative delta (up, pinch out) zooms in. */
export function wheelZoom(scale: Scale, zoom: number, deltaY: number): number {
  if (deltaY === 0) return zoom;
  return clampZoom(scale, zoom * Math.exp(-deltaY * WHEEL_RATE));
}

/**
 * The horizontal scroll that keeps the date `anchorX` px into the view where it is when a
 * day goes from `oldPx` to `newPx` wide.
 */
export function anchoredScroll(scrollLeft: number, anchorX: number, oldPx: number, newPx: number): number {
  return Math.max(0, ((scrollLeft + anchorX) * newPx) / oldPx - anchorX);
}

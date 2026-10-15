/** The part of the width the task sheet takes beside the Gantt; the rest is the chart. */
export const DEFAULT_SHARE = 0.52;
const MIN_SHARE = 0.15;
export const MAX_SHARE = 0.85;
const KEY = "sheafly.sheet.share";

export function clampShare(share: number): number {
  if (!Number.isFinite(share)) return DEFAULT_SHARE;
  return Math.min(MAX_SHARE, Math.max(MIN_SHARE, share));
}

/** The share kept from an earlier launch; a per-window preference, like the inspector. */
export function readShare(): number {
  try {
    const stored = window.localStorage.getItem(KEY);
    return stored == null ? DEFAULT_SHARE : clampShare(Number(stored));
  } catch {
    return DEFAULT_SHARE;
  }
}

export function saveShare(share: number): void {
  try {
    window.localStorage.setItem(KEY, String(share));
  } catch {
    // Storage can be unavailable; the split still moves for this session.
  }
}

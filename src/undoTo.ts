import type { Snapshot } from "./types";

export interface History {
  /** The current state, after every queued edit has landed. */
  current: () => Promise<Snapshot>;
  undo: () => Promise<Snapshot>;
}

/**
 * Undo every command applied since `cursor` and return the state reached. Stops early
 * if undo stops moving the cursor (nothing left to undo), so it can never spin.
 */
export async function undoTo(cursor: number, history: History): Promise<Snapshot> {
  let state = await history.current();
  while (state.history_cursor > cursor) {
    const next = await history.undo();
    if (next.history_cursor >= state.history_cursor) return next;
    state = next;
  }
  return state;
}

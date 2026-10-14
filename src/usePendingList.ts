import { useRef } from "react";

import type { Snapshot } from "./types";

/**
 * A list the engine holds and the UI replaces whole, one command per change. Each change
 * starts from the list sent last, not the one last drawn, so two quick clicks before the
 * engine answers both count. A new `stored` list takes over only once no change is on
 * its way; a refused change goes back to what the engine holds.
 */
export function usePendingList<T>(
  stored: T[],
  send: (next: T[]) => Promise<Snapshot | void>,
): { latest: () => T[]; apply: (next: T[]) => Promise<void>; settled: () => Promise<boolean> } {
  const latest = useRef(stored);
  const seen = useRef(stored);
  const pending = useRef(0);
  const inFlight = useRef(new Set<Promise<void>>());
  const refused = useRef(false);
  if (seen.current !== stored) {
    seen.current = stored;
    if (pending.current === 0) latest.current = stored;
  }
  function apply(next: T[]): Promise<void> {
    latest.current = next;
    pending.current += 1;
    const sent = (async () => {
      try {
        if (!(await send(next))) {
          latest.current = seen.current;
          refused.current = true;
        }
      } finally {
        pending.current -= 1;
      }
    })();
    inFlight.current.add(sent);
    void sent.finally(() => inFlight.current.delete(sent));
    return sent;
  }
  /** Once every change sent so far is answered: whether none was refused since last asked. */
  async function settled(): Promise<boolean> {
    while (inFlight.current.size > 0) await Promise.all([...inFlight.current]);
    const ok = !refused.current;
    refused.current = false;
    return ok;
  }
  return { latest: () => latest.current, apply, settled };
}

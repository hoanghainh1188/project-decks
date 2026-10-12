import { describe, expect, it, vi } from "vitest";

import { makeProject, makeSnapshot } from "./test/fixtures";
import { undoTo } from "./undoTo";

const at = (history_cursor: number) => ({ ...makeSnapshot(makeProject()), history_cursor });

describe("undoTo", () => {
  it("undoes back to the cursor the dialog opened with", async () => {
    let cursor = 7;
    const undo = vi.fn(async () => at(--cursor));
    const state = await undoTo(4, { current: async () => at(cursor), undo });
    expect(undo).toHaveBeenCalledTimes(3);
    expect(state.history_cursor).toBe(4);
  });

  it("does nothing when nothing changed", async () => {
    const undo = vi.fn(async () => at(0));
    const state = await undoTo(4, { current: async () => at(4), undo });
    expect(undo).not.toHaveBeenCalled();
    expect(state.history_cursor).toBe(4);
  });

  it("stops if undo no longer moves", async () => {
    const undo = vi.fn(async () => at(5));
    const state = await undoTo(2, { current: async () => at(5), undo });
    expect(undo).toHaveBeenCalledOnce();
    expect(state.history_cursor).toBe(5);
  });
});

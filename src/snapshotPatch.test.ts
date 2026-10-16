import { describe, expect, it, vi } from "vitest";

import { applyListPatch, applySnapshotPatch, SnapshotCache, type SnapshotPatch } from "./snapshotPatch";
import { makeProject, makeSnapshot, makeTask } from "./test/fixtures";
import type { Snapshot } from "./types";

const key = (item: { uid: number }) => item.uid;

function base(): Snapshot {
  return makeSnapshot(makeProject({ tasks: [1, 2, 3].map((uid) => makeTask({ uid, name: `T${uid}` })) }));
}

function patchOf(previous: Snapshot, rev: number, changes: Partial<SnapshotPatch["project"]> = {}): SnapshotPatch {
  const empty = { changed: [], removed: [], order: null };
  const { tasks: _t, resources: _r, assignments: _a, dependencies: _d, usage: _u, ...rest } = previous.project;
  return {
    ...previous,
    patch: true,
    base: rev - 1,
    rev,
    project: { ...rest, tasks: empty, resources: empty, assignments: empty, dependencies: empty, usage: empty, ...changes },
  } as SnapshotPatch;
}

describe("applyListPatch", () => {
  const list = [{ uid: 1, v: "a" }, { uid: 2, v: "b" }, { uid: 3, v: "c" }];

  it("replaces changed items in place and keeps the others as they were", () => {
    const next = applyListPatch(list, { changed: [{ uid: 2, v: "B" }], removed: [], order: null }, key);
    expect(next.map((item) => item.v)).toEqual(["a", "B", "c"]);
    expect(next[0]).toBe(list[0]);
  });

  it("follows the new order, adding new items and leaving removed ones out", () => {
    const next = applyListPatch(list, { changed: [{ uid: 4, v: "d" }], removed: [2], order: [3, 4, 1] }, key);
    expect(next.map((item) => item.uid)).toEqual([3, 4, 1]);
  });
});

describe("SnapshotCache", () => {
  it("applies a patch on the snapshot it is based on", async () => {
    const cache = new SnapshotCache();
    const whole = vi.fn();
    const first = base();
    await cache.receive({ ...first, rev: 1 }, whole);
    const renamed = makeTask({ uid: 2, name: "Đổi tên" });
    const next = await cache.receive(
      patchOf({ ...first, can_undo: true }, 2, { tasks: { changed: [renamed], removed: [], order: null }, name: "Mới" }),
      whole,
    );
    expect(whole).not.toHaveBeenCalled();
    expect(next.project.tasks.map((task) => task.name)).toEqual(["T1", "Đổi tên", "T3"]);
    expect(next.project.tasks[0]).toBe(first.project.tasks[0]);
    expect(next.project.name).toBe("Mới");
    expect(next.can_undo).toBe(true);
    expect(next).not.toHaveProperty("patch");
    expect(next).not.toHaveProperty("rev");
    // A save changes nothing in the project: the same object comes back.
    const saved = applySnapshotPatch(next, patchOf({ ...next, dirty: false }, 3));
    expect(saved.project).toBe(next.project);
    expect(saved.dirty).toBe(false);
  });

  it("asks for the whole snapshot when it holds another base", async () => {
    const cache = new SnapshotCache();
    const fresh = { ...base(), rev: 7 };
    const whole = vi.fn(async () => fresh);
    // An open answers whole without a number, so no patch can build on it.
    await cache.receive(base(), whole);
    const next = await cache.receive(patchOf(base(), 2), whole);
    expect(whole).toHaveBeenCalledTimes(1);
    expect(next.project.tasks).toHaveLength(3);
    const after = await cache.receive(patchOf(base(), 8), whole);
    expect(whole).toHaveBeenCalledTimes(1);
    expect(after.project.tasks).toHaveLength(3);
  });
});

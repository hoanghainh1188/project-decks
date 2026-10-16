import type { Assignment, Dependency, Project, Resource, Snapshot, Task, UsageRow } from "./types";

/**
 * Mirror of `src-tauri/src/delta.rs`. After an edit, undo or redo the backend sends only
 * what changed in the project's large lists since the snapshot it sent last (`base`);
 * every other project field comes whole.
 */
export interface ListPatch<T, K> {
  changed: T[];
  removed: K[];
  /** Every key in the new order, when the order or the set of keys changed. */
  order: K[] | null;
}

type PatchedLists = {
  tasks: ListPatch<Task, number>;
  resources: ListPatch<Resource, number>;
  assignments: ListPatch<Assignment, number>;
  dependencies: ListPatch<Dependency, number>;
  usage: ListPatch<UsageRow, string>;
};

export interface SnapshotPatch extends Omit<Snapshot, "project"> {
  patch: true;
  base: number;
  rev: number;
  project: Omit<Project, keyof PatchedLists> & PatchedLists;
}

/** A whole snapshot; `rev` is set when the backend will base its next patch on it. */
export type FullSnapshot = Snapshot & { rev?: number };

export type SnapshotAnswer = FullSnapshot | SnapshotPatch;

export function isPatch(answer: SnapshotAnswer): answer is SnapshotPatch {
  return (answer as SnapshotPatch).patch === true;
}

/** The key `delta.rs` gives a usage row: its task, and its assignment if any. */
export function usageKey(row: UsageRow): string {
  return `${row.task_uid}:${row.assignment_uid ?? ""}`;
}

/** `previous` with `patch` applied; items that did not change keep their identity. */
export function applyListPatch<T, K>(previous: T[], patch: ListPatch<T, K>, key: (item: T) => K): T[] {
  const removed = new Set(patch.removed);
  const changed = new Map(patch.changed.map((item) => [key(item), item]));
  if (patch.order == null) {
    return previous.map((item) => changed.get(key(item)) ?? item);
  }
  const kept = new Map<K, T>();
  for (const item of previous) {
    const itemKey = key(item);
    if (!removed.has(itemKey)) kept.set(itemKey, item);
  }
  return patch.order.flatMap((itemKey) => {
    const item = changed.get(itemKey) ?? kept.get(itemKey);
    return item === undefined ? [] : [item];
  });
}

/** The snapshot `patch` describes, built on `previous` (the snapshot numbered `patch.base`). */
export function applySnapshotPatch(previous: Snapshot, patch: SnapshotPatch): Snapshot {
  const { patch: _patch, base: _base, rev: _rev, project, ...rest } = patch;
  const old = previous.project;
  // Nothing in the project changed (a save): keep the same object, so views and the
  // finish forecast that follow the project by identity do not redo their work.
  if (unchanged(old, project)) return { ...rest, project: old };
  return {
    ...rest,
    project: {
      ...project,
      tasks: applyListPatch(old.tasks, project.tasks, (task) => task.uid),
      resources: applyListPatch(old.resources, project.resources, (item) => item.uid),
      assignments: applyListPatch(old.assignments, project.assignments, (item) => item.uid),
      dependencies: applyListPatch(old.dependencies, project.dependencies, (item) => item.uid),
      usage: applyListPatch(old.usage, project.usage, usageKey),
    },
  };
}

const LISTS = ["tasks", "resources", "assignments", "dependencies", "usage"] as const;

function unchanged(old: Project, project: SnapshotPatch["project"]): boolean {
  if (LISTS.some((name) => project[name].changed.length > 0 || project[name].removed.length > 0 || project[name].order != null)) {
    return false;
  }
  const small = (value: object) =>
    JSON.stringify(
      Object.entries(value)
        .filter(([name]) => !(LISTS as readonly string[]).includes(name))
        .sort(([a], [b]) => (a < b ? -1 : 1)),
    );
  return small(old) === small(project);
}

/**
 * The snapshot the frontend holds and its number. A whole snapshot without a number (an
 * open, a save) leaves none, so the next patch, based on an older one, asks for the whole.
 */
export class SnapshotCache {
  private held: { rev: number | null; snapshot: Snapshot } | null = null;

  /** The snapshot an answer stands for; `whole` fetches it when a patch cannot apply. */
  async receive(answer: SnapshotAnswer, whole: () => Promise<FullSnapshot>): Promise<Snapshot> {
    if (!isPatch(answer)) return this.keep(answer);
    if (this.held && this.held.rev === answer.base) {
      const snapshot = applySnapshotPatch(this.held.snapshot, answer);
      this.held = { rev: answer.rev, snapshot };
      return snapshot;
    }
    return this.keep(await whole());
  }

  private keep(answer: FullSnapshot): Snapshot {
    const { rev, ...snapshot } = answer;
    this.held = { rev: rev ?? null, snapshot };
    return snapshot;
  }
}

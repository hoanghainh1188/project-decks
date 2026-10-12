import { pick } from "./i18n";
import type { Assignment, Project, Resource, Task } from "./types";
import { viewRowsMessages } from "./viewRows.i18n";

export type FilterMode = "none" | "critical" | "resource" | "late";
export type GroupMode = "none" | "resource" | "summary";
export type HighlightMode = "none" | "critical" | "late";

export interface TaskViewRow {
  kind: "task";
  key: string;
  task: Task;
  highlighted: boolean;
}

export interface GroupViewRow {
  kind: "group";
  key: string;
  label: string;
}

export type ViewRow = TaskViewRow | GroupViewRow;

/**
 * Filter hides non-matching rows but keeps a summary when any descendant matches.
 * Highlight only tints rows that remain visible. Group then reorders those rows.
 * None of this rewrites WBS codes or the saved task order. Group headers it names itself
 * are in the current interface language.
 */
export function buildViewRows(
  project: Project,
  filter: FilterMode,
  resourceUid: number | null,
  group: GroupMode,
  highlight: HighlightMode,
): ViewRow[] {
  const visible = visibleTasks(project, filter, resourceUid);
  if (group === "resource") return groupByResource(project, visible, highlight);
  if (group === "summary") return groupBySummary(project, visible, highlight);
  return visible.map((task) => taskRow(`task-${task.uid}`, task, highlight));
}

function visibleTasks(project: Project, filter: FilterMode, resourceUid: number | null): Task[] {
  if (filter === "none") return project.tasks;
  const byUid = new Map(project.tasks.map((task) => [task.uid, task]));
  const direct = new Set<number>();
  for (const task of project.tasks) {
    if (matchesFilter(task, project.assignments, filter, resourceUid)) direct.add(task.uid);
  }
  const keep = new Set(direct);
  for (const uid of direct) {
    let parent = byUid.get(uid)?.parent_uid ?? null;
    while (parent != null) {
      const ancestor = byUid.get(parent);
      if (!ancestor) break;
      if (ancestor.summary) keep.add(ancestor.uid);
      parent = ancestor.parent_uid;
    }
  }
  return project.tasks.filter((task) => keep.has(task.uid));
}

function matchesFilter(
  task: Task,
  assignments: Assignment[],
  filter: FilterMode,
  resourceUid: number | null,
): boolean {
  if (filter === "critical") return task.critical;
  if (filter === "late") return task.late;
  if (filter === "resource") {
    if (resourceUid == null) return false;
    return assignments.some((item) => item.task_uid === task.uid && item.resource_uid === resourceUid);
  }
  return true;
}

function matchesHighlight(task: Task, highlight: HighlightMode): boolean {
  if (highlight === "critical") return task.critical;
  if (highlight === "late") return task.late;
  return false;
}

function taskRow(key: string, task: Task, highlight: HighlightMode): TaskViewRow {
  return { kind: "task", key, task, highlighted: matchesHighlight(task, highlight) };
}

function groupByResource(project: Project, tasks: Task[], highlight: HighlightMode): ViewRow[] {
  const buckets = new Map<number, Task[]>();
  const unassigned: Task[] = [];
  for (const task of tasks) {
    const ids = assignedResourceIds(task, project.resources, project.assignments);
    if (ids.length === 0) {
      unassigned.push(task);
      continue;
    }
    for (const id of ids) {
      const list = buckets.get(id) ?? [];
      list.push(task);
      buckets.set(id, list);
    }
  }
  const rows: ViewRow[] = [];
  for (const resource of project.resources) {
    const list = buckets.get(resource.uid);
    if (!list || list.length === 0) continue;
    rows.push({
      kind: "group",
      key: `group-${resource.uid}`,
      label: resource.name.trim() || pick(viewRowsMessages).resourceFallback(resource.uid),
    });
    for (const task of list) {
      rows.push(taskRow(`group-${resource.uid}-task-${task.uid}`, task, highlight));
    }
  }
  if (unassigned.length > 0) {
    rows.push({ kind: "group", key: "group-unassigned", label: pick(viewRowsMessages).unassigned });
    for (const task of unassigned) {
      rows.push(taskRow(`group-unassigned-task-${task.uid}`, task, highlight));
    }
  }
  return rows;
}

/**
 * Leaf tasks sit under their nearest summary ancestor; summaries are headers only.
 * Groups follow the summary's outline position, and top-level leaves go last.
 */
function groupBySummary(project: Project, tasks: Task[], highlight: HighlightMode): ViewRow[] {
  const byUid = new Map(project.tasks.map((task) => [task.uid, task]));
  const buckets = new Map<number, Task[]>();
  const orphans: Task[] = [];
  for (const task of tasks) {
    if (task.summary) continue;
    const owner = nearestSummary(task, byUid);
    if (!owner) {
      orphans.push(task);
      continue;
    }
    const list = buckets.get(owner.uid) ?? [];
    list.push(task);
    buckets.set(owner.uid, list);
  }
  const rows: ViewRow[] = [];
  for (const summary of project.tasks) {
    const list = buckets.get(summary.uid);
    if (!list || list.length === 0) continue;
    rows.push({ kind: "group", key: `group-summary-${summary.uid}`, label: summaryLabel(summary) });
    for (const task of list) {
      rows.push(taskRow(`group-summary-${summary.uid}-task-${task.uid}`, task, highlight));
    }
  }
  if (orphans.length > 0) {
    rows.push({ kind: "group", key: "group-no-summary", label: pick(viewRowsMessages).noSummary });
    for (const task of orphans) {
      rows.push(taskRow(`group-no-summary-task-${task.uid}`, task, highlight));
    }
  }
  return rows;
}

function nearestSummary(task: Task, byUid: Map<number, Task>): Task | null {
  let parent = task.parent_uid;
  while (parent != null) {
    const ancestor = byUid.get(parent);
    if (!ancestor) return null;
    if (ancestor.summary) return ancestor;
    parent = ancestor.parent_uid;
  }
  return null;
}

function summaryLabel(summary: Task): string {
  const name = summary.name.trim() || pick(viewRowsMessages).taskFallback(summary.uid);
  return summary.wbs ? `${summary.wbs} ${name}` : name;
}

function assignedResourceIds(task: Task, resources: Resource[], assignments: Assignment[]): number[] {
  return resources
    .filter((resource) =>
      assignments.some((item) => item.task_uid === task.uid && item.resource_uid === resource.uid),
    )
    .map((resource) => resource.uid);
}

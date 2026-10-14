import { instantToDateKey } from "./format";
import type { Project, Task } from "./types";

const DAY_MS = 86_400_000;

/** Where the span ends: the end of a day, or the finish of a milestone. */
export type SpanEnd = { kind: "date"; date: string } | { kind: "milestone"; uid: number };

/**
 * The instant the span ends at, or null when it names nothing (an empty or bad date, a
 * milestone that is gone). A date ends at midnight after it, so a task finishing that
 * day fits.
 */
export function spanEndInstant(project: Project, end: SpanEnd): number | null {
  if (end.kind === "milestone") return project.tasks.find((task) => task.uid === end.uid && task.milestone)?.finish ?? null;
  if (!/^\d{4}-\d{2}-\d{2}$/.test(end.date)) return null;
  const start = Date.parse(`${end.date}T00:00:00Z`);
  return Number.isNaN(start) ? null : start + DAY_MS;
}

/**
 * Không vừa khoảng: a leaf whose finish, as the engine scheduled it, is after the end of
 * the span or after its own deadline. Summaries show only above such tasks; a milestone
 * chosen as the end fits by its own finish, so only a missed deadline marks it.
 */
export function outsideSpan(task: Task, end: number): boolean {
  if (task.summary) return false;
  return task.finish > end || (task.deadline != null && task.finish > task.deadline);
}

/** The day of the project's last finish, where a new span ends. */
export function lastFinish(project: Project): string {
  const finish = project.tasks.reduce((latest, task) => Math.max(latest, task.finish), project.start);
  return instantToDateKey(finish);
}

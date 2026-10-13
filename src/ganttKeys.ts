import { shiftDays } from "./format";
import { isoDate } from "./ganttLayout";
import type { Calendar, Project, Task } from "./types";
import type { ViewRow } from "./viewRows";

/** What a key press on the focused Gantt asks for. */
export type GanttKeyAction =
  | { kind: "select"; uid: number }
  | { kind: "move"; uid: number; start: number }
  | { kind: "resize"; uid: number; finish: number }
  | { kind: "edit"; uid: number };

export interface KeyPress {
  key: string;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
}

/**
 * Keys on the focused Gantt: ↑ ↓ Home End pick a row; Alt+← → move the selected bar a
 * working day; Alt+Shift+← → move its finish; Enter opens the task. The engine still
 * schedules the result; this only picks the date to ask for.
 */
export function ganttKeyAction(
  press: KeyPress,
  rows: ViewRow[],
  selected: number | null,
  project: Project,
): GanttKeyAction | null {
  if (press.metaKey || press.ctrlKey) return null;
  // Grouped by person, one task can show on several rows; step over each task once.
  const seen = new Set<number>();
  const tasks = rows.flatMap((row) =>
    row.kind === "task" && !seen.has(row.task.uid) && seen.add(row.task.uid) ? [row.task] : [],
  );
  if (tasks.length === 0) return null;
  const at = tasks.findIndex((task) => task.uid === selected);
  const current = at >= 0 ? tasks[at] : null;

  if (!press.altKey && !press.shiftKey) {
    const pick = (index: number): GanttKeyAction => ({
      kind: "select",
      uid: tasks[Math.max(0, Math.min(tasks.length - 1, index))].uid,
    });
    if (press.key === "ArrowDown") return pick(at < 0 ? 0 : at + 1);
    if (press.key === "ArrowUp") return pick(at < 0 ? 0 : at - 1);
    if (press.key === "Home") return pick(0);
    if (press.key === "End") return pick(tasks.length - 1);
    if (press.key === "Enter" && current) return { kind: "edit", uid: current.uid };
    return null;
  }
  if (!press.altKey || !current || current.summary) return null;
  const step = press.key === "ArrowRight" ? 1 : press.key === "ArrowLeft" ? -1 : 0;
  if (step === 0) return null;
  const calendar = calendarOf(project, current);
  if (press.shiftKey) {
    if (current.milestone) return null;
    const finish = nextWorkingDay(calendar, current.finish, step);
    return finish > current.start && finish !== current.finish ? { kind: "resize", uid: current.uid, finish } : null;
  }
  const start = nextWorkingDay(calendar, current.start, step);
  return start !== current.start ? { kind: "move", uid: current.uid, start } : null;
}

function calendarOf(project: Project, task: Task): Calendar | undefined {
  const id = task.calendar_id ?? project.calendar_id;
  return project.calendars.find((calendar) => calendar.id === id) ?? project.calendars[0];
}

/** Same time of day on the next (or previous) day the calendar works. */
export function nextWorkingDay(calendar: Calendar | undefined, ms: number, step: 1 | -1): number {
  let next = shiftDays(ms, step);
  for (let guard = 0; guard < 366 && calendar && !works(calendar, next); guard += 1) {
    next = shiftDays(next, step);
  }
  return next;
}

function works(calendar: Calendar, ms: number): boolean {
  const date = isoDate(ms);
  const exception = calendar.exceptions[date];
  if (exception) return exception.length > 0;
  // A work week covering the date replaces the default week. Dates are "YYYY-MM-DD",
  // so they compare as text.
  const pattern = calendar.work_weeks.find((item) => item.from <= date && date <= item.to)?.week ?? calendar.week;
  // Week index 0 is Monday; getUTCDay() counts from Sunday.
  const weekday = (new Date(ms).getUTCDay() + 6) % 7;
  return (pattern[weekday] ?? []).length > 0;
}

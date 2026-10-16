import type { Assignment, Task } from "./types";

export type RangeEnd = "low" | "high";

/**
 * The effort range after one end is typed in a sheet cell, where only one end is edited at
 * a time: the other end stays, or comes from today's effort (`amount`) when there is no
 * range yet, moved along if the typed end passes it. An emptied end clears the range.
 * Null when the typed end would cross the other end of an existing range.
 */
export function rangeAfterEdit(
  task: Task,
  amount: number,
  end: RangeEnd,
  minutes: number | null,
): { low: number | null; high: number | null } | null {
  if (minutes == null) return { low: null, high: null };
  const { work_low: oldLow, work_high: oldHigh } = task;
  const has = oldLow != null && oldHigh != null;
  if (end === "low") {
    const high = has ? oldHigh : Math.max(minutes, amount);
    return minutes > high ? null : { low: minutes, high };
  }
  const low = has ? oldLow : Math.min(minutes, amount);
  return low > minutes ? null : { low, high: minutes };
}

/** Today's effort, or the duration of a task nobody is on: what a range scales from. */
export function effortAmount(task: Task, assignments: readonly Assignment[]): number {
  return assignments.some((item) => item.task_uid === task.uid) ? task.work : task.duration;
}

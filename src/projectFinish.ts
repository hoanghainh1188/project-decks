import type { Task } from "./types";

export interface ProjectFinish {
  finish: number;
  /** Task, or recurring series, whose finish sets the project finish. */
  driver: string;
}

/**
 * Project finish and what sets it, so a late meeting series does not hide behind a milestone.
 * Background series are left out, as the engine leaves them out of the critical path,
 * unless nothing else is left.
 */
export function projectFinish(tasks: Task[]): ProjectFinish | null {
  const background = new Set(
    tasks.filter((task) => task.recurrence?.spec.background).map((task) => task.uid),
  );
  const leaves = tasks.filter((task) => !task.summary);
  const counted = leaves.filter((task) => task.series_uid == null || !background.has(task.series_uid));
  const last = (counted.length > 0 ? counted : leaves)
    .reduce<Task | null>((latest, task) => (latest && latest.finish >= task.finish ? latest : task), null);
  if (!last) return null;
  const series = last.series_uid == null ? null : tasks.find((task) => task.uid === last.series_uid);
  return { finish: last.finish, driver: (series ?? last).name };
}

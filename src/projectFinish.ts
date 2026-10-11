import type { Task } from "./types";

export interface ProjectFinish {
  finish: number;
  /** Task, or recurring series, whose finish sets the project finish. */
  driver: string;
}

/** Project finish and what sets it, so a late meeting series does not hide behind a milestone. */
export function projectFinish(tasks: Task[]): ProjectFinish | null {
  const last = tasks
    .filter((task) => !task.summary)
    .reduce<Task | null>((latest, task) => (latest && latest.finish >= task.finish ? latest : task), null);
  if (!last) return null;
  const series = last.series_uid == null ? null : tasks.find((task) => task.uid === last.series_uid);
  return { finish: last.finish, driver: (series ?? last).name };
}

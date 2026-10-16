import type { Forecast, Project, Task, TaskForecast } from "../../types";

const DAY_MS = 86_400_000;
/** Below this chance of meeting its deadline a task is shown as at risk. */
export const ON_TIME_WARN = 0.8;

/** Noon of the day `ms` falls on: where a date's mark sits within its day's bar. */
export function middleOfDay(ms: number): number {
  return Math.floor(ms / DAY_MS) * DAY_MS + DAY_MS / 2;
}
/** Below this share of runs on the critical path a task without a deadline is not shown. */
const CRITICAL_SHARE = 0.5;

export interface Bar {
  /** Midnight of the first day of the bar. */
  start: number;
  days: number;
  count: number;
  /** Share of all runs finished by the end of this bar. */
  cumulative: number;
}

/**
 * The histogram as bars from the first to the last day with runs, with no gaps: one a day,
 * or one a week when the days are more than `room` bars.
 */
export function histogramBars(histogram: [number, number][], runs: number, room = 60): Bar[] {
  if (histogram.length === 0 || runs <= 0) return [];
  const first = histogram[0][0];
  const last = histogram[histogram.length - 1][0];
  const total = Math.round((last - first) / DAY_MS) + 1;
  const days = total > room ? 7 : 1;
  const bars: Bar[] = [];
  for (let index = 0; index * days < total; index += 1) {
    bars.push({ start: first + index * days * DAY_MS, days, count: 0, cumulative: 0 });
  }
  for (const [day, count] of histogram) {
    bars[Math.floor(Math.round((day - first) / DAY_MS) / days)].count += count;
  }
  let finished = 0;
  for (const bar of bars) {
    finished += bar.count;
    bar.cumulative = finished / runs;
  }
  return bars;
}

export interface RiskyTask {
  task: Task;
  forecast: TaskForecast;
}

/**
 * The tasks worth a look: those with a deadline, least likely on time first, then those
 * without one that are critical in at least half the runs, most critical first. Summaries
 * only show for a deadline of their own.
 */
export function riskyTasks(project: Project, result: Forecast, limit = 20): RiskyTask[] {
  const byUid = new Map(project.tasks.map((task) => [task.uid, task]));
  const rows = result.tasks.flatMap((forecast) => {
    const task = byUid.get(forecast.uid);
    return task ? [{ task, forecast }] : [];
  });
  const withDeadline = rows
    .filter((row) => row.forecast.on_time != null)
    .sort((a, b) => (a.forecast.on_time ?? 0) - (b.forecast.on_time ?? 0) || a.task.id - b.task.id);
  const critical = rows
    .filter((row) => row.forecast.on_time == null && !row.task.summary && row.forecast.criticality >= CRITICAL_SHARE)
    .sort((a, b) => b.forecast.criticality - a.forecast.criticality || a.task.id - b.task.id);
  return [...withDeadline, ...critical].slice(0, limit);
}

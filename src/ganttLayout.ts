import { utcMidnight } from "./format";
import type { Project, Scale, Task } from "./types";

export const ROW = 28;
export const HEAD = 48;

export interface Timescale {
  origin: number;
  days: number[];
}

export function pixelsPerDay(scale: Scale): number {
  if (scale === "day") return 28;
  if (scale === "week") return 14;
  return 5;
}

/** Same date window the on-screen Gantt paints, including baseline 0. */
export function buildTimescale(project: Project): Timescale {
  let min = Number.POSITIVE_INFINITY;
  let max = Number.NEGATIVE_INFINITY;
  for (const task of project.tasks) {
    min = Math.min(min, task.start, task.finish);
    max = Math.max(max, task.start, task.finish);
    for (const baseline of task.baselines) {
      if (baseline.number === 0) {
        min = Math.min(min, baseline.start);
        max = Math.max(max, baseline.finish);
      }
    }
  }
  if (!Number.isFinite(min) || !Number.isFinite(max)) {
    min = project.start;
    max = project.start + 14 * 86400000;
  }
  const origin = utcMidnight(min) - 2 * 86400000;
  const end = utcMidnight(max) + 21 * 86400000;
  const days: number[] = [];
  for (let cursor = origin; cursor <= end; cursor += 86400000) days.push(cursor);
  return { origin, days };
}

export function barBox(task: Task, row: number, origin: number, px: number) {
  const x = ((task.start - origin) / 86400000) * px;
  const finishX = ((Math.max(task.finish, task.start) - origin) / 86400000) * px;
  const w = Math.max(finishX - x, task.milestone ? 0 : 8);
  const y = task.summary ? row * ROW + 12 : row * ROW + 6;
  const h = task.summary ? 6 : 16;
  return { x, y, w, h };
}

export function barColor(task: Task): string {
  if (task.critical) return "#c23b3b";
  if (task.manual) return "#2a9d8f";
  if (task.summary) return "#1d1d1f";
  return "#2f6fad";
}

/** Full-day exceptions on the project calendar, matching the Gantt shading. */
export function holidayKeys(project: Project): Set<string> {
  const calendar = project.calendars.find((item) => item.id === project.calendar_id);
  const exceptions = calendar?.exceptions ?? {};
  return new Set(Object.keys(exceptions).filter((key) => (exceptions[key] ?? []).length === 0));
}

export function isWeekend(ms: number): boolean {
  const day = new Date(ms).getUTCDay();
  return day === 0 || day === 6;
}

export function isoDate(ms: number): string {
  const date = new Date(ms);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getUTCFullYear()}-${pad(date.getUTCMonth() + 1)}-${pad(date.getUTCDate())}`;
}

export function groupLabel(ms: number, px: number): string {
  const date = new Date(ms);
  if (px >= 20) return `Tháng ${date.getUTCMonth() + 1} ${date.getUTCFullYear()}`;
  const onejan = Date.UTC(date.getUTCFullYear(), 0, 1);
  const week = Math.ceil(((ms - onejan) / 86400000 + new Date(onejan).getUTCDay() + 1) / 7);
  return px >= 10 ? `Tuần ${week}` : `T${date.getUTCMonth() + 1}/${date.getUTCFullYear()}`;
}

export function dayLabel(ms: number, px: number): string {
  const date = new Date(ms);
  if (px < 10) return "";
  if (px < 20) return String(date.getUTCDate());
  const names = ["CN", "T2", "T3", "T4", "T5", "T6", "T7"];
  return `${names[date.getUTCDay()]} ${date.getUTCDate()}`;
}

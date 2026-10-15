import { utcMidnight } from "./format";
import { timescaleMessages } from "./ganttLayout.i18n";
import { pick } from "./i18n";
import { LIGHT_CANVAS, type CanvasPalette } from "./theme";
import type { Project, Scale, Segment, Task } from "./types";

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

/** Split pieces of a leaf bar, or null when the bar is drawn whole. */
export function splitPieces(task: Task, segments: Segment[] | undefined): Segment[] | null {
  if (task.summary || task.milestone || !segments || segments.length < 2) return null;
  return segments;
}

/**
 * For each gap of a split bar, in order, whether leveling made it (to let an urgent task in).
 * Gaps follow the distinct split offsets; the user's split and leveling's at one offset lay out
 * as one gap, counted as leveling's since leveling lengthened it.
 */
export function levelingGaps(task: Task): boolean[] {
  const byOffset = new Map<number, boolean>();
  for (const split of task.splits) {
    byOffset.set(split.offset, (byOffset.get(split.offset) ?? false) || split.leveling === true);
  }
  return [...byOffset.entries()].sort(([a], [b]) => a - b).map(([, leveling]) => leveling);
}

/** Whether content x falls in a gap of the bar that leveling made. */
export function levelingGapAt(task: Task, x: number, origin: number, px: number): boolean {
  const pieces = splitPieces(task, task.segments);
  if (!pieces) return false;
  const marks = levelingGaps(task);
  return pieces.slice(1).some((piece, index) => {
    if (!marks[index]) return false;
    const before = pieceSpan(pieces[index], origin, px);
    const after = pieceSpan(piece, origin, px);
    return x > before.x + before.w && x < after.x;
  });
}

/** Same pieces moved by a drag preview; a resize preview moves only the last finish. */
export function previewPieces(task: Task, start: number, finish: number): Segment[] | null {
  const pieces = splitPieces(task, task.segments);
  if (!pieces) return null;
  const shift = start - task.start;
  return pieces.map((piece, index) => {
    const moved = { start: piece.start + shift, finish: piece.finish + shift };
    if (index === pieces.length - 1 && finish !== task.finish) moved.finish = Math.max(finish, moved.start);
    return moved;
  });
}

/**
 * Index of the piece of a split bar at content x, or null when the bar is not split or
 * x is in a gap. Dragging piece 1 or later moves that piece and the ones after it.
 */
export function segmentAt(task: Task, x: number, origin: number, px: number): number | null {
  const pieces = splitPieces(task, task.segments);
  if (!pieces) return null;
  const index = pieces.findIndex((piece) => {
    const span = pieceSpan(piece, origin, px);
    return x >= span.x - 2 && x <= span.x + span.w + 2;
  });
  return index < 0 ? null : index;
}

/** Pieces while piece `index` is dragged to `start`: it and the later ones move together. */
export function segmentPreview(task: Task, index: number, start: number): Segment[] | null {
  const pieces = splitPieces(task, task.segments);
  if (!pieces || !pieces[index]) return pieces;
  const shift = start - pieces[index].start;
  return pieces.map((piece, at) => (at < index ? piece : { start: piece.start + shift, finish: piece.finish + shift }));
}

/** Horizontal extent of one piece, in content pixels. */
export function pieceSpan(piece: Segment, origin: number, px: number): { x: number; w: number } {
  const x = ((piece.start - origin) / 86400000) * px;
  const w = Math.max(((piece.finish - piece.start) / 86400000) * px, 2);
  return { x, w };
}

/** Width of the done part of each piece, filling pieces in order by `percent` of their total width. */
export function progressWidths(spans: { w: number }[], percent: number): number[] {
  const total = spans.reduce((sum, span) => sum + span.w, 0);
  let left = (total * Math.min(Math.max(percent, 0), 100)) / 100;
  return spans.map((span) => {
    const done = Math.min(span.w, left);
    left -= done;
    return done;
  });
}

/** The bar colour of `task`: light for PDF export, or the on-screen canvas palette. */
export function barColor(task: Task, palette: Pick<CanvasPalette, "critical" | "manual" | "summary" | "task"> = LIGHT_CANVAS): string {
  if (task.critical) return palette.critical;
  if (task.manual) return palette.manual;
  if (task.summary) return palette.summary;
  return palette.task;
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

/** Band label in the current language: a month ("Tháng 9 2026"), a week, or a short month. */
export function groupLabel(ms: number, px: number): string {
  const m = pick(timescaleMessages);
  const date = new Date(ms);
  if (px >= 20) return m.monthBand(date.getUTCMonth() + 1, date.getUTCFullYear());
  const onejan = Date.UTC(date.getUTCFullYear(), 0, 1);
  const week = Math.ceil(((ms - onejan) / 86400000 + new Date(onejan).getUTCDay() + 1) / 7);
  return px >= 10 ? m.weekBand(week) : m.monthShort(date.getUTCMonth() + 1, date.getUTCFullYear());
}

export interface TimescaleBand {
  label: string;
  count: number;
}

/** Roughly how wide a band label is drawn, with its padding: 7 px a letter, 12 px around. */
function labelFits(label: string, width: number): boolean {
  return label.length * 7 + 12 <= width;
}

/**
 * Each band is an exact run of equal day columns. Callers must not flex-shrink it. A band
 * too narrow for its name (the part of a week or month at either end) has no label, so
 * the window never shows a name cut in half.
 */
export function timescaleBands(days: number[], px: number): TimescaleBand[] {
  const bands: TimescaleBand[] = [];
  let start = 0;
  for (let index = 1; index <= days.length; index += 1) {
    const previous = days[index - 1];
    const day = days[index];
    if (index === days.length || groupLabel(previous, px) !== groupLabel(day, px)) {
      const label = groupLabel(days[start], px);
      const count = index - start;
      bands.push({ label: labelFits(label, count * px) ? label : "", count });
      start = index;
    }
  }
  return bands;
}

export interface DayLabelParts {
  weekday: string;
  date: string;
}

/**
 * Weekday and date for a day column; the window stacks them so they never touch. On the
 * week scale a column is too narrow for every two-digit date, so only Mondays, the first
 * day of each week band, carry one.
 */
export function dayLabelParts(ms: number, px: number): DayLabelParts {
  const date = new Date(ms);
  if (px < 10) return { weekday: "", date: "" };
  if (px < 20) return { weekday: "", date: date.getUTCDay() === 1 ? String(date.getUTCDate()) : "" };
  // Zoomed between the scales, a column can be too narrow for "Wed" above the date.
  if (px < 26) return { weekday: "", date: String(date.getUTCDate()) };
  return { weekday: pick(timescaleMessages).weekday(date.getUTCDay()), date: String(date.getUTCDate()) };
}

/** One-line day label, used by the PDF export where the text is shrunk to fit. */
export function dayLabel(ms: number, px: number): string {
  const { weekday, date } = dayLabelParts(ms, px);
  return weekday ? `${weekday} ${date}` : date;
}

const LABEL_GAP = 6;

/** Left edge of a bar's resource label: right of the bar, or left of it when it would be cut off. */
export function resourceLabelX(barLeft: number, barRight: number, textWidth: number, canvasWidth: number): number {
  const right = barRight + LABEL_GAP;
  if (right + textWidth <= canvasWidth) return right;
  const left = barLeft - LABEL_GAP - textWidth;
  return left >= 0 ? left : right;
}

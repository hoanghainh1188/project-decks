import { jsPDF } from "jspdf";

import fontDataUrl from "./fonts/BeVietnamPro-Regular.ttf?inline";
import { formatInstant } from "./format";
import {
  HEAD,
  ROW,
  barBox,
  barColor,
  buildTimescale,
  dayLabel,
  groupLabel,
  holidayKeys,
  isWeekend,
  isoDate,
  pieceSpan,
  pixelsPerDay,
  progressWidths,
  splitPieces,
} from "./ganttLayout";
import type { Project, Scale, Segment, Task } from "./types";
import type { ViewRow } from "./viewRows";

const PAGE_W = 297;
const PAGE_H = 210;
const MARGIN_X = 8;
const MARGIN_TOP = 8;
const MARGIN_BOTTOM = 8;
const TITLE_H = 6;
const FOOTER_H = 4;
const PX_TO_MM = 0.24;
const NAME_W = 46;
const DATE_W = 28;
const SHEET_W = NAME_W + DATE_W * 2;
const FONT = "BeVietnamPro";

export interface GanttPdfInput {
  project: Project;
  rows: ViewRow[];
  scale: Scale;
  selected: number | null;
  tracking: boolean;
}

interface Rgb {
  r: number;
  g: number;
  b: number;
}

interface Clip {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

function embeddedFont(): string {
  const comma = fontDataUrl.indexOf(",");
  return comma >= 0 ? fontDataUrl.slice(comma + 1) : fontDataUrl;
}

/** Suggested save-dialog name: the project name plus ".pdf". */
export function pdfSuggestedName(projectName: string): string {
  const trimmed = projectName.trim();
  const base = trimmed.length > 0 ? trimmed : "Dự án";
  const safe = base.replace(/[\\/:*?"<>|]/g, "-");
  return safe.toLowerCase().endsWith(".pdf") ? safe : `${safe}.pdf`;
}

/** Reads the current view and returns PDF bytes. Does not change the project. */
export function buildGanttPdf(input: GanttPdfInput): Uint8Array {
  return renderGanttPdf(input, embeddedFont());
}

/** Draws the visible Gantt rows. `fontData` is the Be Vietnam Pro TTF as base64 or a binary string. */
export function renderGanttPdf(input: GanttPdfInput, fontData: string): Uint8Array {
  const doc = new jsPDF({ orientation: "landscape", unit: "mm", format: "a4", compress: true });
  doc.addFileToVFS("BeVietnamPro-Regular.ttf", fontData);
  doc.addFont("BeVietnamPro-Regular.ttf", FONT, "normal");
  doc.setFont(FONT, "normal");
  const title = input.project.name.trim() || "Dự án";
  doc.setProperties({ title, creator: "ProjectDesk" });

  const timescale = buildTimescale(input.project);
  const px = pixelsPerDay(input.scale);
  const headerTop = MARGIN_TOP + TITLE_H;
  const headMm = HEAD * PX_TO_MM;
  const bodyTop = headerTop + headMm;
  const bodyBottomLimit = PAGE_H - MARGIN_BOTTOM - FOOTER_H;
  const bodyHeight = Math.max(headMm, bodyBottomLimit - bodyTop);
  const rowMm = Math.min(ROW * PX_TO_MM, bodyHeight);
  const rowsPerPage = Math.max(1, Math.floor(bodyHeight / rowMm));
  const ganttLeft = MARGIN_X + SHEET_W;
  const ganttWidth = PAGE_W - MARGIN_X - ganttLeft;
  const naturalDay = px * PX_TO_MM;
  const dayMm = Math.min(naturalDay, ganttWidth);
  const daysPerPage = Math.max(1, Math.floor((ganttWidth + 1e-6) / dayMm));
  const rowPages = Math.max(1, Math.ceil(input.rows.length / rowsPerPage));
  const dayPages = Math.max(1, Math.ceil(timescale.days.length / daysPerPage));
  const pageCount = rowPages * dayPages;
  const holidays = holidayKeys(input.project);

  let pageNumber = 0;
  for (let rowPage = 0; rowPage < rowPages; rowPage += 1) {
    const rowStart = rowPage * rowsPerPage;
    const rowEnd = Math.min(input.rows.length, rowStart + rowsPerPage);
    for (let dayPage = 0; dayPage < dayPages; dayPage += 1) {
      if (pageNumber > 0) doc.addPage();
      pageNumber += 1;
      const dayStart = dayPage * daysPerPage;
      const dayEnd = Math.min(timescale.days.length, dayStart + daysPerPage);
      drawPage(doc, input, {
        title,
        pageNumber,
        pageCount,
        timescale,
        px,
        holidays,
        headerTop,
        headMm,
        bodyTop,
        rowMm,
        dayMm,
        ganttLeft,
        rowStart,
        rowEnd,
        dayStart,
        dayEnd,
      });
    }
  }

  return new Uint8Array(doc.output("arraybuffer"));
}

interface PageGeom {
  title: string;
  pageNumber: number;
  pageCount: number;
  timescale: { origin: number; days: number[] };
  px: number;
  holidays: Set<string>;
  headerTop: number;
  headMm: number;
  bodyTop: number;
  rowMm: number;
  dayMm: number;
  ganttLeft: number;
  rowStart: number;
  rowEnd: number;
  dayStart: number;
  dayEnd: number;
}

function drawPage(doc: jsPDF, input: GanttPdfInput, geom: PageGeom) {
  const usedRows = Math.max(geom.rowEnd - geom.rowStart, 0);
  const usedDays = Math.max(geom.dayEnd - geom.dayStart, 0);
  const bodyHeight = usedRows * geom.rowMm;
  const usedW = usedDays * geom.dayMm;
  const tableBottom = geom.bodyTop + bodyHeight;
  const tableRight = geom.ganttLeft + usedW;

  doc.setFont(FONT, "normal");
  doc.setFontSize(11);
  setText(doc, "#1d1d1f");
  doc.text(fitText(doc, geom.title, PAGE_W - MARGIN_X * 2), MARGIN_X, MARGIN_TOP + TITLE_H / 2, {
    baseline: "middle",
  });

  paintSheetHeader(doc, geom);
  paintTimescale(doc, geom, usedW);
  paintSheetRows(doc, input, geom, bodyHeight);
  const ganttClip: Clip = {
    left: geom.ganttLeft,
    top: geom.bodyTop,
    right: tableRight,
    bottom: tableBottom,
  };
  paintGanttBody(doc, input, geom, ganttClip);

  setDraw(doc, "#d0d0d5");
  doc.setLineWidth(0.2);
  doc.rect(MARGIN_X, geom.headerTop, SHEET_W + usedW, geom.headMm + bodyHeight);
  doc.line(geom.ganttLeft, geom.headerTop, geom.ganttLeft, tableBottom);

  doc.setFontSize(8);
  setText(doc, "#6d6d72");
  doc.text(`Trang ${geom.pageNumber}/${geom.pageCount}`, PAGE_W - MARGIN_X, PAGE_H - MARGIN_BOTTOM + 1.5, {
    align: "right",
    baseline: "middle",
  });
}

function paintSheetHeader(doc: jsPDF, geom: PageGeom) {
  setFill(doc, "#f6f6f8");
  doc.rect(MARGIN_X, geom.headerTop, SHEET_W, geom.headMm, "F");
  doc.setFontSize(8);
  setText(doc, "#1d1d1f");
  const mid = geom.headerTop + geom.headMm / 2;
  doc.text("Tên", MARGIN_X + 1.6, mid, { baseline: "middle" });
  doc.text("Bắt đầu", MARGIN_X + NAME_W + 1.2, mid, { baseline: "middle" });
  doc.text("Kết thúc", MARGIN_X + NAME_W + DATE_W + 1.2, mid, { baseline: "middle" });
  setDraw(doc, "#d5d5da");
  doc.setLineWidth(0.15);
  doc.line(MARGIN_X + NAME_W, geom.headerTop, MARGIN_X + NAME_W, geom.headerTop + geom.headMm);
  doc.line(MARGIN_X + NAME_W + DATE_W, geom.headerTop, MARGIN_X + NAME_W + DATE_W, geom.headerTop + geom.headMm);
}

function paintTimescale(doc: jsPDF, geom: PageGeom, usedW: number) {
  if (usedW <= 0) return;
  setFill(doc, "#f6f6f8");
  doc.rect(geom.ganttLeft, geom.headerTop, usedW, geom.headMm, "F");
  const days = geom.timescale.days;
  doc.setFontSize(7);
  let groupStart = geom.dayStart;
  for (let index = geom.dayStart; index <= geom.dayEnd; index += 1) {
    const day = days[index];
    const previous = days[index - 1];
    const split =
      index === geom.dayEnd || !day || !previous || groupLabel(previous, geom.px) !== groupLabel(day, geom.px);
    if (split && index > groupStart) {
      const label = groupLabel(days[groupStart], geom.px);
      const x = geom.ganttLeft + (groupStart - geom.dayStart) * geom.dayMm;
      const w = (index - groupStart) * geom.dayMm;
      setText(doc, "#1d1d1f");
      drawShrunk(doc, label, x + w / 2, geom.headerTop + 14 * PX_TO_MM, Math.max(1, w - 1), 7, {
        align: "center",
        baseline: "middle",
      });
      setDraw(doc, "#d5d5da");
      doc.setLineWidth(0.15);
      doc.line(x + w, geom.headerTop, x + w, geom.headerTop + geom.headMm);
      groupStart = index;
    }
    if (index === geom.dayEnd || !day) continue;
    const x = geom.ganttLeft + (index - geom.dayStart) * geom.dayMm;
    const label = dayLabel(day, geom.px);
    if (label) {
      setText(doc, isWeekend(day) ? "#8e8e93" : "#3a3a3c");
      drawShrunk(doc, label, x + geom.dayMm / 2, geom.headerTop + 34 * PX_TO_MM, Math.max(1, geom.dayMm - 0.5), 6.5, {
        align: "center",
        baseline: "middle",
      });
    }
    setDraw(doc, "#e4e4e8");
    doc.setLineWidth(0.1);
    doc.line(x, geom.headerTop + 24 * PX_TO_MM, x, geom.headerTop + geom.headMm);
  }
}

function paintSheetRows(doc: jsPDF, input: GanttPdfInput, geom: PageGeom, bodyHeight: number) {
  for (let index = geom.rowStart; index < geom.rowEnd; index += 1) {
    const row = input.rows[index];
    const y = geom.bodyTop + (index - geom.rowStart) * geom.rowMm;
    if (row.kind === "group") {
      setFill(doc, "#ececf1");
      doc.rect(MARGIN_X, y, SHEET_W, geom.rowMm, "F");
      doc.setFontSize(8);
      setText(doc, "#1d1d1f");
      doc.text(fitText(doc, row.label, SHEET_W - 3), MARGIN_X + 1.6, y + geom.rowMm / 2, { baseline: "middle" });
    } else {
      paintRowWash(doc, row.highlighted, row.task.uid === input.selected, MARGIN_X, y, SHEET_W, geom.rowMm);
      doc.setFontSize(8);
      const nameX = MARGIN_X + 1.6 + Math.max(0, row.task.outline_level - 1) * 14 * PX_TO_MM;
      const nameW = Math.max(4, MARGIN_X + NAME_W - nameX - 1);
      setText(doc, row.task.critical ? "#b42318" : "#1d1d1f");
      doc.text(fitText(doc, row.task.name, nameW), nameX, y + geom.rowMm / 2, { baseline: "middle" });
      setText(doc, "#1d1d1f");
      doc.text(fitText(doc, formatInstant(row.task.start), DATE_W - 1.6), MARGIN_X + NAME_W + 1.2, y + geom.rowMm / 2, {
        baseline: "middle",
      });
      doc.text(
        fitText(doc, formatInstant(row.task.finish), DATE_W - 1.6),
        MARGIN_X + NAME_W + DATE_W + 1.2,
        y + geom.rowMm / 2,
        { baseline: "middle" },
      );
    }
    setDraw(doc, row.kind === "group" ? "#e4e4e8" : "#f0f0f2");
    doc.setLineWidth(0.1);
    doc.line(MARGIN_X, y + geom.rowMm, MARGIN_X + SHEET_W, y + geom.rowMm);
  }
  if (bodyHeight > 0) {
    setDraw(doc, "#e4e4e8");
    doc.setLineWidth(0.1);
    doc.line(MARGIN_X + NAME_W, geom.bodyTop, MARGIN_X + NAME_W, geom.bodyTop + bodyHeight);
    doc.line(MARGIN_X + NAME_W + DATE_W, geom.bodyTop, MARGIN_X + NAME_W + DATE_W, geom.bodyTop + bodyHeight);
  }
}

function paintGanttBody(doc: jsPDF, input: GanttPdfInput, geom: PageGeom, clip: Clip) {
  if (clip.right <= clip.left || clip.bottom <= clip.top) return;
  const xOf = (contentPx: number) => geom.ganttLeft + (contentPx - geom.dayStart * geom.px) * (geom.dayMm / geom.px);
  const yOf = (contentPx: number) => geom.bodyTop + (contentPx - geom.rowStart * ROW) * (geom.rowMm / ROW);
  const yScale = geom.rowMm / ROW;

  for (let index = geom.dayStart; index < geom.dayEnd; index += 1) {
    const day = geom.timescale.days[index];
    const x = geom.ganttLeft + (index - geom.dayStart) * geom.dayMm;
    if (isWeekend(day) || geom.holidays.has(isoDate(day))) {
      setFill(doc, "#f3f3f5");
      doc.rect(x, clip.top, geom.dayMm, clip.bottom - clip.top, "F");
    }
    setDraw(doc, "#ececf0");
    doc.setLineWidth(0.1);
    doc.line(x, clip.top, x, clip.bottom);
  }

  for (let index = geom.rowStart; index < geom.rowEnd; index += 1) {
    const row = input.rows[index];
    const y = geom.bodyTop + (index - geom.rowStart) * geom.rowMm;
    if (row.kind === "group") {
      setFill(doc, "#ececf1");
      doc.rect(clip.left, y, clip.right - clip.left, geom.rowMm, "F");
      doc.setFontSize(8);
      setText(doc, "#1d1d1f");
      doc.text(fitText(doc, row.label, Math.max(4, clip.right - clip.left - 3)), clip.left + 1.6, y + geom.rowMm / 2, {
        baseline: "middle",
      });
    } else {
      paintRowWash(doc, row.highlighted, row.task.uid === input.selected, clip.left, y, clip.right - clip.left, geom.rowMm);
      if (input.tracking) {
        const baseline = row.task.baselines.find((item) => item.number === 0);
        if (baseline) {
          const pieces = splitPieces(row.task, baseline.segments);
          drawBar(doc, row.task, index, geom, xOf, yOf, yScale, clip, "#c5c5cb", true, baseline.start, baseline.finish, pieces);
        }
      }
      const pieces = splitPieces(row.task, row.task.segments);
      drawBar(doc, row.task, index, geom, xOf, yOf, yScale, clip, barColor(row.task), false, row.task.start, row.task.finish, pieces);
      if (!row.task.summary && row.task.percent_complete > 0) {
        drawProgress(doc, row.task, index, geom, xOf, yOf, yScale, clip);
      }
    }
    setDraw(doc, row.kind === "group" ? "#e4e4e8" : "#f0f0f2");
    doc.setLineWidth(0.1);
    doc.line(clip.left, y + geom.rowMm, clip.right, y + geom.rowMm);
  }

  paintLinks(doc, input, geom, xOf, yOf, clip);
}

function paintRowWash(
  doc: jsPDF,
  highlighted: boolean,
  selected: boolean,
  x: number,
  y: number,
  w: number,
  h: number,
) {
  if (w <= 0 || h <= 0) return;
  if (highlighted && selected) setFill(doc, "#fff1c2");
  else if (highlighted) setFill(doc, "#fff6d0");
  else if (selected) setFill(doc, "#e8f2ff");
  else return;
  doc.rect(x, y, w, h, "F");
  if (highlighted) {
    setFill(doc, "#e6b325");
    doc.rect(x, y, Math.min(0.7, w), h, "F");
  }
}

function drawBar(
  doc: jsPDF,
  task: Task,
  row: number,
  geom: PageGeom,
  xOf: (contentPx: number) => number,
  yOf: (contentPx: number) => number,
  yScale: number,
  clip: Clip,
  color: string,
  baseline: boolean,
  start: number,
  finish: number,
  pieces: Segment[] | null,
) {
  const shaped = { ...task, start, finish };
  const rect = barBox(shaped, row, geom.timescale.origin, geom.px);
  setFill(doc, color);
  if (pieces) {
    // Split task: one block per segment, joined by a dotted line across each gap.
    const mm = geom.dayMm / geom.px;
    const y = baseline ? yOf(row * ROW + 20) : yOf(rect.y);
    const h = (baseline ? 5 : rect.h) * yScale;
    const spans = pieces.map((piece) => pieceSpan(piece, geom.timescale.origin, geom.px));
    for (const span of spans) {
      const clipped = clipRect(xOf(span.x), y, span.w * mm, h, clip);
      if (clipped) doc.rect(clipped.x, clipped.y, clipped.w, clipped.h, "F");
    }
    setDraw(doc, color);
    doc.setLineWidth(0.25);
    doc.setLineDashPattern([0.6, 0.8], 0);
    const mid = y + h / 2;
    for (let index = 0; index + 1 < spans.length; index += 1) {
      const from = Math.max(xOf(spans[index].x + spans[index].w), clip.left);
      const to = Math.min(xOf(spans[index + 1].x), clip.right);
      if (to > from && mid >= clip.top && mid <= clip.bottom) doc.line(from, mid, to, mid);
    }
    doc.setLineDashPattern([], 0);
    return;
  }
  if (task.milestone && !baseline) {
    const cx = xOf(rect.x);
    const cy = yOf(row * ROW + ROW / 2);
    const size = 7 * yScale;
    if (cx < clip.left - size || cx > clip.right + size || cy < clip.top || cy > clip.bottom) return;
    doc.triangle(cx, cy - size, cx + size, cy, cx, cy + size, "F");
    doc.triangle(cx, cy - size, cx - size, cy, cx, cy + size, "F");
    return;
  }
  const x = xOf(rect.x);
  const y = baseline ? yOf(row * ROW + 20) : yOf(rect.y);
  const rawW = rect.w * (geom.dayMm / geom.px);
  const w = Math.max(rect.w, 2) * (geom.dayMm / geom.px);
  const h = (baseline ? 5 : rect.h) * yScale;
  const clipped = clipRect(x, y, w, h, clip);
  if (clipped) doc.rect(clipped.x, clipped.y, clipped.w, clipped.h, "F");
  if (task.summary && !baseline) {
    const cap = 7 * (geom.dayMm / geom.px);
    const top = y;
    const bottom = y + h;
    if (x >= clip.left && x <= clip.right) doc.triangle(x, top, x + cap, top, x, bottom, "F");
    const end = x + rawW;
    if (end >= clip.left && end <= clip.right) doc.triangle(end, top, end - cap, top, end, bottom, "F");
  }
}

function drawProgress(
  doc: jsPDF,
  task: Task,
  row: number,
  geom: PageGeom,
  xOf: (contentPx: number) => number,
  yOf: (contentPx: number) => number,
  yScale: number,
  clip: Clip,
) {
  const rect = barBox(task, row, geom.timescale.origin, geom.px);
  const pieces = splitPieces(task, task.segments);
  const spans = pieces
    ? pieces.map((piece) => pieceSpan(piece, geom.timescale.origin, geom.px))
    : [{ x: rect.x, w: rect.w }];
  const overlay = blend(rgb("#000000"), rgb(barColor(task)), 0.28);
  doc.setFillColor(overlay.r, overlay.g, overlay.b);
  progressWidths(spans, task.percent_complete).forEach((done, index) => {
    if (done <= 0) return;
    const clipped = clipRect(
      xOf(spans[index].x),
      yOf(rect.y),
      done * (geom.dayMm / geom.px),
      rect.h * yScale,
      clip,
    );
    if (clipped) doc.rect(clipped.x, clipped.y, clipped.w, clipped.h, "F");
  });
}

function paintLinks(
  doc: jsPDF,
  input: GanttPdfInput,
  geom: PageGeom,
  xOf: (contentPx: number) => number,
  yOf: (contentPx: number) => number,
  clip: Clip,
) {
  const indexOf = new Map<number, number>();
  input.rows.forEach((item, index) => {
    if (item.kind === "task" && !indexOf.has(item.task.uid)) indexOf.set(item.task.uid, index);
  });
  doc.setLineWidth(1.2 * PX_TO_MM);
  for (const dep of input.project.dependencies) {
    const from = indexOf.get(dep.predecessor_uid);
    const to = indexOf.get(dep.successor_uid);
    if (from === undefined || to === undefined) continue;
    const linkTop = Math.min(from, to);
    const linkBottom = Math.max(from, to);
    if (linkBottom < geom.rowStart || linkTop >= geom.rowEnd) continue;
    const predRow = input.rows[from];
    const succRow = input.rows[to];
    if (predRow.kind !== "task" || succRow.kind !== "task") continue;
    const pred = predRow.task;
    const succ = succRow.task;
    const a = barBox(pred, from, geom.timescale.origin, geom.px);
    const b = barBox(succ, to, geom.timescale.origin, geom.px);
    const startX = dep.kind === "ss" || dep.kind === "sf" ? a.x : a.x + a.w;
    const startY = a.y + a.h / 2;
    const endX = dep.kind === "ff" || dep.kind === "sf" ? b.x + b.w : b.x;
    const endY = b.y + b.h / 2;
    const color = pred.critical && succ.critical ? "#c23b3b" : "#6d6d72";
    setDraw(doc, color);
    const points = [
      [xOf(startX), yOf(startY)],
      [xOf(startX + (endX >= startX ? 10 : -10)), yOf(startY)],
      [xOf(startX + (endX >= startX ? 10 : -10)), yOf(endY)],
      [xOf(endX), yOf(endY)],
    ];
    for (let index = 0; index < points.length - 1; index += 1) {
      const [x1, y1] = points[index];
      const [x2, y2] = points[index + 1];
      const segment = clipSegment(x1, y1, x2, y2, clip);
      if (!segment) continue;
      doc.line(segment[0], segment[1], segment[2], segment[3]);
    }
    const headX = xOf(endX);
    const headY = yOf(endY);
    if (headX < clip.left || headX > clip.right || headY < clip.top || headY > clip.bottom) continue;
    setFill(doc, color);
    doc.triangle(headX, headY, xOf(endX - 5), yOf(endY - 3), xOf(endX - 5), yOf(endY + 3), "F");
  }
}

function clipRect(
  x: number,
  y: number,
  w: number,
  h: number,
  clip: Clip,
): { x: number; y: number; w: number; h: number } | null {
  const right = Math.min(x + w, clip.right);
  const bottom = Math.min(y + h, clip.bottom);
  const left = Math.max(x, clip.left);
  const top = Math.max(y, clip.top);
  if (right - left <= 0 || bottom - top <= 0) return null;
  return { x: left, y: top, w: right - left, h: bottom - top };
}

function clipSegment(
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  clip: Clip,
): [number, number, number, number] | null {
  const leftBit = 1;
  const rightBit = 2;
  const topBit = 4;
  const bottomBit = 8;
  const code = (x: number, y: number) => {
    let bits = 0;
    if (x < clip.left) bits |= leftBit;
    else if (x > clip.right) bits |= rightBit;
    if (y < clip.top) bits |= topBit;
    else if (y > clip.bottom) bits |= bottomBit;
    return bits;
  };
  let c1 = code(x1, y1);
  let c2 = code(x2, y2);
  for (let guard = 0; guard < 12; guard += 1) {
    if ((c1 | c2) === 0) return [x1, y1, x2, y2];
    if ((c1 & c2) !== 0) return null;
    const out = c1 !== 0 ? c1 : c2;
    let x = x1;
    let y = y1;
    if (out & (bottomBit | topBit)) {
      if (y2 === y1) return null;
      const edge = out & bottomBit ? clip.bottom : clip.top;
      x = x1 + ((x2 - x1) * (edge - y1)) / (y2 - y1);
      y = edge;
    } else {
      if (x2 === x1) return null;
      const edge = out & rightBit ? clip.right : clip.left;
      y = y1 + ((y2 - y1) * (edge - x1)) / (x2 - x1);
      x = edge;
    }
    if (out === c1) {
      x1 = x;
      y1 = y;
      c1 = code(x1, y1);
    } else {
      x2 = x;
      y2 = y;
      c2 = code(x2, y2);
    }
  }
  return null;
}

function setText(doc: jsPDF, hex: string) {
  const color = rgb(hex);
  doc.setTextColor(color.r, color.g, color.b);
}

function setFill(doc: jsPDF, hex: string) {
  const color = rgb(hex);
  doc.setFillColor(color.r, color.g, color.b);
}

function setDraw(doc: jsPDF, hex: string) {
  const color = rgb(hex);
  doc.setDrawColor(color.r, color.g, color.b);
}

function rgb(hex: string): Rgb {
  const value = hex.replace("#", "");
  return {
    r: Number.parseInt(value.slice(0, 2), 16),
    g: Number.parseInt(value.slice(2, 4), 16),
    b: Number.parseInt(value.slice(4, 6), 16),
  };
}

function blend(fg: Rgb, bg: Rgb, alpha: number): Rgb {
  return {
    r: Math.round(fg.r * alpha + bg.r * (1 - alpha)),
    g: Math.round(fg.g * alpha + bg.g * (1 - alpha)),
    b: Math.round(fg.b * alpha + bg.b * (1 - alpha)),
  };
}

function drawShrunk(
  doc: jsPDF,
  text: string,
  x: number,
  y: number,
  maxWidth: number,
  baseSize: number,
  options: { align?: "left" | "center" | "right"; baseline?: "middle" },
) {
  doc.setFontSize(baseSize);
  const width = doc.getTextWidth(text);
  const size = width > maxWidth && width > 0 ? Math.max(4.5, (baseSize * maxWidth) / width) : baseSize;
  doc.setFontSize(size);
  doc.text(text, x, y, options);
  doc.setFontSize(baseSize);
}

function fitText(doc: jsPDF, text: string, maxWidth: number): string {
  if (maxWidth <= 0) return "";
  if (doc.getTextWidth(text) <= maxWidth) return text;
  const chars = Array.from(text);
  let end = chars.length;
  while (end > 0 && doc.getTextWidth(`${chars.slice(0, end).join("")}...`) > maxWidth) end -= 1;
  return end === 0 ? "" : `${chars.slice(0, end).join("")}...`;
}

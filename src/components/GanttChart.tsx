import { useEffect, useMemo, useRef, useState, type CSSProperties, type RefObject } from "react";

import { shiftDays } from "../format";
import {
  ROW,
  barBox,
  barColor,
  buildTimescale,
  dayLabel,
  holidayKeys,
  isWeekend,
  isoDate,
  pieceSpan,
  pixelsPerDay,
  previewPieces,
  progressWidths,
  splitPieces,
  timescaleBands,
  type Timescale,
} from "../ganttLayout";
import type { Project, Scale, Segment, Task } from "../types";
import type { ViewRow } from "../viewRows";

interface DragState {
  kind: "move" | "resize" | "link";
  uid: number;
  row: number;
  originX: number;
  originY: number;
  start: number;
  finish: number;
  x: number;
  y: number;
  moved: boolean;
}

interface Props {
  project: Project;
  rows: ViewRow[];
  scale: Scale;
  selected: number | null;
  tracking: boolean;
  bodyRef: RefObject<HTMLDivElement | null>;
  onSelect: (uid: number) => void;
  onMove: (uid: number, start: number) => void;
  onResize: (uid: number, finish: number) => void;
  onLink: (from: number, to: number) => void;
  onScroll: () => void;
}

export function GanttChart({
  project,
  rows,
  scale,
  selected,
  tracking,
  bodyRef,
  onSelect,
  onMove,
  onResize,
  onLink,
  onScroll,
}: Props) {
  const headRef = useRef<HTMLDivElement>(null);
  const bodyCanvas = useRef<HTMLCanvasElement>(null);
  const [drag, setDrag] = useState<DragState | null>(null);
  const px = pixelsPerDay(scale);
  const model = useMemo(() => buildTimescale(project), [project]);
  const bands = useMemo(() => timescaleBands(model.days, px), [model.days, px]);
  const timelineWidth = model.days.length * px;

  function paint() {
    const canvas = bodyCanvas.current;
    const scroller = bodyRef.current;
    if (!canvas || !scroller) return;
    paintBody(
      canvas,
      project,
      rows,
      model,
      px,
      selected,
      tracking,
      drag,
      scroller.scrollTop,
      scroller.scrollLeft,
      scroller.clientHeight,
    );
  }

  useEffect(() => {
    paint();
    const scroller = bodyRef.current;
    if (!scroller) return;
    const observer = new ResizeObserver(() => paint());
    observer.observe(scroller);
    return () => observer.disconnect();
  }, [project, rows, model, px, selected, tracking, drag]);

  function contentPoint(event: { clientX: number; clientY: number; currentTarget: HTMLCanvasElement }) {
    const bounds = event.currentTarget.getBoundingClientRect();
    return {
      x: event.clientX - bounds.left,
      y: event.clientY - bounds.top + (bodyRef.current?.scrollTop ?? 0),
    };
  }

  function hit(x: number, y: number) {
    const row = Math.floor(y / ROW);
    const item = rows[row];
    if (!item || item.kind !== "task") return null;
    const task = item.task;
    const rect = barBox(task, row, model.origin, px);
    const handle = { x: rect.x + Math.max(rect.w, 8), y: rect.y + rect.h / 2 };
    if (Math.hypot(x - handle.x, y - handle.y) <= 7) return { task, row, zone: "link" as const, rect };
    if (!task.summary && !task.milestone && x >= rect.x + rect.w - 6 && x <= rect.x + rect.w + 2 && y >= rect.y && y <= rect.y + rect.h) {
      return { task, row, zone: "resize" as const, rect };
    }
    if (x >= rect.x - 4 && x <= rect.x + Math.max(rect.w, 10) && y >= rect.y - 2 && y <= rect.y + rect.h + 2) {
      return { task, row, zone: "body" as const, rect };
    }
    return { task, row, zone: "row" as const, rect };
  }

  return (
    <div className="gantt">
      <div className="gantt-head" ref={headRef}>
        <div className="gantt-scale" style={{ width: timelineWidth, minWidth: timelineWidth }}>
          <div className="gantt-bands">
            {bands.map((band, index) => (
              <div key={`${band.label}-${index}`} className="gantt-band" style={fixedSpan(band.count * px)}>
                {band.label}
              </div>
            ))}
          </div>
          <div className="gantt-days">
            {model.days.map((day) => (
              <div key={day} className={`gantt-day${isWeekend(day) ? " weekend" : ""}`} style={fixedSpan(px)}>
                {dayLabel(day, px)}
              </div>
            ))}
          </div>
        </div>
      </div>
      <div
        className="gantt-body"
        ref={bodyRef}
        onScroll={(event) => {
          if (headRef.current) headRef.current.scrollLeft = event.currentTarget.scrollLeft;
          const canvas = bodyCanvas.current;
          if (canvas) {
            paintBody(
              canvas,
              project,
              rows,
              model,
              px,
              selected,
              tracking,
              drag,
              event.currentTarget.scrollTop,
              event.currentTarget.scrollLeft,
              event.currentTarget.clientHeight,
            );
          }
          onScroll();
        }}
      >
        <div
          className="gantt-canvas-host"
          style={{
            width: timelineWidth,
            minWidth: timelineWidth,
            height: rows.length * ROW,
          }}
        >
        <canvas
          ref={bodyCanvas}
          onPointerDown={(event) => {
            const { x, y } = contentPoint(event);
            const found = hit(x, y);
            if (!found) return;
            onSelect(found.task.uid);
            if (found.zone === "row") return;
            event.currentTarget.setPointerCapture(event.pointerId);
            setDrag({
              kind: found.zone === "link" ? "link" : found.zone === "resize" ? "resize" : "move",
              uid: found.task.uid,
              row: found.row,
              originX: x,
              originY: y,
              start: found.task.start,
              finish: found.task.finish,
              x,
              y,
              moved: false,
            });
          }}
          onPointerMove={(event) => {
            if (!drag) return;
            const { x, y } = contentPoint(event);
            setDrag({ ...drag, x, y, moved: drag.moved || Math.hypot(x - drag.originX, y - drag.originY) > 4 });
          }}
          onPointerUp={(event) => {
            if (!drag) return;
            const { x, y } = contentPoint(event);
            if (drag.moved) {
              const days = Math.round((x - drag.originX) / px);
              if (drag.kind === "move" && days !== 0) onMove(drag.uid, shiftDays(drag.start, days));
              if (drag.kind === "resize" && days !== 0) onResize(drag.uid, shiftDays(drag.finish, days));
              if (drag.kind === "link") {
                const found = hit(x, y);
                if (found && found.task.uid !== drag.uid) onLink(drag.uid, found.task.uid);
              }
            }
            setDrag(null);
          }}
        />
        </div>
      </div>
    </div>
  );
}

function fixedSpan(width: number): CSSProperties {
  // flex-shrink 0 keeps later weeks and months the same width as the current period.
  return { width, minWidth: width, maxWidth: width, flex: `0 0 ${width}px` };
}

function paintBody(
  canvas: HTMLCanvasElement,
  project: Project,
  rows: ViewRow[],
  model: Timescale,
  px: number,
  selected: number | null,
  tracking: boolean,
  drag: DragState | null,
  scrollTop: number,
  scrollLeft: number,
  viewHeight: number,
) {
  const width = model.days.length * px;
  const contentHeight = Math.max(rows.length * ROW, 1);
  const canvasHeight = Math.max(1, Math.min(Math.max(viewHeight, ROW), contentHeight));
  const ratio = window.devicePixelRatio || 1;
  canvas.style.top = `${scrollTop}px`;
  sizeCanvas(canvas, width, canvasHeight, ratio);
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.setTransform(ratio, 0, 0, ratio, 0, -scrollTop * ratio);
  ctx.clearRect(0, scrollTop, width, canvasHeight);
  const windowHeight = Math.max(viewHeight, canvasHeight);
  const rowStart = Math.max(0, Math.floor(scrollTop / ROW) - 6);
  const rowEnd = Math.min(rows.length, rowStart + Math.ceil(windowHeight / ROW) + 12);
  const holidays = holidayKeys(project);
  model.days.forEach((day, index) => {
    const key = isoDate(day);
    if (isWeekend(day) || holidays.has(key)) {
      ctx.fillStyle = "#f3f3f5";
      ctx.fillRect(index * px, scrollTop, px, canvasHeight);
    }
    ctx.strokeStyle = "#ececf0";
    ctx.beginPath();
    ctx.moveTo(index * px + 0.5, scrollTop);
    ctx.lineTo(index * px + 0.5, scrollTop + canvasHeight);
    ctx.stroke();
  });
  for (let row = rowStart; row < rowEnd; row += 1) {
    const item = rows[row];
    const y = row * ROW;
    if (item.kind === "group") {
      ctx.fillStyle = "#ececf1";
      ctx.fillRect(0, y, width, ROW);
      ctx.fillStyle = "#1d1d1f";
      ctx.font = "600 12px -apple-system, sans-serif";
      ctx.textAlign = "left";
      ctx.textBaseline = "middle";
      ctx.fillText(item.label, scrollLeft + 8, y + ROW / 2);
      ctx.strokeStyle = "#e4e4e8";
      ctx.beginPath();
      ctx.moveTo(0, y + ROW - 0.5);
      ctx.lineTo(width, y + ROW - 0.5);
      ctx.stroke();
      continue;
    }
    const task = item.task;
    if (item.highlighted) {
      // Amber wash stays lighter than the critical-path bar so both remain readable.
      ctx.fillStyle = "rgba(255, 214, 64, 0.38)";
      ctx.fillRect(0, y, width, ROW);
    }
    if (task.uid === selected) {
      ctx.fillStyle = "rgba(10, 132, 255, 0.10)";
      ctx.fillRect(0, y, width, ROW);
    }
    ctx.strokeStyle = "#f0f0f2";
    ctx.beginPath();
    ctx.moveTo(0, y + ROW - 0.5);
    ctx.lineTo(width, y + ROW - 0.5);
    ctx.stroke();
    if (tracking) {
      const baseline = task.baselines.find((baselineItem) => baselineItem.number === 0);
      if (baseline) {
        const pieces = splitPieces(task, baseline.segments);
        drawBar(ctx, task, row, model.origin, px, "#c5c5cb", true, baseline.start, baseline.finish, pieces);
      }
    }
    const moved = drag && drag.moved && drag.uid === task.uid && drag.kind !== "link" ? drag : null;
    const start = moved?.kind === "move" ? shiftDays(task.start, Math.round((moved.x - moved.originX) / px)) : task.start;
    const finish = moved?.kind === "resize" ? shiftDays(task.finish, Math.round((moved.x - moved.originX) / px)) : task.finish;
    const preview = { ...task, start, finish };
    const pieces = previewPieces(task, start, finish);
    const color = barColor(task);
    drawBar(ctx, preview, row, model.origin, px, color, false, preview.start, preview.finish, pieces);
    if (!task.summary && task.percent_complete > 0) {
      const rect = barBox(preview, row, model.origin, px);
      ctx.fillStyle = "rgba(0,0,0,0.28)";
      if (pieces) {
        const spans = pieces.map((piece) => pieceSpan(piece, model.origin, px));
        progressWidths(spans, task.percent_complete).forEach((done, index) => {
          if (done > 0) ctx.fillRect(spans[index].x, rect.y, done, rect.h);
        });
      } else {
        ctx.fillRect(rect.x, rect.y, rect.w * Math.min(task.percent_complete, 100) / 100, rect.h);
      }
    }
  }
  const indexOf = new Map<number, number>();
  rows.forEach((item, index) => {
    if (item.kind === "task" && !indexOf.has(item.task.uid)) indexOf.set(item.task.uid, index);
  });
  const viewTop = scrollTop;
  const viewBottom = scrollTop + canvasHeight;
  ctx.lineWidth = 1.2;
  for (const dep of project.dependencies) {
    const from = indexOf.get(dep.predecessor_uid);
    const to = indexOf.get(dep.successor_uid);
    if (from === undefined || to === undefined) continue;
    const linkTop = Math.min(from, to) * ROW;
    const linkBottom = Math.max(from, to) * ROW + ROW;
    if (linkBottom < viewTop || linkTop > viewBottom) continue;
    const predRow = rows[from];
    const succRow = rows[to];
    if (predRow.kind !== "task" || succRow.kind !== "task") continue;
    const pred = predRow.task;
    const succ = succRow.task;
    const a = barBox(pred, from, model.origin, px);
    const b = barBox(succ, to, model.origin, px);
    const startX = dep.kind === "ss" || dep.kind === "sf" ? a.x : a.x + a.w;
    const startY = a.y + a.h / 2;
    const endX = dep.kind === "ff" || dep.kind === "sf" ? b.x + b.w : b.x;
    const endY = b.y + b.h / 2;
    ctx.strokeStyle = pred.critical && succ.critical ? "#c23b3b" : "#6d6d72";
    ctx.beginPath();
    ctx.moveTo(startX, startY);
    const mid = startX + (endX >= startX ? 10 : -10);
    ctx.lineTo(mid, startY);
    ctx.lineTo(mid, endY);
    ctx.lineTo(endX, endY);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(endX, endY);
    ctx.lineTo(endX - 5, endY - 3);
    ctx.lineTo(endX - 5, endY + 3);
    ctx.closePath();
    ctx.fillStyle = ctx.strokeStyle;
    ctx.fill();
  }
  if (drag?.kind === "link" && drag.moved) {
    const from = rows[drag.row];
    if (from?.kind === "task") {
      const rect = barBox(from.task, drag.row, model.origin, px);
      ctx.strokeStyle = "#0a84ff";
      ctx.setLineDash([4, 3]);
      ctx.beginPath();
      ctx.moveTo(rect.x + rect.w, rect.y + rect.h / 2);
      ctx.lineTo(drag.x, drag.y);
      ctx.stroke();
      ctx.setLineDash([]);
    }
  }
}

function drawBar(
  ctx: CanvasRenderingContext2D,
  task: Task,
  row: number,
  origin: number,
  px: number,
  color: string,
  baseline: boolean,
  start: number,
  finish: number,
  pieces: Segment[] | null,
) {
  const shaped = { ...task, start, finish };
  const rect = barBox(shaped, row, origin, px);
  const y = baseline ? row * ROW + 20 : rect.y;
  const h = baseline ? 5 : rect.h;
  ctx.fillStyle = color;
  if (pieces) {
    // Split task: one block per segment, joined by a dotted line across each gap.
    const spans = pieces.map((piece) => pieceSpan(piece, origin, px));
    spans.forEach((span) => ctx.fillRect(span.x, y, span.w, h));
    ctx.save();
    ctx.strokeStyle = color;
    ctx.lineWidth = 1.2;
    ctx.setLineDash([2, 3]);
    for (let index = 0; index + 1 < spans.length; index += 1) {
      const from = spans[index].x + spans[index].w;
      ctx.beginPath();
      ctx.moveTo(from, y + h / 2);
      ctx.lineTo(spans[index + 1].x, y + h / 2);
      ctx.stroke();
    }
    ctx.setLineDash([]);
    if (!baseline) {
      const last = spans[spans.length - 1];
      ctx.fillStyle = "#fff";
      ctx.beginPath();
      ctx.arc(last.x + last.w, y + h / 2, 3.5, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
    }
    ctx.restore();
    return;
  }
  if (task.milestone && !baseline) {
    const size = 7;
    const cx = rect.x;
    const cy = row * ROW + ROW / 2;
    ctx.beginPath();
    ctx.moveTo(cx, cy - size);
    ctx.lineTo(cx + size, cy);
    ctx.lineTo(cx, cy + size);
    ctx.lineTo(cx - size, cy);
    ctx.closePath();
    ctx.fill();
    ctx.fillStyle = "#fff";
    ctx.beginPath();
    ctx.arc(cx + 11, cy, 3.5, 0, Math.PI * 2);
    ctx.fill();
    ctx.strokeStyle = color;
    ctx.stroke();
    return;
  }
  ctx.fillRect(rect.x, y, Math.max(rect.w, 2), h);
  if (task.summary && !baseline) {
    ctx.beginPath();
    ctx.moveTo(rect.x, y);
    ctx.lineTo(rect.x + 7, y);
    ctx.lineTo(rect.x, y + h);
    ctx.moveTo(rect.x + rect.w, y);
    ctx.lineTo(rect.x + rect.w - 7, y);
    ctx.lineTo(rect.x + rect.w, y + h);
    ctx.fill();
  } else if (!baseline && !task.summary) {
    ctx.fillStyle = "#fff";
    ctx.beginPath();
    ctx.arc(rect.x + rect.w, y + h / 2, 3.5, 0, Math.PI * 2);
    ctx.fill();
    ctx.strokeStyle = color;
    ctx.stroke();
  }
}

function sizeCanvas(canvas: HTMLCanvasElement, width: number, height: number, ratio: number) {
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;
  canvas.style.minWidth = `${width}px`;
  canvas.style.maxWidth = "none";
  canvas.style.flexShrink = "0";
  canvas.width = Math.max(1, Math.floor(width * ratio));
  canvas.height = Math.max(1, Math.floor(height * ratio));
}


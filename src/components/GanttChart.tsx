import { useEffect, useMemo, useRef, useState, type RefObject } from "react";

import { shiftDays, utcMidnight } from "../format";
import type { Project, Scale, Task } from "../types";

const ROW = 28;
const HEAD = 48;

interface DragState {
  kind: "move" | "resize" | "link";
  uid: number;
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
  const headCanvas = useRef<HTMLCanvasElement>(null);
  const bodyCanvas = useRef<HTMLCanvasElement>(null);
  const [drag, setDrag] = useState<DragState | null>(null);
  const px = scale === "day" ? 28 : scale === "week" ? 14 : 5;
  const model = useMemo(() => buildModel(project), [project]);

  function paint() {
    const head = headCanvas.current;
    const canvas = bodyCanvas.current;
    const scroller = bodyRef.current;
    if (!head || !canvas || !scroller) return;
    paintHeader(head, model, px);
    paintBody(
      canvas,
      project,
      model,
      px,
      selected,
      tracking,
      drag,
      scroller.scrollTop,
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
  }, [project, model, px, selected, tracking, drag]);

  function contentPoint(event: { clientX: number; clientY: number; currentTarget: HTMLCanvasElement }) {
    const bounds = event.currentTarget.getBoundingClientRect();
    return {
      x: event.clientX - bounds.left,
      y: event.clientY - bounds.top + (bodyRef.current?.scrollTop ?? 0),
    };
  }

  function hit(x: number, y: number) {
    const row = Math.floor(y / ROW);
    const task = project.tasks[row];
    if (!task) return null;
    const rect = barBox(task, row, model.origin, px);
    const handle = { x: rect.x + Math.max(rect.w, 8), y: rect.y + rect.h / 2 };
    if (Math.hypot(x - handle.x, y - handle.y) <= 7) return { task, zone: "link" as const, rect };
    if (!task.summary && !task.milestone && x >= rect.x + rect.w - 6 && x <= rect.x + rect.w + 2 && y >= rect.y && y <= rect.y + rect.h) {
      return { task, zone: "resize" as const, rect };
    }
    if (x >= rect.x - 4 && x <= rect.x + Math.max(rect.w, 10) && y >= rect.y - 2 && y <= rect.y + rect.h + 2) {
      return { task, zone: "body" as const, rect };
    }
    return { task, zone: "row" as const, rect };
  }

  return (
    <div className="gantt">
      <div className="gantt-head" ref={headRef}>
        <canvas ref={headCanvas} />
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
              model,
              px,
              selected,
              tracking,
              drag,
              event.currentTarget.scrollTop,
              event.currentTarget.clientHeight,
            );
          }
          onScroll();
        }}
      >
        <div
          style={{
            position: "relative",
            width: model.days.length * px,
            height: Math.max(project.tasks.length, 1) * ROW,
          }}
        >
        <canvas
          ref={bodyCanvas}
          style={{ position: "sticky", top: 0, display: "block" }}
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

interface Model {
  origin: number;
  days: number[];
}

function buildModel(project: Project): Model {
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

function barBox(task: Task, row: number, origin: number, px: number) {
  const x = ((task.start - origin) / 86400000) * px;
  const finishX = ((Math.max(task.finish, task.start) - origin) / 86400000) * px;
  const w = Math.max(finishX - x, task.milestone ? 0 : 8);
  const y = task.summary ? row * ROW + 12 : row * ROW + 6;
  const h = task.summary ? 6 : 16;
  return { x, y, w, h };
}

function paintHeader(canvas: HTMLCanvasElement, model: Model, px: number) {
  const width = model.days.length * px;
  const ratio = window.devicePixelRatio || 1;
  sizeCanvas(canvas, width, HEAD, ratio);
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
  ctx.clearRect(0, 0, width, HEAD);
  ctx.fillStyle = "#f6f6f8";
  ctx.fillRect(0, 0, width, HEAD);
  ctx.font = "11px -apple-system, sans-serif";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  let groupStart = 0;
  for (let index = 0; index <= model.days.length; index += 1) {
    const day = model.days[index];
    const previous = model.days[index - 1];
    const split = index === model.days.length || !previous || groupLabel(previous, px) !== groupLabel(day, px);
    if (split && index > 0) {
      const label = groupLabel(model.days[groupStart], px);
      const x = groupStart * px;
      const w = (index - groupStart) * px;
      ctx.fillStyle = "#1d1d1f";
      ctx.fillText(label, x + w / 2, 14, w - 4);
      ctx.strokeStyle = "#d5d5da";
      ctx.beginPath();
      ctx.moveTo(x + w, 0);
      ctx.lineTo(x + w, HEAD);
      ctx.stroke();
      groupStart = index;
    }
    if (!day) continue;
    ctx.fillStyle = isWeekend(day) ? "#8e8e93" : "#3a3a3c";
    ctx.fillText(dayLabel(day, px), index * px + px / 2, 34, px - 2);
    ctx.strokeStyle = "#e4e4e8";
    ctx.beginPath();
    ctx.moveTo(index * px, 24);
    ctx.lineTo(index * px, HEAD);
    ctx.stroke();
  }
  ctx.strokeStyle = "#d0d0d5";
  ctx.beginPath();
  ctx.moveTo(0, HEAD - 0.5);
  ctx.lineTo(width, HEAD - 0.5);
  ctx.stroke();
}

function paintBody(
  canvas: HTMLCanvasElement,
  project: Project,
  model: Model,
  px: number,
  selected: number | null,
  tracking: boolean,
  drag: DragState | null,
  scrollTop: number,
  viewHeight: number,
) {
  const width = model.days.length * px;
  const contentHeight = Math.max(project.tasks.length, 1) * ROW;
  const canvasHeight = Math.max(1, Math.min(Math.max(viewHeight, ROW), contentHeight));
  const ratio = window.devicePixelRatio || 1;
  sizeCanvas(canvas, width, canvasHeight, ratio);
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.setTransform(ratio, 0, 0, ratio, 0, -scrollTop * ratio);
  ctx.clearRect(0, scrollTop, width, canvasHeight);
  const windowHeight = Math.max(viewHeight, canvasHeight);
  const rowStart = Math.max(0, Math.floor(scrollTop / ROW) - 6);
  const rowEnd = Math.min(project.tasks.length, rowStart + Math.ceil(windowHeight / ROW) + 12);
  const holidays = new Set(Object.keys(project.calendars.find((item) => item.id === project.calendar_id)?.exceptions ?? {}).filter((key) => (project.calendars.find((item) => item.id === project.calendar_id)?.exceptions[key] ?? []).length === 0));
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
    const task = project.tasks[row];
    const y = row * ROW;
    if (task.uid === selected) {
      ctx.fillStyle = "rgba(10, 132, 255, 0.08)";
      ctx.fillRect(0, y, width, ROW);
    }
    ctx.strokeStyle = "#f0f0f2";
    ctx.beginPath();
    ctx.moveTo(0, y + ROW - 0.5);
    ctx.lineTo(width, y + ROW - 0.5);
    ctx.stroke();
    if (tracking) {
      const baseline = task.baselines.find((item) => item.number === 0);
      if (baseline) drawBar(ctx, task, row, model.origin, px, "#c5c5cb", true, baseline.start, baseline.finish);
    }
    const moved = drag && drag.moved && drag.uid === task.uid && drag.kind !== "link" ? drag : null;
    const start = moved?.kind === "move" ? shiftDays(task.start, Math.round((moved.x - moved.originX) / px)) : task.start;
    const finish = moved?.kind === "resize" ? shiftDays(task.finish, Math.round((moved.x - moved.originX) / px)) : task.finish;
    const preview = { ...task, start, finish };
    const color = task.critical ? "#c23b3b" : task.manual ? "#2a9d8f" : task.summary ? "#1d1d1f" : "#2f6fad";
    drawBar(ctx, preview, row, model.origin, px, color, false, preview.start, preview.finish);
    if (!task.summary && task.percent_complete > 0) {
      const rect = barBox(preview, row, model.origin, px);
      ctx.fillStyle = "rgba(0,0,0,0.28)";
      ctx.fillRect(rect.x, rect.y, rect.w * Math.min(task.percent_complete, 100) / 100, rect.h);
    }
  }
  const indexOf = new Map(project.tasks.map((task, index) => [task.uid, index]));
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
    const pred = project.tasks[from];
    const succ = project.tasks[to];
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
    const from = indexOf.get(drag.uid);
    if (from !== undefined) {
      const rect = barBox(project.tasks[from], from, model.origin, px);
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
) {
  const shaped = { ...task, start, finish };
  const rect = barBox(shaped, row, origin, px);
  const y = baseline ? row * ROW + 20 : rect.y;
  const h = baseline ? 5 : rect.h;
  ctx.fillStyle = color;
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
  canvas.width = Math.max(1, Math.floor(width * ratio));
  canvas.height = Math.max(1, Math.floor(height * ratio));
}

function isWeekend(ms: number): boolean {
  const day = new Date(ms).getUTCDay();
  return day === 0 || day === 6;
}

function isoDate(ms: number): string {
  const date = new Date(ms);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getUTCFullYear()}-${pad(date.getUTCMonth() + 1)}-${pad(date.getUTCDate())}`;
}

function groupLabel(ms: number, px: number): string {
  const date = new Date(ms);
  if (px >= 20) return `Tháng ${date.getUTCMonth() + 1} ${date.getUTCFullYear()}`;
  const onejan = Date.UTC(date.getUTCFullYear(), 0, 1);
  const week = Math.ceil(((ms - onejan) / 86400000 + new Date(onejan).getUTCDay() + 1) / 7);
  return px >= 10 ? `Tuần ${week}` : `T${date.getUTCMonth() + 1}/${date.getUTCFullYear()}`;
}

function dayLabel(ms: number, px: number): string {
  const date = new Date(ms);
  if (px < 10) return "";
  if (px < 20) return String(date.getUTCDate());
  const names = ["CN", "T2", "T3", "T4", "T5", "T6", "T7"];
  return `${names[date.getUTCDay()]} ${date.getUTCDate()}`;
}

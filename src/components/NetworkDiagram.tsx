import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import { formatDuration, formatInstant } from "../format";
import { barColor } from "../ganttLayout";
import { edgeLabel, nodeKind, occurrenceCounts } from "../networkLabels";
import type { NetworkEdge, NetworkLayout, NetworkNode, NetworkPoint, Project, Task } from "../types";

const MIN_ZOOM = 0.2;
const MAX_ZOOM = 2;
const PAD = 10;
const CRITICAL = "#c23b3b";
const LINK = "#6d6d72";
const SELECTED = "#0a84ff";

interface Props {
  project: Project;
  layout: NetworkLayout | null;
  selected: number | null;
  highlighted: Set<number>;
  onSelect: (uid: number) => void;
  onEdit: (uid: number) => void;
}

interface PanState {
  pointerId: number;
  x: number;
  y: number;
  left: number;
  top: number;
}

export function NetworkDiagram({ project, layout, selected, highlighted, onSelect, onEdit }: Props) {
  const bodyRef = useRef<HTMLDivElement>(null);
  const [zoom, setZoom] = useState(1);
  const [panning, setPanning] = useState(false);
  const pan = useRef<PanState | null>(null);
  // Content point kept under the cursor (or centre) while the zoom changes.
  const anchor = useRef<{ x: number; y: number; offsetX: number; offsetY: number } | null>(null);
  const tasks = useMemo(() => new Map(project.tasks.map((task) => [task.uid, task])), [project.tasks]);
  const occurrences = useMemo(() => occurrenceCounts(project.tasks), [project.tasks]);

  function zoomTo(next: number, offsetX?: number, offsetY?: number) {
    const body = bodyRef.current;
    const clamped = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, next));
    if (!body || clamped === zoom) return;
    const ox = offsetX ?? body.clientWidth / 2;
    const oy = offsetY ?? body.clientHeight / 2;
    anchor.current = {
      x: (body.scrollLeft + ox) / zoom,
      y: (body.scrollTop + oy) / zoom,
      offsetX: ox,
      offsetY: oy,
    };
    setZoom(clamped);
  }

  function fit() {
    const body = bodyRef.current;
    if (!body || !layout || layout.width === 0) return;
    const next = Math.min(1, body.clientWidth / layout.width, body.clientHeight / layout.height);
    anchor.current = null;
    setZoom(Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, next)));
    body.scrollLeft = 0;
    body.scrollTop = 0;
  }

  useLayoutEffect(() => {
    const body = bodyRef.current;
    const point = anchor.current;
    if (!body || !point) return;
    body.scrollLeft = point.x * zoom - point.offsetX;
    body.scrollTop = point.y * zoom - point.offsetY;
    anchor.current = null;
  }, [zoom]);

  // Pinch or Cmd/Ctrl + wheel zooms; plain wheel and trackpad scroll pan natively.
  const zoomRef = useRef((factor: number, offsetX: number, offsetY: number) => zoomTo(zoom * factor, offsetX, offsetY));
  zoomRef.current = (factor, offsetX, offsetY) => zoomTo(zoom * factor, offsetX, offsetY);
  useEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    const onWheel = (event: WheelEvent) => {
      if (!event.ctrlKey && !event.metaKey) return;
      event.preventDefault();
      const bounds = body.getBoundingClientRect();
      // Clamp so one mouse-wheel notch is a gentle step while pinch deltas stay smooth.
      const delta = Math.max(-50, Math.min(50, event.deltaY));
      const factor = Math.exp(-delta * 0.005);
      zoomRef.current(factor, event.clientX - bounds.left, event.clientY - bounds.top);
    };
    body.addEventListener("wheel", onWheel, { passive: false });
    return () => body.removeEventListener("wheel", onWheel);
  }, []);

  // Bring the shared selection into view, e.g. after switching from the Gantt.
  useEffect(() => {
    const body = bodyRef.current;
    const node = layout?.nodes.find((item) => item.uid === selected);
    if (!body || !node) return;
    const left = node.x * zoom;
    const top = node.y * zoom;
    const right = (node.x + node.width) * zoom;
    const bottom = (node.y + node.height) * zoom;
    const visible =
      left >= body.scrollLeft &&
      right <= body.scrollLeft + body.clientWidth &&
      top >= body.scrollTop &&
      bottom <= body.scrollTop + body.clientHeight;
    if (visible) return;
    body.scrollLeft = Math.max(0, (left + right) / 2 - body.clientWidth / 2);
    body.scrollTop = Math.max(0, (top + bottom) / 2 - body.clientHeight / 2);
    // Zoom changes keep their own anchor; only selection or layout changes scroll here.
  }, [selected, layout]);

  const width = layout ? layout.width * zoom : 0;
  const height = layout ? layout.height * zoom : 0;

  return (
    <div className="network">
      <div className="network-tools">
        <button type="button" title="Thu nhỏ" onClick={() => zoomTo(zoom / 1.25)}>−</button>
        <button type="button" title="Về 100%" onClick={() => zoomTo(1)}>{Math.round(zoom * 100)}%</button>
        <button type="button" title="Phóng to" onClick={() => zoomTo(zoom * 1.25)}>+</button>
        <button type="button" onClick={fit}>Vừa khung</button>
        <span className="network-hint">Kéo nền để cuộn · ⌘/Ctrl + cuộn để thu phóng · Nháy đúp để sửa</span>
      </div>
      <div
        className={`network-body${panning ? " panning" : ""}`}
        ref={bodyRef}
        onPointerDown={(event) => {
          if (event.button !== 0 || (event.target as Element).closest(".network-node")) return;
          const body = event.currentTarget;
          body.setPointerCapture(event.pointerId);
          pan.current = {
            pointerId: event.pointerId,
            x: event.clientX,
            y: event.clientY,
            left: body.scrollLeft,
            top: body.scrollTop,
          };
          setPanning(true);
        }}
        onPointerMove={(event) => {
          const state = pan.current;
          if (!state || state.pointerId !== event.pointerId) return;
          event.currentTarget.scrollLeft = state.left - (event.clientX - state.x);
          event.currentTarget.scrollTop = state.top - (event.clientY - state.y);
        }}
        onPointerUp={(event) => {
          if (pan.current?.pointerId !== event.pointerId) return;
          pan.current = null;
          setPanning(false);
        }}
        onPointerCancel={() => {
          pan.current = null;
          setPanning(false);
        }}
      >
        {layout && layout.nodes.length === 0 && <p className="empty">Không có công việc nào để vẽ sơ đồ mạng.</p>}
        {layout && layout.nodes.length > 0 && (
          <svg
            className="network-svg"
            width={width}
            height={height}
            viewBox={`0 0 ${layout.width} ${layout.height}`}
            role="img"
            aria-label="Sơ đồ mạng"
          >
            <defs>
              <marker id="network-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto">
                <path d="M0,1 L9,5 L0,9 z" fill={LINK} />
              </marker>
              <marker id="network-arrow-critical" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto">
                <path d="M0,1 L9,5 L0,9 z" fill={CRITICAL} />
              </marker>
            </defs>
            {layout.edges.map((edge) => (
              <EdgePath
                key={`${edge.uid}-${edge.from_uid}-${edge.to_uid}`}
                edge={edge}
                critical={edgeCritical(edge, tasks)}
              />
            ))}
            {layout.nodes.map((node) => {
              const task = tasks.get(node.uid);
              if (!task) return null;
              return (
                <TaskBox
                  key={node.uid}
                  node={node}
                  task={task}
                  kind={nodeKind(task, occurrences)}
                  selected={node.uid === selected}
                  highlighted={highlighted.has(node.uid)}
                  onSelect={onSelect}
                  onEdit={onEdit}
                />
              );
            })}
          </svg>
        )}
      </div>
    </div>
  );
}

function edgeCritical(edge: NetworkEdge, tasks: Map<number, Task>): boolean {
  return Boolean(tasks.get(edge.from_uid)?.critical && tasks.get(edge.to_uid)?.critical);
}

/** Straight runs through waypoints, smooth curves between columns. */
function edgePathData(points: NetworkPoint[]): string {
  if (points.length === 0) return "";
  let path = `M${points[0].x},${points[0].y}`;
  for (let index = 1; index < points.length; index += 1) {
    const from = points[index - 1];
    const to = points[index];
    if (from.y === to.y) {
      path += ` L${to.x},${to.y}`;
      continue;
    }
    const bend = (to.x - from.x) / 2;
    path += ` C${from.x + bend},${from.y} ${to.x - bend},${to.y} ${to.x},${to.y}`;
  }
  return path;
}

function EdgePath({ edge, critical }: { edge: NetworkEdge; critical: boolean }) {
  const label = edgeLabel(edge);
  const [a, b] = edge.points;
  const color = critical ? CRITICAL : LINK;
  return (
    <g className="network-edge">
      <path
        d={edgePathData(edge.points)}
        fill="none"
        stroke={color}
        strokeWidth={critical ? 1.8 : 1.2}
        markerEnd={`url(#${critical ? "network-arrow-critical" : "network-arrow"})`}
      />
      {label && a && b && (
        <text className="network-edge-label" x={(a.x + b.x) / 2} y={(a.y + b.y) / 2 - 4} textAnchor="middle" fill={color}>
          {label}
        </text>
      )}
    </g>
  );
}

function TaskBox({
  node,
  task,
  kind,
  selected,
  highlighted,
  onSelect,
  onEdit,
}: {
  node: NetworkNode;
  task: Task;
  /** "Lặp · 10 lần", "Tóm tắt", "Mốc", or empty for a plain task. */
  kind: string;
  selected: boolean;
  highlighted: boolean;
  onSelect: (uid: number) => void;
  onEdit: (uid: number) => void;
}) {
  const { width: w, height: h } = node;
  const stroke = barColor(task);
  const fill = highlighted ? "#fff3c4" : task.summary ? "#f3f3f6" : "white";
  const percent = Math.max(0, Math.min(100, task.percent_complete));
  const name = task.name.trim() || `Công việc ${task.id}`;
  // Keep text clear of the hexagon's slanted sides.
  const pad = task.milestone && !task.summary ? PAD + 12 : PAD;
  return (
    <g
      className="network-node"
      transform={`translate(${node.x},${node.y})`}
      onClick={() => onSelect(task.uid)}
      onDoubleClick={() => onEdit(task.uid)}
    >
      <title>{`${task.wbs} ${name}${task.critical ? " · Găng" : ""}`}</title>
      {selected && <Shape task={task} w={w} h={h} inset={-4} fill="none" stroke={SELECTED} strokeWidth={2.5} />}
      <Shape task={task} w={w} h={h} inset={0} fill={fill} stroke={stroke} strokeWidth={task.critical ? 2.4 : 1.4} />
      {task.summary && <rect x={1} y={1} width={w - 2} height={26} fill={stroke} opacity={0.12} />}
      <text className="network-name" x={w / 2} y={18} textAnchor="middle">
        {truncate(name, w - 2 * pad, 7)}
      </text>
      <line x1={task.milestone ? 14 : 1} x2={task.milestone ? w - 14 : w - 1} y1={27} y2={27} stroke={stroke} strokeOpacity={0.35} />
      <text className="network-field" x={pad} y={43}>
        {`ID ${task.id}${task.wbs ? ` · WBS ${task.wbs}` : ""}`}
      </text>
      <text className="network-field" x={w - pad} y={43} textAnchor="end">
        {kind || formatDuration(task.duration)}
      </text>
      <text className="network-field" x={pad} y={60}>
        <tspan className="network-label">Bắt đầu</tspan>
        <tspan x={w - pad} textAnchor="end">{formatInstant(task.start)}</tspan>
      </text>
      <text className="network-field" x={pad} y={76}>
        <tspan className="network-label">Kết thúc</tspan>
        <tspan x={w - pad} textAnchor="end">{formatInstant(task.finish)}</tspan>
      </text>
      <text className="network-field" x={pad} y={92}>
        <tspan className="network-label">Hoàn thành</tspan>
        <tspan x={w - pad} textAnchor="end">{`${Math.round(percent)}%`}</tspan>
      </text>
      {percent > 0 && (
        <rect x={pad} y={h - 6} width={((w - 2 * pad) * percent) / 100} height={3} rx={1.5} fill={stroke} opacity={0.6} />
      )}
    </g>
  );
}

/** Milestones are hexagons; tasks and summaries are rectangles (summaries get a tinted header). */
function Shape({
  task,
  w,
  h,
  inset,
  fill,
  stroke,
  strokeWidth,
}: {
  task: Task;
  w: number;
  h: number;
  inset: number;
  fill: string;
  stroke: string;
  strokeWidth: number;
}) {
  if (task.milestone && !task.summary) {
    const cut = 14;
    const x0 = inset;
    const y0 = inset;
    const x1 = w - inset;
    const y1 = h - inset;
    const points = [
      [x0 + cut, y0],
      [x1 - cut, y0],
      [x1, h / 2],
      [x1 - cut, y1],
      [x0 + cut, y1],
      [x0, h / 2],
    ]
      .map(([x, y]) => `${x},${y}`)
      .join(" ");
    return <polygon points={points} fill={fill} stroke={stroke} strokeWidth={strokeWidth} />;
  }
  return (
    <rect
      x={inset}
      y={inset}
      width={w - 2 * inset}
      height={h - 2 * inset}
      rx={task.summary ? 2 : 6}
      fill={fill}
      stroke={stroke}
      strokeWidth={strokeWidth}
    />
  );
}

/** Rough fit by average glyph width; the full name is in the box tooltip. */
function truncate(text: string, width: number, glyph: number): string {
  const max = Math.max(4, Math.floor(width / glyph));
  return text.length <= max ? text : `${text.slice(0, max - 1)}…`;
}

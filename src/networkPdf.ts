import { jsPDF } from "jspdf";

import { formatDuration, formatInstant } from "./format";
import { barColor } from "./ganttLayout";
import { embeddedFont } from "./ganttPdf";
import { pdfMessages } from "./ganttPdf.i18n";
import { pick } from "./i18n";
import { edgeLabel, nodeKind, occurrenceCounts } from "./networkLabels";
import { networkBoxMessages } from "./networkLabels.i18n";
import type { NetworkLayout, NetworkNode, NetworkPoint, Project, Task } from "./types";

const FONT = "BeVietnamPro";
const MIN_W = 297;
const MIN_H = 210;
/** PDF viewers refuse pages above 200 inches; stay below that. */
const MAX_SIDE = 5000;
const MARGIN = 8;
const TITLE_H = 8;
const PX_TO_MM = 0.26;
const PT_PER_MM = 1 / 0.3528;
const CRITICAL = "#c23b3b";
const LINK = "#6d6d72";

export interface NetworkPdfInput {
  project: Project;
  layout: NetworkLayout;
  highlighted: Set<number>;
}

/** One page sized to the diagram (at least A4 landscape), drawn from the engine layout. */
export function buildNetworkPdf(input: NetworkPdfInput): Uint8Array {
  const { project, layout } = input;
  const scale = Math.min(
    PX_TO_MM,
    (MAX_SIDE - 2 * MARGIN) / Math.max(1, layout.width),
    (MAX_SIDE - 2 * MARGIN - TITLE_H) / Math.max(1, layout.height),
  );
  const pageW = Math.max(MIN_W, layout.width * scale + 2 * MARGIN);
  const pageH = Math.max(MIN_H, layout.height * scale + 2 * MARGIN + TITLE_H);
  const doc = new jsPDF({
    orientation: pageW >= pageH ? "landscape" : "portrait",
    unit: "mm",
    format: [pageW, pageH],
    compress: true,
  });
  doc.addFileToVFS("BeVietnamPro-Regular.ttf", embeddedFont());
  doc.addFont("BeVietnamPro-Regular.ttf", FONT, "normal");
  doc.setFont(FONT, "normal");
  const pdf = pick(pdfMessages);
  const title = pdf.networkTitle(project.name.trim() || pdf.untitled);
  doc.setProperties({ title, creator: "ProjectDesk" });
  doc.setFontSize(11);
  setText(doc, "#1d1d1f");
  doc.text(title, MARGIN, MARGIN + 4);

  const ox = MARGIN;
  const oy = MARGIN + TITLE_H;
  const at = (point: NetworkPoint) => ({ x: ox + point.x * scale, y: oy + point.y * scale });
  const tasks = new Map(project.tasks.map((task) => [task.uid, task]));
  const occurrences = occurrenceCounts(project.tasks);

  for (const edge of layout.edges) {
    const critical = Boolean(tasks.get(edge.from_uid)?.critical && tasks.get(edge.to_uid)?.critical);
    const color = critical ? CRITICAL : LINK;
    const points = edge.points.map(at);
    drawEdge(doc, points, color, critical ? 0.45 : 0.3);
    const label = edgeLabel(edge);
    if (label && points.length >= 2) {
      doc.setFontSize(10 * scale * PT_PER_MM);
      setText(doc, color);
      doc.text(label, (points[0].x + points[1].x) / 2, (points[0].y + points[1].y) / 2 - 1, { align: "center" });
    }
  }
  for (const node of layout.nodes) {
    const task = tasks.get(node.uid);
    if (task) drawNode(doc, node, task, nodeKind(task, occurrences), input.highlighted.has(node.uid), ox, oy, scale);
  }
  return new Uint8Array(doc.output("arraybuffer"));
}

function drawEdge(doc: jsPDF, points: { x: number; y: number }[], color: string, width: number) {
  if (points.length < 2) return;
  setDraw(doc, color);
  doc.setLineWidth(width);
  const segments: number[][] = [];
  for (let index = 1; index < points.length; index += 1) {
    const from = points[index - 1];
    const to = points[index];
    const dx = to.x - from.x;
    const dy = to.y - from.y;
    if (dy === 0) segments.push([dx, 0]);
    else segments.push([dx / 2, 0, dx / 2, dy, dx, dy]);
  }
  doc.lines(segments, points[0].x, points[0].y, [1, 1], "S", false);
  const end = points[points.length - 1];
  setFill(doc, color);
  doc.triangle(end.x, end.y, end.x - 2, end.y - 1, end.x - 2, end.y + 1, "F");
}

function drawNode(
  doc: jsPDF,
  node: NetworkNode,
  task: Task,
  kindLabel: string,
  highlighted: boolean,
  ox: number,
  oy: number,
  scale: number,
) {
  const x = ox + node.x * scale;
  const y = oy + node.y * scale;
  const w = node.width * scale;
  const h = node.height * scale;
  const stroke = barColor(task);
  setDraw(doc, stroke);
  setFill(doc, highlighted ? "#fff3c4" : task.summary ? "#f3f3f6" : "#ffffff");
  doc.setLineWidth(task.critical ? 0.6 : 0.3);
  if (task.milestone && !task.summary) {
    const cut = 14 * scale;
    doc.lines(
      [
        [w - 2 * cut, 0],
        [cut, h / 2],
        [-cut, h / 2],
        [-(w - 2 * cut), 0],
        [-cut, -h / 2],
      ],
      x + cut,
      y,
      [1, 1],
      "FD",
      true,
    );
  } else {
    doc.roundedRect(x, y, w, h, task.summary ? 0.5 : 1.5, task.summary ? 0.5 : 1.5, "FD");
  }
  const pad = (task.milestone && !task.summary ? 22 : 10) * scale;
  const px = (value: number) => value * scale;
  const fieldPt = 10.5 * scale * PT_PER_MM;
  setText(doc, "#1d1d1f");
  doc.setFontSize(12 * scale * PT_PER_MM);
  const m = pick(networkBoxMessages);
  const name = task.name.trim() || m.taskFallback(task.id);
  doc.text(fitText(doc, name, w - 2 * pad), x + w / 2, y + px(18), { align: "center" });
  doc.setLineWidth(0.15);
  doc.line(x + (task.milestone ? px(14) : 0), y + px(27), x + w - (task.milestone ? px(14) : 0), y + px(27));
  doc.setFontSize(fieldPt);
  const kind = kindLabel || formatDuration(task.duration);
  doc.text(`ID ${task.id}${task.wbs ? ` · WBS ${task.wbs}` : ""}`, x + pad, y + px(43));
  doc.text(kind, x + w - pad, y + px(43), { align: "right" });
  const rows: [string, string, number][] = [
    [m.start, formatInstant(task.start), 60],
    [m.finish, formatInstant(task.finish), 76],
    [m.complete, `${Math.round(Math.max(0, Math.min(100, task.percent_complete)))}%`, 92],
  ];
  for (const [label, value, top] of rows) {
    setText(doc, "#6e6e73");
    doc.text(label, x + pad, y + px(top));
    setText(doc, "#1d1d1f");
    doc.text(value, x + w - pad, y + px(top), { align: "right" });
  }
}

function fitText(doc: jsPDF, text: string, maxWidth: number): string {
  if (doc.getTextWidth(text) <= maxWidth) return text;
  const chars = Array.from(text);
  let end = chars.length;
  while (end > 0 && doc.getTextWidth(`${chars.slice(0, end).join("")}...`) > maxWidth) end -= 1;
  return end === 0 ? "" : `${chars.slice(0, end).join("")}...`;
}

function setText(doc: jsPDF, hex: string) {
  const [r, g, b] = rgb(hex);
  doc.setTextColor(r, g, b);
}

function setFill(doc: jsPDF, hex: string) {
  const [r, g, b] = rgb(hex);
  doc.setFillColor(r, g, b);
}

function setDraw(doc: jsPDF, hex: string) {
  const [r, g, b] = rgb(hex);
  doc.setDrawColor(r, g, b);
}

function rgb(hex: string): [number, number, number] {
  const value = hex.replace("#", "");
  return [0, 2, 4].map((offset) => Number.parseInt(value.slice(offset, offset + 2), 16)) as [number, number, number];
}

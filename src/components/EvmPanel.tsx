import { CalendarClock, Flag, Maximize2, X } from "lucide-react";
import { useEffect, useState } from "react";

import { formatDateOnly, formatMoney } from "../format";
import { getLang, pick, useMessages } from "../i18n";
import type { BurndownReport, EvmMetrics, ProjectMetrics, SCurveReport } from "../types";
import { Button } from "./Button";
import { evmMessages } from "./EvmPanel.i18n";
import { Modal } from "./Modal";

interface EvmPanelProps {
  metrics: ProjectMetrics | null;
  onOpenProjectInfo: () => void;
  onOpenBaseline: () => void;
  onClose: () => void;
}

type MetricKey = keyof EvmMetrics;

// Each metric's label is the message with the same key in evmMessages.
const ROWS: { key: MetricKey; kind: "money" | "variance" | "index" }[] = [
  { key: "pv", kind: "money" },
  { key: "ev", kind: "money" },
  { key: "ac", kind: "money" },
  { key: "sv", kind: "variance" },
  { key: "cv", kind: "variance" },
  { key: "spi", kind: "index" },
  { key: "cpi", kind: "index" },
  { key: "bac", kind: "money" },
  { key: "eac", kind: "money" },
  { key: "etc", kind: "money" },
  { key: "vac", kind: "variance" },
];

/** Number locale of the interface language: "1,25" in Vietnamese, "1.25" in English. */
function numberLocale(): string {
  return getLang() === "en" ? "en-US" : "vi-VN";
}

function formatIndex(value: number): string {
  return new Intl.NumberFormat(numberLocale(), { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(value);
}

/** Work in hours: "12,5 giờ" in Vietnamese, "12.5 hrs" in English. */
export function formatWorkHours(minutes: number): string {
  const hours = new Intl.NumberFormat(numberLocale(), { maximumFractionDigits: 1 }).format(minutes / 60);
  return pick(evmMessages).hours(hours);
}

/** A chart axis date: "05/01" in Vietnamese, "Jan 5" in English. */
export function formatTickDate(ms: number): string {
  const date = new Date(ms);
  return pick(evmMessages).tickDate(date.getUTCDate(), date.getUTCMonth() + 1);
}

/** Axis money: "8,4 Tr" in Vietnamese, "8.4M" or "120K" in English. */
export function formatCompactMoney(value: number): string {
  return new Intl.NumberFormat(numberLocale(), { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

export function EvmPanel({ metrics, onOpenProjectInfo, onOpenBaseline, onClose }: EvmPanelProps) {
  const m = useMessages(evmMessages);
  const evm = metrics?.evm;
  const [large, setLarge] = useState(false);
  // A dialog hidden because metrics went away must not come back on its own.
  useEffect(() => {
    if (!metrics) setLarge(false);
  }, [metrics]);
  return (
    <aside className="evm-panel" aria-label={m.panel}>
      <div className="evm-head">
        <strong>{m.heading}</strong>
        <span className="evm-head-actions">
          <Button
            Icon={Maximize2}
            disabled={!metrics}
            aria-haspopup="dialog"
            title={m.enlargeTitle}
            onClick={() => setLarge(true)}
          >
            {m.enlarge}
          </Button>
          <Button Icon={X} onClick={onClose} aria-label={m.closePanel}>{m.close}</Button>
        </span>
      </div>
      {large && metrics && (
        <ChartsDialog metrics={metrics} onClose={() => setLarge(false)} />
      )}
      {!metrics && <p className="evm-note">{m.loading}</p>}
      {evm && (
        <>
          <p className="evm-note">
            {evm.status_date != null ? m.asOf(formatDateOnly(evm.status_date)) : ""}
          </p>
          {!evm.has_baseline && (
            <div className="evm-empty">
              {m.noBaseline}
              <Button Icon={Flag} onClick={onOpenBaseline}>{m.setBaseline}</Button>
            </div>
          )}
          {evm.status_date == null && (
            <div className="evm-empty">
              {m.noStatusDate}
              <Button Icon={CalendarClock} onClick={onOpenProjectInfo}>{m.setStatusDate}</Button>
            </div>
          )}
          {evm.metrics && <MetricTable metrics={evm.metrics} />}
        </>
      )}
      {metrics && <BurndownSection report={metrics.burndown} />}
      {metrics && <SCurveSection report={metrics.s_curve} />}
    </aside>
  );
}

/** Burn-down and S-curve at a readable size, over the window. */
function ChartsDialog({ metrics, onClose }: { metrics: ProjectMetrics; onClose: () => void }) {
  const m = useMessages(evmMessages);
  const status = metrics.evm.status_date;
  return (
    <Modal title={m.chartsDialog} className="charts-dialog" onClose={onClose}>
      {status != null && <p className="evm-note">{m.asOf(formatDateOnly(status))}</p>}
      <BurndownSection report={metrics.burndown} box={LARGE_CHART} />
      <SCurveSection report={metrics.s_curve} box={LARGE_CHART} />
    </Modal>
  );
}

function MetricTable({ metrics }: { metrics: EvmMetrics }) {
  const m = useMessages(evmMessages);
  return (
    <table className="evm-table">
      <tbody>
        {ROWS.map((row) => {
          const value = metrics[row.key];
          const negative = row.kind === "variance" ? value != null && value < 0 : row.kind === "index" && value != null && value < 1;
          return (
            <tr key={row.key}>
              <th scope="row">{m[row.key]}</th>
              <td className={negative ? "bad" : undefined} title={value == null ? m.undefinedHint : undefined}>
                {value == null ? "—" : row.kind === "index" ? formatIndex(value) : formatMoney(value)}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

interface ChartBox {
  width: number;
  height: number;
  left: number;
  right: number;
  top: number;
  bottom: number;
}

/** The small charts in the side panel. */
const CHART: ChartBox = { width: 280, height: 180, left: 44, right: 10, top: 10, bottom: 30 };
/** The same charts enlarged in their own window. */
const LARGE_CHART: ChartBox = { width: 880, height: 340, left: 56, right: 16, top: 14, bottom: 30 };

function BurndownSection({ report, box }: { report: BurndownReport; box?: ChartBox }) {
  const m = useMessages(evmMessages);
  const hasIdeal = report.ideal.length >= 2;
  const hasActual = report.actual.length > 0;
  return (
    <section className="burndown">
      <strong>{m.burndown}</strong>
      <p className="evm-note">
        {m.remainingNow(formatWorkHours(report.current_remaining_work))}
        {hasIdeal ? m.baselineWork(formatWorkHours(report.baseline_work)) : ""}
      </p>
      {!hasIdeal && !hasActual ? (
        <div className="evm-empty">{m.burndownEmpty}</div>
      ) : (
        <BurndownChart report={report} box={box} />
      )}
      {hasIdeal && !hasActual && <p className="evm-note">{m.noActualPoints}</p>}
    </section>
  );
}

function BurndownChart({ report, box = CHART }: { report: BurndownReport; box?: ChartBox }) {
  const m = useMessages(evmMessages);
  const dates = [
    ...report.ideal.map((point) => point.date),
    ...report.actual.map((point) => point.date),
    ...(report.status_date != null ? [report.status_date] : []),
  ];
  const minX = Math.min(...dates);
  const maxX = Math.max(...dates, minX + 24 * 60 * 60 * 1000);
  const maxY = Math.max(
    60,
    ...report.ideal.map((point) => point.remaining_work),
    ...report.actual.map((point) => point.remaining_work),
  );
  const plotW = box.width - box.left - box.right;
  const plotH = box.height - box.top - box.bottom;
  const x = (date: number) => box.left + ((date - minX) / (maxX - minX)) * plotW;
  const y = (minutes: number) => box.top + plotH - (minutes / maxY) * plotH;
  const path = (points: { date: number; remaining_work: number }[]) =>
    points.map((point, index) => `${index === 0 ? "M" : "L"}${x(point.date).toFixed(1)},${y(point.remaining_work).toFixed(1)}`).join(" ");
  const xTicks = [minX, minX + (maxX - minX) / 2, maxX];
  const yTicks = [0, maxY / 2, maxY];
  return (
    <figure className="burndown-figure">
      <svg
        width={box.width}
        height={box.height}
        viewBox={`0 0 ${box.width} ${box.height}`}
        role="img"
        aria-label={m.burndownChart}
      >
        <rect x={box.left} y={box.top} width={plotW} height={plotH} fill="#fafafa" stroke="#e4e4e8" />
        {yTicks.map((tick) => (
          <g key={`y-${tick}`}>
            <line x1={box.left} x2={box.left + plotW} y1={y(tick)} y2={y(tick)} stroke="#ececf0" />
            <text x={box.left - 4} y={y(tick) + 3} textAnchor="end" fontSize="10" fill="#6e6e73">
              {Math.round(tick / 60)}
            </text>
          </g>
        ))}
        {xTicks.map((tick, index) => (
          <text
            key={`x-${tick}`}
            x={x(tick)}
            y={box.height - box.bottom + 14}
            textAnchor={index === 0 ? "start" : index === xTicks.length - 1 ? "end" : "middle"}
            fontSize="10"
            fill="#6e6e73"
          >
            {formatTickDate(tick)}
          </text>
        ))}
        <text x={4} y={box.top + 8} fontSize="10" fill="#6e6e73">{m.hoursAxis}</text>
        {report.status_date != null && (
          <line
            x1={x(report.status_date)}
            x2={x(report.status_date)}
            y1={box.top}
            y2={box.top + plotH}
            stroke="#c23b3b"
            strokeDasharray="3 3"
          />
        )}
        {report.ideal.length >= 2 && (
          <path d={path(report.ideal)} fill="none" stroke="#8e8e93" strokeWidth={1.5} strokeDasharray="5 4" />
        )}
        {report.planned.length >= 2 && (
          <path d={path(report.planned)} fill="none" stroke="#2a9d8f" strokeWidth={1.2} />
        )}
        {report.actual.length > 0 && (
          <>
            <path d={path(report.actual)} fill="none" stroke="#0a84ff" strokeWidth={2} />
            {report.actual.map((point) => (
              <circle key={point.date} cx={x(point.date)} cy={y(point.remaining_work)} r={3} fill="#0a84ff">
                <title>{m.remainingAt(formatDateOnly(point.date), formatWorkHours(point.remaining_work))}</title>
              </circle>
            ))}
          </>
        )}
      </svg>
      <figcaption className="burndown-legend">
        <span><i className="swatch ideal" /> {m.ideal}</span>
        {report.planned.length >= 2 && (
          <span title={m.plannedTitle}>
            <i className="swatch planned" /> {m.planned}
          </span>
        )}
        <span><i className="swatch actual" /> {m.actual}</span>
        {report.status_date != null && <span><i className="swatch status" /> {m.statusDate}</span>}
      </figcaption>
    </figure>
  );
}

function SCurveSection({ report, box }: { report: SCurveReport; box?: ChartBox }) {
  const m = useMessages(evmMessages);
  return (
    <section className="burndown">
      <strong>{m.sCurve}</strong>
      {report.points.length < 2 ? (
        <div className="evm-empty">{m.sCurveEmpty}</div>
      ) : (
        <>
          <p className="evm-note">{m.sCurveNote(report.step !== "day")}</p>
          <SCurveChart report={report} box={box} />
        </>
      )}
    </section>
  );
}

type CurveKey = "pv" | "ev" | "ac";

// Each curve's legend is the message named by `label` in evmMessages.
const CURVES: { key: CurveKey; label: "curvePv" | "curveEv" | "curveAc"; color: string; dash?: string }[] = [
  { key: "pv", label: "curvePv", color: "#8e8e93", dash: "5 4" },
  { key: "ev", label: "curveEv", color: "#34a853" },
  { key: "ac", label: "curveAc", color: "#e8890c" },
];

function SCurveChart({ report, box = CHART }: { report: SCurveReport; box?: ChartBox }) {
  const m = useMessages(evmMessages);
  const points = report.points;
  const minX = points[0].date;
  const maxX = Math.max(points[points.length - 1].date, minX + 24 * 60 * 60 * 1000);
  const maxY = Math.max(1, ...points.flatMap((point) => [point.pv, point.ev ?? 0, point.ac ?? 0]));
  const plotW = box.width - box.left - box.right;
  const plotH = box.height - box.top - box.bottom;
  const x = (date: number) => box.left + ((date - minX) / (maxX - minX)) * plotW;
  const y = (value: number) => box.top + plotH - (value / maxY) * plotH;
  const path = (key: CurveKey) =>
    points
      .flatMap((point) => {
        const value = point[key];
        return value == null ? [] : [`${x(point.date).toFixed(1)},${y(value).toFixed(1)}`];
      })
      .map((xy, index) => `${index === 0 ? "M" : "L"}${xy}`)
      .join(" ");
  const xTicks = [minX, minX + (maxX - minX) / 2, maxX];
  const yTicks = [0, maxY / 2, maxY];
  return (
    <figure className="burndown-figure">
      <svg
        width={box.width}
        height={box.height}
        viewBox={`0 0 ${box.width} ${box.height}`}
        role="img"
        aria-label={m.sCurveChart}
      >
        <rect x={box.left} y={box.top} width={plotW} height={plotH} fill="#fafafa" stroke="#e4e4e8" />
        {yTicks.map((tick) => (
          <g key={`y-${tick}`}>
            <line x1={box.left} x2={box.left + plotW} y1={y(tick)} y2={y(tick)} stroke="#ececf0" />
            <text x={box.left - 4} y={y(tick) + 3} textAnchor="end" fontSize="10" fill="#6e6e73">
              {formatCompactMoney(tick)}
            </text>
          </g>
        ))}
        {xTicks.map((tick, index) => (
          <text
            key={`x-${tick}`}
            x={x(tick)}
            y={box.height - box.bottom + 14}
            textAnchor={index === 0 ? "start" : index === xTicks.length - 1 ? "end" : "middle"}
            fontSize="10"
            fill="#6e6e73"
          >
            {formatTickDate(tick)}
          </text>
        ))}
        {report.status_date != null && (
          <line
            x1={x(report.status_date)}
            x2={x(report.status_date)}
            y1={box.top}
            y2={box.top + plotH}
            stroke="#c23b3b"
            strokeDasharray="3 3"
          />
        )}
        {CURVES.map((curve) => (
          <path
            key={curve.key}
            d={path(curve.key)}
            fill="none"
            stroke={curve.color}
            strokeWidth={curve.dash ? 1.5 : 2}
            strokeDasharray={curve.dash}
          />
        ))}
        {points.map((point) => (
          <rect
            key={`hit-${point.date}`}
            x={x(point.date) - 3}
            y={box.top}
            width={6}
            height={plotH}
            fill="transparent"
          >
            <title>
              {[
                formatDateOnly(point.date),
                `PV ${formatMoney(point.pv)}`,
                point.ev != null ? `EV ${formatMoney(point.ev)}` : null,
                point.ac != null ? `AC ${formatMoney(point.ac)}` : null,
              ]
                .filter(Boolean)
                .join(" · ")}
            </title>
          </rect>
        ))}
      </svg>
      <figcaption className="burndown-legend">
        {CURVES.map((curve) => (
          <span key={curve.key}><i className={`swatch ${curve.key}`} /> {m[curve.label]}</span>
        ))}
        {report.status_date != null && <span><i className="swatch status" /> {m.statusDate}</span>}
      </figcaption>
    </figure>
  );
}

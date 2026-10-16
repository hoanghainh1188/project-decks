import { formatDateOnly } from "../../format";
import { useMessages } from "../../i18n";
import type { Forecast, Project } from "../../types";
import { forecastViewMessages } from "./ForecastView.i18n";
import { histogramBars, middleOfDay, ON_TIME_WARN, riskyTasks } from "./forecastData";
import type { ForecastState } from "./useForecast";

const DAY_MS = 86_400_000;

/** Whole calendar days from `from` to `to`, by their dates. */
function daysBetween(from: number, to: number): number {
  return Math.floor(to / DAY_MS) - Math.floor(from / DAY_MS);
}

function percent(share: number): string {
  return `${Math.round(share * 100)}%`;
}

/**
 * Dự báo: the finish forecast of the project. The project finish as scheduled, once
 * leveled and at three chances; the spread of finish dates over the runs; and the tasks
 * worth a look. The numbers come from the engine (`forecast`), run by `useForecast`.
 */
export function ForecastView({
  project,
  state,
  onPickTask,
}: {
  project: Project;
  state: ForecastState;
  onPickTask: (uid: number) => void;
}) {
  const m = useMessages(forecastViewMessages);
  const hasRanges = project.tasks.some((task) => task.work_low != null && !task.summary && !task.milestone);
  const { result } = state;
  return (
    <div className="forecast-view">
      {state.error && (
        <p className="bad" role="alert">
          {state.error}
        </p>
      )}
      <p className="meta" role="status">
        {state.running ? (result ? m.refreshing : m.working) : ""}
      </p>
      {!hasRanges && <p className="meta">{m.noRanges}</p>}
      {result && (
        // While it works out the latest change, the last numbers stay, dimmed.
        <div className={`forecast-result${state.current ? "" : " stale"}`}>
          <Summary result={result} />
          <Spread result={result} />
          <Risky project={project} result={result} onPickTask={onPickTask} />
          <p className="meta">{m.limits}</p>
        </div>
      )}
    </div>
  );
}

function Summary({ result }: { result: Forecast }) {
  const m = useMessages(forecastViewMessages);
  const shift = result.leveled_finish !== result.planned_finish;
  const card = (label: string, date: number, note?: string) => (
    <li className="forecast-card">
      <span className="forecast-card-label">{label}</span>
      <strong>{formatDateOnly(date)}</strong>
      {note && <span className="meta">{note}</span>}
    </li>
  );
  const against = (date: number) => m.vsLeveled(daysBetween(result.leveled_finish, date));
  return (
    <ul className="forecast-summary" aria-label={m.summary}>
      {card(m.planned, result.planned_finish)}
      {card(m.leveled, result.leveled_finish, shift ? m.vsPlanned(daysBetween(result.planned_finish, result.leveled_finish)) : undefined)}
      {card(m.chance(50), result.finish.p50, against(result.finish.p50))}
      {card(m.chance(80), result.finish.p80, against(result.finish.p80))}
      {card(m.chance(90), result.finish.p90, against(result.finish.p90))}
    </ul>
  );
}

const WIDTH = 720;
const HEIGHT = 196;
// The marks' names sit in the band above the bars, where nothing covers them.
const PAD = { top: 26, right: 12, bottom: 24, left: 12 };

/** Bars of runs finishing on each day (or week), the share finished so far as a line, and the P50/P80/P90 marks. */
function Spread({ result }: { result: Forecast }) {
  const m = useMessages(forecastViewMessages);
  const bars = histogramBars(result.histogram, result.iterations);
  if (bars.length === 0) return null;
  const plotW = WIDTH - PAD.left - PAD.right;
  const plotH = HEIGHT - PAD.top - PAD.bottom;
  const first = bars[0].start;
  const span = bars.length * bars[0].days * DAY_MS;
  const x = (ms: number) => PAD.left + ((ms - first) / span) * plotW;
  const barW = plotW / bars.length;
  const tallest = Math.max(...bars.map((bar) => bar.count));
  const line = bars.map((bar, index) => `${PAD.left + (index + 1) * barW},${PAD.top + plotH * (1 - bar.cumulative)}`).join(" ");
  const marks: [string, number][] = [
    ["P50", result.finish.p50],
    ["P80", result.finish.p80],
    ["P90", result.finish.p90],
  ];
  const label = m.chart(result.iterations, formatDateOnly(result.finish.p50), formatDateOnly(result.finish.p90));
  return (
    <figure className="forecast-spread">
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} role="img" aria-label={label}>
        {bars.map((bar, index) => {
          const h = (bar.count / tallest) * plotH;
          return (
            <rect
              key={bar.start}
              x={PAD.left + index * barW + 1}
              y={PAD.top + plotH - h}
              width={Math.max(1, barW - 2)}
              height={h}
              style={{ fill: "var(--chart-planned)" }}
            />
          );
        })}
        <polyline points={`${PAD.left},${PAD.top + plotH} ${line}`} style={{ fill: "none", stroke: "var(--chart-ac)", strokeWidth: 2 }} />
        {marks.map(([name, date]) => {
          const at = Math.min(WIDTH - PAD.right, Math.max(PAD.left, x(middleOfDay(date))));
          return (
            <g key={name}>
              <line x1={at} x2={at} y1={PAD.top - 6} y2={PAD.top + plotH} style={{ stroke: "var(--text-muted)", strokeDasharray: "4 3" }} />
              <text x={at} y={PAD.top - 10} textAnchor="middle" style={{ fill: "var(--text-muted)", fontSize: 11 }}>
                {name}
              </text>
            </g>
          );
        })}
        <text x={PAD.left} y={HEIGHT - 6} style={{ fill: "var(--text-muted)", fontSize: 11 }}>
          {formatDateOnly(first)}
        </text>
        <text x={WIDTH - PAD.right} y={HEIGHT - 6} textAnchor="end" style={{ fill: "var(--text-muted)", fontSize: 11 }}>
          {formatDateOnly(bars[bars.length - 1].start + (bars[0].days - 1) * DAY_MS)}
        </text>
      </svg>
      <figcaption className="meta">{bars[0].days === 7 ? m.byWeek : m.byDay}</figcaption>
    </figure>
  );
}

function Risky({ project, result, onPickTask }: { project: Project; result: Forecast; onPickTask: (uid: number) => void }) {
  const m = useMessages(forecastViewMessages);
  const rows = riskyTasks(project, result);
  return (
    <section className="forecast-risky">
      <h3 className="dialog-subhead">{m.risky}</h3>
      <p className="meta">{m.riskyNote}</p>
      {rows.length === 0 ? (
        <p className="meta">{m.none}</p>
      ) : (
        <div className="suggestion-wrap">
          <table className="suggestions" aria-label={m.risky}>
            <thead>
              <tr>
                <th scope="col">{m.task}</th>
                <th scope="col">{m.deadline}</th>
                <th scope="col">{m.p50}</th>
                <th scope="col">{m.p90}</th>
                <th scope="col">{m.onTime}</th>
                <th scope="col">{m.critical}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map(({ task, forecast }) => (
                <tr key={task.uid}>
                  <th scope="row">
                    <button type="button" className="link-button" onClick={() => onPickTask(task.uid)}>
                      {task.name.trim() || String(task.id)}
                    </button>
                  </th>
                  <td className="num">{task.deadline == null ? "" : formatDateOnly(task.deadline)}</td>
                  <td className="num">{formatDateOnly(forecast.p50)}</td>
                  <td className="num">{formatDateOnly(forecast.p90)}</td>
                  <td className={`num${forecast.on_time != null && forecast.on_time < ON_TIME_WARN ? " bad" : ""}`}>
                    {forecast.on_time == null ? "" : percent(forecast.on_time)}
                  </td>
                  <td className="num">{percent(forecast.criticality)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

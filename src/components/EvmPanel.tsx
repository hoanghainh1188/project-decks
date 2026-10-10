import { formatDateOnly, formatMoney } from "../format";
import type { BurndownReport, EvmMetrics, ProjectMetrics, SCurveReport } from "../types";

interface EvmPanelProps {
  metrics: ProjectMetrics | null;
  onOpenProjectInfo: () => void;
  onOpenBaseline: () => void;
  onClose: () => void;
}

type MetricKey = keyof EvmMetrics;

const ROWS: { key: MetricKey; label: string; kind: "money" | "variance" | "index" }[] = [
  { key: "pv", label: "Giá trị kế hoạch (PV, BCWS)", kind: "money" },
  { key: "ev", label: "Giá trị đạt được (EV, BCWP)", kind: "money" },
  { key: "ac", label: "Chi phí thực tế (AC, ACWP)", kind: "money" },
  { key: "sv", label: "Chênh lệch tiến độ (SV)", kind: "variance" },
  { key: "cv", label: "Chênh lệch chi phí (CV)", kind: "variance" },
  { key: "spi", label: "Chỉ số tiến độ (SPI)", kind: "index" },
  { key: "cpi", label: "Chỉ số chi phí (CPI)", kind: "index" },
  { key: "bac", label: "Ngân sách khi hoàn thành (BAC)", kind: "money" },
  { key: "eac", label: "Ước tính khi hoàn thành (EAC)", kind: "money" },
  { key: "etc", label: "Ước tính phần còn lại (ETC)", kind: "money" },
  { key: "vac", label: "Chênh lệch khi hoàn thành (VAC)", kind: "variance" },
];

const UNDEFINED_HINT = "Không xác định: mẫu số không lớn hơn 0";

function formatIndex(value: number): string {
  return new Intl.NumberFormat("vi-VN", { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(value);
}

function formatWorkHours(minutes: number): string {
  const hours = new Intl.NumberFormat("vi-VN", { maximumFractionDigits: 1 }).format(minutes / 60);
  return `${hours} giờ`;
}

export function EvmPanel({ metrics, onOpenProjectInfo, onOpenBaseline, onClose }: EvmPanelProps) {
  const evm = metrics?.evm;
  return (
    <aside className="evm-panel" aria-label="Chỉ số EVM và burn-down">
      <div className="evm-head">
        <strong>Chỉ số EVM</strong>
        <button type="button" onClick={onClose} aria-label="Đóng bảng EVM">Đóng</button>
      </div>
      {!metrics && <p className="evm-note">Đang tải số liệu…</p>}
      {evm && (
        <>
          <p className="evm-note">
            {evm.status_date != null ? `Tính đến ngày trạng thái ${formatDateOnly(evm.status_date)}.` : ""}
          </p>
          {!evm.has_baseline && (
            <div className="evm-empty">
              Chưa có đường cơ sở 0, nên chưa tính được EVM và đường lý tưởng.
              <button type="button" onClick={onOpenBaseline}>Đặt đường cơ sở…</button>
            </div>
          )}
          {evm.status_date == null && (
            <div className="evm-empty">
              Chưa có ngày trạng thái, nên chưa tính được giá trị kế hoạch.
              <button type="button" onClick={onOpenProjectInfo}>Đặt ngày trạng thái…</button>
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

function MetricTable({ metrics }: { metrics: EvmMetrics }) {
  return (
    <table className="evm-table">
      <tbody>
        {ROWS.map((row) => {
          const value = metrics[row.key];
          const negative = row.kind === "variance" ? value != null && value < 0 : row.kind === "index" && value != null && value < 1;
          return (
            <tr key={row.key}>
              <th scope="row">{row.label}</th>
              <td className={negative ? "bad" : undefined} title={value == null ? UNDEFINED_HINT : undefined}>
                {value == null ? "—" : row.kind === "index" ? formatIndex(value) : formatMoney(value)}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

const CHART = { width: 280, height: 180, left: 44, right: 10, top: 10, bottom: 30 };

function BurndownSection({ report }: { report: BurndownReport }) {
  const hasIdeal = report.ideal.length >= 2;
  const hasActual = report.actual.length > 0;
  return (
    <section className="burndown">
      <strong>Burn-down</strong>
      <p className="evm-note">
        Việc còn lại hiện tại: {formatWorkHours(report.current_remaining_work)}
        {hasIdeal ? ` · Công việc baseline: ${formatWorkHours(report.baseline_work)}` : ""}
      </p>
      {!hasIdeal && !hasActual ? (
        <div className="evm-empty">
          Chưa có dữ liệu. Đặt đường cơ sở 0 để có đường lý tưởng; đường thực tế thêm một điểm mỗi lần lưu tệp hoặc đổi ngày trạng thái.
        </div>
      ) : (
        <BurndownChart report={report} />
      )}
      {hasIdeal && !hasActual && (
        <p className="evm-note">Chưa có điểm thực tế. Lưu tệp hoặc đổi ngày trạng thái để ghi điểm đầu tiên.</p>
      )}
    </section>
  );
}

function BurndownChart({ report }: { report: BurndownReport }) {
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
  const plotW = CHART.width - CHART.left - CHART.right;
  const plotH = CHART.height - CHART.top - CHART.bottom;
  const x = (date: number) => CHART.left + ((date - minX) / (maxX - minX)) * plotW;
  const y = (minutes: number) => CHART.top + plotH - (minutes / maxY) * plotH;
  const path = (points: { date: number; remaining_work: number }[]) =>
    points.map((point, index) => `${index === 0 ? "M" : "L"}${x(point.date).toFixed(1)},${y(point.remaining_work).toFixed(1)}`).join(" ");
  const xTicks = [minX, minX + (maxX - minX) / 2, maxX];
  const yTicks = [0, maxY / 2, maxY];
  return (
    <figure className="burndown-figure">
      <svg
        width={CHART.width}
        height={CHART.height}
        viewBox={`0 0 ${CHART.width} ${CHART.height}`}
        role="img"
        aria-label="Biểu đồ burn-down: việc còn lại theo ngày"
      >
        <rect x={CHART.left} y={CHART.top} width={plotW} height={plotH} fill="#fafafa" stroke="#e4e4e8" />
        {yTicks.map((tick) => (
          <g key={`y-${tick}`}>
            <line x1={CHART.left} x2={CHART.left + plotW} y1={y(tick)} y2={y(tick)} stroke="#ececf0" />
            <text x={CHART.left - 4} y={y(tick) + 3} textAnchor="end" fontSize="10" fill="#6e6e73">
              {Math.round(tick / 60)}
            </text>
          </g>
        ))}
        {xTicks.map((tick, index) => (
          <text
            key={`x-${tick}`}
            x={x(tick)}
            y={CHART.height - CHART.bottom + 14}
            textAnchor={index === 0 ? "start" : index === xTicks.length - 1 ? "end" : "middle"}
            fontSize="10"
            fill="#6e6e73"
          >
            {formatDateOnly(tick).slice(0, 5)}
          </text>
        ))}
        <text x={4} y={CHART.top + 8} fontSize="10" fill="#6e6e73">giờ</text>
        {report.status_date != null && (
          <line
            x1={x(report.status_date)}
            x2={x(report.status_date)}
            y1={CHART.top}
            y2={CHART.top + plotH}
            stroke="#c23b3b"
            strokeDasharray="3 3"
          />
        )}
        {report.ideal.length >= 2 && (
          <path d={path(report.ideal)} fill="none" stroke="#8e8e93" strokeWidth={1.5} strokeDasharray="5 4" />
        )}
        {report.actual.length > 0 && (
          <>
            <path d={path(report.actual)} fill="none" stroke="#0a84ff" strokeWidth={2} />
            {report.actual.map((point) => (
              <circle key={point.date} cx={x(point.date)} cy={y(point.remaining_work)} r={3} fill="#0a84ff">
                <title>{`${formatDateOnly(point.date)}: còn ${formatWorkHours(point.remaining_work)}`}</title>
              </circle>
            ))}
          </>
        )}
      </svg>
      <figcaption className="burndown-legend">
        <span><i className="swatch ideal" /> Lý tưởng (baseline 0)</span>
        <span><i className="swatch actual" /> Thực tế</span>
        {report.status_date != null && <span><i className="swatch status" /> Ngày trạng thái</span>}
      </figcaption>
    </figure>
  );
}

function formatCompactMoney(value: number): string {
  return new Intl.NumberFormat("vi-VN", { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

function SCurveSection({ report }: { report: SCurveReport }) {
  return (
    <section className="burndown">
      <strong>Đường cong S</strong>
      {report.points.length < 2 ? (
        <div className="evm-empty">Cần đường cơ sở 0 để vẽ đường cong S.</div>
      ) : (
        <>
          <p className="evm-note">
            Giá trị cộng dồn theo {report.step === "day" ? "ngày" : "tuần"}. EV và AC tính đến ngày trạng thái.
          </p>
          <SCurveChart report={report} />
        </>
      )}
    </section>
  );
}

type CurveKey = "pv" | "ev" | "ac";

const CURVES: { key: CurveKey; label: string; color: string; dash?: string }[] = [
  { key: "pv", label: "PV (kế hoạch)", color: "#8e8e93", dash: "5 4" },
  { key: "ev", label: "EV (đạt được)", color: "#34a853" },
  { key: "ac", label: "AC (thực tế)", color: "#e8890c" },
];

function SCurveChart({ report }: { report: SCurveReport }) {
  const points = report.points;
  const minX = points[0].date;
  const maxX = Math.max(points[points.length - 1].date, minX + 24 * 60 * 60 * 1000);
  const maxY = Math.max(1, ...points.flatMap((point) => [point.pv, point.ev ?? 0, point.ac ?? 0]));
  const plotW = CHART.width - CHART.left - CHART.right;
  const plotH = CHART.height - CHART.top - CHART.bottom;
  const x = (date: number) => CHART.left + ((date - minX) / (maxX - minX)) * plotW;
  const y = (value: number) => CHART.top + plotH - (value / maxY) * plotH;
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
        width={CHART.width}
        height={CHART.height}
        viewBox={`0 0 ${CHART.width} ${CHART.height}`}
        role="img"
        aria-label="Đường cong S: PV, EV và AC cộng dồn theo thời gian"
      >
        <rect x={CHART.left} y={CHART.top} width={plotW} height={plotH} fill="#fafafa" stroke="#e4e4e8" />
        {yTicks.map((tick) => (
          <g key={`y-${tick}`}>
            <line x1={CHART.left} x2={CHART.left + plotW} y1={y(tick)} y2={y(tick)} stroke="#ececf0" />
            <text x={CHART.left - 4} y={y(tick) + 3} textAnchor="end" fontSize="10" fill="#6e6e73">
              {formatCompactMoney(tick)}
            </text>
          </g>
        ))}
        {xTicks.map((tick, index) => (
          <text
            key={`x-${tick}`}
            x={x(tick)}
            y={CHART.height - CHART.bottom + 14}
            textAnchor={index === 0 ? "start" : index === xTicks.length - 1 ? "end" : "middle"}
            fontSize="10"
            fill="#6e6e73"
          >
            {formatDateOnly(tick).slice(0, 5)}
          </text>
        ))}
        {report.status_date != null && (
          <line
            x1={x(report.status_date)}
            x2={x(report.status_date)}
            y1={CHART.top}
            y2={CHART.top + plotH}
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
            y={CHART.top}
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
          <span key={curve.key}><i className={`swatch ${curve.key}`} /> {curve.label}</span>
        ))}
        {report.status_date != null && <span><i className="swatch status" /> Ngày trạng thái</span>}
      </figcaption>
    </figure>
  );
}

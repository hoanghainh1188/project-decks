import { ChartGantt, ListChecks, Network, Table2, Users, type LucideIcon } from "lucide-react";
import { useRef, type KeyboardEvent } from "react";

import type { Project, Scale, ViewId } from "../types";
import type { FilterMode, GroupMode, HighlightMode } from "../viewRows";

const VIEWS: { id: ViewId; label: string; Icon: LucideIcon }[] = [
  { id: "gantt", label: "Gantt", Icon: ChartGantt },
  { id: "tracking", label: "Theo dõi", Icon: ListChecks },
  { id: "resources", label: "Nguồn lực", Icon: Users },
  { id: "usage", label: "Sử dụng", Icon: Table2 },
  { id: "network", label: "Sơ đồ mạng", Icon: Network },
];

/** The view area the tabs control; App puts this id on its main element. */
export const VIEW_PANEL_ID = "view-panel";

export function viewTabId(view: ViewId): string {
  return `view-tab-${view}`;
}

const SCALES: { id: Scale; label: string }[] = [
  { id: "day", label: "Ngày" },
  { id: "week", label: "Tuần" },
  { id: "month", label: "Tháng" },
];

interface Props {
  project: Project | undefined;
  view: ViewId;
  onView: (view: ViewId) => void;
  scale: Scale;
  onScale: (scale: Scale) => void;
  filter: FilterMode;
  onFilter: (filter: FilterMode) => void;
  filterResource: number | null;
  onFilterResource: (uid: number | null) => void;
  group: GroupMode;
  onGroup: (group: GroupMode) => void;
  highlight: HighlightMode;
  onHighlight: (highlight: HighlightMode) => void;
  networkSummaries: boolean;
  onNetworkSummaries: (show: boolean) => void;
}

/**
 * The second row: the views as tabs on the left, joined to the view below, and the
 * controls of the current view (time scale, filter, group, highlight) on the right.
 * The tabs follow the WAI-ARIA tabs pattern: one tab stop, arrows and Home/End move
 * and switch at once.
 */
export function ViewBar(props: Props) {
  const { project, view } = props;
  const chart = view === "gantt" || view === "tracking";
  const rowControls = project && (chart || view === "network");
  const tabsRef = useRef<HTMLDivElement>(null);

  const onTabKey = (event: KeyboardEvent<HTMLDivElement>) => {
    const at = VIEWS.findIndex((item) => item.id === view);
    const next =
      event.key === "ArrowRight"
        ? (at + 1) % VIEWS.length
        : event.key === "ArrowLeft"
          ? (at - 1 + VIEWS.length) % VIEWS.length
          : event.key === "Home"
            ? 0
            : event.key === "End"
              ? VIEWS.length - 1
              : null;
    if (next === null) return;
    event.preventDefault();
    props.onView(VIEWS[next].id);
    tabsRef.current?.querySelector<HTMLButtonElement>(`#${viewTabId(VIEWS[next].id)}`)?.focus();
  };

  return (
    <div className="viewbar">
      <div ref={tabsRef} className="view-tabs" role="tablist" aria-label="View" onKeyDown={onTabKey}>
        {VIEWS.map(({ id, label, Icon }) => (
          <button
            key={id}
            id={viewTabId(id)}
            type="button"
            role="tab"
            className={view === id ? "view-tab on" : "view-tab"}
            aria-selected={view === id}
            aria-controls={VIEW_PANEL_ID}
            tabIndex={view === id ? 0 : -1}
            onClick={() => props.onView(id)}
          >
            <Icon aria-hidden="true" size={15} strokeWidth={1.9} />
            <span>{label}</span>
          </button>
        ))}
      </div>
      <span className="spacer" />
      {rowControls && (
        <div className="view-controls">
          {chart && (
            // One control in three parts, not three separate buttons.
            <div className="segmented" role="group" aria-label="Thang thời gian">
              {SCALES.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  className={props.scale === item.id ? "on" : ""}
                  aria-pressed={props.scale === item.id}
                  onClick={() => props.onScale(item.id)}
                >
                  {item.label}
                </button>
              ))}
            </div>
          )}
          <label className="view-control">
            Lọc
            <select
              aria-label="Lọc"
              value={props.filter}
              onChange={(event) => {
                const next = event.target.value as FilterMode;
                props.onFilter(next);
                if (next === "resource" && props.filterResource == null) {
                  props.onFilterResource(project.resources[0]?.uid ?? null);
                }
              }}
            >
              <option value="none">Không lọc</option>
              <option value="critical">Đường găng</option>
              <option value="resource">Theo người</option>
              <option value="late">Trễ hạn</option>
            </select>
          </label>
          {props.filter === "resource" && (
            <select
              aria-label="Người"
              value={props.filterResource ?? ""}
              onChange={(event) => props.onFilterResource(event.target.value ? Number(event.target.value) : null)}
            >
              {project.resources.length === 0 && <option value="">Chưa có nguồn lực</option>}
              {project.resources.map((resource) => (
                <option key={resource.uid} value={resource.uid}>
                  {resource.name.trim() || `NL ${resource.uid}`}
                </option>
              ))}
            </select>
          )}
          {view === "network" ? (
            <label className="view-control">
              <input
                type="checkbox"
                checked={props.networkSummaries}
                onChange={(event) => props.onNetworkSummaries(event.target.checked)}
              />
              Hiện công việc tóm tắt
            </label>
          ) : (
            <label className="view-control">
              Nhóm
              <select
                aria-label="Nhóm"
                value={props.group}
                onChange={(event) => props.onGroup(event.target.value as GroupMode)}
              >
                <option value="none">Không nhóm</option>
                <option value="resource">Theo người</option>
                <option value="summary">Theo summary</option>
              </select>
            </label>
          )}
          <label className="view-control">
            Tô sáng
            <select
              aria-label="Tô sáng"
              value={props.highlight}
              onChange={(event) => props.onHighlight(event.target.value as HighlightMode)}
            >
              <option value="none">Không</option>
              <option value="critical">Đường găng</option>
              <option value="late">Trễ hạn</option>
            </select>
          </label>
        </div>
      )}
    </div>
  );
}

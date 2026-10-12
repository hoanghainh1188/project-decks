import type { Project, Scale, ViewId } from "../types";
import type { FilterMode, GroupMode, HighlightMode } from "../viewRows";

const VIEWS: { id: ViewId; label: string }[] = [
  { id: "gantt", label: "Gantt" },
  { id: "tracking", label: "Theo dõi" },
  { id: "resources", label: "Nguồn lực" },
  { id: "usage", label: "Sử dụng" },
  { id: "network", label: "Sơ đồ mạng" },
];

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
 * The second row: the views as navigation on the left, and the controls of the current
 * view (time scale, filter, group, highlight) on the right.
 */
export function ViewBar(props: Props) {
  const { project, view } = props;
  const chart = view === "gantt" || view === "tracking";
  const rowControls = project && (chart || view === "network");
  return (
    <div className="viewbar">
      <nav className="view-tabs" aria-label="View">
        {VIEWS.map((item) => (
          <button
            key={item.id}
            type="button"
            className={view === item.id ? "on" : ""}
            aria-current={view === item.id ? "page" : undefined}
            onClick={() => props.onView(item.id)}
          >
            {item.label}
          </button>
        ))}
      </nav>
      <span className="spacer" />
      {rowControls && (
        <div className="view-controls">
          {chart && (
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

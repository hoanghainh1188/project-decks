import { ChartGantt, ListChecks, Network, Table2, Users, type LucideIcon } from "lucide-react";
import { useRef, type KeyboardEvent } from "react";

import { useMessages } from "../i18n";
import type { Project, Scale, ViewId } from "../types";
import type { FilterMode, GroupMode, HighlightMode } from "../viewRows";
import { viewBarMessages } from "./ViewBar.i18n";

const VIEWS: { id: ViewId; Icon: LucideIcon }[] = [
  { id: "gantt", Icon: ChartGantt },
  { id: "tracking", Icon: ListChecks },
  { id: "resources", Icon: Users },
  { id: "usage", Icon: Table2 },
  { id: "network", Icon: Network },
];

/** The view area the tabs control; App puts this id on its main element. */
export const VIEW_PANEL_ID = "view-panel";

export function viewTabId(view: ViewId): string {
  return `view-tab-${view}`;
}

const SCALES: Scale[] = ["day", "week", "month"];

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
  const m = useMessages(viewBarMessages);

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
      <div ref={tabsRef} className="view-tabs" role="tablist" aria-label={m.views} onKeyDown={onTabKey}>
        {VIEWS.map(({ id, Icon }) => (
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
            <span>{m[id]}</span>
          </button>
        ))}
      </div>
      <span className="spacer" />
      {rowControls && (
        <div className="view-controls">
          {chart && (
            // One control in three parts, not three separate buttons.
            <div className="segmented" role="group" aria-label={m.scale}>
              {SCALES.map((scale) => (
                <button
                  key={scale}
                  type="button"
                  className={props.scale === scale ? "on" : ""}
                  aria-pressed={props.scale === scale}
                  onClick={() => props.onScale(scale)}
                >
                  {m[scale]}
                </button>
              ))}
            </div>
          )}
          <label className="view-control">
            {m.filter}
            <select
              aria-label={m.filter}
              value={props.filter}
              onChange={(event) => {
                const next = event.target.value as FilterMode;
                props.onFilter(next);
                if (next === "resource" && props.filterResource == null) {
                  props.onFilterResource(project.resources[0]?.uid ?? null);
                }
              }}
            >
              <option value="none">{m.filterNone}</option>
              <option value="critical">{m.critical}</option>
              <option value="resource">{m.byPerson}</option>
              <option value="late">{m.late}</option>
            </select>
          </label>
          {props.filter === "resource" && (
            <select
              aria-label={m.person}
              value={props.filterResource ?? ""}
              onChange={(event) => props.onFilterResource(event.target.value ? Number(event.target.value) : null)}
            >
              {project.resources.length === 0 && <option value="">{m.noResources}</option>}
              {project.resources.map((resource) => (
                <option key={resource.uid} value={resource.uid}>
                  {resource.name.trim() || m.resourceFallback(resource.uid)}
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
              {m.showSummaries}
            </label>
          ) : (
            <label className="view-control">
              {m.group}
              <select
                aria-label={m.group}
                value={props.group}
                onChange={(event) => props.onGroup(event.target.value as GroupMode)}
              >
                <option value="none">{m.groupNone}</option>
                <option value="resource">{m.byPerson}</option>
                <option value="summary">{m.bySummary}</option>
              </select>
            </label>
          )}
          <label className="view-control">
            {m.highlight}
            <select
              aria-label={m.highlight}
              value={props.highlight}
              onChange={(event) => props.onHighlight(event.target.value as HighlightMode)}
            >
              <option value="none">{m.highlightNone}</option>
              <option value="critical">{m.critical}</option>
              <option value="late">{m.late}</option>
            </select>
          </label>
        </div>
      )}
    </div>
  );
}

import { ChartGantt, Grid3x3, ListChecks, Network, Table2, Users, type LucideIcon } from "lucide-react";
import { useEffect, useMemo, useRef, type KeyboardEvent } from "react";

import { distinctValues, formatFieldValue, valueKey } from "../fieldValues";
import { useMessages } from "../i18n";
import { lastFinish, type SpanEnd } from "../spanFit";
import type { Project, Scale, ViewId } from "../types";
import { fieldMode, fieldOfMode, type FilterMode, type GroupMode, type HighlightMode } from "../viewRows";
import { viewBarMessages } from "./ViewBar.i18n";

const VIEWS: { id: ViewId; Icon: LucideIcon }[] = [
  { id: "gantt", Icon: ChartGantt },
  { id: "tracking", Icon: ListChecks },
  { id: "resources", Icon: Users },
  { id: "usage", Icon: Table2 },
  { id: "load", Icon: Grid3x3 },
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
  /** How much the Gantt timeline is zoomed past the scale (1 for none). */
  zoom?: number;
  onZoomReset?: () => void;
  filter: FilterMode;
  onFilter: (filter: FilterMode) => void;
  filterResource: number | null;
  onFilterResource: (uid: number | null) => void;
  /** The custom field value a field filter keeps, as `valueKey` gives it ("" for none). */
  filterValue: string;
  onFilterValue: (key: string) => void;
  group: GroupMode;
  onGroup: (group: GroupMode) => void;
  highlight: HighlightMode;
  onHighlight: (highlight: HighlightMode) => void;
  networkSummaries: boolean;
  onNetworkSummaries: (show: boolean) => void;
  /** Whether the task sheet shows the interrupt flag as a column. */
  interruptsColumn: boolean;
  onInterruptsColumn: (show: boolean) => void;
  /** Where the span of "Không vừa khoảng" ends. */
  spanEnd: SpanEnd;
  onSpanEnd: (end: SpanEnd) => void;
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
  const filterField = project ? fieldOfMode(project, props.filter) : null;
  const filterValues = useMemo(
    () => (filterField && project ? distinctValues(filterField, project.tasks) : []),
    [filterField, project],
  );
  const tabsRef = useRef<HTMLDivElement>(null);
  const m = useMessages(viewBarMessages);

  // An empty end starts at the last finish, so the span shows something at once.
  const startSpan = (mode: string) => {
    if (mode === "span" && project && props.spanEnd.kind === "date" && props.spanEnd.date === "") {
      props.onSpanEnd({ kind: "date", date: lastFinish(project) });
    }
  };

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
          {chart && props.zoom != null && props.zoom !== 1 && props.onZoomReset && (
            <button type="button" className="zoom-reset" title={m.zoomHint} onClick={props.onZoomReset}>
              {m.zoomReset(Math.round(props.zoom * 100))}
            </button>
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
                const field = fieldOfMode(project, next);
                // Start on the first value tasks have, so the filter shows something.
                if (field) props.onFilterValue(valueKey(distinctValues(field, project.tasks)[0]));
                startSpan(next);
              }}
            >
              <option value="none">{m.filterNone}</option>
              <option value="critical">{m.critical}</option>
              <option value="resource">{m.byPerson}</option>
              <option value="late">{m.late}</option>
              <option value="interrupts">{m.interrupts}</option>
              <option value="span">{m.outsideSpan}</option>
              {project.fields.map((field) => (
                <option key={field.id} value={fieldMode(field)}>
                  {m.byField(field.name)}
                </option>
              ))}
            </select>
          </label>
          {filterField && (
            <select
              aria-label={m.fieldValue}
              value={props.filterValue}
              onChange={(event) => props.onFilterValue(event.target.value)}
            >
              {filterValues.map((value) => (
                <option key={valueKey(value)} value={valueKey(value)}>
                  {formatFieldValue(value)}
                </option>
              ))}
              <option value="">{m.noFieldValue}</option>
            </select>
          )}
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
                {project.fields.map((field) => (
                  <option key={field.id} value={fieldMode(field)}>
                    {m.byField(field.name)}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label className="view-control">
            {m.highlight}
            <select
              aria-label={m.highlight}
              value={props.highlight}
              onChange={(event) => {
                const next = event.target.value as HighlightMode;
                props.onHighlight(next);
                startSpan(next);
              }}
            >
              <option value="none">{m.highlightNone}</option>
              <option value="critical">{m.critical}</option>
              <option value="late">{m.late}</option>
              <option value="interrupts">{m.interrupts}</option>
              <option value="span">{m.outsideSpan}</option>
            </select>
          </label>
          {(props.filter === "span" || props.highlight === "span") && (
            <SpanEndControl project={project} end={props.spanEnd} onEnd={props.onSpanEnd} />
          )}
          {chart && (
            <label className="view-control">
              <input
                type="checkbox"
                checked={props.interruptsColumn}
                onChange={(event) => props.onInterruptsColumn(event.target.checked)}
              />
              {m.interruptsColumn}
            </label>
          )}
        </div>
      )}
    </div>
  );
}

/** The end of the span: a date, or a milestone of the project (its finish). */
function SpanEndControl({ project, end, onEnd }: { project: Project; end: SpanEnd; onEnd: (end: SpanEnd) => void }) {
  const m = useMessages(viewBarMessages);
  const milestones = project.tasks.filter((task) => task.milestone);
  // The date typed last, so going to a milestone and back keeps it.
  const lastDate = useRef(end.kind === "date" ? end.date : "");
  useEffect(() => {
    if (end.kind === "date" && end.date) lastDate.current = end.date;
  }, [end]);
  return (
    <label className="view-control">
      {m.spanEnd}
      <select
        aria-label={m.spanEnd}
        value={end.kind === "milestone" ? `milestone:${end.uid}` : "date"}
        onChange={(event) => {
          const value = event.target.value;
          if (value === "date") onEnd({ kind: "date", date: lastDate.current || lastFinish(project) });
          else onEnd({ kind: "milestone", uid: Number(value.slice("milestone:".length)) });
        }}
      >
        <option value="date">{m.spanByDate}</option>
        {milestones.map((task) => (
          <option key={task.uid} value={`milestone:${task.uid}`}>
            {task.name.trim() || m.taskFallback(task.id)}
          </option>
        ))}
      </select>
      {end.kind === "date" && (
        <input
          type="date"
          aria-label={m.spanDate}
          value={end.date}
          onChange={(event) => event.target.value && onEnd({ kind: "date", date: event.target.value })}
        />
      )}
    </label>
  );
}

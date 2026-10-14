import { useEffect, useMemo, useRef, useState, type RefObject } from "react";

import { DateTimeField } from "./DateTimeField";
import { FieldInput } from "./FieldInput";
import { valueOf } from "../fieldValues";
import { formatDuration, formatInstant, parseDuration } from "../format";
import { useMessages } from "../i18n";
import { resourceNamesByTask } from "../resourceNames";
import type { Assignment, Calendar, FieldDef, FieldValue, FillField, Resource, Task } from "../types";
import type { ViewRow } from "../viewRows";
import { taskSheetMessages } from "./TaskSheet.i18n";

const ROW = 28;
/** Keep in sync with .sheet-head / .sheet-row column-gap and horizontal padding. */
const COLUMN_GAP = 4;
const COLUMN_PAD = 12;
const NAME_WIDTH = 200;

type SheetMessages = (typeof taskSheetMessages)["vi"];

interface SheetColumn {
  /** Unique in the sheet; a custom field may share a built-in column's label. */
  key: string;
  label: string;
  width: number;
  /** The field fill-down copies when this column's cell had focus last. */
  fill?: FillField;
}

/** Column width of a custom field by kind; numbers are short, text gets the most room. */
const FIELD_WIDTH = { text: 140, number: 90, choice: 120 } as const;

interface ColumnOptions {
  tracking: boolean;
  interrupts: boolean;
  nameWidth: number;
  fields: FieldDef[];
}

function sheetColumns(m: SheetMessages, { tracking, interrupts, nameWidth, fields }: ColumnOptions): SheetColumn[] {
  const built: Omit<SheetColumn, "key">[] = [
    { label: m.id, width: 42 },
    { label: m.wbs, width: 72 },
    { label: m.name, width: nameWidth, fill: "name" },
    { label: m.duration, width: 100, fill: "duration" },
    ...(tracking
      ? [
          { label: m.percent, width: 52, fill: "percent_complete" as const },
          { label: m.remaining, width: 84 },
        ]
      : []),
    { label: m.start, width: 168, fill: "start" },
    { label: m.finish, width: 168 },
    ...(tracking
      ? [
          { label: m.actualStart, width: 168 },
          { label: m.actualFinish, width: 168 },
          { label: m.remainingWork, width: 96 },
          { label: m.overtime, width: 84 },
          { label: m.calendar, width: 120, fill: "calendar" as const },
        ]
      : []),
    { label: m.predecessors, width: 120, fill: "predecessors" },
    { label: m.resources, width: 160 },
    { label: m.slack, width: 80 },
    ...(interrupts ? [{ label: m.interrupts, width: 84, fill: "interrupts" as const }] : []),
  ];
  // Custom fields follow the built-in columns, in the order they were added.
  return [
    ...built.map((column) => ({ ...column, key: column.label })),
    ...fields.map((field) => ({
      key: `field-${field.id}`,
      label: field.name,
      width: FIELD_WIDTH[field.kind.type],
      fill: { field: field.id },
    })),
  ];
}

function sheetWidthOf(columns: SheetColumn[]): number {
  const tracks = columns.reduce((sum, column) => sum + column.width, 0);
  return tracks + COLUMN_GAP * Math.max(0, columns.length - 1) + COLUMN_PAD;
}

interface Props {
  rows: ViewRow[];
  calendars: Calendar[];
  /** Custom fields, each shown as a column after the built-in ones. */
  fields: FieldDef[];
  assignments: Assignment[];
  resources: Resource[];
  selected: number | null;
  /** Rows of a Shift+click range (copy, paste and fill-down act on them); may be empty. */
  selection?: ReadonlySet<number>;
  tracking: boolean;
  /** Show the interrupt flag as a column after the built-in ones. */
  interruptsColumn: boolean;
  bodyRef: RefObject<HTMLDivElement | null>;
  /** `extend` is true for Shift+click, which selects every row from the selected one. */
  onSelect: (uid: number, extend: boolean) => void;
  /** The column of the focused cell, or null for one fill-down does not copy. */
  onFillField?: (field: FillField | null) => void;
  onRename: (uid: number, name: string) => void;
  onDuration: (uid: number, minutes: number) => void;
  onStart: (uid: number, start: number) => void;
  onPredecessors: (uid: number, text: string) => void;
  onPercent: (uid: number, percent: number) => void;
  onRemaining: (uid: number, minutes: number) => void;
  onActualStart: (uid: number, start: number | null) => void;
  onActualFinish: (uid: number, finish: number | null) => void;
  onRemainingWork: (uid: number, minutes: number) => void;
  onOvertime: (uid: number, minutes: number) => void;
  onCalendar: (uid: number, calendarId: number | null) => void;
  /** A custom field value, or null to clear it. */
  onFieldValue: (uid: number, fieldId: number, value: FieldValue | null) => void;
  onInterrupts: (uid: number, interrupts: boolean) => void;
  onEditSeries: (uid: number) => void;
  onScroll: () => void;
}

export function TaskSheet(props: Props) {
  const m = useMessages(taskSheetMessages);
  const headRef = useRef<HTMLDivElement>(null);
  const [windowRange, setWindowRange] = useState({ start: 0, end: 40 });
  const [paneWidth, setPaneWidth] = useState(0);
  const options = { tracking: props.tracking, interrupts: props.interruptsColumn, fields: props.fields };
  const baseWidth = sheetWidthOf(sheetColumns(m, { ...options, nameWidth: NAME_WIDTH }));
  const nameWidth = NAME_WIDTH + Math.max(0, paneWidth - baseWidth);
  const columns = sheetColumns(m, { ...options, nameWidth });
  const template = columns.map((column) => `${column.width}px`).join(" ");
  // Header and body use this same pixel width so tracks, including columns past the viewport, stay aligned.
  const sheetWidth = sheetWidthOf(columns);

  function measure() {
    const body = props.bodyRef.current;
    if (!body) return;
    const start = Math.max(0, Math.floor(body.scrollTop / ROW) - 6);
    const visible = Math.ceil(body.clientHeight / ROW) + 12;
    const end = Math.min(props.rows.length, start + visible);
    setWindowRange((current) => (current.start === start && current.end === end ? current : { start, end }));
    setPaneWidth((current) => (current === body.clientWidth ? current : body.clientWidth));
  }

  useEffect(() => {
    measure();
    const body = props.bodyRef.current;
    if (!body) return;
    const observer = new ResizeObserver(() => measure());
    observer.observe(body);
    return () => observer.disconnect();
  }, [props.rows.length, props.tracking]);

  const visible = props.rows.slice(windowRange.start, windowRange.end);
  const names = useMemo(
    () => resourceNamesByTask(props.assignments, props.resources),
    [props.assignments, props.resources],
  );

  return (
    <div className="sheet">
      <div className="sheet-head-clip" ref={headRef}>
        <div className="sheet-head" style={{ gridTemplateColumns: template, width: sheetWidth, minWidth: sheetWidth }}>
          {columns.map((column) => (
            <span key={column.key} title={column.label}>
              {column.label}
            </span>
          ))}
        </div>
      </div>
      <div
        className="sheet-body"
        ref={props.bodyRef}
        onScroll={() => {
          if (headRef.current && props.bodyRef.current) {
            headRef.current.scrollLeft = props.bodyRef.current.scrollLeft;
          }
          measure();
          props.onScroll();
        }}
      >
        <div style={{ height: props.rows.length * ROW, width: sheetWidth, minWidth: sheetWidth, position: "relative" }}>
          {visible.map((row, offset) => {
            const top = (windowRange.start + offset) * ROW;
            if (row.kind === "group") {
              return (
                <div
                  key={row.key}
                  className="sheet-row group-head"
                  style={{
                    position: "absolute",
                    top,
                    left: 0,
                    width: sheetWidth,
                    height: ROW,
                    gridTemplateColumns: template,
                  }}
                >
                  <span className="group-name">{row.label}</span>
                </div>
              );
            }
            const task = row.task;
            return (
            <div
              key={row.key}
              className={`sheet-row${task.uid === props.selected ? " selected" : ""}${props.selection?.has(task.uid) && props.selection.size > 1 ? " in-selection" : ""}${task.critical ? " critical" : ""}${row.highlighted ? " highlighted" : ""}`}
              style={{
                position: "absolute",
                top,
                left: 0,
                width: sheetWidth,
                height: ROW,
                gridTemplateColumns: template,
              }}
              onMouseDown={(event) => {
                // Shift+click selects rows, not page text (which Cmd+C would then copy).
                if (event.shiftKey) event.preventDefault();
              }}
              onClick={(event) => props.onSelect(task.uid, event.shiftKey)}
              onFocus={(event) => {
                const cell = Array.from(event.currentTarget.children).findIndex((child) => child.contains(event.target));
                props.onFillField?.(columns[cell]?.fill ?? null);
              }}
            >
              <span className="id">{task.id}</span>
              <span className="ro">{task.wbs}</span>
              <NameCell task={task} onRename={props.onRename} onEditSeries={props.onEditSeries} />
              <DurationCell task={task} onCommit={props.onDuration} />
              {props.tracking && (
                <>
                  <NumberCell
                    value={String(Math.round(task.percent_complete))}
                    disabled={task.summary}
                    onCommit={(value) => props.onPercent(task.uid, value)}
                  />
                  <DurationCell
                    task={task}
                    minutes={task.remaining_duration}
                    disabled={task.summary}
                    onCommit={(_uid, minutes) => props.onRemaining(task.uid, minutes)}
                  />
                </>
              )}
              <DateCell value={task.start} disabled={task.summary} onCommit={(ms) => props.onStart(task.uid, ms)} />
              <span className="ro">{formatInstant(task.finish)}</span>
              {props.tracking && (
                <>
                  <OptionalDateCell
                    value={task.actual_start}
                    disabled={task.summary}
                    onCommit={(ms) => props.onActualStart(task.uid, ms)}
                  />
                  <OptionalDateCell
                    value={task.actual_finish}
                    disabled={task.summary}
                    onCommit={(ms) => props.onActualFinish(task.uid, ms)}
                  />
                  <DurationCell
                    task={task}
                    minutes={task.remaining_work}
                    disabled={task.summary}
                    onCommit={(_uid, minutes) => props.onRemainingWork(task.uid, minutes)}
                  />
                  <DurationCell
                    task={task}
                    minutes={overtimeOf(props.assignments, task.uid)}
                    disabled={task.summary}
                    onCommit={(_uid, minutes) => props.onOvertime(task.uid, minutes)}
                  />
                  <select
                    disabled={task.summary}
                    value={task.calendar_id ?? ""}
                    onChange={(event) =>
                      props.onCalendar(task.uid, event.target.value ? Number(event.target.value) : null)
                    }
                  >
                    <option value="">{m.projectCalendar}</option>
                    {props.calendars.map((calendar) => (
                      <option key={calendar.id} value={calendar.id}>{calendar.name}</option>
                    ))}
                  </select>
                </>
              )}
              <TextCell
                value={task.predecessors}
                disabled={false}
                onCommit={(text) => props.onPredecessors(task.uid, text)}
              />
              <ResourceCell names={names.get(task.uid) ?? ""} />
              <span className={`ro${task.total_slack < 0 ? " bad" : ""}`}>{formatDuration(task.total_slack)}</span>
              {props.interruptsColumn && (
                <span className="check-cell">
                  <input
                    type="checkbox"
                    aria-label={m.interruptsOf(task.name)}
                    checked={task.interrupts}
                    disabled={task.summary || task.milestone}
                    // WebKit does not focus a checkbox on click, so the row's onFocus may not run.
                    onClick={() => props.onFillField?.("interrupts")}
                    onChange={(event) => props.onInterrupts(task.uid, event.target.checked)}
                  />
                </span>
              )}
              {props.fields.map((field) => (
                <FieldInput
                  key={field.id}
                  field={field}
                  value={valueOf(task, field)}
                  ariaLabel={field.name}
                  onCommit={(value) => props.onFieldValue(task.uid, field.id, value)}
                />
              ))}
            </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

function NameCell({
  task,
  onRename,
  onEditSeries,
}: {
  task: Task;
  onRename: (uid: number, name: string) => void;
  onEditSeries: (uid: number) => void;
}) {
  const m = useMessages(taskSheetMessages);
  const [value, setValue] = useState(task.name);
  const [editing, setEditing] = useState(false);
  const shown = editing ? value : task.name;
  const indent = 8 + (task.outline_level - 1) * 14;
  const marker = task.recurrence ? (
    <button
      type="button"
      className="series-icon"
      title={m.seriesTip}
      aria-label={m.editSeries}
      onClick={(event) => {
        event.stopPropagation();
        onEditSeries(task.uid);
      }}
    >
      ↻
    </button>
  ) : task.series_uid != null ? (
    <span
      className={`series-icon occurrence${task.detached ? " detached" : ""}`}
      title={task.detached ? m.detachedTip : m.occurrenceTip}
    >
      {task.detached ? "↺" : "·"}
    </span>
  ) : null;
  if (marker) {
    return (
      <span className="name-cell" style={{ paddingLeft: indent - 4 }}>
        {marker}
        <NameInput task={task} shown={shown} value={value} setValue={setValue} setEditing={setEditing} onRename={onRename} />
      </span>
    );
  }
  return (
    <NameInput
      task={task}
      shown={shown}
      value={value}
      setValue={setValue}
      setEditing={setEditing}
      onRename={onRename}
      paddingLeft={indent}
    />
  );
}

function NameInput({
  task,
  shown,
  value,
  setValue,
  setEditing,
  onRename,
  paddingLeft,
}: {
  task: Task;
  shown: string;
  value: string;
  setValue: (value: string) => void;
  setEditing: (editing: boolean) => void;
  onRename: (uid: number, name: string) => void;
  paddingLeft?: number;
}) {
  return (
    <input
      className="name"
      style={paddingLeft != null ? { paddingLeft } : undefined}
      value={shown}
      onChange={(event) => setValue(event.target.value)}
      onFocus={() => {
        setEditing(true);
        setValue(task.name);
      }}
      onBlur={() => {
        setEditing(false);
        if (value.trim() && value !== task.name) onRename(task.uid, value.trim());
      }}
    />
  );
}

function DurationCell({
  task,
  minutes,
  disabled,
  onCommit,
}: {
  task: Task;
  minutes?: number;
  disabled?: boolean;
  onCommit: (uid: number, minutes: number) => void;
}) {
  const source = minutes ?? task.duration;
  const [value, setValue] = useState(formatDuration(source));
  const [editing, setEditing] = useState(false);
  return (
    <input
      disabled={disabled || task.summary}
      value={editing ? value : formatDuration(source)}
      onChange={(event) => setValue(event.target.value)}
      onFocus={() => {
        setEditing(true);
        setValue(formatDuration(source));
      }}
      onBlur={() => {
        setEditing(false);
        const parsed = parseDuration(value);
        if (parsed !== null && parsed !== source) onCommit(task.uid, parsed);
      }}
    />
  );
}

function overtimeOf(assignments: Assignment[], uid: number): number {
  return assignments.filter((item) => item.task_uid === uid).reduce((sum, item) => sum + item.overtime_work, 0);
}

function OptionalDateCell({
  value,
  disabled,
  onCommit,
}: {
  value: number | null;
  disabled?: boolean;
  onCommit: (ms: number | null) => void;
}) {
  const m = useMessages(taskSheetMessages);
  return <DateTimeField optional value={value} disabled={disabled} ariaLabel={m.actualDate} onCommit={onCommit} />;
}

function DateCell({
  value,
  disabled,
  onCommit,
}: {
  value: number;
  disabled?: boolean;
  onCommit: (ms: number) => void;
}) {
  const m = useMessages(taskSheetMessages);
  return (
    <DateTimeField
      value={value}
      disabled={disabled}
      ariaLabel={m.start}
      onCommit={(ms) => {
        if (ms != null) onCommit(ms);
      }}
    />
  );
}

function TextCell({
  value,
  disabled,
  onCommit,
}: {
  value: string;
  disabled?: boolean;
  onCommit: (text: string) => void;
}) {
  const [text, setText] = useState(value);
  const [editing, setEditing] = useState(false);
  return (
    <input
      disabled={disabled}
      value={editing ? text : value}
      onChange={(event) => setText(event.target.value)}
      onFocus={() => {
        setEditing(true);
        setText(value);
      }}
      onBlur={() => {
        setEditing(false);
        if (text !== value) onCommit(text);
      }}
    />
  );
}

function NumberCell({
  value,
  disabled,
  onCommit,
}: {
  value: string;
  disabled?: boolean;
  onCommit: (value: number) => void;
}) {
  const [text, setText] = useState(value);
  const [editing, setEditing] = useState(false);
  return (
    <input
      disabled={disabled}
      value={editing ? text : value}
      onChange={(event) => setText(event.target.value)}
      onFocus={() => {
        setEditing(true);
        setText(value);
      }}
      onBlur={() => {
        setEditing(false);
        // The shown value is rounded, so committing it unchanged would overwrite the real one.
        const raw = text.trim();
        if (!raw || raw === value) return;
        const parsed = Number(raw.replace(",", "."));
        if (Number.isFinite(parsed)) onCommit(parsed);
      }}
    />
  );
}

function ResourceCell({ names }: { names: string }) {
  // Assignments are edited in the inspector; the sheet only shows them.
  return (
    <span className="ro" title={names || undefined}>
      {names}
    </span>
  );
}

import { useEffect, useRef, useState, type RefObject } from "react";

import { DateTimeField } from "./DateTimeField";
import { formatDuration, formatInstant, parseDuration } from "../format";
import type { Assignment, Calendar, Task } from "../types";
import type { ViewRow } from "../viewRows";

const ROW = 28;
/** Keep in sync with .sheet-head / .sheet-row column-gap and horizontal padding. */
const COLUMN_GAP = 4;
const COLUMN_PAD = 12;
const NAME_WIDTH = 200;

interface SheetColumn {
  label: string;
  width: number;
}

function sheetColumns(tracking: boolean, nameWidth: number): SheetColumn[] {
  return [
    { label: "ID", width: 42 },
    { label: "WBS", width: 72 },
    { label: "Tên", width: nameWidth },
    { label: "Thời lượng", width: 100 },
    ...(tracking
      ? [
          { label: "%", width: 52 },
          { label: "Còn lại", width: 84 },
        ]
      : []),
    { label: "Bắt đầu", width: 168 },
    { label: "Kết thúc", width: 168 },
    ...(tracking
      ? [
          { label: "Bắt đầu thực", width: 168 },
          { label: "Kết thúc thực", width: 168 },
          { label: "Việc còn lại", width: 96 },
          { label: "Làm thêm", width: 84 },
          { label: "Lịch", width: 120 },
        ]
      : []),
    { label: "Tiền nhiệm", width: 120 },
    { label: "Trễ", width: 80 },
  ];
}

function sheetWidthOf(columns: SheetColumn[]): number {
  const tracks = columns.reduce((sum, column) => sum + column.width, 0);
  return tracks + COLUMN_GAP * Math.max(0, columns.length - 1) + COLUMN_PAD;
}

interface Props {
  rows: ViewRow[];
  calendars: Calendar[];
  assignments: Assignment[];
  selected: number | null;
  tracking: boolean;
  bodyRef: RefObject<HTMLDivElement | null>;
  onSelect: (uid: number) => void;
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
  onEditSeries: (uid: number) => void;
  onScroll: () => void;
}

export function TaskSheet(props: Props) {
  const headRef = useRef<HTMLDivElement>(null);
  const [windowRange, setWindowRange] = useState({ start: 0, end: 40 });
  const [paneWidth, setPaneWidth] = useState(0);
  const baseColumns = sheetColumns(props.tracking, NAME_WIDTH);
  const baseWidth = sheetWidthOf(baseColumns);
  const nameWidth = NAME_WIDTH + Math.max(0, paneWidth - baseWidth);
  const columns = sheetColumns(props.tracking, nameWidth);
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

  return (
    <div className="sheet">
      <div className="sheet-head-clip" ref={headRef}>
        <div className="sheet-head" style={{ gridTemplateColumns: template, width: sheetWidth, minWidth: sheetWidth }}>
          {columns.map((column) => (
            <span key={column.label}>{column.label}</span>
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
              className={`sheet-row${task.uid === props.selected ? " selected" : ""}${task.critical ? " critical" : ""}${row.highlighted ? " highlighted" : ""}`}
              style={{
                position: "absolute",
                top,
                left: 0,
                width: sheetWidth,
                height: ROW,
                gridTemplateColumns: template,
              }}
              onClick={() => props.onSelect(task.uid)}
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
                    <option value="">Lịch dự án</option>
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
              <span className={`ro${task.total_slack < 0 ? " bad" : ""}`}>{formatDuration(task.total_slack)}</span>
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
  const [value, setValue] = useState(task.name);
  const [editing, setEditing] = useState(false);
  const shown = editing ? value : task.name;
  const indent = 8 + (task.outline_level - 1) * 14;
  const marker = task.recurrence ? (
    <button
      type="button"
      className="series-icon"
      title="Công việc lặp — bấm để sửa cả chuỗi"
      aria-label="Sửa chuỗi lặp"
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
      title={task.detached ? "Lần lặp đã sửa riêng, không theo chuỗi khi sinh lại" : "Một lần của chuỗi lặp"}
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
  return <DateTimeField optional value={value} disabled={disabled} ariaLabel="Ngày thực" onCommit={onCommit} />;
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
  return (
    <DateTimeField
      value={value}
      disabled={disabled}
      ariaLabel="Bắt đầu"
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

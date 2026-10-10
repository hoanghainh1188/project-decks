import { useEffect, useRef, useState, type RefObject } from "react";

import { DateTimeField } from "./DateTimeField";
import { formatDuration, formatInstant, parseDuration } from "../format";
import type { Assignment, Calendar, Task } from "../types";

const ROW = 28;

interface Props {
  tasks: Task[];
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
  onScroll: () => void;
}

export function TaskSheet(props: Props) {
  const headRef = useRef<HTMLDivElement>(null);
  const [windowRange, setWindowRange] = useState({ start: 0, end: 40 });
  const columns = props.tracking
    ? ["ID", "WBS", "Tên", "Thời lượng", "%", "Còn lại", "Bắt đầu", "Kết thúc", "Bắt đầu thực", "Kết thúc thực", "Việc còn lại", "Làm thêm", "Lịch", "Tiền nhiệm", "Trễ"]
    : ["ID", "WBS", "Tên", "Thời lượng", "Bắt đầu", "Kết thúc", "Tiền nhiệm", "Trễ"];
  const template = props.tracking
    ? "42px 72px minmax(140px, 1.4fr) 92px 52px 84px 176px 146px 210px 210px 96px 84px 120px 110px 72px"
    : "42px 72px minmax(160px, 1.6fr) 100px 176px 146px 120px 80px";
  const minWidth = props.tracking ? 1860 : undefined;

  function measure() {
    const body = props.bodyRef.current;
    if (!body) return;
    const start = Math.max(0, Math.floor(body.scrollTop / ROW) - 6);
    const visible = Math.ceil(body.clientHeight / ROW) + 12;
    const end = Math.min(props.tasks.length, start + visible);
    setWindowRange((current) => (current.start === start && current.end === end ? current : { start, end }));
  }

  useEffect(() => {
    measure();
    const body = props.bodyRef.current;
    if (!body) return;
    const observer = new ResizeObserver(() => measure());
    observer.observe(body);
    return () => observer.disconnect();
  }, [props.tasks.length, props.tracking]);

  const visible = props.tasks.slice(windowRange.start, windowRange.end);

  return (
    <div className="sheet">
      <div className="sheet-head-clip" ref={headRef}>
        <div className="sheet-head" style={{ gridTemplateColumns: template, minWidth }}>
          {columns.map((column) => (
            <span key={column}>{column}</span>
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
        <div style={{ height: props.tasks.length * ROW, minWidth, position: "relative" }}>
          {visible.map((task, offset) => (
            <div
              key={task.uid}
              className={`sheet-row${task.uid === props.selected ? " selected" : ""}${task.critical ? " critical" : ""}`}
              style={{
                position: "absolute",
                top: (windowRange.start + offset) * ROW,
                left: 0,
                right: 0,
                height: ROW,
                gridTemplateColumns: template,
              }}
              onClick={() => props.onSelect(task.uid)}
            >
              <span className="id">{task.id}</span>
              <span className="ro">{task.wbs}</span>
              <NameCell task={task} onRename={props.onRename} />
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
          ))}
        </div>
      </div>
    </div>
  );
}

function NameCell({ task, onRename }: { task: Task; onRename: (uid: number, name: string) => void }) {
  const [value, setValue] = useState(task.name);
  const [editing, setEditing] = useState(false);
  const shown = editing ? value : task.name;
  return (
    <input
      className="name"
      style={{ paddingLeft: 8 + (task.outline_level - 1) * 14 }}
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
        const parsed = Number(text.replace(",", "."));
        if (Number.isFinite(parsed)) onCommit(parsed);
      }}
    />
  );
}

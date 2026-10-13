import { CalendarPlus, Pencil, Plus, Trash2, X } from "lucide-react";
import { useState } from "react";

import { formatDateKeyDisplay, minuteToTime } from "../format";
import { useMessages } from "../i18n";
import type { Calendar, Command, Snapshot, TimeRange, WorkWeek } from "../types";
import { Button } from "./Button";
import { calendarEditorMessages, WEEK_INDEXES } from "./CalendarEditor.i18n";
import { DateKeyField, TimeField } from "./DateTimeField";

const DEFAULT_RANGES: TimeRange[] = [
  { start_minute: 8 * 60, end_minute: 12 * 60 },
  { start_minute: 13 * 60, end_minute: 17 * 60 },
];

interface CalendarEditorProps {
  calendar: Calendar;
  send: (command: Command) => Promise<Snapshot | void | undefined>;
}

interface ExceptionSpan {
  from: string;
  to: string;
  ranges: TimeRange[];
}

export function CalendarEditor({ calendar, send }: CalendarEditorProps) {
  const m = useMessages(calendarEditorMessages);
  const [span, setSpan] = useState<"day" | "range">("day");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [kind, setKind] = useState<"off" | "hours">("off");
  const [hours, setHours] = useState<TimeRange[]>(DEFAULT_RANGES);
  const [draft, setDraft] = useState<WorkWeek>(() => emptyWeek(calendar));
  const [drafting, setDrafting] = useState(false);
  // The start date of the work week being edited, which the engine uses as its key.
  const [editing, setEditing] = useState<string | null>(null);
  const exceptions = groupExceptions(calendar.exceptions);

  function commitWeek(week: TimeRange[][]) {
    void send({ type: "set_calendar_week", calendar_id: calendar.id, week });
  }

  async function addWeek() {
    // A week whose start date changed is a new week: the old one goes first.
    if (editing && editing !== draft.from) {
      const cleared = await send({ type: "clear_calendar_work_week", calendar_id: calendar.id, from: editing });
      if (!cleared) return;
    }
    const result = await send({
      type: "set_calendar_work_week",
      calendar_id: calendar.id,
      name: draft.name.trim(),
      from: draft.from,
      to: draft.to,
      week: draft.week,
    });
    if (result) {
      setDraft(emptyWeek(calendar));
      setDrafting(false);
      setEditing(null);
    }
  }

  async function addException() {
    const end = span === "day" ? from : to;
    if (!from || !end) return;
    const ranges = kind === "off" ? [] : hours;
    if (kind === "hours" && !rangesValid(ranges)) return;
    const result = await send({
      type: "set_calendar_exceptions",
      calendar_id: calendar.id,
      from,
      to: end,
      ranges,
    });
    if (result) {
      setFrom("");
      setTo("");
    }
  }

  return (
    <div className="calendar-editor">
      <label>{m.calendarName}</label>
      <input
        key={`${calendar.id}-${calendar.name}`}
        aria-label={m.calendarName}
        defaultValue={calendar.name}
        onBlur={(event) => {
          const name = event.target.value.trim();
          if (!name) {
            event.target.value = calendar.name;
            return;
          }
          if (name !== calendar.name) {
            void send({ type: "rename_calendar", calendar_id: calendar.id, name });
          }
        }}
      />

      <h3>{m.workWeek}</h3>
      <WeekPattern week={calendar.week} onChange={commitWeek} />

      <h3>{m.otherWeeks}</h3>
      <p className="meta">{m.otherWeeksHint}</p>
      <ul>
        {calendar.work_weeks.map((item) => (
          <li key={item.from}>
            <span>
              {item.name ? `${item.name} · ` : ""}
              {formatDateKeyDisplay(item.from)} – {formatDateKeyDisplay(item.to)}
            </span>
            <Button
              Icon={Pencil}
              onClick={() => {
                setDraft({ ...item });
                setEditing(item.from);
                setDrafting(true);
              }}
            >
              {m.editWeek}
            </Button>
            <Button
              variant="danger"
              Icon={Trash2}
              onClick={() => void send({ type: "clear_calendar_work_week", calendar_id: calendar.id, from: item.from })}
            >
              {m.remove}
            </Button>
          </li>
        ))}
      </ul>
      {calendar.work_weeks.length === 0 && <p className="meta">{m.noOtherWeeks}</p>}
      {!drafting && (
        <Button
          Icon={Plus}
          onClick={() => {
            setDraft(emptyWeek(calendar));
            setEditing(null);
            setDrafting(true);
          }}
        >
          {m.newWeek}
        </Button>
      )}
      {drafting && (
        <div className="exception-form" role="group" aria-label={m.newWeek}>
          <input
            aria-label={m.weekName}
            placeholder={m.weekNamePlaceholder}
            value={draft.name}
            onChange={(event) => setDraft({ ...draft, name: event.target.value })}
          />
          <label>
            {m.from}
            <DateKeyField ariaLabel={m.fromDate} value={draft.from} onCommit={(from) => setDraft({ ...draft, from })} />
          </label>
          <label>
            {m.to}
            <DateKeyField ariaLabel={m.toDate} value={draft.to} onCommit={(to) => setDraft({ ...draft, to })} />
          </label>
          <WeekPattern week={draft.week} onChange={(week) => setDraft({ ...draft, week })} />
          <Button
            variant="primary"
            Icon={CalendarPlus}
            disabled={!draft.from || !draft.to || draft.to < draft.from}
            onClick={() => void addWeek()}
          >
            {m.addWeek}
          </Button>
          <Button Icon={X} onClick={() => { setDrafting(false); setEditing(null); }}>
            {m.cancelWeek}
          </Button>
        </div>
      )}

      <h3>{m.exceptionsHeading}</h3>
      <div className="exception-form">
        <select value={span} onChange={(event) => setSpan(event.target.value as "day" | "range")}>
          <option value="day">{m.oneDay}</option>
          <option value="range">{m.dateRange}</option>
        </select>
        <label>
          {span === "day" ? m.date : m.from}
          <DateKeyField ariaLabel={span === "day" ? m.date : m.fromDate} value={from} onCommit={setFrom} />
        </label>
        {span === "range" && (
          <label>
            {m.to}
            <DateKeyField ariaLabel={m.toDate} value={to} onCommit={setTo} />
          </label>
        )}
        <select value={kind} onChange={(event) => setKind(event.target.value as "off" | "hours")}>
          <option value="off">{m.nonworking}</option>
          <option value="hours">{m.customHours}</option>
        </select>
        {kind === "hours" && (
          <>
            <RangeRows ranges={hours} onChange={setHours} />
            <Button
              Icon={Plus}
              disabled={!suggestRange(hours)}
              onClick={() => {
                const extra = suggestRange(hours);
                if (extra) setHours([...hours, extra]);
              }}
            >
              {m.addRange}
            </Button>
          </>
        )}
        <Button
          variant="primary"
          Icon={CalendarPlus}
          disabled={!from || (span === "range" && !to) || (kind === "hours" && !rangesValid(hours))}
          onClick={() => void addException()}
        >
          {m.addException}
        </Button>
      </div>
      {exceptions.length === 0 && <p className="meta">{m.noExceptions}</p>}
      <ul>
        {exceptions.map((item) => (
          <li key={`${item.from}-${item.to}-${rangeKey(item.ranges)}`}>
            <span>
              {item.from === item.to
                ? formatDateKeyDisplay(item.from)
                : `${formatDateKeyDisplay(item.from)} – ${formatDateKeyDisplay(item.to)}`}
              {" · "}
              {item.ranges.length === 0 ? m.nonworking : item.ranges.map(formatRange).join(", ")}
            </span>
            <Button
              variant="danger"
              Icon={Trash2}
              onClick={() =>
                void send({
                  type: "clear_calendar_exceptions",
                  calendar_id: calendar.id,
                  from: item.from,
                  to: item.to,
                })
              }
            >
              {m.remove}
            </Button>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** The seven days of a weekly pattern, each off or with its ranges of hours. */
function WeekPattern({ week, onChange }: { week: TimeRange[][]; onChange: (week: TimeRange[][]) => void }) {
  const m = useMessages(calendarEditorMessages);
  function setDay(index: number, ranges: TimeRange[]) {
    if (ranges.length > 0 && !rangesValid(ranges)) return;
    onChange(week.map((day, dayIndex) => (dayIndex === index ? ranges : day)));
  }
  return (
    <>
      {WEEK_INDEXES.map((index) => {
        const label = m.weekday(index);
        const ranges = week[index] ?? [];
        const working = ranges.length > 0;
        const extra = suggestRange(ranges);
        return (
          <div className="week-day" key={index}>
            <label className="check">
              <input
                type="checkbox"
                checked={working}
                onChange={(event) => setDay(index, event.target.checked ? DEFAULT_RANGES : [])}
              />
              {label}
            </label>
            {working && (
              <>
                <RangeRows ranges={ranges} onChange={(next) => setDay(index, next)} />
                <Button Icon={Plus} disabled={!extra} onClick={() => extra && setDay(index, [...ranges, extra])}>
                  {m.addRange}
                </Button>
              </>
            )}
          </div>
        );
      })}
    </>
  );
}

/** A new work week starts as a copy of the default week, to be changed. */
function emptyWeek(calendar: Calendar): WorkWeek {
  return { name: "", from: "", to: "", week: calendar.week.map((day) => day.map((range) => ({ ...range }))) };
}

function RangeRows({ ranges, onChange }: { ranges: TimeRange[]; onChange: (ranges: TimeRange[]) => void }) {
  const m = useMessages(calendarEditorMessages);
  return (
    <div className="range-rows">
      {ranges.map((range, index) => (
        <div className="range-row" key={index}>
          <label>
            {m.from}
            <TimeField
              ariaLabel={m.startTime}
              minute={range.start_minute}
              onCommit={(minute) =>
                onChange(ranges.map((item, itemIndex) => (itemIndex === index ? { ...item, start_minute: minute } : item)))
              }
            />
          </label>
          <label>
            {m.to}
            <TimeField
              ariaLabel={m.finishTime}
              minute={range.end_minute}
              onCommit={(minute) =>
                onChange(ranges.map((item, itemIndex) => (itemIndex === index ? { ...item, end_minute: minute } : item)))
              }
            />
          </label>
          <Button Icon={X} onClick={() => onChange(ranges.filter((_, itemIndex) => itemIndex !== index))}>
            {m.remove}
          </Button>
        </div>
      ))}
    </div>
  );
}

function rangesValid(ranges: TimeRange[]): boolean {
  const ordered = [...ranges].sort((a, b) => a.start_minute - b.start_minute || a.end_minute - b.end_minute);
  let last = 0;
  for (const range of ordered) {
    if (range.start_minute >= range.end_minute || range.end_minute > 24 * 60) return false;
    if (range.start_minute < last) return false;
    last = range.end_minute;
  }
  return ranges.length > 0 && ordered.length === ranges.length;
}

function suggestRange(ranges: TimeRange[]): TimeRange | null {
  return DEFAULT_RANGES.find((candidate) => rangesValid([...ranges, candidate])) ?? null;
}

function formatRange(range: TimeRange): string {
  return `${minuteToTime(range.start_minute)}–${minuteToTime(range.end_minute)}`;
}

function rangeKey(ranges: TimeRange[]): string {
  return ranges.map((range) => `${range.start_minute}-${range.end_minute}`).join("|");
}

function nextDateKey(key: string): string {
  const [year, month, day] = key.split("-").map(Number);
  const date = new Date(Date.UTC(year, (month ?? 1) - 1, (day ?? 1) + 1));
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getUTCFullYear()}-${pad(date.getUTCMonth() + 1)}-${pad(date.getUTCDate())}`;
}

function sameRanges(left: TimeRange[], right: TimeRange[]): boolean {
  return (
    left.length === right.length &&
    left.every(
      (range, index) =>
        range.start_minute === right[index]?.start_minute && range.end_minute === right[index]?.end_minute,
    )
  );
}

function groupExceptions(exceptions: Record<string, TimeRange[]>): ExceptionSpan[] {
  const spans: ExceptionSpan[] = [];
  for (const key of Object.keys(exceptions).sort()) {
    const ranges = exceptions[key] ?? [];
    const last = spans[spans.length - 1];
    if (last && nextDateKey(last.to) === key && sameRanges(last.ranges, ranges)) {
      last.to = key;
    } else {
      spans.push({ from: key, to: key, ranges });
    }
  }
  return spans;
}

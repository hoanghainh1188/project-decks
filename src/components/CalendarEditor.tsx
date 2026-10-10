import { useState } from "react";

import { formatDateKeyDisplay, minuteToTime } from "../format";
import type { Calendar, Command, Snapshot, TimeRange } from "../types";
import { DateKeyField, TimeField } from "./DateTimeField";

const WEEKDAYS = ["Thứ Hai", "Thứ Ba", "Thứ Tư", "Thứ Năm", "Thứ Sáu", "Thứ Bảy", "Chủ Nhật"];

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
  const [span, setSpan] = useState<"day" | "range">("day");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [kind, setKind] = useState<"off" | "hours">("off");
  const [hours, setHours] = useState<TimeRange[]>(DEFAULT_RANGES);
  const exceptions = groupExceptions(calendar.exceptions);

  function commitWeek(week: TimeRange[][]) {
    void send({ type: "set_calendar_week", calendar_id: calendar.id, week });
  }

  function setDay(index: number, ranges: TimeRange[]) {
    if (ranges.length > 0 && !rangesValid(ranges)) return;
    commitWeek(calendar.week.map((day, dayIndex) => (dayIndex === index ? ranges : day)));
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
      <label>Tên lịch</label>
      <input
        key={`${calendar.id}-${calendar.name}`}
        aria-label="Tên lịch"
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

      <h3>Tuần làm việc</h3>
      {WEEKDAYS.map((label, index) => {
        const ranges = calendar.week[index] ?? [];
        const working = ranges.length > 0;
        const extra = suggestRange(ranges);
        return (
          <div className="week-day" key={label}>
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
                <button type="button" disabled={!extra} onClick={() => extra && setDay(index, [...ranges, extra])}>
                  Thêm khoảng
                </button>
              </>
            )}
          </div>
        );
      })}

      <h3>Ngày nghỉ và ngoại lệ</h3>
      <div className="exception-form">
        <select value={span} onChange={(event) => setSpan(event.target.value as "day" | "range")}>
          <option value="day">Một ngày</option>
          <option value="range">Từ–Đến</option>
        </select>
        <label>
          {span === "day" ? "Ngày" : "Từ"}
          <DateKeyField ariaLabel={span === "day" ? "Ngày" : "Từ ngày"} value={from} onCommit={setFrom} />
        </label>
        {span === "range" && (
          <label>
            Đến
            <DateKeyField ariaLabel="Đến ngày" value={to} onCommit={setTo} />
          </label>
        )}
        <select value={kind} onChange={(event) => setKind(event.target.value as "off" | "hours")}>
          <option value="off">Nghỉ cả ngày</option>
          <option value="hours">Giờ làm riêng</option>
        </select>
        {kind === "hours" && (
          <>
            <RangeRows ranges={hours} onChange={setHours} />
            <button
              type="button"
              disabled={!suggestRange(hours)}
              onClick={() => {
                const extra = suggestRange(hours);
                if (extra) setHours([...hours, extra]);
              }}
            >
              Thêm khoảng
            </button>
          </>
        )}
        <button
          type="button"
          disabled={!from || (span === "range" && !to) || (kind === "hours" && !rangesValid(hours))}
          onClick={() => void addException()}
        >
          Thêm ngoại lệ
        </button>
      </div>
      {exceptions.length === 0 && <p className="meta">Chưa có ngày nghỉ hay ngoại lệ.</p>}
      <ul>
        {exceptions.map((item) => (
          <li key={`${item.from}-${item.to}-${rangeKey(item.ranges)}`}>
            <span>
              {item.from === item.to
                ? formatDateKeyDisplay(item.from)
                : `${formatDateKeyDisplay(item.from)} – ${formatDateKeyDisplay(item.to)}`}
              {" · "}
              {item.ranges.length === 0 ? "Nghỉ cả ngày" : item.ranges.map(formatRange).join(", ")}
            </span>
            <button
              type="button"
              onClick={() =>
                void send({
                  type: "clear_calendar_exceptions",
                  calendar_id: calendar.id,
                  from: item.from,
                  to: item.to,
                })
              }
            >
              Bỏ
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

function RangeRows({ ranges, onChange }: { ranges: TimeRange[]; onChange: (ranges: TimeRange[]) => void }) {
  return (
    <div className="range-rows">
      {ranges.map((range, index) => (
        <div className="range-row" key={index}>
          <label>
            Từ
            <TimeField
              ariaLabel="Giờ bắt đầu"
              minute={range.start_minute}
              onCommit={(minute) =>
                onChange(ranges.map((item, itemIndex) => (itemIndex === index ? { ...item, start_minute: minute } : item)))
              }
            />
          </label>
          <label>
            Đến
            <TimeField
              ariaLabel="Giờ kết thúc"
              minute={range.end_minute}
              onCommit={(minute) =>
                onChange(ranges.map((item, itemIndex) => (itemIndex === index ? { ...item, end_minute: minute } : item)))
              }
            />
          </label>
          <button type="button" onClick={() => onChange(ranges.filter((_, itemIndex) => itemIndex !== index))}>
            Bỏ
          </button>
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

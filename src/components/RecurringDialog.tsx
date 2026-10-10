import { useState } from "react";

import { DateTimeField } from "./DateTimeField";
import { formatDuration, parseDuration } from "../format";
import type { Command, Project, RecurrenceEnd, RecurrencePattern, RecurrenceSpec, Task } from "../types";

const WEEKDAYS = ["T2", "T3", "T4", "T5", "T6", "T7", "CN"];
const DAY_MS = 86_400_000;

type PatternKind = RecurrencePattern["kind"];

interface Props {
  project: Project;
  /** Existing series to edit; null creates a new one. */
  initial: RecurrenceSpec | null;
  defaultStart: number;
  onClose: () => void;
  onSubmit: (spec: RecurrenceSpec) => Promise<unknown>;
}

/** Updates the series when `seriesUid` is a recurring summary; otherwise creates one after `afterUid`. */
export function recurringCommand(
  tasks: Task[],
  seriesUid: number,
  afterUid: number | null,
  spec: RecurrenceSpec,
): Command {
  const series = tasks.find((item) => item.uid === seriesUid);
  return series?.recurrence
    ? { type: "update_recurring_task", uid: series.uid, spec }
    : { type: "create_recurring_task", after_uid: afterUid, spec };
}

/** Instants are UTC-formatted wall-clock time, so the weekday comes from getUTCDay. */
function weekdayOf(ms: number): number {
  return (new Date(ms).getUTCDay() + 6) % 7;
}

export function RecurringDialog({ project, initial, defaultStart, onClose, onSubmit }: Props) {
  const start0 = initial?.start ?? defaultStart;
  const pattern0 = initial?.pattern;
  const [name, setName] = useState(initial?.name ?? "Họp định kỳ");
  const [durationText, setDurationText] = useState(formatDuration(initial?.duration ?? 60));
  const [start, setStart] = useState(start0);
  const [kind, setKind] = useState<PatternKind>(pattern0?.kind ?? "weekly");
  const [every, setEvery] = useState(String(pattern0?.every ?? 1));
  const [weekdays, setWeekdays] = useState<number[]>(
    pattern0?.kind === "weekly" ? pattern0.weekdays : [weekdayOf(start0)],
  );
  const [monthDay, setMonthDay] = useState(
    String(pattern0?.kind === "monthly" ? pattern0.day : new Date(start0).getUTCDate()),
  );
  const [endKind, setEndKind] = useState<RecurrenceEnd["kind"]>(initial?.end.kind ?? "count");
  const [count, setCount] = useState(String(initial?.end.kind === "count" ? initial.end.count : 5));
  const [until, setUntil] = useState(
    initial?.end.kind === "until" ? initial.end.date : start0 + 30 * DAY_MS,
  );
  const [calendarId, setCalendarId] = useState<number | null>(initial?.calendar_id ?? null);
  const [resources, setResources] = useState<number[]>(initial?.resource_uids ?? []);
  const [problem, setProblem] = useState<string | null>(null);

  function buildSpec(): RecurrenceSpec | string {
    const duration = parseDuration(durationText);
    if (duration === null) return "Thời lượng không hợp lệ";
    const step = Math.floor(Number(every));
    if (!Number.isFinite(step) || step < 1) return "Chu kỳ lặp phải từ 1 trở lên";
    let pattern: RecurrencePattern;
    if (kind === "daily") {
      pattern = { kind, every: step };
    } else if (kind === "weekly") {
      if (weekdays.length === 0) return "Hãy chọn ít nhất một thứ trong tuần";
      pattern = { kind, every: step, weekdays: [...weekdays].sort((a, b) => a - b) };
    } else {
      const day = Math.floor(Number(monthDay));
      if (!Number.isFinite(day) || day < 1 || day > 31) return "Ngày trong tháng phải từ 1 đến 31";
      pattern = { kind, every: step, day };
    }
    let end: RecurrenceEnd;
    if (endKind === "count") {
      const times = Math.floor(Number(count));
      if (!Number.isFinite(times) || times < 1 || times > 999) return "Số lần lặp phải từ 1 đến 999";
      end = { kind: "count", count: times };
    } else {
      end = { kind: "until", date: until };
    }
    return {
      name: name.trim() || "Công việc lặp",
      duration,
      start,
      pattern,
      end,
      calendar_id: calendarId,
      resource_uids: resources,
    };
  }

  async function submit() {
    const spec = buildSpec();
    if (typeof spec === "string") {
      setProblem(spec);
      return;
    }
    setProblem(null);
    const result = await onSubmit(spec);
    if (result) onClose();
  }

  function toggle(list: number[], value: number): number[] {
    return list.includes(value) ? list.filter((item) => item !== value) : [...list, value];
  }

  const unit = kind === "daily" ? "ngày làm việc" : kind === "weekly" ? "tuần" : "tháng";

  return (
    <div className="modal">
      <div className="dialog wide recurring-dialog">
        <h2>{initial ? "Sửa công việc lặp" : "Công việc lặp"}</h2>
        <label>Tên</label>
        <input aria-label="Tên chuỗi lặp" value={name} onChange={(event) => setName(event.target.value)} />
        <label>Thời lượng mỗi lần</label>
        <input
          aria-label="Thời lượng mỗi lần"
          value={durationText}
          onChange={(event) => setDurationText(event.target.value)}
        />
        <label>Lặp theo</label>
        <div className="row recurring-kinds">
          {(["daily", "weekly", "monthly"] as const).map((item) => (
            <label key={item} className="check">
              <input type="radio" name="recurring-kind" checked={kind === item} onChange={() => setKind(item)} />
              {item === "daily" ? "Ngày" : item === "weekly" ? "Tuần" : "Tháng"}
            </label>
          ))}
        </div>
        <label>Mỗi</label>
        <div className="row recurring-inline">
          <input aria-label="Chu kỳ lặp" value={every} onChange={(event) => setEvery(event.target.value)} />
          <span>{unit}</span>
        </div>
        {kind === "weekly" && (
          <>
            <label>Vào các thứ</label>
            <div className="row recurring-weekdays">
              {WEEKDAYS.map((label, index) => (
                <label key={label} className="check">
                  <input
                    type="checkbox"
                    checked={weekdays.includes(index)}
                    onChange={() => setWeekdays((current) => toggle(current, index))}
                  />
                  {label}
                </label>
              ))}
            </div>
          </>
        )}
        {kind === "monthly" && (
          <>
            <label>Vào ngày</label>
            <div className="row recurring-inline">
              <input aria-label="Ngày trong tháng" value={monthDay} onChange={(event) => setMonthDay(event.target.value)} />
              <span>(ngày lớn hơn số ngày của tháng rơi vào ngày cuối tháng)</span>
            </div>
          </>
        )}
        <label>Bắt đầu từ</label>
        <DateTimeField
          ariaLabel="Bắt đầu chuỗi lặp"
          value={start}
          onCommit={(ms) => {
            if (ms != null) setStart(ms);
          }}
        />
        <label>Kết thúc</label>
        <div className="row recurring-inline">
          <label className="check">
            <input type="radio" name="recurring-end" checked={endKind === "count"} onChange={() => setEndKind("count")} />
            Sau
          </label>
          <input
            aria-label="Số lần lặp"
            disabled={endKind !== "count"}
            value={count}
            onChange={(event) => setCount(event.target.value)}
          />
          <span>lần</span>
        </div>
        <div className="row recurring-inline">
          <label className="check">
            <input type="radio" name="recurring-end" checked={endKind === "until"} onChange={() => setEndKind("until")} />
            Đến ngày
          </label>
          <DateTimeField
            ariaLabel="Ngày kết thúc chuỗi"
            disabled={endKind !== "until"}
            value={until}
            onCommit={(ms) => {
              if (ms != null) setUntil(ms);
            }}
          />
        </div>
        <label>Lịch</label>
        <select
          aria-label="Lịch chuỗi lặp"
          value={calendarId ?? ""}
          onChange={(event) => setCalendarId(event.target.value ? Number(event.target.value) : null)}
        >
          <option value="">Lịch dự án</option>
          {project.calendars.map((calendar) => (
            <option key={calendar.id} value={calendar.id}>{calendar.name}</option>
          ))}
        </select>
        <label>Nguồn lực</label>
        {project.resources.length === 0 ? (
          <span className="meta">Chưa có nguồn lực</span>
        ) : (
          <div className="row recurring-resources">
            {project.resources.map((resource) => (
              <label key={resource.uid} className="check">
                <input
                  type="checkbox"
                  checked={resources.includes(resource.uid)}
                  onChange={() => setResources((current) => toggle(current, resource.uid))}
                />
                {resource.name.trim() || `NL ${resource.uid}`}
              </label>
            ))}
          </div>
        )}
        {initial && (
          <p className="meta">Các lần đã sửa riêng được giữ nguyên khi sinh lại chuỗi.</p>
        )}
        {problem && <p className="meta bad">{problem}</p>}
        <div className="row">
          <button type="button" onClick={() => void submit()}>{initial ? "Cập nhật chuỗi" : "Tạo chuỗi"}</button>
          <button type="button" onClick={onClose}>Hủy</button>
        </div>
      </div>
    </div>
  );
}

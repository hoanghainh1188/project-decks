import { Check, Repeat, X } from "lucide-react";
import { useState } from "react";

import { Button } from "./Button";
import { DateTimeField } from "./DateTimeField";
import { Modal } from "./Modal";
import { formatDuration, parseDuration } from "../format";
import type { Command, Project, RecurrenceEnd, RecurrencePattern, RecurrenceSpec, Task } from "../types";

const WEEKDAYS = ["T2", "T3", "T4", "T5", "T6", "T7", "CN"];
const WEEKDAY_NAMES = ["thứ Hai", "thứ Ba", "thứ Tư", "thứ Năm", "thứ Sáu", "thứ Bảy", "Chủ nhật"];
const ORDINALS = [
  { value: 1, label: "đầu tiên" },
  { value: 2, label: "thứ hai" },
  { value: 3, label: "thứ ba" },
  { value: 4, label: "thứ tư" },
  { value: 5, label: "cuối cùng" },
];
const DAY_MS = 86_400_000;

type Group = "daily" | "weekly" | "monthly" | "yearly";

function groupOf(kind: RecurrencePattern["kind"] | undefined): Group {
  if (kind === "daily" || kind === "calendar_daily") return "daily";
  if (kind === "monthly" || kind === "monthly_nth") return "monthly";
  if (kind === "yearly" || kind === "yearly_nth") return "yearly";
  return "weekly";
}

/** Which occurrence of its weekday a date is in its month: 1–4, or 5 (last) past day 28. */
function ordinalOf(ms: number): number {
  const day = new Date(ms).getUTCDate();
  return day > 28 ? 5 : Math.ceil(day / 7);
}

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
  const [group, setGroup] = useState<Group>(groupOf(pattern0?.kind));
  const [calendarDays, setCalendarDays] = useState(pattern0?.kind === "calendar_daily");
  const [byNth, setByNth] = useState(pattern0?.kind === "monthly_nth" || pattern0?.kind === "yearly_nth");
  const [every, setEvery] = useState(String(pattern0?.every ?? 1));
  const [weekdays, setWeekdays] = useState<number[]>(
    pattern0?.kind === "weekly" ? pattern0.weekdays : [weekdayOf(start0)],
  );
  const [monthDay, setMonthDay] = useState(
    String(
      pattern0?.kind === "monthly" || pattern0?.kind === "yearly" ? pattern0.day : new Date(start0).getUTCDate(),
    ),
  );
  const [ordinal, setOrdinal] = useState(
    pattern0?.kind === "monthly_nth" || pattern0?.kind === "yearly_nth" ? pattern0.ordinal : ordinalOf(start0),
  );
  const [nthWeekday, setNthWeekday] = useState(
    pattern0?.kind === "monthly_nth" || pattern0?.kind === "yearly_nth" ? pattern0.weekday : weekdayOf(start0),
  );
  const [month, setMonth] = useState(
    pattern0?.kind === "yearly" || pattern0?.kind === "yearly_nth" ? pattern0.month : new Date(start0).getUTCMonth() + 1,
  );
  const [endKind, setEndKind] = useState<RecurrenceEnd["kind"]>(initial?.end.kind ?? "count");
  const [count, setCount] = useState(String(initial?.end.kind === "count" ? initial.end.count : 5));
  const [until, setUntil] = useState(
    initial?.end.kind === "until" ? initial.end.date : start0 + 30 * DAY_MS,
  );
  const [calendarId, setCalendarId] = useState<number | null>(initial?.calendar_id ?? null);
  const [background, setBackground] = useState(initial?.background ?? false);
  // Units text per selected resource UID; series saved before units existed read as 1.
  const [resourceUnits, setResourceUnits] = useState<Record<number, string>>(() => {
    const entries: Record<number, string> = {};
    for (const uid of initial?.resource_uids ?? []) entries[uid] = "1";
    for (const resource of initial?.resources ?? []) entries[resource.uid] = String(resource.units);
    return entries;
  });
  const [problem, setProblem] = useState<string | null>(null);

  function buildSpec(): RecurrenceSpec | string {
    const duration = parseDuration(durationText);
    if (duration === null) return "Thời lượng không hợp lệ";
    const step = Math.floor(Number(every));
    if (!Number.isFinite(step) || step < 1) return "Chu kỳ lặp phải từ 1 trở lên";
    const day = Math.floor(Number(monthDay));
    const dayValid = Number.isFinite(day) && day >= 1 && day <= 31;
    let pattern: RecurrencePattern;
    if (group === "daily") {
      pattern = { kind: calendarDays ? "calendar_daily" : "daily", every: step };
    } else if (group === "weekly") {
      if (weekdays.length === 0) return "Hãy chọn ít nhất một thứ trong tuần";
      pattern = { kind: "weekly", every: step, weekdays: [...weekdays].sort((a, b) => a - b) };
    } else if (group === "monthly") {
      if (byNth) {
        pattern = { kind: "monthly_nth", every: step, ordinal, weekday: nthWeekday };
      } else {
        if (!dayValid) return "Ngày trong tháng phải từ 1 đến 31";
        pattern = { kind: "monthly", every: step, day };
      }
    } else if (byNth) {
      pattern = { kind: "yearly_nth", every: step, month, ordinal, weekday: nthWeekday };
    } else {
      if (!dayValid) return "Ngày trong tháng phải từ 1 đến 31";
      pattern = { kind: "yearly", every: step, month, day };
    }
    const resources = [];
    for (const [uid, text] of Object.entries(resourceUnits)) {
      const units = Number(text.replace(",", "."));
      if (!Number.isFinite(units) || units <= 0) return "Đơn vị nguồn lực phải lớn hơn 0 (1 = 100%)";
      resources.push({ uid: Number(uid), units });
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
      resources,
      background,
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

  function toggleResource(uid: number) {
    setResourceUnits((current) => {
      const next = { ...current };
      if (uid in next) delete next[uid];
      else next[uid] = "1";
      return next;
    });
  }

  const unit =
    group === "daily"
      ? calendarDays
        ? "ngày lịch"
        : "ngày làm việc"
      : group === "weekly"
        ? "tuần"
        : group === "monthly"
          ? "tháng"
          : "năm";

  const nthFields = (
    <>
      <select aria-label="Thứ trong tuần" disabled={!byNth} value={nthWeekday} onChange={(event) => setNthWeekday(Number(event.target.value))}>
        {WEEKDAY_NAMES.map((label, index) => (
          <option key={label} value={index}>{label}</option>
        ))}
      </select>
      <select aria-label="Lần thứ mấy trong tháng" disabled={!byNth} value={ordinal} onChange={(event) => setOrdinal(Number(event.target.value))}>
        {ORDINALS.map((item) => (
          <option key={item.value} value={item.value}>{item.label}</option>
        ))}
      </select>
    </>
  );

  return (
    <Modal
      title={initial ? "Sửa công việc lặp" : "Công việc lặp"}
      className="wide recurring-dialog"
      onClose={onClose}
      footer={
        <>
          <Button Icon={X} onClick={onClose}>Hủy</Button>
          <Button variant="primary" Icon={initial ? Check : Repeat} onClick={() => void submit()}>
            {initial ? "Cập nhật chuỗi" : "Tạo chuỗi"}
          </Button>
        </>
      }
    >
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
        {(["daily", "weekly", "monthly", "yearly"] as const).map((item) => (
          <label key={item} className="check">
            <input type="radio" name="recurring-kind" checked={group === item} onChange={() => setGroup(item)} />
            {item === "daily" ? "Ngày" : item === "weekly" ? "Tuần" : item === "monthly" ? "Tháng" : "Năm"}
          </label>
        ))}
      </div>
      {group === "daily" && (
        <div className="row recurring-inline">
          <label className="check">
            <input type="radio" name="recurring-days" checked={!calendarDays} onChange={() => setCalendarDays(false)} />
            Ngày làm việc
          </label>
          <label className="check">
            <input type="radio" name="recurring-days" checked={calendarDays} onChange={() => setCalendarDays(true)} />
            Ngày lịch (ngày nghỉ dời sang giờ làm việc kế tiếp)
          </label>
        </div>
      )}
      <label>Mỗi</label>
      <div className="row recurring-inline">
        <input aria-label="Chu kỳ lặp" value={every} onChange={(event) => setEvery(event.target.value)} />
        <span>{unit}</span>
      </div>
      {group === "weekly" && (
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
      {(group === "monthly" || group === "yearly") && (
        <>
          {group === "yearly" && (
            <>
              <label>Vào tháng</label>
              <select aria-label="Tháng trong năm" value={month} onChange={(event) => setMonth(Number(event.target.value))}>
                {Array.from({ length: 12 }, (_, index) => (
                  <option key={index} value={index + 1}>Tháng {index + 1}</option>
                ))}
              </select>
            </>
          )}
          <label>Vào</label>
          <div className="row recurring-inline">
            <label className="check">
              <input type="radio" name="recurring-on" checked={!byNth} onChange={() => setByNth(false)} />
              Theo ngày
            </label>
            <input
              aria-label="Ngày trong tháng"
              disabled={byNth}
              value={monthDay}
              onChange={(event) => setMonthDay(event.target.value)}
            />
            <span>(lớn hơn số ngày của tháng thì rơi vào ngày cuối tháng)</span>
          </div>
          <div className="row recurring-inline">
            <label className="check">
              <input type="radio" name="recurring-on" checked={byNth} onChange={() => setByNth(true)} />
              Theo thứ
            </label>
            {nthFields}
            <span>của tháng</span>
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
      <label>Đường găng</label>
      <label className="check" title="Ví dụ họp hằng tuần kéo dài sau mốc phát hành">
        <input type="checkbox" checked={background} onChange={() => setBackground((value) => !value)} />
        Chuỗi nền: không quyết định ngày kết thúc dự án và đường găng
      </label>
      <label>Nguồn lực</label>
      {project.resources.length === 0 ? (
        <span className="meta">Chưa có nguồn lực</span>
      ) : (
        <div className="row recurring-resources">
          {project.resources.map((resource) => {
            const label = resource.name.trim() || `NL ${resource.uid}`;
            const selected = resource.uid in resourceUnits;
            return (
              <span key={resource.uid} className="recurring-resource">
                <label className="check">
                  <input type="checkbox" checked={selected} onChange={() => toggleResource(resource.uid)} />
                  {label}
                </label>
                {selected && (
                  <input
                    aria-label={`Đơn vị của ${label}`}
                    title="Đơn vị: 1 = 100%"
                    value={resourceUnits[resource.uid]}
                    onChange={(event) =>
                      setResourceUnits((current) => ({ ...current, [resource.uid]: event.target.value }))
                    }
                  />
                )}
              </span>
            );
          })}
        </div>
      )}
      {initial && (
        <p className="meta">Các lần đã sửa riêng được giữ nguyên khi sinh lại chuỗi.</p>
      )}
      {problem && <p className="meta bad">{problem}</p>}
    </Modal>
  );
}

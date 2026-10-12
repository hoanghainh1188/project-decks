import { Check, Repeat, X } from "lucide-react";
import { useState } from "react";

import { Button } from "./Button";
import { DateTimeField } from "./DateTimeField";
import { Modal } from "./Modal";
import { formatDuration, parseDuration } from "../format";
import { useLang, useMessages } from "../i18n";
import type { Command, Project, RecurrenceEnd, RecurrencePattern, RecurrenceSpec, Task } from "../types";
import { ORDINAL_VALUES, recurringDialogMessages, WEEKDAY_INDEXES } from "./RecurringDialog.i18n";

/** Same limit as `MAX_OCCURRENCES` in the engine (`recurrence.rs`). */
const MAX_OCCURRENCES = 999;

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
  const m = useMessages(recurringDialogMessages);
  const lang = useLang();
  const start0 = initial?.start ?? defaultStart;
  const pattern0 = initial?.pattern;
  const [name, setName] = useState(initial?.name ?? m.defaultName);
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
    if (duration === null) return m.badDuration;
    const step = Math.floor(Number(every));
    if (!Number.isFinite(step) || step < 1) return m.badEvery;
    const day = Math.floor(Number(monthDay));
    const dayValid = Number.isFinite(day) && day >= 1 && day <= 31;
    let pattern: RecurrencePattern;
    if (group === "daily") {
      pattern = { kind: calendarDays ? "calendar_daily" : "daily", every: step };
    } else if (group === "weekly") {
      if (weekdays.length === 0) return m.noWeekday;
      pattern = { kind: "weekly", every: step, weekdays: [...weekdays].sort((a, b) => a - b) };
    } else if (group === "monthly") {
      if (byNth) {
        pattern = { kind: "monthly_nth", every: step, ordinal, weekday: nthWeekday };
      } else {
        if (!dayValid) return m.badDay;
        pattern = { kind: "monthly", every: step, day };
      }
    } else if (byNth) {
      pattern = { kind: "yearly_nth", every: step, month, ordinal, weekday: nthWeekday };
    } else {
      if (!dayValid) return m.badDay;
      pattern = { kind: "yearly", every: step, month, day };
    }
    const resources = [];
    for (const [uid, text] of Object.entries(resourceUnits)) {
      const units = Number(text.replace(",", "."));
      if (!Number.isFinite(units) || units <= 0) return m.badUnits;
      resources.push({ uid: Number(uid), units });
    }
    let end: RecurrenceEnd;
    if (endKind === "count") {
      const times = Math.floor(Number(count));
      if (!Number.isFinite(times) || times < 1 || times > MAX_OCCURRENCES) return m.badCount(MAX_OCCURRENCES);
      end = { kind: "count", count: times };
    } else {
      end = { kind: "until", date: until };
    }
    return {
      name: name.trim() || m.fallbackName,
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

  const interval = Math.floor(Number(every));
  const unit =
    group === "daily"
      ? calendarDays
        ? m.unitCalendarDays(interval)
        : m.unitWorkingDays(interval)
      : group === "weekly"
        ? m.unitWeeks(interval)
        : group === "monthly"
          ? m.unitMonths(interval)
          : m.unitYears(interval);

  const weekdaySelect = (
    <select key="weekday" aria-label={m.weekdayLabel} disabled={!byNth} value={nthWeekday} onChange={(event) => setNthWeekday(Number(event.target.value))}>
      {WEEKDAY_INDEXES.map((index) => (
        <option key={index} value={index}>{m.weekdayName(index)}</option>
      ))}
    </select>
  );
  const ordinalSelect = (
    <select key="ordinal" aria-label={m.ordinalLabel} disabled={!byNth} value={ordinal} onChange={(event) => setOrdinal(Number(event.target.value))}>
      {ORDINAL_VALUES.map((value) => (
        <option key={value} value={value}>{m.ordinal(value)}</option>
      ))}
    </select>
  );
  // Vietnamese reads "thứ Hai đầu tiên", English "the first Monday".
  const nthFields = lang === "en" ? [ordinalSelect, weekdaySelect] : [weekdaySelect, ordinalSelect];

  return (
    <Modal
      title={initial ? m.titleEdit : m.titleNew}
      className="wide recurring-dialog"
      onClose={onClose}
      footer={
        <>
          <Button Icon={X} onClick={onClose}>{m.cancel}</Button>
          <Button variant="primary" Icon={initial ? Check : Repeat} onClick={() => void submit()}>
            {initial ? m.update : m.create}
          </Button>
        </>
      }
    >
      <label>{m.name}</label>
      <input aria-label={m.nameLabel} value={name} onChange={(event) => setName(event.target.value)} />
      <label>{m.duration}</label>
      <input
        aria-label={m.duration}
        value={durationText}
        onChange={(event) => setDurationText(event.target.value)}
      />
      <label>{m.repeats}</label>
      <div className="row recurring-kinds">
        {(["daily", "weekly", "monthly", "yearly"] as const).map((item) => (
          <label key={item} className="check">
            <input type="radio" name="recurring-kind" checked={group === item} onChange={() => setGroup(item)} />
            {m[item]}
          </label>
        ))}
      </div>
      {group === "daily" && (
        <div className="row recurring-inline">
          <label className="check">
            <input type="radio" name="recurring-days" checked={!calendarDays} onChange={() => setCalendarDays(false)} />
            {m.workingDays}
          </label>
          <label className="check">
            <input type="radio" name="recurring-days" checked={calendarDays} onChange={() => setCalendarDays(true)} />
            {m.calendarDays}
          </label>
        </div>
      )}
      <label>{m.every}</label>
      <div className="row recurring-inline">
        <input aria-label={m.everyLabel} value={every} onChange={(event) => setEvery(event.target.value)} />
        <span>{unit}</span>
      </div>
      {group === "weekly" && (
        <>
          <label>{m.onWeekdays}</label>
          <div className="row recurring-weekdays">
            {WEEKDAY_INDEXES.map((index) => (
              <label key={index} className="check">
                <input
                  type="checkbox"
                  checked={weekdays.includes(index)}
                  onChange={() => setWeekdays((current) => toggle(current, index))}
                />
                {m.weekdayShort(index)}
              </label>
            ))}
          </div>
        </>
      )}
      {(group === "monthly" || group === "yearly") && (
        <>
          {group === "yearly" && (
            <>
              <label>{m.inMonth}</label>
              <select aria-label={m.monthLabel} value={month} onChange={(event) => setMonth(Number(event.target.value))}>
                {Array.from({ length: 12 }, (_, index) => (
                  <option key={index} value={index + 1}>{m.monthName(index + 1)}</option>
                ))}
              </select>
            </>
          )}
          <label>{m.on}</label>
          <div className="row recurring-inline">
            <label className="check">
              <input type="radio" name="recurring-on" checked={!byNth} onChange={() => setByNth(false)} />
              {m.byDay}
            </label>
            <input
              aria-label={m.dayLabel}
              disabled={byNth}
              value={monthDay}
              onChange={(event) => setMonthDay(event.target.value)}
            />
            <span>{m.dayHint}</span>
          </div>
          <div className="row recurring-inline">
            <label className="check">
              <input type="radio" name="recurring-on" checked={byNth} onChange={() => setByNth(true)} />
              {m.byWeekday}
            </label>
            {nthFields}
            <span>{m.ofMonth}</span>
          </div>
        </>
      )}
      <label>{m.startFrom}</label>
      <DateTimeField
        ariaLabel={m.startLabel}
        value={start}
        onCommit={(ms) => {
          if (ms != null) setStart(ms);
        }}
      />
      <label>{m.end}</label>
      <div className="row recurring-inline">
        <label className="check">
          <input type="radio" name="recurring-end" checked={endKind === "count"} onChange={() => setEndKind("count")} />
          {m.endAfter}
        </label>
        <input
          aria-label={m.countLabel}
          disabled={endKind !== "count"}
          value={count}
          onChange={(event) => setCount(event.target.value)}
        />
        <span>{m.occurrences(Math.floor(Number(count)))}</span>
      </div>
      <div className="row recurring-inline">
        <label className="check">
          <input type="radio" name="recurring-end" checked={endKind === "until"} onChange={() => setEndKind("until")} />
          {m.endBy}
        </label>
        <DateTimeField
          ariaLabel={m.endByLabel}
          disabled={endKind !== "until"}
          value={until}
          onCommit={(ms) => {
            if (ms != null) setUntil(ms);
          }}
        />
      </div>
      <label>{m.calendar}</label>
      <select
        aria-label={m.calendarLabel}
        value={calendarId ?? ""}
        onChange={(event) => setCalendarId(event.target.value ? Number(event.target.value) : null)}
      >
        <option value="">{m.projectCalendar}</option>
        {project.calendars.map((calendar) => (
          <option key={calendar.id} value={calendar.id}>{calendar.name}</option>
        ))}
      </select>
      <label>{m.criticalPath}</label>
      <label className="check" title={m.backgroundHint}>
        <input type="checkbox" checked={background} onChange={() => setBackground((value) => !value)} />
        {m.background}
      </label>
      <label>{m.resources}</label>
      {project.resources.length === 0 ? (
        <span className="meta">{m.noResources}</span>
      ) : (
        <div className="row recurring-resources">
          {project.resources.map((resource) => {
            const label = resource.name.trim() || m.resourceFallback(resource.uid);
            const selected = resource.uid in resourceUnits;
            return (
              <span key={resource.uid} className="recurring-resource">
                <label className="check">
                  <input type="checkbox" checked={selected} onChange={() => toggleResource(resource.uid)} />
                  {label}
                </label>
                {selected && (
                  <input
                    aria-label={m.unitsFor(label)}
                    title={m.unitsHint}
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
        <p className="meta">{m.keepsEdited}</p>
      )}
      {problem && <p className="meta bad">{problem}</p>}
    </Modal>
  );
}

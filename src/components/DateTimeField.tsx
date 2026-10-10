import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type MouseEvent, type SyntheticEvent } from "react";
import { createPortal } from "react-dom";

import {
  dateKeyToInstant,
  formatDateOnly,
  formatInstant,
  instantToDateKey,
  minuteToTime,
  parseDateOnly,
  parseInstant,
  parseTimeOfDay,
  todayInstant,
  utcMidnight,
} from "../format";

export type PickerMode = "datetime" | "date" | "time";

const WEEKDAYS = ["T2", "T3", "T4", "T5", "T6", "T7", "CN"];
const MINUTE_STEP = 15;
const DEFAULT_MINUTE = 8 * 60;
const MINUTE_MS = 60_000;
const DAY_MS = 24 * 60 * MINUTE_MS;

interface PickerFieldProps {
  mode: PickerMode;
  // Project instant in ms for "datetime" and "date"; minutes of the day for "time".
  value: number | null;
  optional?: boolean;
  disabled?: boolean;
  ariaLabel?: string;
  placeholder?: string;
  onCommit: (value: number | null) => void;
}

function formatValue(mode: PickerMode, value: number): string {
  if (mode === "time") return minuteToTime(value);
  if (mode === "date") return formatDateOnly(value);
  return formatInstant(value);
}

function parseValue(mode: PickerMode, text: string): number | null {
  if (mode === "time") return parseTimeOfDay(text);
  if (mode === "date") return parseDateOnly(text);
  return parseInstant(text);
}

export function PickerField({ mode, value, optional, disabled, ariaLabel, placeholder, onCommit }: PickerFieldProps) {
  const shown = value == null ? "" : formatValue(mode, value);
  const [text, setText] = useState(shown);
  const [editing, setEditing] = useState(false);
  const [open, setOpen] = useState(false);
  const wrapRef = useRef<HTMLSpanElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (disabled) setOpen(false);
  }, [disabled]);

  function commitText() {
    if (!editing) return;
    setEditing(false);
    const raw = text.trim();
    if (!raw) {
      if (optional && value != null) onCommit(null);
      return;
    }
    const parsed = parseValue(mode, raw);
    if (parsed !== null && parsed !== value) onCommit(parsed);
  }

  function pick(next: number | null) {
    setOpen(false);
    setEditing(false);
    if (next !== value) onCommit(next);
  }

  function close(refocus: boolean) {
    setOpen(false);
    if (refocus) inputRef.current?.focus();
  }

  // Keeps focus in the text field so its blur does not commit half-typed text.
  const keepFocus = (event: MouseEvent) => event.preventDefault();

  const typed = editing ? parseValue(mode, text.trim()) : null;
  const seed = typed ?? value;

  return (
    <span className="datetime-field" ref={wrapRef}>
      <input
        ref={inputRef}
        type="text"
        inputMode="numeric"
        autoComplete="off"
        spellCheck={false}
        disabled={disabled}
        aria-label={ariaLabel}
        aria-haspopup="dialog"
        aria-expanded={open}
        placeholder={placeholder}
        value={editing ? text : shown}
        onChange={(event) => {
          setEditing(true);
          setText(event.target.value);
        }}
        onFocus={() => {
          setEditing(true);
          setText(shown);
        }}
        onClick={() => {
          if (!disabled) setOpen(true);
        }}
        onBlur={commitText}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            setOpen(false);
            event.currentTarget.blur();
          } else if (event.key === "ArrowDown" && !open) {
            event.preventDefault();
            setOpen(true);
          }
        }}
      />
      <button
        type="button"
        className="picker-toggle"
        disabled={disabled}
        aria-label={mode === "time" ? "Chọn giờ" : "Mở lịch chọn ngày"}
        title={mode === "time" ? "Chọn giờ" : "Chọn ngày"}
        onMouseDown={keepFocus}
        onClick={() => setOpen((current) => !current)}
      >
        {mode === "time" ? <ClockIcon /> : <CalendarIcon />}
      </button>
      {optional && value != null && (
        <button
          type="button"
          className="picker-clear"
          disabled={disabled}
          aria-label="Xóa ngày"
          title="Xóa"
          onMouseDown={keepFocus}
          onClick={() => pick(null)}
        >
          ×
        </button>
      )}
      {open && wrapRef.current && (
        <PickerPopover
          anchor={wrapRef.current}
          mode={mode}
          seed={seed}
          optional={optional === true && value != null}
          onPick={pick}
          onClose={close}
        />
      )}
    </span>
  );
}

interface PickerPopoverProps {
  anchor: HTMLElement;
  mode: PickerMode;
  seed: number | null;
  optional: boolean;
  onPick: (value: number | null) => void;
  onClose: (refocus: boolean) => void;
}

function splitSeed(mode: PickerMode, seed: number | null): { day: number; minute: number } {
  if (mode === "time") return { day: todayInstant(), minute: seed ?? DEFAULT_MINUTE };
  if (seed == null) return { day: todayInstant(), minute: DEFAULT_MINUTE };
  const day = utcMidnight(seed);
  return { day, minute: mode === "date" ? 0 : Math.round((seed - day) / MINUTE_MS) };
}

function PickerPopover({ anchor, mode, seed, optional, onPick, onClose }: PickerPopoverProps) {
  const initial = splitSeed(mode, seed);
  const [day, setDay] = useState(initial.day);
  const [minute, setMinute] = useState(initial.minute);
  const [view, setView] = useState(() => {
    const date = new Date(initial.day);
    return { year: date.getUTCFullYear(), month: date.getUTCMonth() };
  });
  const [style, setStyle] = useState<CSSProperties>({ top: -9999, left: -9999, visibility: "hidden" });
  const popRef = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useLayoutEffect(() => {
    function place() {
      const pop = popRef.current;
      if (!pop) return;
      const rect = anchor.getBoundingClientRect();
      const gap = 4;
      const margin = 8;
      const height = pop.offsetHeight;
      const width = pop.offsetWidth;
      const below = window.innerHeight - rect.bottom - gap - margin;
      const above = rect.top - gap - margin;
      const flip = height > below && above > below;
      let top = flip ? rect.top - gap - height : rect.bottom + gap;
      top = Math.max(margin, Math.min(top, window.innerHeight - height - margin));
      const left = Math.max(margin, Math.min(rect.left, window.innerWidth - width - margin));
      setStyle({ top, left });
    }
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [anchor, mode]);

  useEffect(() => {
    function onPointer(event: PointerEvent) {
      const target = event.target as Node | null;
      if (!target) return;
      if (anchor.contains(target) || popRef.current?.contains(target)) return;
      onCloseRef.current(false);
    }
    function onKey(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      onCloseRef.current(true);
    }
    document.addEventListener("pointerdown", onPointer, true);
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("pointerdown", onPointer, true);
      document.removeEventListener("keydown", onKey, true);
    };
  }, [anchor]);

  function result(nextDay: number, nextMinute: number): number {
    if (mode === "time") return nextMinute;
    if (mode === "date") return nextDay;
    return nextDay + nextMinute * MINUTE_MS;
  }

  function shiftMonth(delta: number) {
    setView((current) => {
      const date = new Date(Date.UTC(current.year, current.month + delta, 1));
      return { year: date.getUTCFullYear(), month: date.getUTCMonth() };
    });
  }

  function chooseDay(next: number) {
    if (mode === "date") {
      onPick(next);
      return;
    }
    setDay(next);
    const date = new Date(next);
    setView({ year: date.getUTCFullYear(), month: date.getUTCMonth() });
  }

  function goToday() {
    chooseDay(todayInstant());
  }

  // Lets <select> keep its native behavior while other clicks leave focus in the field.
  function keepFocus(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (target.closest("select")) return;
    event.preventDefault();
  }

  const today = todayInstant();
  const first = Date.UTC(view.year, view.month, 1);
  const offset = (new Date(first).getUTCDay() + 6) % 7;
  const gridStart = first - offset * DAY_MS;
  const cells = Array.from({ length: 42 }, (_, index) => gridStart + index * DAY_MS);

  return createPortal(
    <div
      ref={popRef}
      className="picker-popover"
      role="dialog"
      aria-label={mode === "time" ? "Chọn giờ" : "Chọn ngày"}
      style={style}
      onMouseDown={(event) => {
        event.stopPropagation();
        keepFocus(event);
      }}
      onPointerDown={stopReactBubble}
      onClick={stopReactBubble}
      onDoubleClick={stopReactBubble}
      onKeyDown={stopReactBubble}
    >
      {mode !== "time" && (
        <>
          <div className="picker-head">
            <button type="button" aria-label="Tháng trước" onClick={() => shiftMonth(-1)}>
              ‹
            </button>
            <span className="picker-title">
              Tháng {view.month + 1}, {view.year}
            </span>
            <button type="button" aria-label="Tháng sau" onClick={() => shiftMonth(1)}>
              ›
            </button>
          </div>
          <div className="picker-grid" role="grid">
            {WEEKDAYS.map((label) => (
              <span key={label} className="picker-weekday" role="columnheader">
                {label}
              </span>
            ))}
            {cells.map((cell) => {
              const date = new Date(cell);
              const classes = ["picker-day"];
              if (date.getUTCMonth() !== view.month) classes.push("outside");
              if (cell === today) classes.push("today");
              if (cell === day) classes.push("selected");
              return (
                <button
                  key={cell}
                  type="button"
                  role="gridcell"
                  aria-selected={cell === day}
                  aria-label={formatDateOnly(cell)}
                  className={classes.join(" ")}
                  onClick={() => chooseDay(cell)}
                  onDoubleClick={() => onPick(result(cell, minute))}
                >
                  {date.getUTCDate()}
                </button>
              );
            })}
          </div>
        </>
      )}
      {mode !== "date" && <TimeSelect mode={mode} minute={minute} onChange={setMinute} />}
      <div className="picker-foot">
        {mode !== "time" && (
          <button type="button" onClick={goToday}>
            Hôm nay
          </button>
        )}
        {optional && (
          <button type="button" onClick={() => onPick(null)}>
            Xóa
          </button>
        )}
        <span className="spacer" />
        {mode !== "date" && (
          <>
            <button type="button" onClick={() => onClose(true)}>
              Hủy
            </button>
            <button type="button" className="on" onClick={() => onPick(result(day, minute))}>
              Chọn
            </button>
          </>
        )}
      </div>
    </div>,
    document.body,
  );
}

// React bubbles portal events through the component tree, so stop them before they reach
// row selection or modal handlers that wrap the field.
function stopReactBubble(event: SyntheticEvent) {
  event.stopPropagation();
}

function TimeSelect({ mode, minute, onChange }: { mode: PickerMode; minute: number; onChange: (minute: number) => void }) {
  const hour = Math.floor(minute / 60);
  const rest = minute % 60;
  // Working-time ranges may end at 24:00; instants roll over to the next day instead.
  const maxHour = mode === "time" ? 24 : 23;
  const hours = Array.from({ length: maxHour + 1 }, (_, index) => index);
  const minutes = Array.from({ length: 60 / MINUTE_STEP }, (_, index) => index * MINUTE_STEP);
  if (!minutes.includes(rest)) minutes.push(rest);
  minutes.sort((a, b) => a - b);
  const pad = (value: number) => String(value).padStart(2, "0");
  return (
    <div className="picker-time">
      <span>Giờ</span>
      <select
        aria-label="Giờ"
        value={hour}
        onChange={(event) => {
          const nextHour = Number(event.target.value);
          onChange(nextHour === 24 ? 24 * 60 : nextHour * 60 + rest);
        }}
      >
        {hours.map((value) => (
          <option key={value} value={value}>
            {pad(value)}
          </option>
        ))}
      </select>
      <span>:</span>
      <select
        aria-label="Phút"
        value={rest}
        disabled={hour === 24}
        onChange={(event) => onChange(hour * 60 + Number(event.target.value))}
      >
        {minutes.map((value) => (
          <option key={value} value={value}>
            {pad(value)}
          </option>
        ))}
      </select>
      <span className="meta">Gõ giờ chính xác vào ô nếu cần</span>
    </div>
  );
}

function CalendarIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      <rect x="1.5" y="2.5" width="13" height="12" rx="2" fill="none" stroke="currentColor" strokeWidth="1.3" />
      <path d="M1.5 6h13M5 1v3M11 1v3" stroke="currentColor" strokeWidth="1.3" fill="none" />
    </svg>
  );
}

function ClockIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      <circle cx="8" cy="8" r="6.5" fill="none" stroke="currentColor" strokeWidth="1.3" />
      <path d="M8 4v4l2.5 2" stroke="currentColor" strokeWidth="1.3" fill="none" />
    </svg>
  );
}

interface DateTimeFieldProps {
  value: number | null;
  optional?: boolean;
  disabled?: boolean;
  ariaLabel?: string;
  onCommit: (ms: number | null) => void;
}

export function DateTimeField(props: DateTimeFieldProps) {
  return <PickerField mode="datetime" {...props} />;
}

interface DateKeyFieldProps {
  value: string;
  disabled?: boolean;
  ariaLabel?: string;
  onCommit: (key: string) => void;
}

// Calendar exceptions use "YYYY-MM-DD" keys; an empty string means no date yet.
export function DateKeyField({ value, disabled, ariaLabel, onCommit }: DateKeyFieldProps) {
  return (
    <PickerField
      mode="date"
      value={value ? dateKeyToInstant(value) : null}
      disabled={disabled}
      ariaLabel={ariaLabel}
      placeholder="dd/mm/yyyy"
      onCommit={(ms) => onCommit(ms == null ? "" : instantToDateKey(ms))}
    />
  );
}

interface TimeFieldProps {
  minute: number;
  disabled?: boolean;
  ariaLabel?: string;
  onCommit: (minute: number) => void;
}

export function TimeField({ minute, disabled, ariaLabel, onCommit }: TimeFieldProps) {
  return (
    <PickerField
      mode="time"
      value={minute}
      disabled={disabled}
      ariaLabel={ariaLabel}
      onCommit={(next) => {
        if (next != null) onCommit(next);
      }}
    />
  );
}

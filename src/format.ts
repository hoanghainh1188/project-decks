const DAY = 8 * 60;

/** Hundredths of a unit as Vietnamese text: 150 -> "1,5", 75 -> "0,75". */
function hundredths(value: number): string {
  const whole = Math.floor(value / 100);
  const fraction = value % 100;
  if (fraction === 0) return String(whole);
  return `${whole},${String(fraction).padStart(2, "0").replace(/0$/, "")}`;
}

/**
 * Working minutes as days, the way Project shows durations ("7,5 ngày", a milestone
 * "0 ngày"). Falls back to hours, then minutes, only when days with two decimals would
 * not be exact, so text shown in an editor parses back to the same number of minutes.
 */
export function formatDuration(minutes: number): string {
  if (!Number.isFinite(minutes)) return "";
  const sign = minutes < 0 ? "-" : "";
  const abs = Math.abs(Math.round(minutes));
  if (abs === 0) return "0 ngày";
  if ((abs * 100) % DAY === 0) return `${sign}${hundredths((abs * 100) / DAY)} ngày`;
  if ((abs * 100) % 60 === 0) return `${sign}${hundredths((abs * 100) / 60)} giờ`;
  return `${sign}${abs} phút`;
}

export function parseDuration(text: string): number | null {
  const cleaned = text.trim().toLowerCase().replace(",", ".");
  if (!cleaned) return null;
  const match = cleaned.match(/^(-?\d+(?:\.\d+)?)\s*(ngày|ngay|giờ|gio|phút|phut|d|h|m)?$/);
  if (!match) return null;
  const value = Number(match[1]);
  const unit = match[2] ?? "ngày";
  if (unit === "d" || unit === "ngày" || unit === "ngay") return Math.round(value * DAY);
  if (unit === "h" || unit === "giờ" || unit === "gio") return Math.round(value * 60);
  return Math.round(value);
}

export function formatInstant(ms: number): string {
  const date = new Date(ms);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${pad(date.getUTCDate())}/${pad(date.getUTCMonth() + 1)}/${date.getUTCFullYear()} ${pad(date.getUTCHours())}:${pad(date.getUTCMinutes())}`;
}

export function parseInstant(text: string): number | null {
  const match = text.trim().match(/^(\d{1,2})\/(\d{1,2})\/(\d{4})(?:\s+(\d{1,2}):(\d{2}))?$/);
  if (!match) return null;
  const day = Number(match[1]);
  const month = Number(match[2]);
  const year = Number(match[3]);
  const hour = match[4] ? Number(match[4]) : 8;
  const minute = match[5] ? Number(match[5]) : 0;
  if (hour > 23 || minute > 59) return null;
  const ms = Date.UTC(year, month - 1, day, hour, minute);
  if (Number.isNaN(ms)) return null;
  const check = new Date(ms);
  // Reject dates like 31/02 that Date.UTC would silently roll over.
  if (check.getUTCDate() !== day || check.getUTCMonth() !== month - 1) return null;
  return ms;
}

export function formatDateOnly(ms: number): string {
  return formatInstant(ms).slice(0, 10);
}

// Date-only text maps to UTC midnight of that wall-clock day.
export function parseDateOnly(text: string): number | null {
  if (!/^\d{1,2}\/\d{1,2}\/\d{4}$/.test(text.trim())) return null;
  return parseInstant(`${text.trim()} 00:00`);
}

export function dateKeyToInstant(key: string): number | null {
  const match = key.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (!match) return null;
  const ms = Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
  return Number.isNaN(ms) ? null : ms;
}

export function instantToDateKey(ms: number): string {
  const date = new Date(ms);
  return `${date.getUTCFullYear()}-${pad2(date.getUTCMonth() + 1)}-${pad2(date.getUTCDate())}`;
}

// Time-of-day in minutes; 24:00 is allowed because working ranges may end at midnight.
export function parseTimeOfDay(text: string): number | null {
  const match = text.trim().match(/^(\d{1,2})(?::|h)?(\d{2})$/);
  if (!match) return null;
  const hour = Number(match[1]);
  const minute = Number(match[2]);
  if (minute > 59 || hour > 24 || (hour === 24 && minute !== 0)) return null;
  return hour * 60 + minute;
}

// Today's wall-clock date in the user's timezone, as UTC midnight to match project instants.
export function todayInstant(): number {
  const now = new Date();
  return Date.UTC(now.getFullYear(), now.getMonth(), now.getDate());
}

function pad2(value: number): string {
  return String(value).padStart(2, "0");
}

// Project wall-clock is Unix milliseconds shown with UTC fields, matching the engine.
export function instantToDatetimeLocal(ms: number): string {
  const date = new Date(ms);
  return `${date.getUTCFullYear()}-${pad2(date.getUTCMonth() + 1)}-${pad2(date.getUTCDate())}T${pad2(date.getUTCHours())}:${pad2(date.getUTCMinutes())}`;
}

export function datetimeLocalToInstant(value: string): number | null {
  const match = value.trim().match(/^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})/);
  if (!match) return null;
  const ms = Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3]), Number(match[4]), Number(match[5]));
  return Number.isNaN(ms) ? null : ms;
}

export function minuteToTime(minute: number): string {
  const clamped = Math.max(0, Math.min(24 * 60, Math.round(minute)));
  return `${pad2(Math.floor(clamped / 60))}:${pad2(clamped % 60)}`;
}

export function timeToMinute(value: string): number | null {
  const match = value.trim().match(/^(\d{2}):(\d{2})/);
  if (!match) return null;
  const hour = Number(match[1]);
  const minute = Number(match[2]);
  if (hour > 23 || minute > 59) return null;
  return hour * 60 + minute;
}

export function formatDateKeyDisplay(key: string): string {
  const match = key.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (!match) return key;
  return `${match[3]}/${match[2]}/${match[1]}`;
}

export function formatDateKey(text: string): string | null {
  const match = text.trim().match(/^(\d{1,2})\/(\d{1,2})\/(\d{4})$/);
  if (!match) return null;
  const day = match[1].padStart(2, "0");
  const month = match[2].padStart(2, "0");
  return `${match[3]}-${month}-${day}`;
}

export function formatMoney(value: number): string {
  return new Intl.NumberFormat("vi-VN", { maximumFractionDigits: 0 }).format(Math.round(value));
}

export function formatHours(value: number): string {
  if (Math.abs(value) < 0.05) return "";
  const rounded = Math.round(value * 10) / 10;
  return Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(1);
}

export function shiftDays(ms: number, days: number): number {
  const date = new Date(ms);
  return Date.UTC(
    date.getUTCFullYear(),
    date.getUTCMonth(),
    date.getUTCDate() + days,
    date.getUTCHours(),
    date.getUTCMinutes(),
  );
}

export function utcMidnight(ms: number): number {
  const date = new Date(ms);
  return Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate());
}

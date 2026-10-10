const DAY = 8 * 60;

export function formatDuration(minutes: number): string {
  if (!Number.isFinite(minutes)) return "";
  const sign = minutes < 0 ? "-" : "";
  const abs = Math.abs(Math.round(minutes));
  if (abs === 0) return "0 phút";
  if (abs % DAY === 0) {
    const days = abs / DAY;
    return `${sign}${days} ngày`;
  }
  if (abs % 60 === 0) return `${sign}${abs / 60} giờ`;
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
  const ms = Date.UTC(year, month - 1, day, hour, minute);
  return Number.isNaN(ms) ? null : ms;
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

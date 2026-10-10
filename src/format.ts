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

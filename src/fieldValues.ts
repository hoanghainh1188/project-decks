import { getLang } from "./i18n/lang";
import type { FieldDef, FieldValue, Task } from "./types";

/** Longest text value, as the engine and a Microsoft Project text field hold. */
export const MAX_TEXT = 255;

// Custom field values as the sheet, the inspector and the views show them. The engine
// stores numbers as numbers and the rest as text; a number is shown with the language's
// decimal mark and no grouping, so the text in an editor reads back to the same number.

/** The task's value for `field`, if it has one. */
export function valueOf(task: Task, field: FieldDef): FieldValue | undefined {
  return task.values[String(field.id)];
}

/** No exponent, no grouping, every digit: what `parseFieldValue` reads back exactly. */
const PLAIN = new Intl.NumberFormat("en-US", { useGrouping: false, maximumFractionDigits: 20 });

export function formatFieldValue(value: FieldValue | undefined): string {
  if (value === undefined) return "";
  if (typeof value === "string") return value;
  const text = PLAIN.format(value);
  return getLang() === "en" ? text : text.replace(".", ",");
}

/** A plain decimal: digits with at most one decimal mark, no grouping, no exponent. */
const DECIMAL = /^[+-]?(\d+([.,]\d*)?|[.,]\d+)$/;

/**
 * What typed `text` sets the field to: a value, `null` to clear it (empty text), or
 * `undefined` when it does not fit (a number field given no number, a choice not listed).
 * A number reads with a dot or a comma as the decimal mark, in either language, and no
 * thousands separator, the way `formatFieldValue` shows it. Text keeps what the engine
 * keeps: trimmed, at most 255 characters.
 */
export function parseFieldValue(field: FieldDef, text: string): FieldValue | null | undefined {
  const trimmed = text.trim();
  if (!trimmed) return null;
  if (field.kind.type === "number") {
    if (!DECIMAL.test(trimmed)) return undefined;
    const number = Number(trimmed.replace(",", "."));
    return Number.isFinite(number) ? number : undefined;
  }
  if ([...trimmed].length > MAX_TEXT) return undefined;
  if (field.kind.type === "choice") return field.kind.options.includes(trimmed) ? trimmed : undefined;
  return trimmed;
}

/** A value as one string, so values compare and key a filter: "" is no value. */
export function valueKey(value: FieldValue | undefined): string {
  return value === undefined ? "" : String(value);
}

/**
 * The values tasks have for `field`, each once, in the order a list shows them: a choice
 * field's own order, numbers from smallest, text alphabetically.
 */
export function distinctValues(field: FieldDef, tasks: Task[]): FieldValue[] {
  const seen = new Map<string, FieldValue>();
  for (const task of tasks) {
    const value = valueOf(task, field);
    if (value !== undefined) seen.set(valueKey(value), value);
  }
  const values = [...seen.values()];
  if (field.kind.type === "choice") {
    const order = field.kind.options;
    const rank = (value: FieldValue) => {
      const at = order.indexOf(String(value));
      return at < 0 ? order.length : at;
    };
    return values.sort((a, b) => rank(a) - rank(b));
  }
  if (field.kind.type === "number") return values.sort((a, b) => Number(a) - Number(b));
  return values.sort((a, b) => String(a).localeCompare(String(b), getLang()));
}

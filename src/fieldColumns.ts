import type { FieldDef } from "./types";

/** The custom fields shown as sheet columns: every field except those the user hid, in their own order. */
export function visibleFields(fields: FieldDef[], hidden: ReadonlySet<number>): FieldDef[] {
  return hidden.size === 0 ? fields : fields.filter((field) => !hidden.has(field.id));
}

/** The hidden set after showing or hiding one field; a new set, so React sees the change. */
export function toggleHidden(hidden: ReadonlySet<number>, fieldId: number, show: boolean): ReadonlySet<number> {
  const next = new Set(hidden);
  if (show) next.delete(fieldId);
  else next.add(fieldId);
  return next;
}

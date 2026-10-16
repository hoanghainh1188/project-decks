import { formatFieldValue } from "./fieldValues";
import type { FieldDef, Task } from "./types";

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

/**
 * The plain text other apps receive when tasks are copied: one line per task, the name
 * then each shown custom field's value, apart by tabs so a spreadsheet puts them in columns.
 */
export function tasksAsText(tasks: Task[], uids: number[], fields: FieldDef[]): string {
  const byUid = new Map(tasks.map((task) => [task.uid, task]));
  return uids
    .map((uid) => {
      const task = byUid.get(uid);
      if (!task) return "";
      const name = task.name.replace(/[\t\r\n]+/g, " ");
      const values = fields.map((field) => formatFieldValue(task.values?.[field.id]).replace(/[\t\r\n]+/g, " "));
      return [name, ...values].join("\t");
    })
    .join("\n");
}

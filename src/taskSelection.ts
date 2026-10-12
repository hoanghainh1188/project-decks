import type { ViewRow } from "./viewRows";

/**
 * The task rows from `anchor` to `end` in screen order, both included, as Shift+click
 * selects them in the sheet. Without `end`, or when either row is hidden, only `anchor`.
 */
export function selectionRange(rows: ViewRow[], anchor: number | null, end: number | null): number[] {
  if (anchor == null) return [];
  const uids = rows.flatMap((row) => (row.kind === "task" ? [row.task.uid] : []));
  const from = uids.indexOf(anchor);
  const to = end == null ? from : uids.indexOf(end);
  if (from < 0 || to < 0) return [anchor];
  return uids.slice(Math.min(from, to), Math.max(from, to) + 1);
}

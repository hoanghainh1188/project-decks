import { formatDuration } from "./format";
import { pick } from "./i18n";
import { networkBoxMessages } from "./networkLabels.i18n";
import type { NetworkEdge, Task } from "./types";

/** "SS+4 giờ" ("SS+4 hrs"), "FF-1 ngày"; empty for the default FS with no lag. */
export function edgeLabel(edge: Pick<NetworkEdge, "kind" | "lag">): string {
  if (edge.kind === "fs" && edge.lag === 0) return "";
  const kind = edge.kind.toUpperCase();
  if (edge.lag === 0) return kind;
  return `${kind}${edge.lag > 0 ? "+" : ""}${formatDuration(edge.lag)}`;
}

/** Occurrences per recurring series, keyed by the series summary UID. */
export function occurrenceCounts(tasks: Task[]): Map<number, number> {
  const counts = new Map<number, number>();
  for (const task of tasks) {
    if (task.series_uid != null) counts.set(task.series_uid, (counts.get(task.series_uid) ?? 0) + 1);
  }
  return counts;
}

/** What a box is beyond a plain task: a recurring series (one box for all occurrences), a summary, or a milestone. */
export function nodeKind(task: Task, occurrences: Map<number, number>): string {
  const m = pick(networkBoxMessages);
  if (task.recurrence) return m.recurring(occurrences.get(task.uid) ?? 0);
  if (task.summary) return m.summary;
  return task.milestone ? m.milestone : "";
}

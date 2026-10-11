import { formatDuration } from "./format";
import type { NetworkEdge } from "./types";

/** "SS+4 giờ", "FF-1 ngày"; empty for the default FS with no lag. */
export function edgeLabel(edge: Pick<NetworkEdge, "kind" | "lag">): string {
  if (edge.kind === "fs" && edge.lag === 0) return "";
  const kind = edge.kind.toUpperCase();
  if (edge.lag === 0) return kind;
  return `${kind}${edge.lag > 0 ? "+" : ""}${formatDuration(edge.lag)}`;
}

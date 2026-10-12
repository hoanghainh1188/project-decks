import type { Snapshot } from "./types";

/** Window title: the project name, marked while there are unsaved changes. */
export function windowTitle(snap: Snapshot | null): string {
  const name = snap?.project.name.trim() || "ProjectDesk";
  return snap?.dirty ? `${name} — Đã sửa` : name;
}

/** The last part of a file path, for the status bar; the full path goes in its tooltip. */
export function fileName(path: string): string {
  const parts = path.split(/[\\/]/).filter((part) => part !== "");
  return parts[parts.length - 1] ?? path;
}

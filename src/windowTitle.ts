import { pick } from "./i18n";
import type { Snapshot } from "./types";
import { windowTitleMessages } from "./windowTitle.i18n";

/** Window title: the project name, marked while there are unsaved changes (in the current language). */
export function windowTitle(snap: Snapshot | null): string {
  const name = snap?.project.name.trim() || "ProjectDesk";
  return snap?.dirty ? pick(windowTitleMessages).edited(name) : name;
}

/** The last part of a file path, for the status bar; the full path goes in its tooltip. */
export function fileName(path: string): string {
  const parts = path.split(/[\\/]/).filter((part) => part !== "");
  return parts[parts.length - 1] ?? path;
}

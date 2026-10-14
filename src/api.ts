import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { defineMessages, isLang, pick, type Lang } from "./i18n";
import { isThemeChoice, type ThemeChoice } from "./theme";
import type {
  Command,
  ImportProblem,
  ImportResource,
  ImportSheet,
  ImportTask,
  NetworkLayout,
  NetworkOptions,
  ProjectMetrics,
  Snapshot,
  Suggestion,
  TaskClipboard,
} from "./types";

const apiMessages = defineMessages(
  { failed: "Không thực hiện được thao tác" },
  { failed: "The action could not be completed" },
);

export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

// Edits, undo, redo, and reads of the open project run on the backend's thread pool,
// so two calls in flight could finish in either order. Each waits for the one before,
// which keeps the project in step with the keys the user pressed.
let documentQueue: Promise<unknown> = Promise.resolve();

function inOrder<T>(work: () => Promise<T>): Promise<T> {
  const result = documentQueue.then(work);
  documentQueue = result.catch(() => undefined);
  return result;
}

// File commands may show a dialog. They wait for queued edits so a save includes them,
// but later calls do not wait behind the dialog: the backend refuses edits while it is open.
function afterQueued<T>(work: () => Promise<T>): Promise<T> {
  return documentQueue.then(work);
}

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return pick(apiMessages).failed;
}

/** The language the user chose in the menu, or null to follow the system. */
export async function getLanguage(): Promise<Lang | null> {
  const saved = await invoke<string | null>("get_language");
  return isLang(saved) ? saved : null;
}

/**
 * Tell the backend which language to use for the native menu, dialogs and error
 * messages. `remember` stores it as the user's choice; a language followed from the
 * system is not stored, so a later system change still applies.
 */
export function setLanguage(language: Lang, remember: boolean): Promise<void> {
  return invoke<void>("set_language", { language, remember });
}

/** The theme the user chose in View → Appearance; "system" when there is none. */
export async function getTheme(): Promise<ThemeChoice> {
  const saved = await invoke<string>("get_theme");
  return isThemeChoice(saved) ? saved : "system";
}

/** Remember the theme, check it in the native menu, and match the window to it. */
export function setTheme(theme: ThemeChoice): Promise<void> {
  return invoke<void>("set_theme", { theme });
}

export function loadState(): Promise<Snapshot> {
  return inOrder(() => invoke<Snapshot>("project_state"));
}

export function loadMetrics(): Promise<ProjectMetrics> {
  return inOrder(() => invoke<ProjectMetrics>("project_metrics"));
}

export function loadNetworkLayout(options: NetworkOptions): Promise<NetworkLayout> {
  return inOrder(() => invoke<NetworkLayout>("network_layout", { options }));
}

/** Who the assignment rules would give each unassigned task to; a read, never logged. */
export function suggestAssignments(): Promise<Suggestion[]> {
  return inOrder(() => invoke<Suggestion[]>("suggest_assignments"));
}

/** Ask for a CSV or workbook and read its first sheet; null when cancelled. */
export function pickTable(): Promise<ImportSheet | null> {
  return afterQueued(() => invoke<ImportSheet | null>("pick_table"));
}

/** Another sheet of the workbook `pickTable` read. */
export function readTableSheet(sheet: string): Promise<ImportSheet> {
  return invoke<ImportSheet>("read_table_sheet", { sheet });
}

/** What `import_rows` would refuse, row by row; empty when it would succeed. */
export function checkImport(afterUid: number | null, resources: ImportResource[], tasks: ImportTask[]): Promise<ImportProblem[]> {
  return inOrder(() => invoke<ImportProblem[]>("check_import", { afterUid, resources, tasks }));
}

/** The tasks `uids` with their subtrees, ready for a `paste_tasks` command. */
export function copyTasks(uids: number[]): Promise<TaskClipboard> {
  return inOrder(() => invoke<TaskClipboard>("copy_tasks", { uids }));
}

export function applyCommand(command: Command): Promise<Snapshot> {
  return inOrder(() => invoke<Snapshot>("apply_command", { command }));
}

export function undo(): Promise<Snapshot> {
  return inOrder(() => invoke<Snapshot>("undo"));
}

export function redo(): Promise<Snapshot> {
  return inOrder(() => invoke<Snapshot>("redo"));
}

export function newProject(): Promise<Snapshot> {
  return afterQueued(() => invoke<Snapshot>("new_project"));
}

export function openProject(): Promise<Snapshot> {
  return afterQueued(() => invoke<Snapshot>("open_project"));
}

/** File → Open Recent → the file at `index` (0 is the most recent). */
export function openRecent(index: number): Promise<Snapshot> {
  return afterQueued(() => invoke<Snapshot>("open_recent", { index }));
}

/**
 * Open the next file the system asked to open (a double-clicked project). `null` when
 * nothing is queued or a dialog is up; the file then stays queued for a later call.
 */
export function openPending(): Promise<Snapshot | null> {
  return afterQueued(() => invoke<Snapshot | null>("open_pending"));
}

/** The system asked to open a project file; call `openPending` when no dialog is up. */
export function onOpenFile(handler: () => void): Promise<UnlistenFn> {
  return listen("open-file", () => handler());
}

export function saveProject(): Promise<Snapshot> {
  return afterQueued(() => invoke<Snapshot>("save_project"));
}

export function saveProjectAs(): Promise<Snapshot> {
  return afterQueued(() => invoke<Snapshot>("save_project_as"));
}

export function importMspdi(): Promise<Snapshot> {
  return afterQueued(() => invoke<Snapshot>("import_mspdi"));
}

/** After a crash, offers the autosaved project back; the restored snapshot, or null. */
export function checkAutosave(): Promise<Snapshot | null> {
  return afterQueued(() => invoke<Snapshot | null>("check_autosave"));
}

/** Asks, then folds old undo steps into the project's starting state. */
export function compactHistory(): Promise<Snapshot> {
  return afterQueued(() => invoke<Snapshot>("compact_history"));
}

export function exportMspdi(): Promise<void> {
  return afterQueued(() => invoke<void>("export_mspdi"));
}

export function savePdf(bytes: Uint8Array, fileName: string): Promise<void> {
  return invoke<void>("save_pdf", { bytes: Array.from(bytes), fileName });
}

export function onMenu(handler: (id: string) => void): Promise<UnlistenFn> {
  return listen<string>("menu-action", (event) => handler(event.payload));
}

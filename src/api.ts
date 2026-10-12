import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { Command, NetworkLayout, NetworkOptions, ProjectMetrics, Snapshot } from "./types";

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
  return "Không thực hiện được thao tác";
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

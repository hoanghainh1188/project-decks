import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { Command, ProjectMetrics, Snapshot } from "./types";

export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Không thực hiện được thao tác";
}

export function loadState(): Promise<Snapshot> {
  return invoke<Snapshot>("project_state");
}

export function loadMetrics(): Promise<ProjectMetrics> {
  return invoke<ProjectMetrics>("project_metrics");
}

export function applyCommand(command: Command): Promise<Snapshot> {
  return invoke<Snapshot>("apply_command", { command });
}

export function undo(): Promise<Snapshot> {
  return invoke<Snapshot>("undo");
}

export function redo(): Promise<Snapshot> {
  return invoke<Snapshot>("redo");
}

export function newProject(): Promise<Snapshot> {
  return invoke<Snapshot>("new_project");
}

export function openProject(): Promise<Snapshot> {
  return invoke<Snapshot>("open_project");
}

export function saveProject(): Promise<Snapshot> {
  return invoke<Snapshot>("save_project");
}

export function saveProjectAs(): Promise<Snapshot> {
  return invoke<Snapshot>("save_project_as");
}

export function importMspdi(): Promise<Snapshot> {
  return invoke<Snapshot>("import_mspdi");
}

export function exportMspdi(): Promise<void> {
  return invoke<void>("export_mspdi");
}

export function savePdf(bytes: Uint8Array, fileName: string): Promise<void> {
  return invoke<void>("save_pdf", { bytes: Array.from(bytes), fileName });
}

export function onMenu(handler: (id: string) => void): Promise<UnlistenFn> {
  return listen<string>("menu-action", (event) => handler(event.payload));
}

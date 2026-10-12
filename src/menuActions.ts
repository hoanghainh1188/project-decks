import {
  compactHistory,
  exportMspdi,
  importMspdi,
  newProject,
  openProject,
  openRecent,
  redo,
  saveProject,
  saveProjectAs,
  undo,
} from "./api";
import type { Lang } from "./i18n";
import { commitFocusedField } from "./shortcuts";
import { isThemeChoice, type ThemeChoice } from "./theme";
import { toolbarStates } from "./toolbarState";
import type { Command, Scale, Snapshot, Task, ViewId } from "./types";

type Work = () => Promise<Snapshot | void | null>;

/** What a menu item or shortcut can act on; App supplies its state and setters. */
export interface MenuContext {
  snap: Snapshot | null;
  task: Task | null;
  view: ViewId;
  fileDialogBusy: boolean;
  /** History cursor when Thông tin dự án opened; null while it is closed. */
  infoStart: number | null;
  run: (work: Work) => Promise<Snapshot | void>;
  send: (command: Command) => Promise<Snapshot | void>;
  startFileDialog: (work: Work) => void;
  exportPdf: () => void;
  setView: (update: ViewId | ((current: ViewId) => ViewId)) => void;
  setScale: (scale: Scale) => void;
  setEvmOpen: (open: boolean) => void;
  openProjectInfo: () => void;
  openBaseline: () => void;
  openRecurring: () => void;
  openSplit: (uid: number) => void;
  changeLang: (lang: Lang) => void;
  changeTheme: (theme: ThemeChoice) => void;
}

const VIEWS: Record<string, ViewId> = {
  "view-gantt": "gantt",
  "view-tracking": "tracking",
  "view-resources": "resources",
  "view-usage": "usage",
  "view-network": "network",
};

const SCALES: Record<string, Scale> = { "scale-day": "day", "scale-week": "week", "scale-month": "month" };

/** Commands on the whole project or document; true when `id` was one of them. */
function documentAction(id: string, ctx: MenuContext): boolean {
  switch (id) {
    case "new":
      ctx.startFileDialog(newProject);
      return true;
    case "open":
      ctx.startFileDialog(openProject);
      return true;
    case "save":
    case "save-as":
      commitFocusedField();
      ctx.startFileDialog(id === "save" ? saveProject : saveProjectAs);
      return true;
    case "import-xml":
      ctx.startFileDialog(importMspdi);
      return true;
    case "export-xml":
      ctx.startFileDialog(async () => {
        await exportMspdi();
      });
      return true;
    case "export-pdf":
      ctx.exportPdf();
      return true;
    case "undo":
      // Inside Thông tin dự án, undo stops at the state the dialog opened with, so Hủy
      // can still restore exactly that.
      if (!(ctx.infoStart != null && (ctx.snap?.history_cursor ?? 0) <= ctx.infoStart)) void ctx.run(undo);
      return true;
    case "redo":
      void ctx.run(redo);
      return true;
    case "level":
      void ctx.send({ type: "level_resources" });
      return true;
    case "clear-level":
      void ctx.send({ type: "clear_leveling" });
      return true;
    case "reschedule-uncompleted":
      void ctx.send({ type: "reschedule_uncompleted" });
      return true;
    case "compact-history":
      ctx.startFileDialog(compactHistory);
      return true;
    default:
      return false;
  }
}

/** Views, dialogs, language and theme; true when `id` was one of them. */
function viewAction(id: string, ctx: MenuContext): boolean {
  if (id.startsWith("recent:")) {
    ctx.startFileDialog(() => openRecent(Number(id.slice("recent:".length))));
    return true;
  }
  if (VIEWS[id]) {
    ctx.setView(VIEWS[id]);
    return true;
  }
  if (SCALES[id]) {
    ctx.setScale(SCALES[id]);
    return true;
  }
  if (id.startsWith("theme-")) {
    const theme = id.slice("theme-".length);
    if (isThemeChoice(theme)) ctx.changeTheme(theme);
    return true;
  }
  switch (id) {
    case "view-evm":
      ctx.setView((current) => (current === "gantt" || current === "tracking" ? current : "gantt"));
      ctx.setEvmOpen(true);
      return true;
    case "project-info":
      ctx.openProjectInfo();
      return true;
    case "lang-vi":
    case "lang-en":
      ctx.changeLang(id === "lang-vi" ? "vi" : "en");
      return true;
    case "baseline":
      ctx.openBaseline();
      return true;
    case "recurring":
      ctx.openRecurring();
      return true;
    default:
      return false;
  }
}

/** Commands on the selected task, under the same rules as the toolbar buttons. */
function taskAction(id: string, ctx: MenuContext) {
  const { task } = ctx;
  if (!task) return;
  const states = toolbarStates({ snap: ctx.snap, task, view: ctx.view, fileDialogBusy: ctx.fileDialogBusy });
  if (id === "split-task" && states.split.enabled) ctx.openSplit(task.uid);
  if (id === "indent" && states.indent.enabled) void ctx.send({ type: "indent", uid: task.uid });
  if (id === "outdent" && states.outdent.enabled) void ctx.send({ type: "outdent", uid: task.uid });
  if (id === "delete" && states.remove.enabled) void ctx.send({ type: "delete_task", uid: task.uid });
}

/** Run the native menu item or shortcut `id` (the menu ids of src-tauri/src/i18n.rs). */
export function handleMenuAction(id: string, ctx: MenuContext) {
  if (documentAction(id, ctx) || viewAction(id, ctx)) return;
  taskAction(id, ctx);
}

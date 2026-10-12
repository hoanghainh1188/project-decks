import { canSplit } from "./components/inspector/SplitSection";
import { commandReasons } from "./components/Toolbar.i18n";
import { pick } from "./i18n";
import type { Dependency, Snapshot, Task, ViewId } from "./types";

/** Whether a toolbar command can run now, and if not, why (shown as its tooltip). */
export interface CommandState {
  enabled: boolean;
  reason?: string;
}

export interface ToolbarContext {
  snap: Snapshot | null;
  task: Task | null;
  view: ViewId;
  fileDialogBusy: boolean;
}

export interface ToolbarStates {
  undo: CommandState;
  redo: CommandState;
  add: CommandState;
  indent: CommandState;
  outdent: CommandState;
  link: CommandState;
  unlink: CommandState;
  split: CommandState;
  remove: CommandState;
  project: CommandState;
  evm: CommandState;
  exportPdf: CommandState;
}

const ON: CommandState = { enabled: true };
const off = (reason: string): CommandState => ({ enabled: false, reason });

/** Views where the selected task is on screen, so task commands make sense there. */
const TASK_VIEWS: ViewId[] = ["gantt", "tracking", "network"];
const PDF_VIEWS: ViewId[] = ["gantt", "tracking", "network"];

/**
 * The task just before `task` in outline order, which the link commands pair it with.
 * It is not always the row above on screen while a filter or grouping is on.
 */
export function taskAbove(tasks: Task[], task: Task | null): Task | null {
  if (!task) return null;
  const index = tasks.findIndex((item) => item.uid === task.uid);
  return index > 0 ? tasks[index - 1] : null;
}

/** The link from the task above to `task`, if there is one. */
export function linkFromAbove(snap: Snapshot | null, task: Task | null): Dependency | null {
  if (!snap) return null;
  const above = taskAbove(snap.project.tasks, task);
  if (!above || !task) return null;
  return (
    snap.project.dependencies.find(
      (dep) => dep.predecessor_uid === above.uid && dep.successor_uid === task.uid,
    ) ?? null
  );
}

/** Reasons come in the current interface language. */
export function toolbarStates({ snap, task, view, fileDialogBusy }: ToolbarContext): ToolbarStates {
  const reasons = pick(commandReasons);
  if (!snap) {
    const none = off(reasons.noProject);
    return {
      undo: none,
      redo: none,
      add: none,
      indent: none,
      outdent: none,
      link: none,
      unlink: none,
      split: none,
      remove: none,
      project: none,
      evm: none,
      exportPdf: none,
    };
  }
  const tasks = snap.project.tasks;
  // Every task command first needs a task the user can see selected.
  const needsTask = (): CommandState | null => {
    if (!TASK_VIEWS.includes(view)) return off(reasons.wrongView);
    if (!task) return off(reasons.noTask);
    return null;
  };
  const occurrence = task?.series_uid != null;
  const above = taskAbove(tasks, task);

  const indent =
    needsTask() ??
    (occurrence
      ? off(reasons.occurrenceOutline)
      : !above
        ? off(reasons.firstTask)
        : ON);
  const outdent =
    needsTask() ??
    (occurrence
      ? off(reasons.occurrenceOutline)
      : (task?.outline_level ?? 1) <= 1
        ? off(reasons.topLevel)
        : ON);
  // "Above" is the previous task in outline order; a parent summary there would be a
  // link from a group to its own task, which scheduling rejects.
  const existing = linkFromAbove(snap, task);
  const parentAbove = above != null && above.summary && (task?.outline_level ?? 0) > above.outline_level;
  const link =
    needsTask() ??
    (!above
      ? off(reasons.nothingAbove)
      : parentAbove
        ? off(reasons.parentAbove)
        : existing
          ? off(reasons.alreadyLinked)
          : ON);
  const unlink = needsTask() ?? (existing ? ON : off(reasons.notLinked));
  const split =
    needsTask() ?? (canSplit(task) ? ON : off(reasons.cannotSplit));

  return {
    undo: snap.can_undo ? ON : off(reasons.nothingToUndo),
    redo: snap.can_redo ? ON : off(reasons.nothingToRedo),
    add: ON,
    indent,
    outdent,
    link,
    unlink,
    split,
    remove: needsTask() ?? ON,
    project: ON,
    evm: ON,
    exportPdf: !PDF_VIEWS.includes(view)
      ? off(reasons.pdfView)
      : fileDialogBusy
        ? off(reasons.dialogOpen)
        : ON,
  };
}

import { canSplit } from "./components/inspector/SplitSection";
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

export function toolbarStates({ snap, task, view, fileDialogBusy }: ToolbarContext): ToolbarStates {
  if (!snap) {
    const none = off("Chưa mở dự án");
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
    if (!TASK_VIEWS.includes(view)) return off("Chọn công việc ở Gantt, Theo dõi hoặc Sơ đồ mạng");
    if (!task) return off("Chọn một công việc trước");
    return null;
  };
  const occurrence = task?.series_uid != null;
  const above = taskAbove(tasks, task);

  const indent =
    needsTask() ??
    (occurrence
      ? off("Không đổi cấp một lần của chuỗi lặp")
      : !above
        ? off("Công việc đầu tiên không thụt vào được")
        : ON);
  const outdent =
    needsTask() ??
    (occurrence
      ? off("Không đổi cấp một lần của chuỗi lặp")
      : (task?.outline_level ?? 1) <= 1
        ? off("Công việc đã ở cấp ngoài cùng")
        : ON);
  // "Above" is the previous task in outline order; a parent summary there would be a
  // link from a group to its own task, which scheduling rejects.
  const existing = linkFromAbove(snap, task);
  const parentAbove = above != null && above.summary && (task?.outline_level ?? 0) > above.outline_level;
  const link =
    needsTask() ??
    (!above
      ? off("Không có công việc liền trước để liên kết")
      : parentAbove
        ? off("Việc liền trước là nhóm chứa việc này")
        : existing
          ? off("Đã liên kết với việc liền trước; gỡ trước khi đổi kiểu")
          : ON);
  const unlink = needsTask() ?? (existing ? ON : off("Chưa có liên kết với việc liền trước"));
  const split =
    needsTask() ?? (canSplit(task) ? ON : off("Chỉ tách được công việc có thời lượng, không phải tóm tắt hay mốc"));

  return {
    undo: snap.can_undo ? ON : off("Không còn gì để hoàn tác"),
    redo: snap.can_redo ? ON : off("Không còn gì để làm lại"),
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
      ? off("Xuất PDF từ Gantt, Theo dõi hoặc Sơ đồ mạng")
      : fileDialogBusy
        ? off("Đang mở một hộp thoại")
        : ON,
  };
}

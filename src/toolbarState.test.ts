import { describe, expect, it } from "vitest";

import { makeProject, makeSnapshot, makeTask } from "./test/fixtures";
import { linkFromAbove, toolbarStates, type ToolbarContext } from "./toolbarState";
import type { Snapshot, Task, ViewId } from "./types";

const first = makeTask({ uid: 1, name: "Một" });
const second = makeTask({ uid: 2, name: "Hai", outline_level: 2 });
const milestone = makeTask({ uid: 3, name: "Mốc", milestone: true, duration: 0 });
const occurrence = makeTask({ uid: 4, name: "Họp 1", series_uid: 9 });

function snap(overrides: Partial<Snapshot> = {}): Snapshot {
  return {
    ...makeSnapshot(
      makeProject({
        tasks: [first, second, milestone, occurrence],
        dependencies: [{ uid: 7, predecessor_uid: 1, successor_uid: 2, kind: "fs", lag: 0 }],
      }),
    ),
    ...overrides,
  };
}

function states(task: Task | null, view: ViewId = "gantt", extra: Partial<ToolbarContext> = {}) {
  return toolbarStates({ snap: snap(), task, view, fileDialogBusy: false, ...extra });
}

describe("toolbarStates", () => {
  it("offers assigning by rules only once the project has a rule", () => {
    expect(states(null).assignByRules).toEqual({
      enabled: false,
      reason: "Dự án chưa có quy tắc giao việc; tạo ở Dự án → Quy tắc giao việc…",
    });
    const rule = { conditions: [], target: { type: "resource" as const, uid: 1 }, units: 1 };
    const ruled = makeSnapshot(makeProject({ tasks: [first], assignment_rules: [rule] }));
    expect(toolbarStates({ snap: ruled, task: null, view: "resources", fileDialogBusy: false }).assignByRules.enabled).toBe(true);
  });

  it("turns everything off without a project", () => {
    const all = toolbarStates({ snap: null, task: null, view: "gantt", fileDialogBusy: false });
    expect(Object.values(all).every((state) => !state.enabled && state.reason === "Chưa mở dự án")).toBe(true);
  });

  it("follows the undo and redo history", () => {
    const at = toolbarStates({ snap: snap({ can_undo: true, can_redo: false }), task: null, view: "gantt", fileDialogBusy: false });
    expect(at.undo.enabled).toBe(true);
    expect(at.redo).toEqual({ enabled: false, reason: "Không còn gì để làm lại" });
  });

  it("asks for a selected task before task commands", () => {
    const none = states(null);
    for (const key of ["indent", "outdent", "link", "unlink", "split", "remove"] as const) {
      expect(none[key]).toEqual({ enabled: false, reason: "Chọn một công việc trước" });
    }
    expect(none.add.enabled).toBe(true);
  });

  it("keeps task commands off in views without a task selection", () => {
    for (const view of ["resources", "usage"] as const) {
      expect(states(second, view).remove.reason).toBe("Chọn công việc ở Gantt, Theo dõi hoặc Sơ đồ mạng");
    }
    expect(states(second, "network").remove.enabled).toBe(true);
  });

  it("explains outline limits", () => {
    expect(states(first).indent.reason).toBe("Công việc đầu tiên không thụt vào được");
    expect(states(first).outdent.reason).toBe("Công việc đã ở cấp ngoài cùng");
    expect(states(second).indent.enabled).toBe(true);
    expect(states(second).outdent.enabled).toBe(true);
    expect(states(occurrence).indent.reason).toBe("Không đổi cấp một lần của chuỗi lặp");
  });

  it("links to the task above and unlinks only an existing link", () => {
    expect(states(first).link.reason).toBe("Không có công việc liền trước để liên kết");
    expect(states(second).link.reason).toBe("Đã liên kết với việc liền trước; gỡ trước khi đổi kiểu");
    expect(states(second).unlink.enabled).toBe(true);
    expect(states(milestone).link.enabled).toBe(true);
    expect(states(milestone).unlink.reason).toBe("Chưa có liên kết với việc liền trước");
    expect(linkFromAbove(snap(), second)?.uid).toBe(7);
  });

  it("does not link a task to the group that holds it", () => {
    const group = makeTask({ uid: 1, summary: true });
    const child = makeTask({ uid: 2, outline_level: 2 });
    const context = { snap: { ...makeSnapshot(makeProject({ tasks: [group, child] })) }, view: "gantt" as const, fileDialogBusy: false };
    expect(toolbarStates({ ...context, task: child }).link.reason).toBe("Việc liền trước là nhóm chứa việc này");
  });

  it("splits only tasks with a duration", () => {
    expect(states(second).split.enabled).toBe(true);
    expect(states(milestone).split.enabled).toBe(false);
  });

  it("exports PDF from the chart views when no dialog is open", () => {
    expect(states(null, "network").exportPdf.enabled).toBe(true);
    expect(states(null, "usage").exportPdf.reason).toBe("Xuất PDF từ Gantt, Theo dõi hoặc Sơ đồ mạng");
    expect(states(null, "gantt", { fileDialogBusy: true }).exportPdf.reason).toBe("Đang mở một hộp thoại");
  });
});

describe("copy, paste and fill-down states", () => {
  it("pastes only after a copy, and only in task views", () => {
    expect(states(first).pasteTask.enabled).toBe(false);
    expect(states(first, "gantt", { hasClipboard: true }).pasteTask.enabled).toBe(true);
    expect(states(null, "gantt", { hasClipboard: true }).pasteTask.enabled).toBe(true);
    expect(states(first, "resources", { hasClipboard: true }).pasteTask.enabled).toBe(false);
  });

  it("fills down only a range of rows in the sheet", () => {
    expect(states(first).fillDown.reason).toMatch(/Shift/);
    expect(states(first, "gantt", { selection: [1, 2] }).fillDown.reason).toMatch(/ô/);
    expect(states(first, "gantt", { selection: [1, 2], fillField: "name" }).fillDown.enabled).toBe(true);
    expect(states(first, "network", { selection: [1, 2], fillField: "name" }).fillDown.enabled).toBe(false);
  });

  it("copies the selected task", () => {
    expect(states(first).copyTask.enabled).toBe(true);
    expect(states(null).copyTask.enabled).toBe(false);
  });
});

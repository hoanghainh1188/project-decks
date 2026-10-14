import { describe, expect, it, vi } from "vitest";

import { handleMenuAction, type MenuContext } from "./menuActions";
import { makeProject, makeSnapshot, makeTask } from "./test/fixtures";

function context(overrides: Partial<MenuContext> = {}): MenuContext {
  const project = makeProject({ tasks: [makeTask({ uid: 1 }), makeTask({ uid: 2 })] });
  return {
    snap: makeSnapshot(project),
    task: project.tasks[1],
    view: "gantt",
    fileDialogBusy: false,
    infoStart: null,
    run: vi.fn(async () => undefined),
    send: vi.fn(async () => undefined),
    startFileDialog: vi.fn(),
    exportPdf: vi.fn(),
    setView: vi.fn(),
    setScale: vi.fn(),
    setEvmOpen: vi.fn(),
    openProjectInfo: vi.fn(),
    openCustomFields: vi.fn(),
    openLevelingOrder: vi.fn(),
    openWorkingTime: vi.fn(),
    openBaseline: vi.fn(),
    openRecurring: vi.fn(),
    openSplit: vi.fn(),
    changeLang: vi.fn(),
    changeTheme: vi.fn(),
    selection: [2],
    clipboard: null,
    setClipboard: vi.fn(),
    fillField: "duration",
    ...overrides,
  };
}

const CLIPBOARD = { tasks: [], links: [] };

describe("copy, paste and fill-down", () => {
  it("pastes the copied tasks after the last selected row", () => {
    const ctx = context({ clipboard: CLIPBOARD, selection: [1, 2] });
    handleMenuAction("paste-task", ctx);
    expect(ctx.send).toHaveBeenCalledWith({ type: "paste_tasks", after_uid: 2, clipboard: CLIPBOARD });
  });

  it("does not paste before anything is copied", () => {
    const ctx = context();
    handleMenuAction("paste-task", ctx);
    expect(ctx.send).not.toHaveBeenCalled();
  });

  it("fills the focused column from the first row down", () => {
    const ctx = context({ selection: [1, 2], fillField: "name" });
    handleMenuAction("fill-down", ctx);
    expect(ctx.send).toHaveBeenCalledWith({ type: "fill_down", from_uid: 1, to_uids: [2], field: "name" });
  });

  it("does not fill down from a column it cannot fill", () => {
    const ctx = context({ selection: [1, 2], fillField: null });
    handleMenuAction("fill-down", ctx);
    expect(ctx.send).not.toHaveBeenCalled();
  });

  it("needs more than one row to fill down", () => {
    const ctx = context();
    handleMenuAction("fill-down", ctx);
    expect(ctx.send).not.toHaveBeenCalled();
  });

  it("copies through the backend", () => {
    const ctx = context();
    handleMenuAction("copy-task", ctx);
    expect(ctx.run).toHaveBeenCalledTimes(1);
  });
});

describe("handleMenuAction", () => {
  it("switches views, scales, language and theme", () => {
    const ctx = context();
    handleMenuAction("view-network", ctx);
    handleMenuAction("scale-week", ctx);
    handleMenuAction("lang-en", ctx);
    handleMenuAction("theme-dark", ctx);
    handleMenuAction("theme-system", ctx);
    handleMenuAction("theme-sepia", ctx);
    expect(ctx.setView).toHaveBeenCalledWith("network");
    expect(ctx.setScale).toHaveBeenCalledWith("week");
    expect(ctx.changeLang).toHaveBeenCalledWith("en");
    expect(vi.mocked(ctx.changeTheme).mock.calls).toEqual([["dark"], ["system"]]);
  });

  it("opens EVM beside a Gantt view", () => {
    const ctx = context({ view: "resources" });
    handleMenuAction("view-evm", ctx);
    const update = vi.mocked(ctx.setView).mock.calls[0][0] as (view: string) => string;
    expect([update("resources"), update("tracking")]).toEqual(["gantt", "tracking"]);
    expect(ctx.setEvmOpen).toHaveBeenCalledWith(true);
  });

  it("runs file commands through the file dialog and recent files by index", () => {
    const ctx = context();
    for (const id of ["new", "open", "save", "save-as", "import-xml", "export-xml", "compact-history", "recent:3"]) {
      handleMenuAction(id, ctx);
    }
    expect(ctx.startFileDialog).toHaveBeenCalledTimes(8);
  });

  it("stops undo at the state Thông tin dự án opened with", () => {
    const at = context({ infoStart: 0 });
    handleMenuAction("undo", at);
    expect(at.run).not.toHaveBeenCalled();
    const snap = { ...makeSnapshot(), history_cursor: 3 };
    const past = context({ infoStart: 1, snap });
    handleMenuAction("undo", past);
    expect(past.run).toHaveBeenCalledOnce();
  });

  it("acts on the selected task only when the toolbar would allow it", () => {
    const ctx = context();
    handleMenuAction("indent", ctx);
    handleMenuAction("delete", ctx);
    handleMenuAction("split-task", ctx);
    expect(vi.mocked(ctx.send).mock.calls).toEqual([[{ type: "indent", uid: 2 }], [{ type: "delete_task", uid: 2 }]]);
    expect(ctx.openSplit).toHaveBeenCalledWith(2);
    // The first task has nothing above it to indent under.
    const first = context({ task: makeTask({ uid: 1 }) });
    handleMenuAction("indent", first);
    expect(first.send).not.toHaveBeenCalled();
    const none = context({ task: null });
    handleMenuAction("delete", none);
    expect(none.send).not.toHaveBeenCalled();
  });
});

describe("working time", () => {
  it("opens Lịch làm việc from the menu", () => {
    const ctx = context();
    handleMenuAction("working-time", ctx);
    expect(ctx.openWorkingTime).toHaveBeenCalled();
  });
});

describe("custom fields", () => {
  it("opens Thông tin dự án on its custom fields tab", () => {
    const ctx = context();
    handleMenuAction("custom-fields", ctx);
    expect(ctx.openCustomFields).toHaveBeenCalled();
    expect(ctx.openProjectInfo).not.toHaveBeenCalled();
  });

  it("opens Thông tin dự án on its leveling tab", () => {
    const ctx = context();
    handleMenuAction("leveling-order", ctx);
    expect(ctx.openLevelingOrder).toHaveBeenCalled();
    expect(ctx.openProjectInfo).not.toHaveBeenCalled();
  });

  it("fills a custom field column down", () => {
    const ctx = context({ selection: [1, 2, 3], fillField: { field: 4 } });
    handleMenuAction("fill-down", ctx);
    expect(ctx.send).toHaveBeenCalledWith({ type: "fill_down", from_uid: 1, to_uids: [2, 3], field: { field: 4 } });
  });
});

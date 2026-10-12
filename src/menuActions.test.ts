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
    openBaseline: vi.fn(),
    openRecurring: vi.fn(),
    openSplit: vi.fn(),
    changeLang: vi.fn(),
    changeTheme: vi.fn(),
    ...overrides,
  };
}

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

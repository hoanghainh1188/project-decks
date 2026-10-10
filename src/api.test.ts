import { describe, expect, it } from "vitest";

import {
  applyCommand,
  errorMessage,
  inTauri,
  loadMetrics,
  loadNetworkLayout,
  loadState,
  savePdf,
  undo,
} from "./api";
import { fakeBackend } from "./test/fakeBackend";
import { makeMetrics, makeProject, makeSnapshot, outlineProject } from "./test/fixtures";

describe("api over the fake backend", () => {
  it("loads the snapshot and metrics through invoke", async () => {
    fakeBackend.reset(makeSnapshot(outlineProject()), makeMetrics({ has_baseline: false }));
    const snap = await loadState();
    const metrics = await loadMetrics();
    expect(snap.project.tasks).toHaveLength(8);
    expect(metrics.evm.has_baseline).toBe(false);
    expect(fakeBackend.calls.map((call) => call.cmd)).toEqual(["project_state", "project_metrics"]);
  });

  it("wraps a command in { command } and returns the next snapshot", async () => {
    fakeBackend.reset(makeSnapshot(makeProject({ name: "Cũ" })));
    const next = await applyCommand({ type: "rename_project", name: "Mới" });
    expect(fakeBackend.calls[0]).toEqual({
      cmd: "apply_command",
      args: { command: { type: "rename_project", name: "Mới" } },
    });
    expect(next.project.name).toBe("Mới");
    expect(next.dirty).toBe(true);
  });

  it("sends PDF bytes as a plain number array", async () => {
    await savePdf(new Uint8Array([37, 80, 68, 70]), "gantt.pdf");
    expect(fakeBackend.calls[0]).toEqual({
      cmd: "save_pdf",
      args: { bytes: [37, 80, 68, 70], fileName: "gantt.pdf" },
    });
  });

  it("asks the engine for the network layout with the view options", async () => {
    const layout = await loadNetworkLayout({ include_summary: true, visible_uids: [2, 3] });
    expect(fakeBackend.calls[0]).toEqual({
      cmd: "network_layout",
      args: { options: { include_summary: true, visible_uids: [2, 3] } },
    });
    expect(layout.nodes).toEqual([]);
  });

  it("surfaces backend errors as rejected promises", async () => {
    fakeBackend.failNext("undo", "Không còn gì để hoàn tác");
    await expect(undo()).rejects.toBe("Không còn gì để hoàn tác");
  });
});

describe("errorMessage", () => {
  it("reads strings, Errors and falls back for anything else", () => {
    expect(errorMessage("Lỗi engine")).toBe("Lỗi engine");
    expect(errorMessage(new Error("Hỏng"))).toBe("Hỏng");
    expect(errorMessage({ code: 1 })).toBe("Không thực hiện được thao tác");
  });
});

describe("inTauri", () => {
  it("is false in tests and true once the Tauri internals exist", () => {
    expect(inTauri()).toBe(false);
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    try {
      expect(inTauri()).toBe(true);
    } finally {
      delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    }
  });
});

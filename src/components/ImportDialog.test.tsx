import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { applyCommand } from "../api";
import { fakeBackend } from "../test/fakeBackend";
import { makeProject, makeResource, makeSnapshot, makeTask } from "../test/fixtures";
import type { Command, FieldDef, ImportSheet } from "../types";
import { ImportDialog } from "./ImportDialog";

const FIELDS: FieldDef[] = [{ id: 1, name: "Mảng", kind: { type: "choice", options: ["BE", "FE"] } }];
const project = makeProject({
  tasks: [makeTask({ uid: 1, name: "Mở đầu" })],
  resources: [makeResource({ uid: 10, name: "An" })],
  fields: FIELDS,
  next_task_uid: 2,
});

function sheet(rows: string[][], lines = rows.map((_, index) => index + 1), sheets: string[] = []): ImportSheet {
  return { sheets, sheet: sheets[0] ?? null, rows, lines };
}

const TASKS = sheet(
  [
    ["Tên", "Effort", "Người", "Mảng", "Ghi chú"],
    ["Thiết kế", "16", "An", "BE", "x"],
    ["Làm", "8", "An", "FE", ""],
  ],
  [1, 2, 4],
);

function renderDialog(initial: ImportSheet, overrides: Partial<Parameters<typeof ImportDialog>[0]> = {}) {
  fakeBackend.reset(makeSnapshot(project), undefined);
  const send = vi.fn((command: Command) => applyCommand(command));
  const onClose = vi.fn();
  render(
    <ImportDialog project={project} initial={initial} afterTask={project.tasks[0]} error={null} send={send} onClose={onClose} {...overrides} />,
  );
  return { send, onClose };
}

function importButton() {
  return screen.getByRole("button", { name: /^Nhập/ });
}

describe("ImportDialog", () => {
  it("guesses columns, previews rows by their file line and imports after the selected task", async () => {
    const { send, onClose } = renderDialog(TASKS);
    expect(screen.getByRole("combobox", { name: "Cột 1: Tên" })).toHaveValue("name");
    expect(screen.getByRole("combobox", { name: "Cột 2: Effort" })).toHaveValue("work");
    expect(screen.getByRole("combobox", { name: "Cột 3: Người" })).toHaveValue("assignees");
    expect(screen.getByRole("combobox", { name: "Cột 4: Mảng" })).toHaveValue("field:1");
    expect(screen.getByRole("combobox", { name: "Cột 5: Ghi chú" })).toHaveValue("skip");
    const preview = screen.getByRole("table", { name: "Xem trước" });
    expect(within(preview).getByRole("rowheader", { name: "4" })).toBeInTheDocument();
    expect(screen.getByText(/Thêm sau “Mở đầu”/)).toBeInTheDocument();
    await waitFor(() => expect(importButton()).toBeEnabled());
    expect(importButton()).toHaveTextContent("Nhập 2 dòng");
    fireEvent.click(importButton());
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(send).toHaveBeenCalledWith({
      type: "import_rows",
      after_uid: 1,
      resources: [],
      tasks: [
        { name: "Thiết kế", level: 0, work: 960, values: [["Mảng", "BE"]], assignees: [{ name: "An", units: 1 }], after: [] },
        { name: "Làm", level: 0, work: 480, values: [["Mảng", "FE"]], assignees: [{ name: "An", units: 1 }], after: [] },
      ],
    });
    const check = fakeBackend.calls.find((call) => call.cmd === "check_import");
    expect(check?.args?.afterUid).toBe(1);
  });

  it("marks a cell it cannot read, lists it by line and keeps Nhập off", async () => {
    renderDialog(sheet([["Tên", "Effort"], ["A", "nhiều"]]));
    const cell = await screen.findByRole("cell", { name: "nhiều" });
    expect(cell).toHaveClass("bad");
    expect(screen.getByRole("list", { name: "Lỗi" })).toHaveTextContent("Dòng 2, cột Effort: Effort không đọc được");
    expect(importButton()).toBeDisabled();
    expect(fakeBackend.calls.some((call) => call.cmd === "check_import")).toBe(false);
  });

  it("shows what the engine refuses at the file line", async () => {
    renderDialog(TASKS);
    fakeBackend.importProblems = [{ table: "tasks", row: 1, column: "assignees", message: "Dự án không có người tên này" }];
    // Any change asks again.
    fireEvent.click(screen.getByRole("radio", { name: "Nguồn lực" }));
    fireEvent.click(screen.getByRole("radio", { name: "Công việc" }));
    const list = await screen.findByRole("list", { name: "Lỗi" });
    expect(list).toHaveTextContent("Dòng 4, cột Người: Dự án không có người tên này");
    expect(importButton()).toBeDisabled();
  });

  it("lets a column be left out and switches to importing people", async () => {
    renderDialog(sheet([["Họ tên", "Kỹ năng"], ["Bình", "SQL"]]));
    fireEvent.change(screen.getByRole("combobox", { name: "Cột 2: Kỹ năng" }), { target: { value: "skip" } });
    expect(screen.getByRole("combobox", { name: "Cột 2: Kỹ năng" })).toHaveValue("skip");
    fireEvent.click(screen.getByRole("radio", { name: "Nguồn lực" }));
    expect(screen.getByRole("combobox", { name: "Cột 1: Họ tên" })).toHaveValue("name");
    expect(screen.getByRole("combobox", { name: "Cột 2: Kỹ năng" })).toHaveValue("skills");
    expect(screen.getByText(/Người đã có trong dự án giữ nguyên/)).toBeInTheDocument();
    await waitFor(() => expect(importButton()).toBeEnabled());
  });

  it("reads another sheet of a workbook", async () => {
    renderDialog(sheet([["Tên"], ["Từ trang một"]], [1, 2], ["Một", "Hai"]));
    fakeBackend.table = sheet([["Tên"], ["Từ trang hai"]], [1, 2], ["Một", "Hai"]);
    fireEvent.change(screen.getByRole("combobox", { name: "Trang" }), { target: { value: "Hai" } });
    expect(await screen.findByRole("cell", { name: "Từ trang hai" })).toBeInTheDocument();
  });

  it("starts the data below the header row chosen", async () => {
    renderDialog(sheet([["Báo cáo"], ["Tên"], ["A"]]));
    fireEvent.change(screen.getByRole("combobox", { name: "Dòng tiêu đề" }), { target: { value: "1" } });
    expect(screen.getByRole("combobox", { name: "Cột 1: Tên" })).toHaveValue("name");
    await waitFor(() => expect(importButton()).toHaveTextContent("Nhập 1 dòng"));
  });
});

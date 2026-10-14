import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { I18nProvider } from "../i18n";
import { makeProject, makeTask } from "../test/fixtures";
import type { FieldDef, Project } from "../types";
import { ProjectDialog } from "./ProjectDialog";

function renderTab(project: Project, error: string | null = null) {
  // Accepted unless a test says otherwise: the App's send resolves to a snapshot.
  const send = vi.fn().mockResolvedValue({});
  render(
    <I18nProvider lang="vi">
      <ProjectDialog
        project={project}
        statistics={null}
        changed={false}
        onSave={vi.fn()}
        onCancel={vi.fn()}
        send={send}
        onOpenWorkingTime={vi.fn()}
        initialTab="fields"
        error={error}
      />
    </I18nProvider>,
  );
  return { send, user: userEvent.setup() };
}

const AREA: FieldDef = { id: 4, name: "Mảng", kind: { type: "choice", options: ["BE", "FE"] } };

describe("Custom fields tab", () => {
  it("opens on its tab and adds a field with a free name", async () => {
    const project = makeProject({ fields: [{ id: 1, name: "Trường 1", kind: { type: "text" } }] });
    const { send, user } = renderTab(project);
    expect(screen.getByRole("tab", { name: "Trường tùy chỉnh" })).toHaveAttribute("aria-selected", "true");
    await user.click(screen.getByRole("button", { name: "Thêm trường" }));
    expect(send).toHaveBeenCalledWith({ type: "add_field", name: "Trường 2", kind: { type: "text" } });
  });

  it("renames a field and changes its kind", async () => {
    const project = makeProject({ fields: [{ id: 1, name: "Điểm", kind: { type: "text" } }] });
    const { send, user } = renderTab(project);
    const name = screen.getByRole("textbox", { name: "Tên trường" });
    await user.clear(name);
    await user.type(name, "Điểm rủi ro");
    await user.tab();
    expect(send).toHaveBeenCalledWith({
      type: "update_field",
      id: 1,
      name: "Điểm rủi ro",
      kind: { type: "text" },
      renamed_options: [],
    });
    await user.selectOptions(screen.getByRole("combobox", { name: "Kiểu" }), "number");
    expect(send).toHaveBeenLastCalledWith({
      type: "update_field",
      id: 1,
      name: "Điểm",
      kind: { type: "number" },
      renamed_options: [],
    });
  });

  it("turns a text field into a list of the values tasks already have", async () => {
    const project = makeProject({
      fields: [{ id: 2, name: "Khách", kind: { type: "text" } }],
      tasks: [makeTask({ uid: 1, values: { 2: "Globex" } }), makeTask({ uid: 2, values: { 2: "ACME" } })],
    });
    const { send, user } = renderTab(project);
    expect(screen.getByText("2 công việc có giá trị")).toBeInTheDocument();
    await user.selectOptions(screen.getByRole("combobox", { name: "Kiểu" }), "choice");
    expect(send).toHaveBeenCalledWith({
      type: "update_field",
      id: 2,
      name: "Khách",
      kind: { type: "choice", options: ["ACME", "Globex"] },
      renamed_options: [],
    });
  });

  it("renames, adds and removes choices, each from the list the last one left", async () => {
    const { send, user } = renderTab(makeProject({ fields: [AREA] }));
    const first = screen.getByRole("textbox", { name: "Lựa chọn 1" });
    await user.clear(first);
    await user.type(first, "Backend");
    await user.tab();
    expect(send).toHaveBeenCalledWith({
      type: "update_field",
      id: 4,
      name: "Mảng",
      kind: { type: "choice", options: ["Backend", "FE"] },
      renamed_options: [{ from: "BE", to: "Backend" }],
    });
    // The snapshot has not come back yet: the next edits still build on the rename.
    await user.type(screen.getByRole("textbox", { name: "Lựa chọn mới" }), "QA{Enter}");
    expect(send).toHaveBeenLastCalledWith(
      expect.objectContaining({ kind: { type: "choice", options: ["Backend", "FE", "QA"] }, renamed_options: [] }),
    );
    await user.click(screen.getByRole("button", { name: "Bỏ lựa chọn “FE”" }));
    expect(send).toHaveBeenLastCalledWith(
      expect.objectContaining({ kind: { type: "choice", options: ["Backend", "QA"] } }),
    );
  });

  it("goes back to the stored choices after a refused change", async () => {
    const { send, user } = renderTab(makeProject({ fields: [AREA] }));
    send.mockResolvedValueOnce(undefined);
    const first = screen.getByRole("textbox", { name: "Lựa chọn 1" });
    await user.clear(first);
    await user.type(first, "Backend");
    await user.tab();
    await vi.waitFor(() => expect(first).toHaveValue("BE"));
    await user.click(screen.getByRole("button", { name: "Bỏ lựa chọn “FE”" }));
    expect(send).toHaveBeenLastCalledWith(expect.objectContaining({ kind: { type: "choice", options: ["BE"] } }));
  });

  it("removes a choice just renamed when its own ✕ is clicked", async () => {
    const { send, user } = renderTab(makeProject({ fields: [AREA] }));
    const first = screen.getByRole("textbox", { name: "Lựa chọn 1" });
    await user.clear(first);
    await user.type(first, "Backend");
    // The click blurs the box (sending the rename), then removes the renamed choice.
    await user.click(screen.getByRole("button", { name: "Bỏ lựa chọn “BE”" }));
    expect(send).toHaveBeenLastCalledWith(expect.objectContaining({ kind: { type: "choice", options: ["FE"] } }));
  });

  it("keeps a refused new choice in its box", async () => {
    const { send, user } = renderTab(makeProject({ fields: [AREA] }));
    send.mockResolvedValueOnce(undefined);
    const box = screen.getByRole("textbox", { name: "Lựa chọn mới" });
    await user.type(box, "QA{Enter}");
    await vi.waitFor(() => expect(send).toHaveBeenCalled());
    expect(box).toHaveValue("QA");
  });

  it("keeps the last choice and deletes a field", async () => {
    const only: FieldDef = { id: 7, name: "Một", kind: { type: "choice", options: ["A"] } };
    const { send, user } = renderTab(makeProject({ fields: [only] }));
    expect(screen.getByRole("button", { name: "Bỏ lựa chọn “A”" })).toBeDisabled();
    const card = screen.getByRole("group", { name: "Một" });
    await user.click(within(card).getByRole("button", { name: "Xóa" }));
    expect(send).toHaveBeenCalledWith({ type: "delete_field", id: 7 });
  });

  it("puts a refused name back", async () => {
    const project = makeProject({ fields: [{ id: 1, name: "Khách", kind: { type: "text" } }] });
    const { send, user } = renderTab(project);
    // The App's send resolves to nothing when the engine refuses the command.
    send.mockResolvedValueOnce(undefined);
    const name = screen.getByRole("textbox", { name: "Tên trường" });
    await user.clear(name);
    await user.type(name, "Trùng");
    await user.tab();
    await vi.waitFor(() => expect(name).toHaveValue("Khách"));
  });

  it("closes kinds whose slots are full", () => {
    const numbers = Array.from({ length: 20 }, (_, index) => ({
      id: index + 2,
      name: `Số ${index}`,
      kind: { type: "number" as const },
    }));
    renderTab(makeProject({ fields: [{ id: 1, name: "Chữ", kind: { type: "text" } }, ...numbers] }));
    const kinds = screen.getAllByRole("combobox", { name: "Kiểu" });
    const option = (select: HTMLElement, value: string) =>
      [...select.querySelectorAll("option")].find((item) => item.value === value)!;
    expect(option(kinds[0], "number")).toBeDisabled();
    expect(option(kinds[0], "choice")).not.toBeDisabled();
    expect(option(kinds[1], "number")).not.toBeDisabled();
  });

  it("shows what the engine refused in the footer", () => {
    renderTab(makeProject(), "Đã có trường cùng tên");
    expect(screen.getByRole("alert")).toHaveTextContent("Đã có trường cùng tên");
  });
});

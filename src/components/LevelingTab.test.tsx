import { act, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { I18nProvider } from "../i18n";
import { makeProject } from "../test/fixtures";
import type { Command, Project, Snapshot, SortKey } from "../types";
import { ProjectDialog } from "./ProjectDialog";

function renderTab(project: Project, { error = null, lang = "vi" }: { error?: string | null; lang?: "vi" | "en" } = {}) {
  const send = vi.fn().mockResolvedValue({});
  render(
    <I18nProvider lang={lang}>
      <ProjectDialog
        project={project}
        statistics={null}
        changed={false}
        onSave={vi.fn()}
        onCancel={vi.fn()}
        send={send}
        onOpenWorkingTime={vi.fn()}
        initialTab="leveling"
        error={error}
      />
    </I18nProvider>,
  );
  return { send, user: userEvent.setup() };
}

const STANDARD: SortKey[] = [
  { criterion: { type: "start" }, descending: false },
  { criterion: { type: "priority" }, descending: true },
  { criterion: { type: "slack" }, descending: false },
];

const AREA = { id: 4, name: "Mảng", kind: { type: "choice" as const, options: ["BE", "FE"] } };

function rows() {
  return within(screen.getByRole("list", { name: "Thứ tự cân bằng" })).getAllByRole("listitem");
}

describe("Leveling tab", () => {
  it("opens on its tab and shows the standard order when the list is empty", () => {
    renderTab(makeProject());
    expect(screen.getByRole("tab", { name: "Cân bằng" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("Đang dùng thứ tự chuẩn.")).toBeInTheDocument();
    expect(screen.getByText(/luôn được xếp trước/)).toBeInTheDocument();
    const items = rows();
    expect(items).toHaveLength(3);
    expect(within(items[0]).getByRole("combobox", { name: "Tiêu chí 1" })).toHaveValue("start");
    expect(within(items[1]).getByRole("combobox", { name: "Tiêu chí 2" })).toHaveValue("priority");
    expect(within(items[1]).getByRole("button", { name: "Giảm dần: Ưu tiên" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Chuẩn" })).toHaveAttribute("aria-pressed", "true");
  });

  it("applies the three presets", async () => {
    const { send, user } = renderTab(makeProject({ leveling_order: [{ criterion: { type: "id" }, descending: false }] }));
    expect(screen.getByRole("button", { name: "Theo ID" })).toHaveAttribute("aria-pressed", "true");
    await user.click(screen.getByRole("button", { name: "Chuẩn" }));
    expect(send).toHaveBeenLastCalledWith({ type: "set_leveling_order", order: [] });
    await user.click(screen.getByRole("button", { name: "Ưu tiên trước" }));
    expect(send).toHaveBeenLastCalledWith({
      type: "set_leveling_order",
      order: [
        { criterion: { type: "priority" }, descending: true },
        { criterion: { type: "start" }, descending: false },
        { criterion: { type: "slack" }, descending: false },
      ],
    });
    await user.click(screen.getByRole("button", { name: "Theo ID" }));
    expect(send).toHaveBeenLastCalledWith({
      type: "set_leveling_order",
      order: [{ criterion: { type: "id" }, descending: false }],
    });
  });

  it("changes a criterion to a custom field and flips its direction", async () => {
    const { send, user } = renderTab(makeProject({ fields: [AREA] }));
    const first = rows()[0];
    const select = within(first).getByRole("combobox", { name: "Tiêu chí 1" });
    // A criterion another row uses cannot be chosen twice.
    expect(within(select).getByRole("option", { name: "Ưu tiên" })).toBeDisabled();
    await user.selectOptions(select, "field:4");
    expect(send).toHaveBeenLastCalledWith({
      type: "set_leveling_order",
      order: [{ criterion: { type: "field", id: 4 }, descending: false }, ...STANDARD.slice(1)],
    });
    await user.click(within(rows()[2]).getByRole("button", { name: "Tăng dần: Slack (trước cân bằng)" }));
    expect(send).toHaveBeenLastCalledWith({
      type: "set_leveling_order",
      // Built on the change before it, which this static render does not show.
      order: [{ criterion: { type: "field", id: 4 }, descending: false }, STANDARD[1], { ...STANDARD[2], descending: true }],
    });
  });

  it("moves, removes and adds criteria, each change on the last", async () => {
    const order: SortKey[] = [
      { criterion: { type: "field", id: 4 }, descending: false },
      { criterion: { type: "deadline" }, descending: false },
    ];
    const { send, user } = renderTab(makeProject({ fields: [AREA], leveling_order: order }));
    expect(screen.getByRole("button", { name: "Đưa “Mảng” lên" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Đưa “Hạn chót” xuống" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Đưa “Mảng” xuống" }));
    expect(send).toHaveBeenLastCalledWith({ type: "set_leveling_order", order: [order[1], order[0]] });
    await user.click(screen.getByRole("button", { name: "Bỏ “Hạn chót”" }));
    expect(send).toHaveBeenLastCalledWith({ type: "set_leveling_order", order: [order[0]] });
    await user.click(screen.getByRole("button", { name: "Thêm tiêu chí" }));
    expect(send).toHaveBeenLastCalledWith({
      type: "set_leveling_order",
      order: [order[0], { criterion: { type: "start" }, descending: false }],
    });
  });

  it("stops adding at eight criteria", () => {
    const fields = [1, 2].map((id) => ({ id, name: `T${id}`, kind: { type: "text" as const } }));
    const all: SortKey[] = (["start", "priority", "slack", "deadline", "duration", "id"] as const)
      .map((type) => ({ criterion: { type }, descending: false }) as SortKey)
      .concat(fields.map((field) => ({ criterion: { type: "field", id: field.id }, descending: false })));
    renderTab(makeProject({ fields, leveling_order: all }));
    expect(rows()).toHaveLength(8);
    expect(screen.getByRole("button", { name: "Thêm tiêu chí" })).toBeDisabled();
  });

  it("does not remove the only criterion", () => {
    renderTab(makeProject({ leveling_order: [{ criterion: { type: "id" }, descending: false }] }));
    expect(screen.getByRole("button", { name: "Bỏ “ID”" })).toBeDisabled();
  });

  it("builds each change on the one before it, even before the answer comes back", async () => {
    // The engine's answers wait until the test lets them through.
    const answers: (() => void)[] = [];
    const sent: Command[] = [];
    function Live() {
      const [project, setProject] = useState(makeProject({ fields: [AREA] }));
      const send = (command: Command) => {
        sent.push(command);
        return new Promise<Snapshot>((resolve) =>
          answers.push(() => {
            if (command.type === "set_leveling_order") setProject((p) => ({ ...p, leveling_order: command.order }));
            resolve({} as Snapshot);
          }),
        );
      };
      return (
        <ProjectDialog
          project={project}
          statistics={null}
          changed={false}
          onSave={vi.fn()}
          onCancel={vi.fn()}
          send={send}
          onOpenWorkingTime={vi.fn()}
          initialTab="leveling"
        />
      );
    }
    render(
      <I18nProvider lang="vi">
        <Live />
      </I18nProvider>,
    );
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Đưa “Ngày bắt đầu” xuống" }));
    await user.click(screen.getByRole("button", { name: "Tăng dần: Slack (trước cân bằng)" }));
    expect(sent).toEqual([
      { type: "set_leveling_order", order: [STANDARD[1], STANDARD[0], STANDARD[2]] },
      { type: "set_leveling_order", order: [STANDARD[1], STANDARD[0], { ...STANDARD[2], descending: true }] },
    ]);
    await act(async () => answers.splice(0).forEach((answer) => answer()));
    expect(screen.queryByText("Đang dùng thứ tự chuẩn.")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Chuẩn" })).toHaveAttribute("aria-pressed", "false");

    // A new criterion comes back as a new row; its select keeps the focus.
    await user.selectOptions(screen.getByRole("combobox", { name: "Tiêu chí 1" }), "field:4");
    await act(async () => answers.splice(0).forEach((answer) => answer()));
    const select = screen.getByRole("combobox", { name: "Tiêu chí 1" });
    expect(select).toHaveValue("field:4");
    expect(select).toHaveFocus();
  });

  it("shows what the engine refused in the footer", () => {
    renderTab(makeProject(), { error: "Thứ tự cân bằng có tối đa 8 tiêu chí" });
    expect(screen.getByRole("alert")).toHaveTextContent("Thứ tự cân bằng có tối đa 8 tiêu chí");
  });

  it("speaks English", () => {
    renderTab(makeProject(), { lang: "en" });
    expect(screen.getByRole("tab", { name: "Leveling" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("button", { name: "Priority first" })).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Leveling order" })).toBeInTheDocument();
  });
});

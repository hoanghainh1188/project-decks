import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { formatInstant } from "../format";
import { I18nProvider } from "../i18n";
import { at, makeProject, makeResource, makeTask } from "../test/fixtures";
import type { Suggestion } from "../types";
import { AssignPreviewDialog } from "./AssignPreviewDialog";

const project = makeProject({
  tasks: [1, 2, 3, 4].map((uid) => makeTask({ uid, name: `Việc ${uid}` })),
  resources: [makeResource({ uid: 10, name: "An" }), makeResource({ uid: 11, name: "Bình" })],
});
const DAY1 = at(2026, 1, 5, 17);
const DAY2 = at(2026, 1, 6, 17);
const SUGGESTIONS: Suggestion[] = [
  {
    task_uid: 1,
    rule: 0,
    units: 1,
    picks: [
      { resource_uid: 10, start: at(2026, 1, 5, 8), finish: DAY1 },
      { resource_uid: 11, start: at(2026, 1, 6, 8), finish: DAY2 },
    ],
    reason: null,
  },
  { task_uid: 2, rule: 1, units: 0.5, picks: [{ resource_uid: 11, start: at(2026, 1, 5, 8), finish: DAY2 }], reason: null },
  { task_uid: 3, rule: null, units: 0, picks: [], reason: "no_rule" },
  { task_uid: 4, rule: 2, units: 1, picks: [], reason: "no_room" },
];

function renderDialog(suggestions = SUGGESTIONS, send = vi.fn().mockResolvedValue({}), lang: "vi" | "en" = "vi") {
  const onClose = vi.fn();
  render(
    <I18nProvider lang={lang}>
      <AssignPreviewDialog project={project} suggestions={suggestions} error={null} send={send} onClose={onClose} />
    </I18nProvider>,
  );
  return { send, onClose, user: userEvent.setup() };
}

describe("AssignPreviewDialog", () => {
  it("lists the suggested tasks ticked with their first person and the rest by reason", () => {
    renderDialog();
    expect(screen.getByRole("dialog", { name: "Giao việc theo quy tắc" })).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Áp dụng cho 1. Việc 1" })).toBeChecked();
    expect(screen.getByRole("combobox", { name: "Giao 1. Việc 1 cho" })).toHaveValue("10");
    const choices = within(screen.getByRole("combobox", { name: "Giao 1. Việc 1 cho" })).getAllByRole("option");
    expect(choices.map((option) => option.textContent)).toEqual([
      `An · xong ${formatInstant(DAY1)}`,
      `Bình · xong ${formatInstant(DAY2)}`,
    ]);
    expect(screen.getByText("Không quy tắc nào khớp (1)")).toBeInTheDocument();
    expect(screen.getByText(/Không ai vừa trong bốn năm/)).toBeInTheDocument();
    expect(screen.getByText("Đã chọn 2/2")).toBeInTheDocument();
  });

  it("applies the ticked rows with the chosen people as one command, then offers to level", async () => {
    const { send, onClose, user } = renderDialog();
    await user.selectOptions(screen.getByRole("combobox", { name: "Giao 1. Việc 1 cho" }), "11");
    expect(screen.getByRole("row", { name: /1\. Việc 1/ })).toHaveTextContent(formatInstant(DAY2));
    await user.click(screen.getByRole("button", { name: "Áp dụng (2)" }));
    expect(send).toHaveBeenCalledWith({
      type: "assign_tasks",
      assignments: [
        { task_uid: 1, resource_uid: 11, units: 1 },
        { task_uid: 2, resource_uid: 11, units: 0.5 },
      ],
    });
    expect(await screen.findByRole("status")).toHaveTextContent("Đã giao 2 công việc");
    await user.click(screen.getByRole("button", { name: "Cân bằng nguồn lực" }));
    expect(send).toHaveBeenLastCalledWith({ type: "level_resources" });
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("leaves out unticked rows and cannot apply nothing", async () => {
    const { send, user } = renderDialog();
    await user.click(screen.getByRole("checkbox", { name: "Áp dụng cho 2. Việc 2" }));
    expect(screen.getByRole("button", { name: "Áp dụng (1)" })).toBeEnabled();
    await user.click(screen.getByRole("checkbox", { name: "Áp dụng cho mọi công việc" }));
    expect(screen.getByRole("button", { name: "Áp dụng (2)" })).toBeEnabled();
    await user.click(screen.getByRole("checkbox", { name: "Áp dụng cho mọi công việc" }));
    expect(screen.getByRole("button", { name: "Áp dụng (0)" })).toBeDisabled();
    expect(send).not.toHaveBeenCalled();
  });

  it("stays on the review when the engine refuses", async () => {
    const { user } = renderDialog(SUGGESTIONS, vi.fn().mockResolvedValue(undefined));
    await user.click(screen.getByRole("button", { name: "Áp dụng (2)" }));
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Áp dụng (2)" })).toBeEnabled();
  });

  it("drops rows the project no longer allows when it changes under the dialog", async () => {
    const send = vi.fn().mockResolvedValue({});
    const { rerender } = render(
      <I18nProvider lang="vi">
        <AssignPreviewDialog project={project} suggestions={SUGGESTIONS} error={null} send={send} onClose={vi.fn()} />
      </I18nProvider>,
    );
    // Undo took task 2 and Bình (one of task 1's people) away while the dialog was open.
    const changed = {
      ...project,
      tasks: project.tasks.filter((task) => task.uid !== 2),
      resources: project.resources.filter((item) => item.uid !== 11),
    };
    rerender(
      <I18nProvider lang="vi">
        <AssignPreviewDialog project={changed} suggestions={SUGGESTIONS} error={null} send={send} onClose={vi.fn()} />
      </I18nProvider>,
    );
    expect(screen.queryByRole("checkbox", { name: "Áp dụng cho 2. Việc 2" })).not.toBeInTheDocument();
    expect(screen.getByText(/1 gợi ý không còn áp dụng/)).toBeInTheDocument();
    const people = within(screen.getByRole("combobox", { name: "Giao 1. Việc 1 cho" })).getAllByRole("option");
    expect(people).toHaveLength(1);
    await userEvent.setup().click(screen.getByRole("button", { name: "Áp dụng (1)" }));
    expect(send).toHaveBeenCalledWith({ type: "assign_tasks", assignments: [{ task_uid: 1, resource_uid: 10, units: 1 }] });
  });

  it("leaves out a task that got an assignment meanwhile", () => {
    const staffed = {
      ...project,
      assignments: [
        {
          uid: 1, task_uid: 1, resource_uid: 10, units: 1, work: 480, overtime_work: 0, actual_work: 0,
          remaining_work: 480, cost: 0, actual_cost: 0, manual_actual_cost: null,
        },
      ],
    };
    render(
      <I18nProvider lang="vi">
        <AssignPreviewDialog project={staffed} suggestions={SUGGESTIONS} error={null} send={vi.fn()} onClose={vi.fn()} />
      </I18nProvider>,
    );
    expect(screen.queryByRole("checkbox", { name: "Áp dụng cho 1. Việc 1" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Áp dụng (1)" })).toBeEnabled();
  });

  it("sends once however fast Áp dụng is pressed, and cannot close meanwhile", async () => {
    let answer: (state: unknown) => void = () => {};
    const send = vi.fn().mockReturnValue(new Promise((resolve) => (answer = resolve)));
    const { onClose, user } = renderDialog(SUGGESTIONS, send);
    const apply = screen.getByRole("button", { name: "Áp dụng (2)" });
    await user.dblClick(apply);
    await user.keyboard("{Escape}");
    expect(send).toHaveBeenCalledTimes(1);
    expect(onClose).not.toHaveBeenCalled();
    await act(async () => answer({}));
    expect(await screen.findByRole("status")).toBeInTheDocument();
  });

  it("stays on the done step when leveling is refused", async () => {
    const send = vi.fn().mockResolvedValueOnce({}).mockResolvedValueOnce(undefined);
    const { onClose, user } = renderDialog(SUGGESTIONS, send);
    await user.click(screen.getByRole("button", { name: "Áp dụng (2)" }));
    await user.click(await screen.findByRole("button", { name: "Cân bằng nguồn lực" }));
    expect(send).toHaveBeenLastCalledWith({ type: "level_resources" });
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Đóng" })).toBeEnabled();
  });

  it("marks the all box half ticked when only some rows are", async () => {
    const { user } = renderDialog();
    await user.click(screen.getByRole("checkbox", { name: "Áp dụng cho 2. Việc 2" }));
    const all = screen.getByRole("checkbox", { name: "Áp dụng cho mọi công việc" }) as HTMLInputElement;
    expect(all.indeterminate).toBe(true);
    expect(all).not.toBeChecked();
  });

  it("says when there is nothing to assign, and closes", async () => {
    const { onClose, user } = renderDialog([]);
    expect(screen.getByText(/Không có công việc nào chưa giao/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Áp dụng (0)" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Hủy" }));
    expect(onClose).toHaveBeenCalled();
  });

  it("lists at most twenty tasks under a reason", () => {
    const many: Suggestion[] = Array.from({ length: 23 }, (_, index) => ({
      task_uid: 100 + index,
      rule: null,
      units: 0,
      picks: [],
      reason: "no_rule",
    }));
    renderDialog(many);
    expect(screen.getByText(/và 3 công việc khác/)).toBeInTheDocument();
  });

  it("speaks English", () => {
    renderDialog(SUGGESTIONS, undefined, "en");
    expect(screen.getByRole("button", { name: "Apply (2)" })).toBeInTheDocument();
    expect(screen.getByText("No rule matches (1)")).toBeInTheDocument();
  });
});

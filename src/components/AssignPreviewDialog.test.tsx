import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { formatInstant } from "../format";
import { I18nProvider } from "../i18n";
import { fakeBackend } from "../test/fakeBackend";
import { at, makeProject, makeResource, makeTask } from "../test/fixtures";
import type { PlanPreview, Suggestion } from "../types";
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

function renderDialog(suggestions = SUGGESTIONS, applyPlan = vi.fn().mockResolvedValue({}), lang: "vi" | "en" = "vi") {
  const onClose = vi.fn();
  render(
    <I18nProvider lang={lang}>
      <AssignPreviewDialog project={project} suggestions={suggestions} error={null} applyPlan={applyPlan} onClose={onClose} />
    </I18nProvider>,
  );
  return { applyPlan, onClose, user: userEvent.setup() };
}

/** The assignments of the last `preview_plan` call. */
function previewed() {
  const calls = fakeBackend.calls.filter((call) => call.cmd === "preview_plan");
  return calls[calls.length - 1]?.args?.assignments;
}

const APPLY = "Áp dụng kế hoạch";

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
    expect(screen.getByRole("status")).toHaveTextContent("Đã chọn 2/2");
  });

  it("previews the plan for the chosen people and applies exactly that in one step", async () => {
    const { applyPlan, onClose, user } = renderDialog();
    await user.selectOptions(screen.getByRole("combobox", { name: "Giao 1. Việc 1 cho" }), "11");
    expect(screen.getByRole("row", { name: /1\. Việc 1/ })).toHaveTextContent(formatInstant(DAY2));
    await waitFor(() =>
      expect(previewed()).toEqual([
        { task_uid: 1, resource_uid: 11, units: 1 },
        { task_uid: 2, resource_uid: 11, units: 0.5 },
      ]),
    );
    await user.click(await screen.findByRole("button", { name: APPLY }));
    expect(applyPlan).toHaveBeenCalledWith(expect.objectContaining({ assignments: previewed() }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("shows the tasks that move, those late on their deadline and who stays overallocated", async () => {
    const plan: PlanPreview = {
      assignments: [
        { task_uid: 2, resource_uid: 11, units: 1 },
        { task_uid: 4, resource_uid: 10, units: 1 },
      ],
      delays: [{ uid: 2, minutes: 480 }],
      splits: [],
      moves: [
        { uid: 2, start_before: at(2026, 1, 5, 8), finish_before: DAY1, start: at(2026, 1, 6, 8), finish: DAY2 },
        { uid: 4, start_before: at(2026, 1, 5, 8), finish_before: DAY1, start: at(2026, 1, 6, 8), finish: DAY2 },
      ],
      late: [2],
      overallocated: [10],
    };
    const { user } = renderDialog();
    fakeBackend.planPreview = plan;
    await user.click(screen.getByRole("checkbox", { name: "Áp dụng cho 2. Việc 2" }));
    const moves = await screen.findByRole("table", { name: "Việc dời ngày" });
    const row = within(moves).getByRole("row", { name: /2\. Việc 2/ });
    expect(row).toHaveTextContent(`${formatInstant(DAY1)} → ${formatInstant(DAY2)}`);
    expect(row).toHaveTextContent("Trễ hạn");
    expect(screen.getByText(/Còn quá tải: An/)).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("2 việc dời ngày, 1 việc trễ hạn chót");
    // Task 2 goes to Bình: the filter keeps only Bình's moves.
    await user.selectOptions(screen.getByRole("combobox", { name: "Lọc theo người" }), "11");
    expect(within(moves).queryByRole("row", { name: /4\. Việc 4/ })).not.toBeInTheDocument();
  });

  it("leaves out unticked rows and cannot apply a plan that changes nothing", async () => {
    const { applyPlan, user } = renderDialog();
    await user.click(screen.getByRole("checkbox", { name: "Áp dụng cho 2. Việc 2" }));
    await waitFor(() => expect(previewed()).toEqual([{ task_uid: 1, resource_uid: 10, units: 1 }]));
    expect(screen.getByRole("button", { name: APPLY })).toBeEnabled();
    await user.click(screen.getByRole("checkbox", { name: "Áp dụng cho mọi công việc" }));
    await user.click(screen.getByRole("checkbox", { name: "Áp dụng cho mọi công việc" }));
    await waitFor(() => expect(previewed()).toEqual([]));
    expect(screen.getByRole("button", { name: APPLY })).toBeDisabled();
    expect(applyPlan).not.toHaveBeenCalled();
  });

  it("stays open when the plan is refused", async () => {
    const { onClose, user } = renderDialog(SUGGESTIONS, vi.fn().mockResolvedValue(undefined));
    await user.click(await screen.findByRole("button", { name: APPLY }));
    expect(onClose).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.getByRole("button", { name: APPLY })).toBeEnabled());
  });

  it("drops rows the project no longer allows when it changes under the dialog", async () => {
    const applyPlan = vi.fn().mockResolvedValue({});
    const { rerender } = render(
      <I18nProvider lang="vi">
        <AssignPreviewDialog project={project} suggestions={SUGGESTIONS} error={null} applyPlan={applyPlan} onClose={vi.fn()} />
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
        <AssignPreviewDialog project={changed} suggestions={SUGGESTIONS} error={null} applyPlan={applyPlan} onClose={vi.fn()} />
      </I18nProvider>,
    );
    expect(screen.queryByRole("checkbox", { name: "Áp dụng cho 2. Việc 2" })).not.toBeInTheDocument();
    expect(screen.getByText(/1 gợi ý không còn áp dụng/)).toBeInTheDocument();
    const people = within(screen.getByRole("combobox", { name: "Giao 1. Việc 1 cho" })).getAllByRole("option");
    expect(people).toHaveLength(1);
    await waitFor(() => expect(previewed()).toEqual([{ task_uid: 1, resource_uid: 10, units: 1 }]));
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
        <AssignPreviewDialog project={staffed} suggestions={SUGGESTIONS} error={null} applyPlan={vi.fn()} onClose={vi.fn()} />
      </I18nProvider>,
    );
    expect(screen.queryByRole("checkbox", { name: "Áp dụng cho 1. Việc 1" })).not.toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Đã chọn 1/1");
  });

  it("sends once however fast Áp dụng is pressed, and cannot close meanwhile", async () => {
    let answer: (state: unknown) => void = () => {};
    const applyPlan = vi.fn().mockReturnValue(new Promise((resolve) => (answer = resolve)));
    const { onClose, user } = renderDialog(SUGGESTIONS, applyPlan);
    const apply = await screen.findByRole("button", { name: APPLY });
    await waitFor(() => expect(apply).toBeEnabled());
    await user.dblClick(apply);
    await user.keyboard("{Escape}");
    expect(applyPlan).toHaveBeenCalledTimes(1);
    expect(onClose).not.toHaveBeenCalled();
    await act(async () => answer({}));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("marks the all box half ticked when only some rows are", async () => {
    const { user } = renderDialog();
    await user.click(screen.getByRole("checkbox", { name: "Áp dụng cho 2. Việc 2" }));
    const all = screen.getByRole("checkbox", { name: "Áp dụng cho mọi công việc" }) as HTMLInputElement;
    expect(all.indeterminate).toBe(true);
    expect(all).not.toBeChecked();
  });

  it("says when there is nothing to assign, still previews leveling, and closes", async () => {
    const { onClose, user } = renderDialog([]);
    expect(screen.getByText(/Không có công việc nào chưa giao/)).toBeInTheDocument();
    await waitFor(() => expect(previewed()).toEqual([]));
    expect(screen.getByRole("button", { name: APPLY })).toBeDisabled();
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
    expect(screen.getByRole("button", { name: "Apply the plan" })).toBeInTheDocument();
    expect(screen.getByText("No rule matches (1)")).toBeInTheDocument();
  });
});

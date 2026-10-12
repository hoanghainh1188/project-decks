import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { I18nProvider, setCurrentLang } from "../../i18n";
import { at, makeAssignment, makeProject, makeResource, makeTask } from "../../test/fixtures";
import type { Project, Task } from "../../types";
import { Inspector } from "./Inspector";

const OPEN_KEY = "sheafly.inspector.open";

function renderInspector(task: Task, project: Project = makeProject({ tasks: [task] })) {
  const send = vi.fn();
  const utils = render(
    <Inspector
      project={project}
      task={task}
      send={send}
      onEditSeries={vi.fn()}
      onSplit={vi.fn()}
      onEditCalendar={vi.fn()}
      onCreateCalendar={vi.fn()}
    />,
  );
  return { ...utils, send, user: userEvent.setup() };
}

const leaf = makeTask({ uid: 2, name: "Thiết kế giao diện", wbs: "2.1", duration: 960 });
const summary = makeTask({ uid: 1, name: "Thiết kế", wbs: "2", summary: true });

describe("Inspector", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("says which task it edits", () => {
    renderInspector(leaf);
    const panel = screen.getByRole("region", { name: "Thuộc tính công việc" });
    expect(within(panel).getByRole("heading", { name: "Thiết kế giao diện" })).toBeInTheDocument();
    expect(within(panel).getByText(/ID 2 · WBS 2\.1/)).toBeInTheDocument();
  });

  it("groups the fields", () => {
    renderInspector(leaf);
    for (const name of ["Lịch", "Theo dõi", "Chi phí", "Nguồn lực", "Ghi chú"]) {
      expect(screen.getByRole("group", { name })).toBeInTheDocument();
    }
    expect(within(screen.getByRole("group", { name: "Theo dõi" })).getByLabelText("Bắt đầu thực")).toBeInTheDocument();
  });

  it("hides fields that do not apply to a summary task", () => {
    renderInspector(summary);
    expect(screen.queryByLabelText("Bắt đầu thực")).not.toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "Theo dõi" })).not.toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "Nguồn lực" })).not.toBeInTheDocument();
    expect(screen.queryByText("Kiểu công việc")).not.toBeInTheDocument();
    expect(screen.getByText(/Công việc tóm tắt/)).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Lịch" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Chi phí" })).toBeInTheDocument();
  });

  it("collapses to its header and remembers it", async () => {
    const { user, unmount } = renderInspector(leaf);
    const toggle = screen.getByRole("button", { name: "Chi tiết thuộc tính" });
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    await user.click(toggle);
    expect(screen.queryByRole("group", { name: "Lịch" })).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Thiết kế giao diện" })).toBeInTheDocument();
    expect(window.localStorage.getItem(OPEN_KEY)).toBe("false");
    unmount();

    renderInspector(leaf);
    expect(screen.getByRole("button", { name: "Chi tiết thuộc tính" })).toHaveAttribute("aria-expanded", "false");
  });

  it("still edits assignments in the resources group", () => {
    const project = makeProject({
      tasks: [leaf],
      resources: [makeResource({ uid: 10, name: "An" })],
      assignments: [makeAssignment({ uid: 1, task_uid: 2, resource_uid: 10 })],
    });
    renderInspector(leaf, project);
    expect(within(screen.getByRole("group", { name: "Nguồn lực" })).getByLabelText("Đơn vị An")).toBeInTheDocument();
  });

  it("does not send anything when a duration field is left untouched", () => {
    for (const work of [0, 25, 45, 90, 540, 768, 12060]) {
      const { send, unmount } = renderInspector({ ...leaf, work });
      fireEvent.blur(screen.getByLabelText("Công việc"));
      expect(send).not.toHaveBeenCalled();
      unmount();
    }
  });

  it("sends the same constraint command as before the move", () => {
    const { send } = renderInspector(leaf);
    fireEvent.change(screen.getByLabelText("Ràng buộc"), { target: { value: "snet" } });
    expect(send).toHaveBeenCalledWith({
      type: "set_constraint",
      uid: 2,
      constraint_type: "snet",
      date: leaf.constraint_date,
    });
  });
});

describe("Inspector in English", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });
  afterEach(() => setCurrentLang("vi"));

  function renderEnglish(task: Task, project: Project = makeProject({ tasks: [task] })) {
    const send = vi.fn();
    render(
      <I18nProvider lang="en">
        <Inspector
          project={project}
          task={task}
          send={send}
          onEditSeries={vi.fn()}
          onSplit={vi.fn()}
          onEditCalendar={vi.fn()}
          onCreateCalendar={vi.fn()}
        />
      </I18nProvider>,
    );
    return { send };
  }

  it("groups and labels the fields in English", () => {
    renderEnglish(leaf);
    expect(screen.getByRole("region", { name: "Task information" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Task details" })).toHaveAttribute("title", "Collapse");
    for (const name of ["Schedule", "Tracking", "Cost", "Resources", "Notes"]) {
      expect(screen.getByRole("group", { name })).toBeInTheDocument();
    }
    for (const label of ["Actual start", "Actual finish", "Work", "Remaining work", "Overtime work", "Deadline", "Priority"]) {
      expect(screen.getByLabelText(label)).toBeInTheDocument();
    }
    expect(screen.getByText("Manually scheduled")).toBeInTheDocument();
    expect(screen.getByText("Effort driven")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New task calendar" })).toBeInTheDocument();
  });

  it("uses Project's names for constraints, task types and accrual", () => {
    renderEnglish(leaf);
    const constraint = within(screen.getByLabelText("Constraint"));
    for (const name of [
      "As Soon As Possible",
      "As Late As Possible",
      "Start No Earlier Than",
      "Start No Later Than",
      "Finish No Earlier Than",
      "Finish No Later Than",
      "Must Start On",
      "Must Finish On",
    ]) {
      expect(constraint.getByRole("option", { name })).toBeInTheDocument();
    }
    const taskType = within(screen.getByLabelText("Task type"));
    for (const name of ["Fixed Units", "Fixed Duration", "Fixed Work"]) {
      expect(taskType.getByRole("option", { name })).toBeInTheDocument();
    }
    const accrual = within(screen.getByLabelText("Accrue at"));
    for (const name of ["Start", "Prorated", "End"]) {
      expect(accrual.getByRole("option", { name })).toBeInTheDocument();
    }
    expect(within(screen.getByLabelText("Task calendar")).getByRole("option", { name: "Project calendar" })).toBeInTheDocument();
  });

  it("writes the header facts in English", () => {
    renderEnglish({ ...summary, late: true, name: "" });
    expect(screen.getByRole("heading", { name: "(Untitled)" })).toBeInTheDocument();
    expect(screen.getByText(/ID 1 · WBS 2 · Summary task/)).toBeInTheDocument();
    expect(screen.getByText(/^Cost .* · Actual .* · Late$/)).toBeInTheDocument();
  });

  it("edits assignments and splits in English", () => {
    const split = {
      ...leaf,
      splits: [{ offset: 480, gap: 480 }],
      segments: [
        { start: at(2026, 1, 5, 8), finish: at(2026, 1, 5, 17) },
        { start: at(2026, 1, 7, 8), finish: at(2026, 1, 7, 17) },
      ],
    };
    const project = makeProject({
      tasks: [split],
      resources: [makeResource({ uid: 10, name: "An" })],
      assignments: [makeAssignment({ uid: 1, task_uid: 2, resource_uid: 10 })],
    });
    renderEnglish(split, project);
    const resources = within(screen.getByRole("group", { name: "Resources" }));
    expect(resources.getByText("Assign resources")).toBeInTheDocument();
    expect(resources.getByLabelText("Units for An")).toBeInTheDocument();
    expect(resources.getByLabelText("Units to assign")).toBeInTheDocument();
    expect(resources.getByRole("button", { name: "Remove" })).toBeInTheDocument();
    expect(resources.getByRole("option", { name: "Choose…" })).toBeInTheDocument();
    expect(screen.getByText("Split task")).toBeInTheDocument();
    expect(screen.getByText("Part 1: 2026-01-05 08:00 – 2026-01-05 17:00")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Join with part 2" })).toHaveAttribute(
      "title",
      "Gap of 1 day of working time",
    );
    expect(screen.getByRole("button", { name: "Join all parts" })).toBeInTheDocument();
  });
});

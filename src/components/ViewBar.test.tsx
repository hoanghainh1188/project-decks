import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { makeProject, makeTask } from "../test/fixtures";
import type { Project, ViewId } from "../types";
import { ViewBar } from "./ViewBar";

function renderBar(view: ViewId, project: Project = makeProject(), overrides: Partial<Parameters<typeof ViewBar>[0]> = {}) {
  const props = {
    project,
    view,
    onView: vi.fn(),
    scale: "day" as const,
    onScale: vi.fn(),
    filter: "none" as const,
    onFilter: vi.fn(),
    filterResource: null,
    onFilterResource: vi.fn(),
    filterValue: "",
    onFilterValue: vi.fn(),
    group: "none" as const,
    onGroup: vi.fn(),
    highlight: "none" as const,
    onHighlight: vi.fn(),
    networkSummaries: false,
    onNetworkSummaries: vi.fn(),
    interruptsColumn: false,
    onInterruptsColumn: vi.fn(),
    ...overrides,
  };
  render(<ViewBar {...props} />);
  return { user: userEvent.setup(), props };
}

describe("ViewBar", () => {
  it("shows the views as tabs with one tab stop on the current one", async () => {
    const { user, props } = renderBar("gantt");
    expect(screen.getByRole("tablist", { name: "View" })).toBeInTheDocument();
    const gantt = screen.getByRole("tab", { name: "Gantt" });
    expect(gantt).toHaveAttribute("aria-selected", "true");
    expect(gantt).toHaveAttribute("aria-controls", "view-panel");
    expect(gantt).toHaveAttribute("tabindex", "0");
    expect(screen.getByRole("tab", { name: "Theo dõi" })).toHaveAttribute("tabindex", "-1");
    await user.click(screen.getByRole("tab", { name: "Sơ đồ mạng" }));
    expect(props.onView).toHaveBeenCalledWith("network");
  });

  it("moves between tabs with the arrow keys, Home and End", async () => {
    const { user, props } = renderBar("gantt");
    screen.getByRole("tab", { name: "Gantt" }).focus();
    await user.keyboard("{ArrowRight}");
    expect(props.onView).toHaveBeenLastCalledWith("tracking");
    await user.keyboard("{ArrowLeft}");
    expect(props.onView).toHaveBeenLastCalledWith("network");
    await user.keyboard("{End}");
    expect(props.onView).toHaveBeenLastCalledWith("network");
    await user.keyboard("{Home}");
    expect(props.onView).toHaveBeenLastCalledWith("gantt");
  });

  it("shows the time scale and row controls on the chart views only", async () => {
    const { user, props } = renderBar("tracking");
    await user.click(screen.getByRole("button", { name: "Tuần" }));
    expect(props.onScale).toHaveBeenCalledWith("week");
    expect(screen.getByLabelText("Nhóm")).toBeInTheDocument();
  });

  it("swaps grouping for summaries on the network and hides controls elsewhere", () => {
    renderBar("network");
    expect(screen.queryByRole("group", { name: "Thang thời gian" })).toBeNull();
    expect(screen.getByLabelText("Hiện công việc tóm tắt")).toBeInTheDocument();
  });

  it("filters, highlights and shows a column for urgent tasks", async () => {
    const { user, props } = renderBar("gantt");
    await user.selectOptions(screen.getByLabelText("Lọc"), "interrupts");
    expect(props.onFilter).toHaveBeenCalledWith("interrupts");
    await user.selectOptions(screen.getByLabelText("Tô sáng"), "Việc chen ngang");
    expect(props.onHighlight).toHaveBeenCalledWith("interrupts");
    await user.click(screen.getByRole("checkbox", { name: "Cột chen ngang" }));
    expect(props.onInterruptsColumn).toHaveBeenCalledWith(true);
  });

  it("has no row controls on the resource sheet", () => {
    renderBar("resources");
    expect(screen.queryByLabelText("Lọc")).toBeNull();
  });
});

describe("ViewBar custom fields", () => {
  const project = makeProject({
    fields: [{ id: 2, name: "Mảng", kind: { type: "choice", options: ["FE", "BE"] } }],
    tasks: [makeTask({ uid: 1, values: { 2: "BE" } }), makeTask({ uid: 2, values: { 2: "FE" } })],
  });

  it("filters by a field, starting on the first value tasks have", async () => {
    const { user, props } = renderBar("gantt", project);
    await user.selectOptions(screen.getByRole("combobox", { name: "Lọc" }), "Theo “Mảng”");
    expect(props.onFilter).toHaveBeenCalledWith("field:2");
    expect(props.onFilterValue).toHaveBeenCalledWith("FE");
  });

  it("picks the value to keep, or no value", async () => {
    const { user, props } = renderBar("gantt", project, { filter: "field:2", filterValue: "FE" });
    const values = screen.getByRole("combobox", { name: "Giá trị" });
    expect([...values.querySelectorAll("option")].map((option) => option.textContent)).toEqual(["FE", "BE", "(trống)"]);
    await user.selectOptions(values, "(trống)");
    expect(props.onFilterValue).toHaveBeenCalledWith("");
  });

  it("groups by a field", async () => {
    const { user, props } = renderBar("gantt", project);
    await user.selectOptions(screen.getByRole("combobox", { name: "Nhóm" }), "Theo “Mảng”");
    expect(props.onGroup).toHaveBeenCalledWith("field:2");
  });
});

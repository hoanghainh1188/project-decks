import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { I18nProvider } from "../../i18n";
import { makeAssignment, makeProject, makeTask } from "../../test/fixtures";
import type { Task } from "../../types";
import { EstimateGroup } from "./EstimateGroup";

const DAY = 480;

function renderGroup(task: Task, lang: "vi" | "en" = "vi") {
  const send = vi.fn();
  const project = makeProject({ tasks: [task], assignments: [makeAssignment({ uid: 1, task_uid: task.uid, resource_uid: 1 })] });
  render(
    <I18nProvider lang={lang}>
      <EstimateGroup project={project} task={task} send={send} />
    </I18nProvider>,
  );
  return { send, group: screen.getByRole("group", { name: lang === "vi" ? "Ước lượng" : "Estimate" }) };
}

describe("EstimateGroup", () => {
  it("sets the range once both amounts read", () => {
    const { send, group } = renderGroup(makeTask({ uid: 2, work: DAY }));
    const low = within(group).getByLabelText("Effort thấp");
    const high = within(group).getByLabelText("Effort cao");
    fireEvent.change(low, { target: { value: "4h" } });
    fireEvent.blur(low);
    expect(send).not.toHaveBeenCalled();
    expect(within(group).getByRole("status")).toHaveTextContent("Cần cả mức thấp và mức cao");
    fireEvent.change(high, { target: { value: "2d" } });
    fireEvent.blur(high);
    expect(send).toHaveBeenCalledWith({ type: "set_work_range", uid: 2, low: 240, high: 960 });
  });

  it("waits while focus moves from one end to the other", () => {
    const { send, group } = renderGroup(makeTask({ uid: 2, work: DAY }));
    const low = within(group).getByLabelText("Effort thấp");
    const high = within(group).getByLabelText("Effort cao");
    fireEvent.change(low, { target: { value: "4h" } });
    fireEvent.blur(low, { relatedTarget: high });
    expect(within(group).getByRole("status")).toHaveTextContent("");
    expect(low).toHaveAttribute("aria-invalid", "false");
    expect(send).not.toHaveBeenCalled();
  });

  it("clears the range when both are emptied and refuses a backwards one", () => {
    const { send, group } = renderGroup(makeTask({ uid: 2, work: DAY, work_low: 240, work_high: 960 }));
    const low = within(group).getByLabelText("Effort thấp");
    const high = within(group).getByLabelText("Effort cao");
    fireEvent.change(low, { target: { value: "3d" } });
    fireEvent.blur(low);
    expect(send).not.toHaveBeenCalled();
    expect(within(group).getByRole("status")).toHaveTextContent("Mức thấp lớn hơn mức cao");
    fireEvent.change(low, { target: { value: "" } });
    fireEvent.change(high, { target: { value: "" } });
    fireEvent.blur(high);
    expect(send).toHaveBeenCalledWith({ type: "set_work_range", uid: 2, low: null, high: null });
  });

  it("warns when today's effort lies outside the range", () => {
    const { group } = renderGroup(makeTask({ uid: 2, work: 3 * DAY, work_low: 240, work_high: 960 }));
    expect(within(group).getByText(/Effort hiện tại nằm ngoài khoảng/)).toBeInTheDocument();
  });

  it("is not there for summaries and milestones", () => {
    const { container } = render(
      <EstimateGroup project={makeProject()} task={makeTask({ uid: 1, summary: true })} send={vi.fn()} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("speaks English", () => {
    const { group } = renderGroup(makeTask({ uid: 2, work: DAY }), "en");
    expect(within(group).getByLabelText("Low effort")).toBeInTheDocument();
    expect(within(group).getByLabelText("High effort")).toBeInTheDocument();
  });
});

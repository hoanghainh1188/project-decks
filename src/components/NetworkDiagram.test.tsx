import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { makeProject, makeTask } from "../test/fixtures";
import type { NetworkLayout, Project } from "../types";
import { NetworkDiagram } from "./NetworkDiagram";

function box(uid: number, column: number, y: number) {
  return { uid, column, row: 0, x: 24 + column * 300, y, width: 220, height: 104 };
}

function fixture(): { project: Project; layout: NetworkLayout } {
  const project = makeProject({
    tasks: [
      makeTask({ uid: 1, name: "Đào móng", wbs: "1", critical: true, percent_complete: 50 }),
      makeTask({ uid: 2, name: "Lắp điện", wbs: "2" }),
      makeTask({ uid: 3, name: "Bàn giao", wbs: "3", milestone: true, duration: 0, critical: true }),
    ],
  });
  const layout: NetworkLayout = {
    nodes: [box(1, 0, 24), box(2, 1, 156), box(3, 1, 24)],
    edges: [
      { uid: 1, from_uid: 1, to_uid: 3, kind: "fs", lag: 0, points: [{ x: 244, y: 76 }, { x: 324, y: 76 }] },
      { uid: 2, from_uid: 1, to_uid: 2, kind: "ss", lag: 240, points: [{ x: 244, y: 76 }, { x: 324, y: 208 }] },
    ],
    columns: 2,
    width: 568,
    height: 284,
    crossings: 0,
  };
  return { project, layout };
}

function renderDiagram(selected: number | null = null, highlighted = new Set<number>()) {
  const { project, layout } = fixture();
  const onSelect = vi.fn();
  const onEdit = vi.fn();
  const view = render(
    <NetworkDiagram
      project={project}
      layout={layout}
      selected={selected}
      highlighted={highlighted}
      onSelect={onSelect}
      onEdit={onEdit}
    />,
  );
  const node = (name: string) => {
    const element = screen.getByText(name).closest(".network-node");
    if (!element) throw new Error(`No box for ${name}`);
    return element;
  };
  return { ...view, onSelect, onEdit, node };
}

describe("NetworkDiagram", () => {
  it("draws one box per node with ID, WBS, duration, dates and percent", () => {
    const { container, node } = renderDiagram();
    expect(container.querySelectorAll(".network-node")).toHaveLength(3);
    const text = node("Đào móng").textContent ?? "";
    expect(text).toContain("ID 1 · WBS 1");
    expect(text).toContain("1 ngày");
    expect(text).toContain("05/01/2026 08:00");
    expect(text).toContain("50%");
  });

  it("gives critical tasks a red border and milestones a hexagon", () => {
    const { node } = renderDiagram();
    expect(node("Đào móng").querySelector("rect")).toHaveAttribute("stroke", "#c23b3b");
    expect(node("Lắp điện").querySelector("rect")).toHaveAttribute("stroke", "#2f6fad");
    expect(node("Bàn giao").querySelector("polygon")).toBeInTheDocument();
    expect(node("Bàn giao").textContent).toContain("Mốc");
  });

  it("labels links that are not FS without lag", () => {
    const { container } = renderDiagram();
    const labels = [...container.querySelectorAll(".network-edge-label")].map((item) => item.textContent);
    expect(labels).toEqual(["SS+4 giờ"]);
    expect(container.querySelectorAll(".network-edge path")).toHaveLength(2);
  });

  it("selects on click and asks to edit on double-click", () => {
    const { node, onSelect, onEdit } = renderDiagram();
    fireEvent.click(node("Lắp điện"));
    expect(onSelect).toHaveBeenCalledWith(2);
    fireEvent.doubleClick(node("Lắp điện"));
    expect(onEdit).toHaveBeenCalledWith(2);
  });

  it("outlines the shared selection and tints highlighted tasks", () => {
    const { node } = renderDiagram(2, new Set([1]));
    expect(node("Lắp điện").querySelector('[stroke="#0a84ff"]')).toBeInTheDocument();
    expect(node("Đào móng").querySelector('[fill="#fff3c4"]')).toBeInTheDocument();
  });

  it("zooms with the toolbar buttons", () => {
    const { container } = renderDiagram();
    const svg = () => container.querySelector("svg.network-svg");
    expect(svg()).toHaveAttribute("width", "568");
    fireEvent.click(screen.getByRole("button", { name: "+" }));
    expect(svg()).toHaveAttribute("width", "710");
    expect(screen.getByRole("button", { name: "125%" })).toBeInTheDocument();
  });

  it("says so when the filter leaves nothing to draw", () => {
    const { project } = fixture();
    render(
      <NetworkDiagram
        project={project}
        layout={{ nodes: [], edges: [], columns: 0, width: 0, height: 0, crossings: 0 }}
        selected={null}
        highlighted={new Set()}
        onSelect={() => {}}
        onEdit={() => {}}
      />,
    );
    expect(screen.getByText("Không có công việc nào để vẽ sơ đồ mạng.")).toBeInTheDocument();
  });
});

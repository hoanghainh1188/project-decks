import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { I18nProvider, setCurrentLang } from "../i18n";
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

function renderDiagram(selected: number | null = null, highlighted = new Set<number>(), selection?: ReadonlySet<number>) {
  const { project, layout } = fixture();
  const onSelect = vi.fn();
  const onEdit = vi.fn();
  const view = render(
    <NetworkDiagram
      project={project}
      layout={layout}
      selected={selected}
      selection={selection}
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
    // Colours are theme tokens, so they follow light and dark.
    expect(node("Đào móng").querySelector("rect")?.getAttribute("style")).toContain("stroke: var(--bar-critical)");
    expect(node("Lắp điện").querySelector("rect")?.getAttribute("style")).toContain("stroke: var(--bar-task)");
    expect(node("Bàn giao").querySelector("polygon")).toBeInTheDocument();
    expect(node("Bàn giao").textContent).toContain("Mốc");
  });

  it("labels links that are not FS without lag", () => {
    const { container } = renderDiagram();
    const labels = [...container.querySelectorAll(".network-edge-label")].map((item) => item.textContent);
    expect(labels).toEqual(["SS+0,5 ngày"]);
    expect(container.querySelectorAll(".network-edge path")).toHaveLength(2);
  });

  it("selects on click and asks to edit on double-click", () => {
    const { node, onSelect, onEdit } = renderDiagram();
    fireEvent.click(node("Lắp điện"));
    expect(onSelect).toHaveBeenCalledWith(2, false);
    fireEvent.click(node("Bàn giao"), { shiftKey: true });
    expect(onSelect).toHaveBeenLastCalledWith(3, true);
    fireEvent.doubleClick(node("Lắp điện"));
    expect(onEdit).toHaveBeenCalledWith(2);
  });

  it("outlines every box of a Shift range", () => {
    const { node } = renderDiagram(1, new Set(), new Set([1, 2]));
    const outlined = (name: string) => node(name).querySelector('[style*="stroke: var(--accent)"]');
    expect(outlined("Đào móng")).toBeInTheDocument();
    expect(outlined("Lắp điện")).toBeInTheDocument();
    expect(outlined("Bàn giao")).not.toBeInTheDocument();
  });

  it("outlines the shared selection and tints highlighted tasks", () => {
    const { node } = renderDiagram(2, new Set([1]));
    expect(node("Lắp điện").querySelector('[style*="stroke: var(--accent)"]')).toBeInTheDocument();
    expect(node("Đào móng").querySelector('[style*="fill: var(--network-highlight-box)"]')).toBeInTheDocument();
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

describe("NetworkDiagram in English", () => {
  afterEach(() => setCurrentLang("vi"));

  function renderEnglish(layout?: NetworkLayout) {
    const data = fixture();
    return render(
      <I18nProvider lang="en">
        <NetworkDiagram
          project={data.project}
          layout={layout ?? data.layout}
          selected={null}
          highlighted={new Set()}
          onSelect={() => {}}
          onEdit={() => {}}
        />
      </I18nProvider>,
    );
  }

  it("labels the box fields, kinds and links in English", () => {
    const { container } = renderEnglish();
    expect(screen.getByRole("img", { name: "Network diagram" })).toBeInTheDocument();
    const box = screen.getByText("Đào móng").closest(".network-node");
    const text = box?.textContent ?? "";
    expect(text).toContain("Start");
    expect(text).toContain("Finish");
    expect(text).toContain("Complete");
    expect(text).toContain("1 day");
    expect(text).toContain("2026-01-05 08:00");
    expect(box?.querySelector("title")).toHaveTextContent("1 Đào móng · Critical");
    expect(screen.getByText("Bàn giao").closest(".network-node")?.textContent).toContain("Milestone");
    const labels = [...container.querySelectorAll(".network-edge-label")].map((item) => item.textContent);
    expect(labels).toEqual(["SS+0.5 days"]);
  });

  it("translates the zoom tools and hint", () => {
    renderEnglish();
    expect(screen.getByTitle("Zoom out")).toBeInTheDocument();
    expect(screen.getByTitle("Zoom in")).toBeInTheDocument();
    expect(screen.getByTitle("Reset to 100%")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Fit" })).toBeInTheDocument();
    expect(screen.getByText(/Double-click to edit/)).toBeInTheDocument();
  });

  it("says so in English when there is nothing to draw", () => {
    renderEnglish({ nodes: [], edges: [], columns: 0, width: 0, height: 0, crossings: 0 });
    expect(screen.getByText("There are no tasks to draw a network diagram.")).toBeInTheDocument();
  });
});

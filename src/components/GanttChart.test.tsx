import { act, fireEvent, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { applyTheme } from "../theme";
import { at, makeAssignment, makeProject, makeResource, makeTask } from "../test/fixtures";
import type { Project } from "../types";
import { buildViewRows, type GroupMode } from "../viewRows";
import { GanttChart } from "./GanttChart";

/** A 2D context that records the colours and text drawn on it. */
function recordingContext() {
  const fills: string[] = [];
  const texts: string[] = [];
  /** Stroke colour of each line drawn, with the dash pattern in force. */
  const lines: { color: string; dash: number[] }[] = [];
  let fillStyle = "";
  let dash: number[] = [];
  const target = {
    get fillStyle() {
      return fillStyle;
    },
    set fillStyle(value: string) {
      fillStyle = value;
      fills.push(value);
    },
    strokeStyle: "",
    setLineDash: (pattern: number[]) => {
      dash = pattern;
    },
    stroke: () => lines.push({ color: target.strokeStyle, dash }),
    strokeRect: () => lines.push({ color: target.strokeStyle, dash }),
    fillText: (text: string) => texts.push(text),
    measureText: (text: string) => ({ width: text.length * 6 }),
  };
  const ctx = new Proxy(target, {
    get: (object, key) => (key in object ? object[key as keyof typeof object] : () => {}),
    set: (object, key, value) => {
      (object as Record<string | symbol, unknown>)[key] = value;
      return true;
    },
  });
  return { ctx, fills, texts, lines };
}

function project(): Project {
  return makeProject({
    tasks: [
      makeTask({ uid: 1, name: "Đào móng", critical: true }),
      makeTask({ uid: 2, name: "Đổ bê tông", start: at(2026, 1, 6, 8), finish: at(2026, 1, 6, 17) }),
      makeTask({ uid: 3, name: "Nghiệm thu", milestone: true, duration: 0, start: at(2026, 1, 7, 8), finish: at(2026, 1, 7, 8) }),
    ],
    resources: [makeResource({ uid: 10, name: "An" })],
    assignments: [makeAssignment({ uid: 100, task_uid: 2, resource_uid: 10 })],
  });
}

function renderChart(selected: number | null = 1, group: GroupMode = "none", data: Project = project()) {
  const handlers = { onSelect: vi.fn(), onMove: vi.fn(), onResize: vi.fn(), onMoveSegment: vi.fn(), onLink: vi.fn(), onEdit: vi.fn(), onScroll: vi.fn() };
  const view = render(
    <GanttChart
      project={data}
      rows={buildViewRows(data, "none", null, group, "none")}
      scale="day"
      selected={selected}
      tracking={false}
      bodyRef={createRef<HTMLDivElement>()}
      {...handlers}
    />,
  );
  return { ...view, handlers, chart: screen.getByRole("group", { name: /^Biểu đồ Gantt/ }) };
}

describe("GanttChart", () => {
  let recorded: ReturnType<typeof recordingContext>;
  beforeEach(() => {
    recorded = recordingContext();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(recorded.ctx as never);
  });
  afterEach(() => {
    applyTheme("system");
    document.documentElement.style.removeProperty("--weekend");
  });

  it("draws one header cell per day and marks weekends", () => {
    const { container } = renderChart();
    const days = container.querySelectorAll(".gantt-day");
    expect(days.length).toBeGreaterThan(7);
    // Two weekend days in every week, side by side.
    const weekend = [...days].map((day) => day.classList.contains("weekend"));
    const first = weekend.indexOf(true);
    expect(weekend.slice(first, first + 7)).toEqual([true, true, false, false, false, false, false]);
  });

  it("zooms the timeline with Cmd or Ctrl and the wheel or a pinch, and scrolls otherwise", () => {
    const onZoom = vi.fn();
    const data = project();
    const { container } = render(
      <GanttChart
        project={data}
        rows={buildViewRows(data, "none", null, "none", "none")}
        scale="day"
        zoom={1}
        onZoom={onZoom}
        selected={null}
        tracking={false}
        bodyRef={createRef<HTMLDivElement>()}
        onSelect={vi.fn()}
        onMove={vi.fn()}
        onResize={vi.fn()}
        onMoveSegment={vi.fn()}
        onLink={vi.fn()}
        onEdit={vi.fn()}
        onScroll={vi.fn()}
      />,
    );
    const body = container.querySelector(".gantt-body")!;
    fireEvent.wheel(body, { deltaY: 40 });
    expect(onZoom).not.toHaveBeenCalled();
    fireEvent.wheel(body, { deltaY: -40, ctrlKey: true, clientX: 0 });
    expect(onZoom).toHaveBeenCalledTimes(1);
    expect(onZoom.mock.calls[0][0]).toBeGreaterThan(1);
    fireEvent.wheel(body, { deltaY: 40, metaKey: true, clientX: 0 });
    expect(onZoom.mock.calls[1][0]).toBeLessThan(1);
  });

  it("moves the selection with the arrow keys, Home and End, and opens the task on Enter", () => {
    const { chart, handlers } = renderChart(1);
    fireEvent.keyDown(chart, { key: "ArrowDown" });
    expect(handlers.onSelect).toHaveBeenLastCalledWith(2);
    fireEvent.keyDown(chart, { key: "End" });
    expect(handlers.onSelect).toHaveBeenLastCalledWith(3);
    fireEvent.keyDown(chart, { key: "ArrowUp" });
    // Already the first task: stays on it.
    expect(handlers.onSelect).toHaveBeenLastCalledWith(1);
    fireEvent.keyDown(chart, { key: "Enter" });
    expect(handlers.onEdit).toHaveBeenCalledExactlyOnceWith(1);
  });

  it("moves and resizes by a working day with Alt, and ignores a held key", () => {
    const { chart, handlers } = renderChart(1);
    fireEvent.keyDown(chart, { key: "ArrowRight", altKey: true });
    expect(handlers.onMove).toHaveBeenCalledExactlyOnceWith(1, at(2026, 1, 6, 8));
    fireEvent.keyDown(chart, { key: "ArrowRight", altKey: true, shiftKey: true });
    expect(handlers.onResize).toHaveBeenCalledExactlyOnceWith(1, at(2026, 1, 6, 17));
    fireEvent.keyDown(chart, { key: "ArrowRight", altKey: true, repeat: true });
    expect(handlers.onMove).toHaveBeenCalledOnce();
  });

  it("draws group rows with their label", () => {
    renderChart(1, "resource");
    expect(recorded.texts).toContain("An");
  });

  it("paints with the colours of the theme on screen and repaints when it changes", () => {
    renderChart();
    expect(recorded.fills).toContain("#f3f3f5");
    // The dark tokens, as styles.css would give them.
    document.documentElement.style.setProperty("--weekend", "#27272c");
    recorded.fills.length = 0;
    act(() => applyTheme("dark"));
    expect(recorded.fills).toContain("#27272c");
    expect(recorded.fills).not.toContain("#f3f3f5");
  });
});

describe("GanttChart leveling splits", () => {
  let recorded: ReturnType<typeof recordingContext>;
  beforeEach(() => {
    recorded = recordingContext();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(recorded.ctx as never);
  });

  function splitProject(leveling: boolean): Project {
    return makeProject({
      tasks: [
        makeTask({
          uid: 1,
          name: "Viết API",
          start: at(2026, 1, 5, 8),
          finish: at(2026, 1, 8, 17),
          duration: 960,
          splits: [{ offset: 480, gap: 960, ...(leveling ? { leveling: true } : {}) }],
          segments: [
            { start: at(2026, 1, 5, 8), finish: at(2026, 1, 5, 17) },
            { start: at(2026, 1, 8, 8), finish: at(2026, 1, 8, 17) },
          ],
        }),
      ],
    });
  }

  /** The canvas title after hovering each x along the first row. */
  function tips(canvas: HTMLCanvasElement): Set<string> {
    const seen = new Set<string>();
    for (let x = 0; x < 600; x += 2) {
      fireEvent.pointerMove(canvas, { clientX: x, clientY: 14 });
      seen.add(canvas.title);
    }
    return seen;
  }

  it("draws a gap leveling made dashed in its own colour, with a tooltip", () => {
    const { container } = renderChart(null, "none", splitProject(true));
    expect(recorded.lines).toContainEqual({ color: "#d97706", dash: [4, 3] });
    const canvas = container.querySelector("canvas")!;
    expect(tips(canvas)).toContain("Tách do cân bằng (chen ngang)");
    fireEvent.pointerLeave(canvas);
    expect(canvas.title).toBe("");
  });

  it("draws the user's gap as before, without the tooltip", () => {
    const { container } = renderChart(null, "none", splitProject(false));
    expect(recorded.lines.some((line) => line.color === "#d97706")).toBe(false);
    expect(recorded.lines).toContainEqual({ color: "#2f6fad", dash: [2, 3] });
    const canvas = container.querySelector("canvas")!;
    expect([...tips(canvas)].filter(Boolean)).toEqual([]);
  });
});

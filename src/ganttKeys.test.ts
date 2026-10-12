import { describe, expect, it } from "vitest";

import { ganttKeyAction, nextWorkingDay, type KeyPress } from "./ganttKeys";
import { at, makeProject, makeTask } from "./test/fixtures";
import type { ViewRow } from "./viewRows";

const project = makeProject({
  tasks: [
    makeTask({ uid: 1, start: at(2026, 1, 9, 8), finish: at(2026, 1, 9, 17) }), // a Friday
    makeTask({ uid: 2, start: at(2026, 1, 12, 8), finish: at(2026, 1, 13, 17) }),
    makeTask({ uid: 3, summary: true }),
    makeTask({ uid: 4, milestone: true, duration: 0 }),
  ],
});
const rows: ViewRow[] = [
  { kind: "group", key: "g", label: "Nhóm" },
  ...project.tasks.map((task) => ({ kind: "task" as const, key: String(task.uid), task, highlighted: false })),
];

function press(key: string, mods: Partial<KeyPress> = {}): KeyPress {
  return { key, altKey: false, shiftKey: false, metaKey: false, ctrlKey: false, ...mods };
}

describe("ganttKeyAction", () => {
  it("moves the selection over task rows, skipping group rows", () => {
    expect(ganttKeyAction(press("ArrowDown"), rows, null, project)).toEqual({ kind: "select", uid: 1 });
    expect(ganttKeyAction(press("ArrowDown"), rows, 1, project)).toEqual({ kind: "select", uid: 2 });
    expect(ganttKeyAction(press("ArrowUp"), rows, 1, project)).toEqual({ kind: "select", uid: 1 });
    expect(ganttKeyAction(press("End"), rows, 1, project)).toEqual({ kind: "select", uid: 4 });
    expect(ganttKeyAction(press("Home"), rows, 4, project)).toEqual({ kind: "select", uid: 1 });
    expect(ganttKeyAction(press("Enter"), rows, 2, project)).toEqual({ kind: "edit", uid: 2 });
  });

  it("moves a bar by working days, past the weekend", () => {
    expect(ganttKeyAction(press("ArrowRight", { altKey: true }), rows, 1, project)).toEqual({
      kind: "move",
      uid: 1,
      start: at(2026, 1, 12, 8),
    });
    expect(ganttKeyAction(press("ArrowLeft", { altKey: true }), rows, 2, project)).toEqual({
      kind: "move",
      uid: 2,
      start: at(2026, 1, 9, 8),
    });
  });

  it("moves the finish with Shift, never before the start", () => {
    expect(ganttKeyAction(press("ArrowRight", { altKey: true, shiftKey: true }), rows, 2, project)).toEqual({
      kind: "resize",
      uid: 2,
      finish: at(2026, 1, 14, 17),
    });
    expect(ganttKeyAction(press("ArrowLeft", { altKey: true, shiftKey: true }), rows, 1, project)).toBeNull();
  });

  it("leaves summaries, milestones' finish, and shortcuts alone", () => {
    expect(ganttKeyAction(press("ArrowRight", { altKey: true }), rows, 3, project)).toBeNull();
    expect(ganttKeyAction(press("ArrowRight", { altKey: true, shiftKey: true }), rows, 4, project)).toBeNull();
    expect(ganttKeyAction(press("ArrowDown", { metaKey: true }), rows, 1, project)).toBeNull();
  });
});

describe("nextWorkingDay", () => {
  it("skips days off from the calendar exceptions", () => {
    const calendar = { ...project.calendars[0], exceptions: { "2026-01-12": [] } };
    expect(nextWorkingDay(calendar, at(2026, 1, 9, 8), 1)).toBe(at(2026, 1, 13, 8));
  });
});

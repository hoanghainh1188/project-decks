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

  it("follows a work week that covers the date", () => {
    const base = project.calendars[0];
    const half = [{ start_minute: 9 * 60, end_minute: 13 * 60 }];
    // Mondays and Tuesdays only, 12–25 January.
    const week = [half, half, [], [], [], [], []];
    const calendar = { ...base, work_weeks: [{ name: "Ngắn", from: "2026-01-12", to: "2026-01-25", week }] };
    expect(nextWorkingDay(calendar, at(2026, 1, 13, 8), 1)).toBe(at(2026, 1, 19, 8));
    expect(nextWorkingDay(calendar, at(2026, 1, 20, 8), 1)).toBe(at(2026, 1, 26, 8));
  });
});

describe("ganttKeyAction with Shift", () => {
  const shift = (key: string) => press(key, { shiftKey: true });

  it("moves the end of the range from the selected row, then from the end", () => {
    expect(ganttKeyAction(shift("ArrowDown"), rows, 1, project)).toEqual({ kind: "extend", uid: 2 });
    expect(ganttKeyAction(shift("ArrowDown"), rows, 1, project, 2)).toEqual({ kind: "extend", uid: 3 });
    expect(ganttKeyAction(shift("ArrowUp"), rows, 3, project, 2)).toEqual({ kind: "extend", uid: 1 });
  });

  it("stops at the first and last rows and reaches them with Home and End", () => {
    expect(ganttKeyAction(shift("ArrowUp"), rows, 1, project)).toEqual({ kind: "extend", uid: 1 });
    expect(ganttKeyAction(shift("ArrowDown"), rows, 2, project, 4)).toEqual({ kind: "extend", uid: 4 });
    expect(ganttKeyAction(shift("End"), rows, 2, project)).toEqual({ kind: "extend", uid: 4 });
    expect(ganttKeyAction(shift("Home"), rows, 3, project)).toEqual({ kind: "extend", uid: 1 });
  });

  it("needs a selected row and ignores other keys", () => {
    expect(ganttKeyAction(shift("ArrowDown"), rows, null, project)).toBeNull();
    expect(ganttKeyAction(shift("Enter"), rows, 1, project)).toBeNull();
  });
});

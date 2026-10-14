import { describe, expect, it } from "vitest";

import { makeProject, makeTask, MONDAY_8AM } from "./test/fixtures";
import { outsideSpan, spanEndInstant } from "./spanFit";
import { buildViewRows } from "./viewRows";

const DAY = 86_400_000;
const HOUR = 3_600_000;

// Monday 2026-01-05; Friday 17:00 is MONDAY_8AM + 4 days + 9 hours.
const friday5pm = MONDAY_8AM + 4 * DAY + 9 * HOUR;

function project() {
  return makeProject({
    tasks: [
      makeTask({ uid: 1, summary: true, finish: friday5pm + 3 * DAY }),
      makeTask({ uid: 2, parent_uid: 1, finish: friday5pm }),
      makeTask({ uid: 3, parent_uid: 1, finish: friday5pm + 3 * DAY }),
      makeTask({ uid: 4, finish: MONDAY_8AM + DAY, deadline: MONDAY_8AM }),
      makeTask({ uid: 5, milestone: true, finish: friday5pm }),
    ],
  });
}

describe("spanEndInstant", () => {
  it("ends a date at the end of that day", () => {
    expect(spanEndInstant(project(), { kind: "date", date: "2026-01-09" })).toBe(MONDAY_8AM - 8 * HOUR + 5 * DAY);
  });

  it("ends at a milestone's finish, or nowhere when it is gone or the date is bad", () => {
    expect(spanEndInstant(project(), { kind: "milestone", uid: 5 })).toBe(friday5pm);
    expect(spanEndInstant(project(), { kind: "milestone", uid: 99 })).toBeNull();
    expect(spanEndInstant(project(), { kind: "date", date: "" })).toBeNull();
  });
});

describe("outsideSpan", () => {
  const p = project();
  const end = friday5pm;
  const byUid = (uid: number) => p.tasks.find((task) => task.uid === uid)!;

  it("takes leaves finishing after the end or past their deadline", () => {
    expect(outsideSpan(byUid(2), end)).toBe(false);
    expect(outsideSpan(byUid(3), end)).toBe(true);
    expect(outsideSpan(byUid(4), end)).toBe(true);
    expect(outsideSpan(byUid(5), end)).toBe(false);
  });

  it("never takes a summary", () => {
    expect(outsideSpan(byUid(1), end)).toBe(false);
  });
});

describe("buildViewRows with a span", () => {
  const shape = (rows: ReturnType<typeof buildViewRows>) =>
    rows.map((row) => (row.kind === "group" ? `# ${row.label}` : `${row.task.uid}${row.highlighted ? "*" : ""}`));

  it("filters to what does not fit, keeping their summaries", () => {
    expect(shape(buildViewRows(project(), "span", null, "none", "none", "", friday5pm))).toEqual(["1", "3", "4"]);
  });

  it("highlights what does not fit", () => {
    expect(shape(buildViewRows(project(), "none", null, "none", "span", "", friday5pm))).toEqual(["1", "2", "3*", "4*", "5"]);
  });

  it("highlights inside groups too", () => {
    expect(shape(buildViewRows(project(), "none", null, "summary", "span", "", friday5pm))).toContain("3*");
  });

  it("does nothing without an end", () => {
    expect(shape(buildViewRows(project(), "span", null, "none", "span", "", null))).toEqual(["1", "2", "3", "4", "5"]);
  });
});

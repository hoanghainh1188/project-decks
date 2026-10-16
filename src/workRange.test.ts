import { describe, expect, it } from "vitest";

import { makeTask } from "./test/fixtures";
import { rangeAfterEdit } from "./workRange";

const DAY = 480;

describe("rangeAfterEdit", () => {
  it("fills the other end from today's effort when there is no range yet", () => {
    const task = makeTask({ uid: 1, work: DAY });
    expect(rangeAfterEdit(task, DAY, "low", 240)).toEqual({ low: 240, high: DAY });
    expect(rangeAfterEdit(task, DAY, "high", 2 * DAY)).toEqual({ low: DAY, high: 2 * DAY });
    // An end beyond today's effort takes the other end with it.
    expect(rangeAfterEdit(task, DAY, "low", 2 * DAY)).toEqual({ low: 2 * DAY, high: 2 * DAY });
    expect(rangeAfterEdit(task, DAY, "high", 240)).toEqual({ low: 240, high: 240 });
  });

  it("keeps the other end of a range and refuses a backwards one", () => {
    const task = makeTask({ uid: 1, work: DAY, work_low: 240, work_high: 2 * DAY });
    expect(rangeAfterEdit(task, DAY, "low", 300)).toEqual({ low: 300, high: 2 * DAY });
    expect(rangeAfterEdit(task, DAY, "low", 3 * DAY)).toBeNull();
    expect(rangeAfterEdit(task, DAY, "high", 120)).toBeNull();
  });

  it("clears both ends when one is emptied", () => {
    const task = makeTask({ uid: 1, work: DAY, work_low: 240, work_high: 2 * DAY });
    expect(rangeAfterEdit(task, DAY, "high", null)).toEqual({ low: null, high: null });
  });
});

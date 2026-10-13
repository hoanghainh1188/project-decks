import { describe, expect, it } from "vitest";

import { selectionRange } from "./taskSelection";
import { makeProject, makeTask } from "./test/fixtures";
import { buildViewRows } from "./viewRows";

const rows = buildViewRows(
  makeProject({ tasks: [1, 2, 3, 4].map((uid) => makeTask({ uid, name: `T${uid}` })) }),
  "none",
  null,
  "none",
  "none",
);

describe("selectionRange", () => {
  it("is the rows between both ends in screen order, either way round", () => {
    expect(selectionRange(rows, 2, 4)).toEqual([2, 3, 4]);
    expect(selectionRange(rows, 4, 2)).toEqual([2, 3, 4]);
  });

  it("is the selected row alone without a range, or when an end is hidden", () => {
    expect(selectionRange(rows, 3, null)).toEqual([3]);
    expect(selectionRange(rows, 3, 99)).toEqual([3]);
    expect(selectionRange(rows, null, 2)).toEqual([]);
  });
});

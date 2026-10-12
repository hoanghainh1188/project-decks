import { describe, expect, it } from "vitest";

import { edgeLabel, nodeKind, occurrenceCounts } from "./networkLabels";
import { makeTask } from "./test/fixtures";

describe("edgeLabel", () => {
  it("hides the default finish-to-start link without lag", () => {
    expect(edgeLabel({ kind: "fs", lag: 0 })).toBe("");
  });

  it("names other link types even without lag", () => {
    expect(edgeLabel({ kind: "ss", lag: 0 })).toBe("SS");
    expect(edgeLabel({ kind: "sf", lag: 0 })).toBe("SF");
  });

  it("adds signed lag in working time", () => {
    expect(edgeLabel({ kind: "fs", lag: 480 })).toBe("FS+1 ngày");
    expect(edgeLabel({ kind: "ss", lag: 240 })).toBe("SS+0,5 ngày");
    expect(edgeLabel({ kind: "ff", lag: -480 })).toBe("FF-1 ngày");
  });
});

describe("nodeKind", () => {
  const series = makeTask({
    uid: 10,
    name: "Họp tuần",
    summary: true,
    recurrence: {
      spec: {
        name: "Họp tuần",
        duration: 60,
        start: 0,
        pattern: { kind: "weekly", every: 1, weekdays: [0] },
        end: { kind: "count", count: 3 },
        calendar_id: null,
        resources: [],
      },
      skipped: [],
    },
  });
  const tasks = [
    series,
    makeTask({ uid: 11, series_uid: 10 }),
    makeTask({ uid: 12, series_uid: 10 }),
    makeTask({ uid: 1, summary: true }),
    makeTask({ uid: 2, milestone: true }),
    makeTask({ uid: 3 }),
  ];

  it("names a recurring series box with its occurrence count", () => {
    const counts = occurrenceCounts(tasks);
    expect(nodeKind(series, counts)).toBe("Lặp · 2 lần");
    expect(nodeKind(tasks[3], counts)).toBe("Tóm tắt");
    expect(nodeKind(tasks[4], counts)).toBe("Mốc");
    expect(nodeKind(tasks[5], counts)).toBe("");
  });
});

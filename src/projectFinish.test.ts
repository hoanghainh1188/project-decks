import { describe, expect, it } from "vitest";

import { projectFinish } from "./projectFinish";
import { at, makeTask } from "./test/fixtures";

describe("projectFinish", () => {
  it("names the leaf task that finishes last", () => {
    const tasks = [
      makeTask({ uid: 1, name: "Pha 1", summary: true, finish: at(2026, 1, 9, 17) }),
      makeTask({ uid: 2, name: "Thiết kế", finish: at(2026, 1, 7, 17) }),
      makeTask({ uid: 3, name: "Phát hành", milestone: true, finish: at(2026, 1, 9, 17) }),
    ];
    expect(projectFinish(tasks)).toEqual({ finish: at(2026, 1, 9, 17), driver: "Phát hành" });
  });

  it("names the series when a recurring occurrence finishes last", () => {
    const tasks = [
      makeTask({ uid: 1, name: "Phát hành", milestone: true, finish: at(2026, 1, 9, 17) }),
      makeTask({ uid: 2, name: "Họp tuần", summary: true, finish: at(2026, 2, 2, 9) }),
      makeTask({ uid: 3, name: "Họp tuần 1", series_uid: 2, finish: at(2026, 1, 5, 9) }),
      makeTask({ uid: 4, name: "Họp tuần 5", series_uid: 2, finish: at(2026, 2, 2, 9) }),
    ];
    expect(projectFinish(tasks)).toEqual({ finish: at(2026, 2, 2, 9), driver: "Họp tuần" });
  });

  it("keeps the first task in outline order on a tie", () => {
    const tasks = [
      makeTask({ uid: 1, name: "A", finish: at(2026, 1, 9, 17) }),
      makeTask({ uid: 2, name: "B", finish: at(2026, 1, 9, 17) }),
    ];
    expect(projectFinish(tasks)?.driver).toBe("A");
  });

  it("is null without leaf tasks", () => {
    expect(projectFinish([])).toBeNull();
    expect(projectFinish([makeTask({ uid: 1, summary: true })])).toBeNull();
  });
});

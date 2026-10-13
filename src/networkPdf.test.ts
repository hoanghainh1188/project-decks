import { afterEach, describe, expect, it } from "vitest";

import { setCurrentLang } from "./i18n";
import { networkPage, nodeFields } from "./networkPdf";
import { at, makeTask } from "./test/fixtures";

describe("network PDF layout", () => {
  afterEach(() => setCurrentLang("vi"));

  it("uses at least an A4 landscape page", () => {
    const page = networkPage({ width: 100, height: 80 });
    expect(page.pageW).toBe(297);
    expect(page.pageH).toBe(210);
    expect(page.scale).toBeCloseTo(0.26);
  });

  it("grows the page with the diagram and shrinks a huge one below the viewer limit", () => {
    const wide = networkPage({ width: 2000, height: 400 });
    expect(wide.pageW).toBeCloseTo(2000 * 0.26 + 16);
    const huge = networkPage({ width: 40000, height: 1000 });
    expect(huge.pageW).toBeLessThanOrEqual(5000);
    expect(huge.scale).toBeLessThan(0.26);
  });

  it("lists start, finish and % complete as the boxes on screen do", () => {
    const task = makeTask({ uid: 1, start: at(2026, 1, 5, 8), finish: at(2026, 1, 6, 17), percent_complete: 49.6 });
    expect(nodeFields(task)).toEqual([
      { label: "Bắt đầu", value: "05/01/2026 08:00" },
      { label: "Kết thúc", value: "06/01/2026 17:00" },
      { label: "Hoàn thành", value: "50%" },
    ]);
    setCurrentLang("en");
    expect(nodeFields({ ...task, percent_complete: 140 }).map((row) => row.value)).toEqual([
      "2026-01-05 08:00",
      "2026-01-06 17:00",
      "100%",
    ]);
  });
});

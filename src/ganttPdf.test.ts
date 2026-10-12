import { afterEach, describe, expect, it } from "vitest";

import { ganttPageGrid, pdfSuggestedName } from "./ganttPdf";
import { pdfMessages } from "./ganttPdf.i18n";
import { pick, setCurrentLang } from "./i18n";

describe("PDF labels", () => {
  afterEach(() => setCurrentLang("vi"));

  it("prints Vietnamese headers, footer and network title by default", () => {
    const m = pick(pdfMessages);
    expect([m.name, m.start, m.finish]).toEqual(["Tên", "Bắt đầu", "Kết thúc"]);
    expect(m.page(2, 5)).toBe("Trang 2/5");
    expect(m.networkTitle("Ra mắt")).toBe("Ra mắt — Sơ đồ mạng");
    expect(pdfSuggestedName("  ")).toBe("Dự án.pdf");
  });

  it("prints English headers, footer and network title", () => {
    setCurrentLang("en");
    const m = pick(pdfMessages);
    expect([m.name, m.start, m.finish]).toEqual(["Name", "Start", "Finish"]);
    expect(m.page(2, 5)).toBe("Page 2/5");
    expect(m.networkTitle(m.untitled)).toBe("Project — Network diagram");
    expect(pdfSuggestedName("  ")).toBe("Project.pdf");
    expect(pdfSuggestedName("Launch")).toBe("Launch.pdf");
  });
});

describe("Gantt PDF pages", () => {
  it("fits a small project on one page", () => {
    const grid = ganttPageGrid(5, 20, 28);
    expect(grid.pageCount).toBe(1);
    expect(grid.rowsPerPage).toBeGreaterThanOrEqual(5);
    expect(grid.bodyTop).toBeGreaterThan(grid.headerTop);
  });

  it("pages down the rows and across the days", () => {
    const one = ganttPageGrid(1, 1, 28);
    const grid = ganttPageGrid(one.rowsPerPage * 2 + 1, one.daysPerPage + 1, 28);
    expect([grid.rowPages, grid.dayPages, grid.pageCount]).toEqual([3, 2, 6]);
    // Wider days at the day scale than at the month scale: fewer per page.
    expect(ganttPageGrid(1, 1, 4).daysPerPage).toBeGreaterThan(one.daysPerPage);
  });
});

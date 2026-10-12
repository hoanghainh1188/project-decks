import { afterEach, describe, expect, it } from "vitest";

import { pdfSuggestedName } from "./ganttPdf";
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

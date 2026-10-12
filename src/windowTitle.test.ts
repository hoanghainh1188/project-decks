import { describe, expect, it } from "vitest";

import { makeProject, makeSnapshot } from "./test/fixtures";
import { fileName, windowTitle } from "./windowTitle";

describe("windowTitle", () => {
  it("names the project and marks unsaved changes", () => {
    const snap = makeSnapshot(makeProject({ name: "Ra mắt" }));
    expect(windowTitle({ ...snap, dirty: false })).toBe("Ra mắt");
    expect(windowTitle({ ...snap, dirty: true })).toBe("Ra mắt — Đã sửa");
  });

  it("falls back to the app name", () => {
    expect(windowTitle(null)).toBe("ProjectDesk");
    expect(windowTitle({ ...makeSnapshot(makeProject({ name: "  " })), dirty: false })).toBe("ProjectDesk");
  });
});

describe("fileName", () => {
  it("keeps the last part of a macOS or Windows path", () => {
    expect(fileName("/Users/an/Dự án/ra-mat.pdproj")).toBe("ra-mat.pdproj");
    expect(fileName("C:\\Projects\\ra-mat.pdproj")).toBe("ra-mat.pdproj");
    expect(fileName("ra-mat.pdproj")).toBe("ra-mat.pdproj");
  });
});

import { describe, expect, it } from "vitest";

import { edgeLabel } from "./networkLabels";

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
    expect(edgeLabel({ kind: "ss", lag: 240 })).toBe("SS+4 giờ");
    expect(edgeLabel({ kind: "ff", lag: -480 })).toBe("FF-1 ngày");
  });
});

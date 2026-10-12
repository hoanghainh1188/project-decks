import { describe, expect, it } from "vitest";

import { blend, clipRect, clipSegment, fitText, rgb } from "./pdfDraw";

const clip = { left: 0, top: 0, right: 100, bottom: 50 };
// One unit per character, like a monospaced font.
const mono = { getTextWidth: (text: string) => Array.from(text).length };

describe("PDF drawing helpers", () => {
  it("reads hex colours and blends them like an alpha overlay", () => {
    expect(rgb("#c23b3b")).toEqual({ r: 194, g: 59, b: 59 });
    expect(blend(rgb("#000000"), rgb("#ffffff"), 0.28)).toEqual({ r: 184, g: 184, b: 184 });
    expect(blend(rgb("#2f6fad"), rgb("#ffffff"), 1)).toEqual(rgb("#2f6fad"));
  });

  it("cuts text to the width with an ellipsis, by whole characters", () => {
    expect(fitText(mono, "Đào móng", 20)).toBe("Đào móng");
    expect(fitText(mono, "Đào móng nhà", 8)).toBe("Đào m...");
    expect(fitText(mono, "Đào móng", 2)).toBe("");
    expect(fitText(mono, "Đào", 0)).toBe("");
  });

  it("clips rectangles to the drawing area", () => {
    expect(clipRect(-10, 10, 30, 10, clip)).toEqual({ x: 0, y: 10, w: 20, h: 10 });
    expect(clipRect(90, 45, 30, 30, clip)).toEqual({ x: 90, y: 45, w: 10, h: 5 });
    expect(clipRect(120, 10, 5, 5, clip)).toBeNull();
  });

  it("clips line segments to the drawing area", () => {
    expect(clipSegment(10, 10, 20, 20, clip)).toEqual([10, 10, 20, 20]);
    expect(clipSegment(-50, 25, 150, 25, clip)).toEqual([0, 25, 100, 25]);
    expect(clipSegment(50, -10, 50, 60, clip)).toEqual([50, 0, 50, 50]);
    expect(clipSegment(-10, -10, -5, 60, clip)).toBeNull();
  });
});

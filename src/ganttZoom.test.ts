import { describe, expect, it } from "vitest";

import { anchoredScroll, clampZoom, stepZoom, wheelPixels, wheelZoom, zoomedPx } from "./ganttZoom";

describe("Gantt zoom", () => {
  it("multiplies the scale's day width", () => {
    expect(zoomedPx("day", 1)).toBe(28);
    expect(zoomedPx("week", 2)).toBe(28);
    expect(zoomedPx("month", 0.5)).toBe(2.5);
  });

  it("keeps a day between 2 and 80 px whatever the scale", () => {
    expect(clampZoom("day", 10)).toBeCloseTo(80 / 28);
    expect(clampZoom("month", 0.1)).toBeCloseTo(2 / 5);
    expect(clampZoom("week", Number.NaN)).toBe(1);
  });

  it("steps by a quarter and lands back on 1", () => {
    expect(stepZoom("day", 1, 1)).toBe(1.25);
    expect(stepZoom("day", 1.25, -1)).toBe(1);
    expect(stepZoom("day", 1, -1)).toBe(0.8);
  });

  it("zooms in for a pinch outward or a wheel up, out the other way", () => {
    expect(wheelZoom("week", 1, -100)).toBeGreaterThan(1);
    expect(wheelZoom("week", 1, 100)).toBeLessThan(1);
    expect(wheelZoom("week", 1, 0)).toBe(1);
  });

  it("reads wheel deltas in pixels and keeps a mouse notch to a modest step", () => {
    expect(wheelPixels({ deltaY: 3, deltaMode: 0 })).toBe(3);
    expect(wheelPixels({ deltaY: 1, deltaMode: 1 })).toBe(16);
    expect(wheelPixels({ deltaY: 100, deltaMode: 0 })).toBe(25);
    expect(wheelPixels({ deltaY: -3, deltaMode: 1 })).toBe(-25);
  });

  it("keeps the date under the pointer in place", () => {
    // 10 days in at 20 px, pointer 100 px into the view: day 15 under it.
    expect(anchoredScroll(200, 100, 20, 40)).toBe(500);
    expect(anchoredScroll(0, 0, 20, 10)).toBe(0);
    expect(anchoredScroll(10, 100, 20, 10)).toBe(0);
  });
});

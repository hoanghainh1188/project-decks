import { describe, expect, it } from "vitest";

import {
  barBox,
  barColor,
  buildTimescale,
  dayLabel,
  dayLabelParts,
  holidayKeys,
  resourceLabelX,
  isWeekend,
  isoDate,
  pixelsPerDay,
  ROW,
  timescaleBands,
} from "./ganttLayout";
import { at, DAY_MS, makeProject, makeTask, standardCalendar } from "./test/fixtures";

describe("buildTimescale", () => {
  it("pads two days before and three weeks after the tasks, including baseline 0", () => {
    const project = makeProject({
      tasks: [
        makeTask({
          uid: 1,
          start: at(2026, 1, 5, 8),
          finish: at(2026, 1, 9, 17),
          baselines: [
            { number: 0, start: at(2026, 1, 2, 8), finish: at(2026, 1, 12, 17), work: 0, cost: 0, segments: [] },
            { number: 1, start: at(2025, 12, 1, 8), finish: at(2026, 3, 1, 17), work: 0, cost: 0, segments: [] },
          ],
        }),
      ],
    });
    const { origin, days } = buildTimescale(project);
    expect(origin).toBe(at(2025, 12, 31));
    expect(days[days.length - 1]).toBe(at(2026, 2, 2));
    expect(days.every((day, index) => index === 0 || day - days[index - 1] === DAY_MS)).toBe(true);
  });

  it("falls back to a two-week window from the project start when there are no tasks", () => {
    const { origin, days } = buildTimescale(makeProject({ start: at(2026, 1, 5, 8) }));
    expect(origin).toBe(at(2026, 1, 3));
    expect(days[days.length - 1]).toBe(at(2026, 2, 9));
  });
});

describe("timescale bands and labels", () => {
  const days = Array.from({ length: 70 }, (_, index) => at(2026, 1, 25) + index * DAY_MS);

  it("bands cover every day exactly once at each zoom level", () => {
    for (const scale of ["day", "week", "month"] as const) {
      const bands = timescaleBands(days, pixelsPerDay(scale));
      expect(bands.reduce((sum, band) => sum + band.count, 0)).toBe(days.length);
    }
  });

  it("day zoom groups by month", () => {
    const bands = timescaleBands(days, pixelsPerDay("day"));
    expect(bands.map((band) => band.label)).toEqual(["Tháng 1 2026", "Tháng 2 2026", "Tháng 3 2026", "Tháng 4 2026"]);
    expect(bands[0].count).toBe(7);
  });

  it("day labels depend on zoom", () => {
    const monday = at(2026, 1, 5);
    expect(dayLabel(monday, 28)).toBe("T2 5");
    expect(dayLabel(monday, 14)).toBe("5");
    expect(dayLabel(monday, 5)).toBe("");
  });

  it("splits the day label into weekday and date so neighbours never run together", () => {
    const monday = at(2026, 1, 5);
    const sunday = at(2026, 1, 11);
    expect(dayLabelParts(monday, 28)).toEqual({ weekday: "T2", date: "5" });
    expect(dayLabelParts(sunday, 20)).toEqual({ weekday: "CN", date: "11" });
    expect(dayLabelParts(monday, 14)).toEqual({ weekday: "", date: "5" });
    expect(dayLabelParts(monday, 5)).toEqual({ weekday: "", date: "" });
  });
});

describe("calendar helpers", () => {
  it("isWeekend and isoDate use UTC fields", () => {
    expect(isWeekend(at(2026, 1, 10, 23, 30))).toBe(true);
    expect(isWeekend(at(2026, 1, 12))).toBe(false);
    expect(isoDate(at(2026, 1, 5, 23, 59))).toBe("2026-01-05");
  });

  it("holidayKeys lists only full-day exceptions on the project calendar", () => {
    const calendar = standardCalendar();
    calendar.exceptions = {
      "2026-01-01": [],
      "2026-01-02": [{ start_minute: 480, end_minute: 720 }],
    };
    const other = { ...standardCalendar(2, "Khác"), exceptions: { "2026-01-03": [] } };
    expect([...holidayKeys(makeProject({ calendars: [calendar, other] }))]).toEqual(["2026-01-01"]);
  });
});

describe("bars", () => {
  it("gives short tasks a minimum width and milestones zero width", () => {
    const origin = at(2026, 1, 5);
    const task = makeTask({ uid: 1, start: at(2026, 1, 6), finish: at(2026, 1, 6) });
    expect(barBox(task, 2, origin, 28)).toEqual({ x: 28, y: 2 * ROW + 6, w: 8, h: 16 });
    expect(barBox({ ...task, milestone: true }, 0, origin, 28).w).toBe(0);
    expect(barBox({ ...task, summary: true }, 1, origin, 28)).toMatchObject({ y: ROW + 12, h: 6 });
  });

  it("colors critical before manual before summary", () => {
    const task = makeTask({ uid: 1 });
    expect(barColor({ ...task, critical: true, manual: true })).toBe("#c23b3b");
    expect(barColor({ ...task, manual: true, summary: true })).toBe("#2a9d8f");
    expect(barColor({ ...task, summary: true })).toBe("#1d1d1f");
    expect(barColor(task)).toBe("#2f6fad");
  });
});

describe("resourceLabelX", () => {
  it("puts the label just right of the bar when it fits", () => {
    expect(resourceLabelX(100, 160, 40, 1000)).toBe(166);
  });

  it("moves the label left of the bar near the end of the timescale", () => {
    expect(resourceLabelX(900, 980, 40, 1000)).toBe(854);
  });

  it("keeps it on the right when it fits neither side", () => {
    expect(resourceLabelX(10, 990, 40, 1000)).toBe(996);
  });
});

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { setCurrentLang } from "./i18n";
import {
  barBox,
  barColor,
  buildTimescale,
  dayLabel,
  dayLabelParts,
  groupLabel,
  holidayKeys,
  resourceLabelX,
  isWeekend,
  isoDate,
  levelingGapAt,
  levelingGaps,
  pixelsPerDay,
  ROW,
  segmentAt,
  segmentPreview,
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

  it("labels only Mondays on the week scale, where two-digit dates would run together", () => {
    const week = Array.from({ length: 8 }, (_, index) => at(2026, 9, 28) + index * 86_400_000);
    const labels = week.map((day) => dayLabel(day, 14));
    expect(labels).toEqual(["28", "", "", "", "", "", "", "5"]);
    expect(dayLabelParts(at(2026, 10, 12), 14)).toEqual({ weekday: "", date: "12" });
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

describe("timescale labels in English", () => {
  const days = Array.from({ length: 70 }, (_, index) => at(2026, 1, 25) + index * DAY_MS);

  beforeEach(() => setCurrentLang("en"));
  afterEach(() => setCurrentLang("vi"));

  it("names month bands in full on the day scale, with the same day counts", () => {
    const bands = timescaleBands(days, pixelsPerDay("day"));
    expect(bands.map((band) => band.label)).toEqual(["January 2026", "February 2026", "March 2026", "April 2026"]);
    expect(bands[0].count).toBe(7);
  });

  it("names weeks on the week scale and short months on the month scale", () => {
    expect(groupLabel(at(2026, 9, 14), pixelsPerDay("week"))).toMatch(/^Week \d+$/);
    expect(groupLabel(at(2026, 9, 14), pixelsPerDay("month"))).toBe("Sep 2026");
  });

  it("uses three-letter weekdays where Vietnamese shows T2 to CN", () => {
    const monday = at(2026, 1, 5);
    const sunday = at(2026, 1, 11);
    expect(dayLabel(monday, 28)).toBe("Mon 5");
    expect(dayLabelParts(sunday, 20)).toEqual({ weekday: "Sun", date: "11" });
    expect(dayLabelParts(monday, 14)).toEqual({ weekday: "", date: "5" });
  });
});

describe("dragging one piece of a split bar", () => {
  const split = makeTask({
    uid: 1,
    start: at(2026, 1, 5, 8),
    finish: at(2026, 1, 8, 17),
    segments: [
      { start: at(2026, 1, 5, 8), finish: at(2026, 1, 5, 12) },
      { start: at(2026, 1, 7, 8), finish: at(2026, 1, 8, 17) },
    ],
  });
  const origin = at(2026, 1, 5, 0);

  it("finds the piece under the pointer and none in the gap", () => {
    expect(segmentAt(split, 10, origin, 24)).toBe(0);
    expect(segmentAt(split, 36, origin, 24)).toBeNull();
    expect(segmentAt(split, 60, origin, 24)).toBe(1);
    expect(segmentAt(makeTask({ uid: 2 }), 10, origin, 24)).toBeNull();
  });

  it("moves the dragged piece and the ones after it, not the ones before", () => {
    const pieces = segmentPreview(split, 1, at(2026, 1, 8, 8));
    expect(pieces).toEqual([
      { start: at(2026, 1, 5, 8), finish: at(2026, 1, 5, 12) },
      { start: at(2026, 1, 8, 8), finish: at(2026, 1, 9, 17) },
    ]);
  });
});

describe("gaps leveling made", () => {
  const pieces = [
    { start: at(2026, 1, 5, 8), finish: at(2026, 1, 5, 12) },
    { start: at(2026, 1, 6, 8), finish: at(2026, 1, 6, 12) },
    { start: at(2026, 1, 8, 8), finish: at(2026, 1, 8, 17) },
  ];
  const task = makeTask({
    uid: 1,
    start: at(2026, 1, 5, 8),
    finish: at(2026, 1, 8, 17),
    duration: 1440,
    segments: pieces,
    splits: [
      { offset: 240, gap: 720 },
      { offset: 480, gap: 600, leveling: true },
    ],
  });
  const origin = at(2026, 1, 5, 0);

  it("marks each gap by the splits at its offset", () => {
    expect(levelingGaps(task)).toEqual([false, true]);
    // The user's and leveling's split at one offset lay out as one gap, which leveling lengthened.
    const shared = { ...task, splits: [{ offset: 240, gap: 120 }, { offset: 240, gap: 600, leveling: true }, { offset: 480, gap: 600 }] };
    expect(levelingGaps(shared)).toEqual([true, false]);
    expect(levelingGaps(makeTask({ uid: 2 }))).toEqual([]);
  });

  it("finds a leveling gap under the pointer only", () => {
    // 24 px a day: the second gap runs from Jan 6 12:00 (x 36) to Jan 8 08:00 (x 80).
    expect(levelingGapAt(task, 60, origin, 24)).toBe(true);
    expect(levelingGapAt(task, 20, origin, 24)).toBe(false);
    expect(levelingGapAt(task, 30, origin, 24)).toBe(false);
  });
});

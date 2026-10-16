import { describe, expect, it } from "vitest";

import { makeProject, makeTask } from "../../test/fixtures";
import type { Forecast, TaskForecast } from "../../types";
import { histogramBars, middleOfDay, riskyTasks } from "./forecastData";

const DAY = 86_400_000;
const START = Date.UTC(2026, 0, 5);

function forecastWith(tasks: TaskForecast[], histogram: [number, number][]): Forecast {
  return {
    iterations: 100,
    planned_finish: START,
    leveled_finish: START,
    finish: { p50: START, p80: START, p90: START },
    histogram,
    tasks,
  };
}

function task(uid: number, on_time: number | null, criticality: number): TaskForecast {
  return { uid, p50: START, p80: START, p90: START, on_time, criticality };
}

describe("histogramBars", () => {
  it("keeps one bar a day, with no gaps, and the share finished by each", () => {
    const bars = histogramBars([[START, 30], [START + 2 * DAY, 70]], 100);
    expect(bars.map((bar) => [bar.start, bar.count])).toEqual([[START, 30], [START + DAY, 0], [START + 2 * DAY, 70]]);
    expect(bars.map((bar) => bar.cumulative)).toEqual([0.3, 0.3, 1]);
    expect(bars.every((bar) => bar.days === 1)).toBe(true);
  });

  it("goes by week when the days are too many for the room", () => {
    const histogram: [number, number][] = Array.from({ length: 100 }, (_, index) => [START + index * DAY, 1]);
    const bars = histogramBars(histogram, 100, 60);
    expect(bars.length).toBe(Math.ceil(100 / 7));
    expect(bars[0]).toMatchObject({ start: START, count: 7, days: 7 });
    expect(bars[bars.length - 1].cumulative).toBe(1);
  });

  it("is empty without runs", () => {
    expect(histogramBars([], 0)).toEqual([]);
  });
});

describe("middleOfDay", () => {
  it("puts a finish at 17:00 in the middle of its own day, not the next", () => {
    expect(middleOfDay(START + 17 * 3_600_000)).toBe(START + DAY / 2);
    expect(middleOfDay(START)).toBe(START + DAY / 2);
  });
});

describe("riskyTasks", () => {
  const project = makeProject({
    tasks: [
      makeTask({ uid: 1, name: "Nhóm", summary: true }),
      makeTask({ uid: 2, name: "Có hạn, dễ trễ", deadline: START }),
      makeTask({ uid: 3, name: "Có hạn, an toàn", deadline: START }),
      makeTask({ uid: 4, name: "Găng", }),
      makeTask({ uid: 5, name: "Ít găng" }),
    ],
  });

  it("puts tasks with deadlines first, least likely on time first, then the most critical", () => {
    const result = forecastWith([task(1, null, 1), task(2, 0.4, 0.2), task(3, 0.95, 0.9), task(4, null, 0.8), task(5, null, 0.1)], []);
    expect(riskyTasks(project, result).map((row) => row.task.uid)).toEqual([2, 3, 4]);
  });

  it("stops at the limit", () => {
    const result = forecastWith([task(2, 0.4, 0.2), task(3, 0.5, 0.9), task(4, null, 0.8)], []);
    expect(riskyTasks(project, result, 2)).toHaveLength(2);
  });
});

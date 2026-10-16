import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { fakeBackend } from "../../test/fakeBackend";
import { makeProject } from "../../test/fixtures";
import type { Forecast } from "../../types";
import { useForecast } from "./useForecast";

const RESULT: Forecast = {
  iterations: 1000,
  planned_finish: 1,
  leveled_finish: 1,
  finish: { p50: 2, p80: 3, p90: 4 },
  histogram: [],
  tasks: [],
};

describe("useForecast", () => {
  it("does nothing until it is wanted", () => {
    fakeBackend.reset();
    const project = makeProject();
    const { result } = renderHook(() => useForecast(project, false));
    expect(result.current).toMatchObject({ result: null, running: false });
    expect(fakeBackend.calls.some((call) => call.cmd === "forecast")).toBe(false);
  });

  it("runs for each project and knows when what it shows is for an older one", async () => {
    fakeBackend.reset();
    fakeBackend.forecast = RESULT;
    const first = makeProject({ name: "A" });
    const { result, rerender } = renderHook(({ project }) => useForecast(project, true), { initialProps: { project: first } });
    expect(result.current.running).toBe(true);
    await waitFor(() => expect(result.current.result).toEqual(RESULT));
    expect(result.current).toMatchObject({ running: false, current: true });
    const second = makeProject({ name: "B" });
    fakeBackend.forecast = null;
    rerender({ project: second });
    // A stopped run answers null: the last result stays, marked as for an older project.
    await act(async () => {});
    expect(result.current).toMatchObject({ result: RESULT, current: false });
    expect(fakeBackend.calls.filter((call) => call.cmd === "forecast")).toHaveLength(2);
  });

  it("does not run again when turned back on for the same project", async () => {
    fakeBackend.reset();
    fakeBackend.forecast = RESULT;
    const project = makeProject();
    const { result, rerender } = renderHook(({ wanted }) => useForecast(project, wanted), { initialProps: { wanted: true } });
    await waitFor(() => expect(result.current.current).toBe(true));
    rerender({ wanted: false });
    rerender({ wanted: true });
    expect(fakeBackend.calls.filter((call) => call.cmd === "forecast")).toHaveLength(1);
    expect(result.current).toMatchObject({ current: true, running: false });
  });

  it("reports a failure", async () => {
    fakeBackend.reset();
    fakeBackend.failNext("forecast", "Không có thời gian làm việc");
    // One project for every render: a new object each time would be a new project.
    const project = makeProject();
    const { result } = renderHook(() => useForecast(project, true));
    await waitFor(() => expect(result.current.error).toBe("Không có thời gian làm việc"));
    expect(result.current.running).toBe(false);
  });
});

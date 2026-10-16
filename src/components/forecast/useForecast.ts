import { useEffect, useRef, useState } from "react";

import { errorMessage, forecast } from "../../api";
import type { Forecast, Project } from "../../types";

/** Runs each forecast makes: enough for steady P90s, about a second at 5,000 tasks. */
export const FORECAST_RUNS = 1000;

export interface ForecastState {
  /** The last forecast that finished, possibly for an older version of the project. */
  result: Forecast | null;
  /** Whether `result` is for the project as it is now. */
  current: boolean;
  running: boolean;
  error: string | null;
}

/**
 * The finish forecast of `project`, run again whenever it changes while `wanted` (the
 * forecast view or its sheet columns are on screen). Each run costs about a second of
 * CPU, so nothing runs otherwise. The engine stops a run once the project changes or a
 * newer run starts; this hook also drops any answer that is not for the latest request.
 */
export function useForecast(project: Project | undefined, wanted: boolean): ForecastState {
  const [state, setState] = useState<{ result: Forecast | null; forProject: Project | null; running: boolean; error: string | null }>(
    { result: null, forProject: null, running: false, error: null },
  );
  const latest = useRef(0);
  const shown = useRef<Project | null>(null);
  shown.current = state.forProject;

  useEffect(() => {
    // Turned back on with the project unchanged: the last answer is still right.
    if (!wanted || !project || shown.current === project) return;
    const request = ++latest.current;
    setState((before) => ({ ...before, running: true, error: null }));
    forecast(FORECAST_RUNS)
      .then((result) => {
        if (request !== latest.current) return;
        // Stopped: the project changed again and the next run is under way, or stopped too.
        if (result === null) return setState((before) => ({ ...before, running: false }));
        setState({ result, forProject: project, running: false, error: null });
      })
      .catch((err) => {
        if (request === latest.current) setState((before) => ({ ...before, running: false, error: errorMessage(err) }));
      });
  }, [project, wanted]);

  return {
    result: state.result,
    current: state.result != null && state.forProject === project,
    running: state.running,
    error: state.error,
  };
}

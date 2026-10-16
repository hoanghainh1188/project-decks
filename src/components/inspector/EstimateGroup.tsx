import { useState, type FocusEvent } from "react";

import { formatDuration, parseDuration } from "../../format";
import { useMessages } from "../../i18n";
import type { Command, Project, Task } from "../../types";
import { effortAmount } from "../../workRange";
import { estimateGroupMessages } from "./EstimateGroup.i18n";

interface Props {
  project: Project;
  task: Task;
  send: (command: Command) => unknown;
}

type Problem = "needBoth" | "backwards" | "unreadable" | null;

function show(minutes: number | null | undefined): string {
  return minutes == null ? "" : formatDuration(minutes);
}

/**
 * Ước lượng: the low and high effort of a leaf task for the finish forecast. Both amounts
 * or neither; the range is sent once both read (or both are emptied), so typing one end
 * never sends half a range. The effort the task has today may sit outside it, which the
 * group says.
 */
export function EstimateGroup({ project, task, send }: Props) {
  const m = useMessages(estimateGroupMessages);
  const shown = `${task.uid}:${task.work_low ?? ""}:${task.work_high ?? ""}`;
  const [draft, setDraft] = useState({ key: shown, low: show(task.work_low), high: show(task.work_high) });
  const [problem, setProblem] = useState<Problem>(null);
  // A new task, or a range changed elsewhere (undo, fill-down), replaces what was typed.
  const current = draft.key === shown ? draft : { key: shown, low: show(task.work_low), high: show(task.work_high) };
  if (current !== draft) {
    setDraft(current);
    setProblem(null);
  }
  if (task.summary || task.milestone) return null;

  // Leaving one end for the other is not done yet: check once focus leaves the pair.
  const commit = (event: FocusEvent<HTMLInputElement>) => {
    const next = event.relatedTarget;
    if (next instanceof HTMLElement && next.id.startsWith("estimate-") && next.id.endsWith(`-${task.uid}`)) return;
    const [lowText, highText] = [current.low.trim(), current.high.trim()];
    if (!lowText && !highText) {
      setProblem(null);
      if (task.work_low != null) send({ type: "set_work_range", uid: task.uid, low: null, high: null });
      return;
    }
    if (!lowText || !highText) return setProblem("needBoth");
    const [low, high] = [parseDuration(lowText), parseDuration(highText)];
    if (low == null || high == null) return setProblem("unreadable");
    if (low > high) return setProblem("backwards");
    setProblem(null);
    if (low !== task.work_low || high !== task.work_high) send({ type: "set_work_range", uid: task.uid, low, high });
  };

  const amount = effortAmount(task, project.assignments);
  const outside = task.work_low != null && task.work_high != null && (amount < task.work_low || amount > task.work_high);
  const field = (which: "low" | "high") => (
    <div>
      <label htmlFor={`estimate-${which}-${task.uid}`}>{m[which]}</label>
      <input
        id={`estimate-${which}-${task.uid}`}
        className="narrow"
        value={current[which]}
        aria-invalid={problem != null}
        aria-describedby={`estimate-status-${task.uid}`}
        onChange={(event) => setDraft({ ...current, [which]: event.target.value })}
        onBlur={commit}
      />
    </div>
  );
  return (
    <fieldset className="inspector-group">
      <legend>{m.legend}</legend>
      {field("low")}
      {field("high")}
      <p className="check-note">{m.note}</p>
      {outside && <p className="check-note">{m.outside(formatDuration(amount))}</p>}
      <p className="check-note bad" role="status" id={`estimate-status-${task.uid}`}>
        {problem ? m[problem] : ""}
      </p>
    </fieldset>
  );
}

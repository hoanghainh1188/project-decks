import { Check, X } from "lucide-react";
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";

import { errorMessage, previewPlan } from "../api";
import { formatInstant } from "../format";
import { useMessages } from "../i18n";
import type { NoPick, PlanPreview, Project, Snapshot, Suggestion, TaskAssignment } from "../types";
import { assignPreviewMessages } from "./AssignPreviewDialog.i18n";
import { Button } from "./Button";
import { Modal } from "./Modal";
import { PlanOutcome } from "./PlanOutcome";

/** Task names listed under each reason before the rest are counted. */
const LISTED = 20;
const REASONS: NoPick[] = ["no_rule", "no_one_with_skill", "no_room"];

/**
 * The suggestions of the assignment rules and what the whole plan would do. Every task
 * with a suggestion starts ticked with its first person chosen; the person can be changed
 * among those offered. Each change asks `preview_plan`, which assigns the ticked rows and
 * levels on a copy, and the dialog shows what moves; Áp dụng kế hoạch sends that plan
 * back (`apply_plan`, one undo step), refused if the project changed meanwhile. Not a
 * transaction: nothing changes until then.
 */
export function AssignPreviewDialog({
  project,
  suggestions,
  error,
  applyPlan,
  clearError,
  onClose,
}: {
  project: Project;
  suggestions: Suggestion[];
  /** The last command the engine refused. */
  error: string | null;
  applyPlan: (plan: PlanPreview) => Promise<Snapshot | void>;
  /** Forget `error`: a new plan is being worked out. */
  clearError?: () => void;
  onClose: () => void;
}) {
  const p = useMessages(assignPreviewMessages);
  // Thousands of rows: look names up by uid, not by scanning the lists per row.
  const tasks = useMemo(() => new Map(project.tasks.map((task) => [task.uid, task])), [project.tasks]);
  const people = useMemo(() => new Map(project.resources.map((item) => [item.uid, item.name])), [project.resources]);
  // The project can change under the dialog (undo and redo work while it is open): a task
  // gone or staffed meanwhile, or a person gone, leaves the suggestions.
  const offered = useMemo(() => {
    const staffed = new Set(project.assignments.map((assignment) => assignment.task_uid));
    return suggestions.flatMap((suggestion) => {
      if (!tasks.has(suggestion.task_uid) || staffed.has(suggestion.task_uid)) return [];
      const picks = suggestion.picks.filter((pick) => people.has(pick.resource_uid));
      return picks.length > 0 ? [{ ...suggestion, picks }] : [];
    });
  }, [suggestions, project.assignments, tasks, people]);
  const dropped = suggestions.filter((suggestion) => suggestion.picks.length > 0).length - offered.length;
  const [chosen, setChosen] = useState<Map<number, number>>(
    () => new Map(suggestions.flatMap((suggestion) => suggestion.picks.slice(0, 1).map((pick) => [suggestion.task_uid, pick.resource_uid]))),
  );
  const [unticked, setUnticked] = useState<Set<number>>(() => new Set());
  const [plan, setPlan] = useState<PlanPreview | null>(null);
  const [planError, setPlanError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // Set at once, unlike `busy`: a second click in the same tick must not send again.
  const sending = useRef(false);

  const taskName = (uid: number) => {
    const task = tasks.get(uid);
    return task ? `${task.id}. ${task.name}` : String(uid);
  };
  const personName = useCallback((uid: number) => people.get(uid) ?? String(uid), [people]);
  const rows: TaskAssignment[] = offered
    .filter((suggestion) => !unticked.has(suggestion.task_uid))
    .map((suggestion) => {
      const wanted = chosen.get(suggestion.task_uid);
      const pick = suggestion.picks.find((item) => item.resource_uid === wanted) ?? suggestion.picks[0];
      return { task_uid: suggestion.task_uid, resource_uid: pick.resource_uid, units: suggestion.units };
    });
  const allTicked = offered.length > 0 && rows.length === offered.length;
  const someTicked = rows.length > 0 && !allTicked;

  const toggle = useCallback((uid: number) => {
    setUnticked((current) => {
      const next = new Set(current);
      if (next.has(uid)) next.delete(uid);
      else next.add(uid);
      return next;
    });
  }, []);
  const choose = useCallback((task: number, person: number) => {
    setChosen((current) => new Map(current).set(task, person));
  }, []);

  // The plan for the rows as they are now; only the latest answer counts.
  const wanted = JSON.stringify(rows);
  useEffect(() => {
    let current = true;
    setPlan(null);
    setPlanError(null);
    clearError?.();
    previewPlan(JSON.parse(wanted) as TaskAssignment[])
      .then((next) => {
        if (!current) return;
        setPlanError(null);
        setPlan(next);
      })
      .catch((err) => current && setPlanError(errorMessage(err)));
    return () => {
      current = false;
    };
    // The project too: undo or redo under the dialog changes what leveling would do.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wanted, project]);

  const empty = plan != null && plan.assignments.length === 0 && plan.delays.length === 0 && plan.splits.length === 0;

  async function apply() {
    if (sending.current || !plan || empty) return;
    sending.current = true;
    setBusy(true);
    try {
      if (await applyPlan(plan)) onClose();
    } finally {
      sending.current = false;
      setBusy(false);
    }
  }

  // Closing while a command is on its way would hide whether it landed.
  const close = () => {
    if (!sending.current) onClose();
  };

  const status = error ?? planError;
  const note = status ? (
    <span className="dialog-foot-note bad" role="alert">
      {status}
    </span>
  ) : (
    <span className="meta dialog-foot-note" role="status">
      {[
        offered.length > 0 ? p.chosen(rows.length, offered.length) : null,
        plan == null ? p.planning : empty ? p.noChange : p.summary(plan.moves.length, plan.late.length),
        plan != null && !empty && rows.length === 0 ? p.levelOnly : null,
      ]
        .filter(Boolean)
        .join(" · ")}
    </span>
  );
  const footer = (
    <>
      {note}
      <Button Icon={X} disabled={busy} onClick={close}>
        {p.cancel}
      </Button>
      <Button variant="primary" Icon={Check} disabled={plan == null || empty || busy} onClick={() => void apply()}>
        {p.apply}
      </Button>
    </>
  );

  return (
    <Modal title={p.title} className="wide" onClose={close} footer={footer}>
      {suggestions.length === 0 ? (
        <p>{p.nothing}</p>
      ) : (
        <>
          <p className="meta">{p.intro(suggestions.length)}</p>
          {dropped > 0 && <p className="meta">{p.dropped(dropped)}</p>}
          {offered.length > 0 && (
            <div className="suggestion-wrap">
              <table className="suggestions" aria-label={p.suggestions} aria-rowcount={offered.length + 1}>
                <thead>
                  <tr>
                    <th scope="col">
                      <input
                        type="checkbox"
                        aria-label={p.applyAll}
                        checked={allTicked}
                        ref={(element) => {
                          if (element) element.indeterminate = someTicked;
                        }}
                        onChange={() =>
                          setUnticked(allTicked ? new Set(offered.map((suggestion) => suggestion.task_uid)) : new Set())
                        }
                      />
                    </th>
                    <th scope="col">{p.task}</th>
                    <th scope="col">{p.rule}</th>
                    <th scope="col">{p.assignTo}</th>
                    <th scope="col">{p.finish}</th>
                  </tr>
                </thead>
                <tbody>
                  {offered.map((suggestion) => (
                    <SuggestionRow
                      key={suggestion.task_uid}
                      suggestion={suggestion}
                      name={taskName(suggestion.task_uid)}
                      ticked={!unticked.has(suggestion.task_uid)}
                      person={chosen.get(suggestion.task_uid) ?? suggestion.picks[0].resource_uid}
                      personName={personName}
                      onToggle={toggle}
                      onChoose={choose}
                    />
                  ))}
                </tbody>
              </table>
            </div>
          )}
          <Unmatched suggestions={suggestions} taskName={taskName} />
        </>
      )}
      {plan && <PlanOutcome project={project} plan={plan} />}
    </Modal>
  );
}

/** One suggested task; drawn again only when its own tick or person changes. */
const SuggestionRow = memo(function SuggestionRow({
  suggestion,
  name,
  ticked,
  person,
  personName,
  onToggle,
  onChoose,
}: {
  suggestion: Suggestion;
  name: string;
  ticked: boolean;
  person: number;
  personName: (uid: number) => string;
  onToggle: (task: number) => void;
  onChoose: (task: number, person: number) => void;
}) {
  const p = useMessages(assignPreviewMessages);
  const pick = suggestion.picks.find((item) => item.resource_uid === person) ?? suggestion.picks[0];
  return (
    <tr className={ticked ? undefined : "off"}>
      <td>
        <input type="checkbox" aria-label={p.applyTo(name)} checked={ticked} onChange={() => onToggle(suggestion.task_uid)} />
      </td>
      <th scope="row">{name}</th>
      <td>{suggestion.rule == null ? "" : p.ruleN(suggestion.rule + 1)}</td>
      <td>
        <select
          aria-label={p.assignOf(name)}
          value={pick.resource_uid}
          onChange={(event) => onChoose(suggestion.task_uid, Number(event.target.value))}
        >
          {suggestion.picks.map((item) => (
            <option key={item.resource_uid} value={item.resource_uid}>
              {p.option(personName(item.resource_uid), formatInstant(item.finish))}
            </option>
          ))}
        </select>
      </td>
      <td className="num">{formatInstant(pick.finish)}</td>
    </tr>
  );
});

/** Tasks with no one to offer, grouped by why. */
function Unmatched({ suggestions, taskName }: { suggestions: Suggestion[]; taskName: (uid: number) => string }) {
  const p = useMessages(assignPreviewMessages);
  const groups = REASONS.map((reason) => ({
    reason,
    tasks: suggestions.filter((suggestion) => suggestion.reason === reason).map((suggestion) => suggestion.task_uid),
  })).filter((group) => group.tasks.length > 0);
  if (groups.length === 0) return null;
  return (
    <section aria-label={p.noPick}>
      <h3 className="dialog-subhead">{p.noPick}</h3>
      {groups.map(({ reason, tasks }) => (
        <details key={reason} className="unmatched">
          <summary>
            {p[reason]} ({tasks.length})
          </summary>
          <p className="meta">
            {tasks.slice(0, LISTED).map(taskName).join(", ")}
            {tasks.length > LISTED ? `, ${p.more(tasks.length - LISTED)}` : ""}
          </p>
        </details>
      ))}
    </section>
  );
}

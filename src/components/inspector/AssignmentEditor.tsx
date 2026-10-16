import { useState } from "react";

import { formatUnits, hoursPerDay, parseUnits, showsUnits } from "../../assignmentUnits";
import { formatMoney } from "../../format";
import { useMessages } from "../../i18n";
import { MoneyInput } from "../MoneyInput";
import type { Command, Project, Task } from "../../types";
import { assignmentEditorMessages } from "./AssignmentEditor.i18n";

export function AssignmentEditor({
  project,
  task,
  send,
}: {
  project: Project;
  task: Task;
  send: (command: Command) => void;
}) {
  const m = useMessages(assignmentEditorMessages);
  const [units, setUnits] = useState("1");
  const typed = parseUnits(units);
  const rows = project.assignments.filter((item) => item.task_uid === task.uid);
  return (
    <div className="assign">
      <label>{m.assign}</label>
      {rows.length > 0 && (
        <ul className="assign-list">
          {rows.map((assignment) => {
            const name =
              project.resources.find((resource) => resource.uid === assignment.resource_uid)?.name ??
              m.resourceFallback(assignment.resource_uid);
            return (
              <li key={assignment.uid}>
                <span>{name}</span>
                <label>
                  {m.units}
                  <input
                    aria-label={m.unitsOf(name)}
                    disabled={task.summary}
                    title={m.unitsTip}
                    defaultValue={formatUnits(assignment.units)}
                    key={`${assignment.uid}-${assignment.units}`}
                    onBlur={(event) => {
                      const next = parseUnits(event.target.value);
                      if (next == null) {
                        event.target.value = formatUnits(assignment.units);
                      } else if (!showsUnits(event.target.value, assignment.units)) {
                        send({
                          type: "update_assignment",
                          uid: assignment.uid,
                          units: next,
                          work: null,
                          overtime_work: null,
                        });
                      }
                    }}
                  />
                </label>
                <span className="meta">{hoursPerDay(assignment.units)}</span>
                {project.editable_actual_costs && !task.summary && (
                  <label>
                    {m.actualCost}
                    <MoneyInput
                      ariaLabel={m.actualCostOf(name)}
                      optional
                      value={assignment.manual_actual_cost ?? null}
                      placeholder={formatMoney(assignment.actual_cost)}
                      onCommit={(cost) => send({ type: "set_assignment_actual_cost", uid: assignment.uid, cost })}
                    />
                  </label>
                )}
                <button
                  type="button"
                  disabled={task.summary}
                  onClick={() => send({ type: "remove_assignment", uid: assignment.uid })}
                >
                  {m.remove}
                </button>
              </li>
            );
          })}
        </ul>
      )}
      <div className="assign-add">
        <label>
          {m.units}
          <input
            aria-label={m.newUnits}
            title={m.unitsTip}
            disabled={task.summary}
            value={units}
            onChange={(event) => setUnits(event.target.value)}
          />
        </label>
        {typed != null && <span className="meta">{m.equals(hoursPerDay(typed))}</span>}
        <select
          disabled={task.summary}
          defaultValue=""
          onChange={(event) => {
            const resource = Number(event.target.value);
            if (resource) {
              send({
                type: "assign_resource",
                task_uid: task.uid,
                resource_uid: resource,
                units: typed ?? 1,
              });
            }
            event.target.value = "";
          }}
        >
          <option value="">{m.choose}</option>
          {project.resources.map((resource) => (
            <option key={resource.uid} value={resource.uid}>{resource.name}</option>
          ))}
        </select>
      </div>
    </div>
  );
}

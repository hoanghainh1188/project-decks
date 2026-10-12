import { useState } from "react";

import { useMessages } from "../../i18n";
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
                    defaultValue={String(assignment.units)}
                    key={`${assignment.uid}-${assignment.units}`}
                    onBlur={(event) => {
                      const next = Number(event.target.value);
                      if (Number.isFinite(next) && next !== assignment.units) {
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
            disabled={task.summary}
            value={units}
            onChange={(event) => setUnits(event.target.value)}
          />
        </label>
        <select
          disabled={task.summary}
          defaultValue=""
          onChange={(event) => {
            const resource = Number(event.target.value);
            const parsed = Number(units);
            if (resource) {
              send({
                type: "assign_resource",
                task_uid: task.uid,
                resource_uid: resource,
                units: Number.isFinite(parsed) ? Math.max(0, parsed) : 1,
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

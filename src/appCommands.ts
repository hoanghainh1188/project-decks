import { taskAbove } from "./toolbarState";
import type { Command, DepKind, Project, Resource, Task } from "./types";

/** Link the task above `task` in outline order to `task` with a `kind` link. */
export function linkAbove(
  project: Project | undefined,
  task: Task | null,
  kind: DepKind,
  send: (command: Command) => void,
) {
  const above = project ? taskAbove(project.tasks, task) : null;
  if (!above || !task) return;
  send({ type: "add_dependency", predecessor_uid: above.uid, successor_uid: task.uid, kind, lag: 0 });
}

/** The command that stores every editable field of `resource`. */
export function resourceCommand(resource: Resource): Command {
  return {
    type: "update_resource",
    uid: resource.uid,
    name: resource.name,
    kind: resource.kind,
    max_units: resource.max_units,
    calendar_id: resource.calendar_id,
    standard_rate: resource.standard_rate,
    overtime_rate: resource.overtime_rate,
    cost_per_use: resource.cost_per_use,
  };
}

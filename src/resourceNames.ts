import type { Assignment, Resource } from "./types";

function label(name: string, units: number): string {
  const percent = Math.round(units * 100);
  return percent === 100 ? name : `${name}[${percent}%]`;
}

/** Resource names of every task that has any, in one pass: uid -> "Chi, An[50%]". */
export function resourceNamesByTask(assignments: Assignment[], resources: Resource[]): Map<number, string> {
  const byUid = new Map(resources.map((resource) => [resource.uid, resource.name]));
  const parts = new Map<number, string[]>();
  for (const assignment of assignments) {
    const name = byUid.get(assignment.resource_uid);
    if (name === undefined) continue;
    const names = parts.get(assignment.task_uid);
    if (names) names.push(label(name, assignment.units));
    else parts.set(assignment.task_uid, [label(name, assignment.units)]);
  }
  return new Map([...parts].map(([uid, names]) => [uid, names.join(", ")]));
}

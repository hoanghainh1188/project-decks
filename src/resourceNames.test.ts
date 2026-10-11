import { describe, expect, it } from "vitest";

import { resourceNamesByTask } from "./resourceNames";
import type { Assignment, Resource } from "./types";

function resource(uid: number, name: string): Resource {
  return {
    uid,
    name,
    kind: "person",
    max_units: 1,
    calendar_id: null,
    standard_rate: 0,
    overtime_rate: 0,
    cost_per_use: 0,
  };
}

function assignment(uid: number, taskUid: number, resourceUid: number, units: number): Assignment {
  return {
    uid,
    task_uid: taskUid,
    resource_uid: resourceUid,
    units,
    work: 0,
    overtime_work: 0,
    actual_work: 0,
    remaining_work: 0,
    cost: 0,
    actual_cost: 0,
  };
}

const resources = [resource(1, "An"), resource(2, "Bình"), resource(3, "Chi")];

function resourceNames(taskUid: number, assignments: Assignment[], list: Resource[]): string {
  return resourceNamesByTask(assignments, list).get(taskUid) ?? "";
}

describe("resourceNamesByTask", () => {
  it("lists the task's resources in assignment order", () => {
    const assignments = [assignment(1, 7, 2, 1), assignment(2, 8, 1, 1), assignment(3, 7, 3, 1)];
    expect(resourceNames(7, assignments, resources)).toBe("Bình, Chi");
  });

  it("adds units the way Project does when they are not 100%", () => {
    const assignments = [assignment(1, 7, 3, 1), assignment(2, 7, 1, 0.5), assignment(3, 7, 2, 1.25)];
    expect(resourceNames(7, assignments, resources)).toBe("Chi, An[50%], Bình[125%]");
  });

  it("is empty for a task with no assignments", () => {
    expect(resourceNames(9, [assignment(1, 7, 1, 1)], resources)).toBe("");
  });

  it("skips assignments whose resource no longer exists", () => {
    expect(resourceNames(7, [assignment(1, 7, 42, 1), assignment(2, 7, 1, 1)], resources)).toBe("An");
  });
});

describe("resourceNamesByTask map", () => {
  it("has one entry per task that has resources", () => {
    const assignments = [assignment(1, 7, 2, 1), assignment(2, 8, 1, 0.5), assignment(3, 7, 3, 1)];
    const byTask = resourceNamesByTask(assignments, resources);
    expect(byTask.get(7)).toBe("Bình, Chi");
    expect(byTask.get(8)).toBe("An[50%]");
    expect(byTask.has(9)).toBe(false);
  });
});

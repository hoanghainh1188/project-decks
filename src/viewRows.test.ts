import { describe, expect, it } from "vitest";

import { makeProject, makeTask, outlineProject } from "./test/fixtures";
import {
  buildViewRows,
  type FilterMode,
  type GroupMode,
  type HighlightMode,
  type ViewRow,
} from "./viewRows";

/** Compact view of rows: "# label" for group headers, uid (with "*" when highlighted) for tasks. */
function shape(rows: ViewRow[]): string[] {
  return rows.map((row) =>
    row.kind === "group" ? `# ${row.label}` : `${row.task.uid}${row.highlighted ? "*" : ""}`,
  );
}

describe("buildViewRows without grouping", () => {
  it("keeps outline order and uses task keys", () => {
    const rows = buildViewRows(outlineProject(), "none", null, "none", "none");
    expect(shape(rows)).toEqual(["1", "2", "3", "4", "5", "6", "7", "8"]);
    expect(rows.map((row) => row.key)).toEqual([1, 2, 3, 4, 5, 6, 7, 8].map((uid) => `task-${uid}`));
  });

  it("returns no rows for an empty project", () => {
    expect(buildViewRows(makeProject(), "none", null, "summary", "none")).toEqual([]);
  });

  it("highlights critical or late tasks without hiding the rest", () => {
    const project = outlineProject();
    expect(shape(buildViewRows(project, "none", null, "none", "critical"))).toEqual([
      "1", "2*", "3", "4", "5", "6", "7", "8",
    ]);
    expect(shape(buildViewRows(project, "none", null, "none", "late"))).toEqual([
      "1", "2", "3*", "4", "5", "6", "7", "8",
    ]);
  });
});

describe("buildViewRows filters", () => {
  it("critical filter keeps matching tasks and their summary ancestors", () => {
    expect(shape(buildViewRows(outlineProject(), "critical", null, "none", "none"))).toEqual(["1", "2"]);
  });

  it("late filter keeps matching tasks and their summary ancestors", () => {
    expect(shape(buildViewRows(outlineProject(), "late", null, "none", "none"))).toEqual(["1", "3"]);
  });

  it("keeps every summary up the chain for a deep match", () => {
    const project = outlineProject();
    project.tasks[4].critical = true;
    expect(shape(buildViewRows(project, "critical", null, "none", "none"))).toEqual(["1", "2", "4", "5"]);
  });

  it("resource filter shows tasks assigned to that resource", () => {
    const project = outlineProject();
    expect(shape(buildViewRows(project, "resource", 11, "none", "none"))).toEqual(["1", "3", "6", "7"]);
    expect(shape(buildViewRows(project, "resource", 12, "none", "none"))).toEqual([]);
  });

  it("resource filter without a chosen resource hides everything", () => {
    expect(buildViewRows(outlineProject(), "resource", null, "none", "none")).toEqual([]);
  });

  it("does not mutate the project or its task order", () => {
    const project = outlineProject();
    const before = structuredClone(project);
    buildViewRows(project, "late", null, "summary", "critical");
    buildViewRows(project, "none", null, "resource", "late");
    expect(project).toEqual(before);
  });
});

describe("buildViewRows grouped by person", () => {
  it("follows resource order, lists a task under every assignee and puts unassigned last", () => {
    const rows = buildViewRows(outlineProject(), "none", null, "resource", "none");
    expect(shape(rows)).toEqual([
      "# An", "2", "3",
      "# NL 11", "3", "7",
      "# Chưa gán", "1", "4", "5", "6", "8",
    ]);
  });

  it("skips resources without tasks", () => {
    const rows = buildViewRows(outlineProject(), "none", null, "resource", "none");
    expect(rows.some((row) => row.kind === "group" && row.label === "Chi")).toBe(false);
  });

  it("keeps co-assignees when combined with the resource filter", () => {
    const rows = buildViewRows(outlineProject(), "resource", 10, "resource", "none");
    // Summary 1 is kept by the filter but has no assignment, so it lands in "Chưa gán".
    expect(shape(rows)).toEqual(["# An", "2", "3", "# NL 11", "3", "# Chưa gán", "1"]);
  });

  it("omits the unassigned group when everything is assigned", () => {
    const flat = makeProject({
      tasks: [makeTask({ uid: 1 })],
      resources: outlineProject().resources,
      assignments: [outlineProject().assignments[0]].map((item) => ({ ...item, task_uid: 1 })),
    });
    expect(shape(buildViewRows(flat, "none", null, "resource", "none"))).toEqual(["# An", "1"]);
  });

  it("uses group-prefixed keys so a task under two people stays unique", () => {
    const rows = buildViewRows(outlineProject(), "none", null, "resource", "none");
    const keys = rows.map((row) => row.key);
    expect(keys).toContain("group-10-task-3");
    expect(keys).toContain("group-11-task-3");
    expect(keys).toContain("group-unassigned");
    expect(keys).toContain("group-unassigned-task-8");
  });
});

describe("buildViewRows grouped by summary", () => {
  it("puts leaves under their nearest summary in outline order and orphans last", () => {
    const rows = buildViewRows(outlineProject(), "none", null, "summary", "none");
    expect(shape(rows)).toEqual([
      "# 1 Thiết kế", "2", "3",
      "# 1.3 Kiểm thử", "5",
      "# 2 Thi công", "7",
      "# Không có summary", "8",
    ]);
  });

  it("orders groups by outline position, not by when a leaf is seen", () => {
    const project = outlineProject();
    // Move "Đổ móng" ahead in the list; its summary (uid 6) still follows uid 1 and uid 4.
    const [leaf] = project.tasks.splice(6, 1);
    project.tasks.splice(1, 0, leaf);
    const labels = buildViewRows(project, "none", null, "summary", "none")
      .filter((row) => row.kind === "group")
      .map((row) => (row.kind === "group" ? row.label : ""));
    expect(labels).toEqual(["1 Thiết kế", "1.3 Kiểm thử", "2 Thi công", "Không có summary"]);
  });

  it("labels summaries without WBS or name", () => {
    const project = makeProject({
      tasks: [
        makeTask({ uid: 1, name: "  ", wbs: "", summary: true }),
        makeTask({ uid: 2, parent_uid: 1, outline_level: 2 }),
        makeTask({ uid: 3, name: "Gói B", wbs: "", summary: true }),
        makeTask({ uid: 4, parent_uid: 3, outline_level: 2 }),
      ],
    });
    expect(shape(buildViewRows(project, "none", null, "summary", "none"))).toEqual([
      "# Task 1", "2", "# Gói B", "4",
    ]);
  });

  it("omits the no-summary group when every leaf has a parent", () => {
    const project = outlineProject();
    project.tasks = project.tasks.filter((task) => task.uid !== 8);
    const labels = shape(buildViewRows(project, "none", null, "summary", "none"));
    expect(labels).not.toContain("# Không có summary");
  });
});

describe("buildViewRows combined filter, group and highlight", () => {
  it("filters first, then groups, then highlights", () => {
    const rows = buildViewRows(outlineProject(), "resource", 11, "summary", "late");
    expect(shape(rows)).toEqual(["# 1 Thiết kế", "3*", "# 2 Thi công", "7"]);
  });

  it("highlights inside person groups", () => {
    const rows = buildViewRows(outlineProject(), "none", null, "resource", "critical");
    expect(shape(rows).slice(0, 3)).toEqual(["# An", "2*", "3"]);
  });
});

describe("buildViewRows keys", () => {
  const filters: FilterMode[] = ["none", "critical", "resource", "late"];
  const groups: GroupMode[] = ["none", "resource", "summary"];
  const highlights: HighlightMode[] = ["none", "critical", "late"];

  it("are unique for every combination of modes", () => {
    const project = outlineProject();
    for (const filter of filters) {
      for (const group of groups) {
        for (const highlight of highlights) {
          for (const resource of [null, 10, 11]) {
            const keys = buildViewRows(project, filter, resource, group, highlight).map((row) => row.key);
            expect(new Set(keys).size, `${filter}/${group}/${highlight}/${resource}`).toBe(keys.length);
          }
        }
      }
    }
  });
});

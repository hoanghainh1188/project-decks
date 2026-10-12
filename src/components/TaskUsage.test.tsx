import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { makeProject } from "../test/fixtures";
import { TaskUsage } from "./TaskUsage";

describe("TaskUsage", () => {
  const project = makeProject({
    usage: [
      { task_uid: 1, assignment_uid: null, resource_uid: null, label: "Việc A", days: [{ date: "2026-01-05", hours: 7.2 }] },
      { task_uid: 1, assignment_uid: 10, resource_uid: 1, label: "An", days: [{ date: "2026-01-05", hours: 7.2 }] },
      { task_uid: 2, assignment_uid: null, resource_uid: null, label: "Việc B", days: [{ date: "2026-01-06", hours: 8 }] },
      { task_uid: 2, assignment_uid: 11, resource_uid: 1, label: "An", days: [{ date: "2026-01-06", hours: 8 }] },
    ],
    overallocations: [{ resource_uid: 1, date: "2026-01-05", units: 1.5, max_units: 1 }],
  });

  it("marks only the overloaded cells that carry work", () => {
    render(<TaskUsage project={project} />);
    const hot = Array.from(document.querySelectorAll("td.over"));
    expect(hot.map((cell) => cell.textContent)).toEqual(["7,2"]);
    // An's row for task B has no hours on the overloaded day, so it stays plain.
    expect(screen.getAllByText("An")).toHaveLength(2);
  });

  it("writes hours with a decimal comma", () => {
    render(<TaskUsage project={project} />);
    expect(screen.getAllByText("7,2")).toHaveLength(2);
  });
});

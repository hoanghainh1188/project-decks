import { useMemo } from "react";

import { appMessages } from "../App.i18n";
import { formatInstant } from "../format";
import { useMessages } from "../i18n";
import { projectFinish } from "../projectFinish";
import type { Snapshot } from "../types";
import { fileName } from "../windowTitle";

/** Saved state, file, counts and the project finish along the bottom of the window. */
export function StatusBar({ snap }: { snap: Snapshot | null }) {
  const m = useMessages(appMessages);
  const project = snap?.project;
  const criticalCount = useMemo(
    () => project?.tasks.filter((item) => item.critical && !item.summary).length ?? 0,
    [project],
  );
  const end = project ? projectFinish(project.tasks) : null;
  return (
    <footer className="status">
      <span>{snap?.dirty ? m.unsaved : m.saved}</span>
      <span title={snap?.path ?? undefined}>{snap?.path ? fileName(snap.path) : m.noFile}</span>
      <span>{m.taskCount(project?.tasks.length ?? 0)}</span>
      <span>{m.criticalCount(criticalCount)}</span>
      <span className="status-finish" title={m.finishTip}>
        {end ? m.finishedBy(formatInstant(end.finish), end.driver) : ""}
      </span>
      <span>{m.overallocations(project ? project.overallocations.length : 0)}</span>
    </footer>
  );
}

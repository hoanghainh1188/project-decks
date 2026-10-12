import { formatDuration, formatInstant } from "../../format";
import { useMessages } from "../../i18n";
import type { Command, Task } from "../../types";
import { splitSectionMessages } from "./SplitSection.i18n";

/** Only leaf tasks with work can be split; summaries and milestones cannot. */
export function canSplit(task: Task | null): task is Task {
  return task != null && !task.summary && !task.milestone && task.duration > 0;
}

export function SplitSection({
  task,
  send,
  onSplit,
}: {
  task: Task;
  send: (command: Command) => void;
  onSplit: (uid: number) => void;
}) {
  const m = useMessages(splitSectionMessages);
  return (
    <div className="split-section">
      <label>{m.title}</label>
      {task.segments.length > 1 && (
        <ol className="split-list">
          {task.segments.map((segment, index) => (
            <li key={`${segment.start}-${index}`}>
              <span>{m.segment(index + 1, formatInstant(segment.start), formatInstant(segment.finish))}</span>
              {index + 1 < task.segments.length && (
                <button
                  type="button"
                  title={m.gap(formatDuration(task.splits[index]?.gap ?? 0))}
                  onClick={() => send({ type: "remove_split", uid: task.uid, index })}
                >
                  {m.join(index + 2)}
                </button>
              )}
            </li>
          ))}
        </ol>
      )}
      <div className="row">
        <button type="button" onClick={() => onSplit(task.uid)}>{m.split}</button>
        {task.splits.length > 0 && (
          <button type="button" onClick={() => send({ type: "remove_split", uid: task.uid, index: null })}>
            {m.joinAll}
          </button>
        )}
      </div>
    </div>
  );
}

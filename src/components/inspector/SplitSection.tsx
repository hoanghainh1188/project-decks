import { formatDuration, formatInstant } from "../../format";
import type { Command, Task } from "../../types";

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
  return (
    <div className="split-section">
      <label>Tách công việc</label>
      {task.segments.length > 1 && (
        <ol className="split-list">
          {task.segments.map((segment, index) => (
            <li key={`${segment.start}-${index}`}>
              <span>
                Đoạn {index + 1}: {formatInstant(segment.start)} – {formatInstant(segment.finish)}
              </span>
              {index + 1 < task.segments.length && (
                <button
                  type="button"
                  title={`Ngắt ${formatDuration(task.splits[index]?.gap ?? 0)} làm việc`}
                  onClick={() => send({ type: "remove_split", uid: task.uid, index })}
                >
                  Nối với đoạn {index + 2}
                </button>
              )}
            </li>
          ))}
        </ol>
      )}
      <div className="row">
        <button type="button" onClick={() => onSplit(task.uid)}>Tách…</button>
        {task.splits.length > 0 && (
          <button type="button" onClick={() => send({ type: "remove_split", uid: task.uid, index: null })}>
            Nối tất cả các đoạn
          </button>
        )}
      </div>
    </div>
  );
}

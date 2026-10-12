import { useState } from "react";

import { DateTimeField } from "./DateTimeField";
import { Modal } from "./Modal";
import { formatDuration, formatInstant, parseDuration } from "../format";
import type { Task } from "../types";

const HOUR_MS = 3_600_000;

interface SplitDialogProps {
  task: Task;
  onClose: () => void;
  /** Resolves to a truthy value when the engine accepted the split. */
  onSubmit: (at: number, gap: number) => Promise<unknown>;
}

/** Middle of the first piece of work, on the hour, as a starting suggestion. */
function suggestedPoint(task: Task): number {
  const first = task.segments[0] ?? { start: task.start, finish: task.finish };
  const middle = first.start + (first.finish - first.start) / 2;
  return Math.max(first.start + HOUR_MS, Math.round(middle / HOUR_MS) * HOUR_MS);
}

export function SplitDialog({ task, onClose, onSubmit }: SplitDialogProps) {
  const [at, setAt] = useState(() => suggestedPoint(task));
  const [gapText, setGapText] = useState("1 ngày");
  const [problem, setProblem] = useState<string | null>(null);

  async function submit() {
    const gap = parseDuration(gapText);
    if (gap === null || gap <= 0) {
      setProblem("Khoảng ngắt phải lớn hơn 0, ví dụ 1 ngày, 4 giờ, 30 phút");
      return;
    }
    if (at <= task.start || at >= task.finish) {
      setProblem("Điểm tách phải nằm giữa ngày bắt đầu và ngày kết thúc của công việc");
      return;
    }
    setProblem(null);
    const result = await onSubmit(at, gap);
    if (result) onClose();
  }

  return (
    <Modal title="Tách công việc" className="split-dialog" onClose={onClose}>
      <p className="meta">
        {task.name} · {formatInstant(task.start)} – {formatInstant(task.finish)} · thời lượng {formatDuration(task.duration)}
      </p>
      <label>Tách tại</label>
      <DateTimeField
        ariaLabel="Thời điểm tách"
        value={at}
        onCommit={(ms) => {
          if (ms != null) setAt(ms);
        }}
      />
      <label>Khoảng ngắt (giờ làm việc)</label>
      <input aria-label="Khoảng ngắt" value={gapText} onChange={(event) => setGapText(event.target.value)} />
      <p className="meta">
        Thời lượng giữ nguyên. Phần việc sau điểm tách dời muộn hơn đúng bằng khoảng ngắt, nên ngày kết thúc cũng lùi theo.
      </p>
      {problem && <p className="meta bad">{problem}</p>}
      <div className="row">
        <button type="button" onClick={() => void submit()}>Tách</button>
        <button type="button" onClick={onClose}>Hủy</button>
      </div>
    </Modal>
  );
}

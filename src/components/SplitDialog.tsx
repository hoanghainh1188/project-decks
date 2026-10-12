import { Scissors, X } from "lucide-react";
import { useState } from "react";

import { Button } from "./Button";
import { DateTimeField } from "./DateTimeField";
import { Modal } from "./Modal";
import { formatDuration, formatInstant, parseDuration } from "../format";
import { useMessages } from "../i18n";
import type { Task } from "../types";
import { splitDialogMessages } from "./SplitDialog.i18n";

const HOUR_MS = 3_600_000;
const DAY_MINUTES = 8 * 60;

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
  const m = useMessages(splitDialogMessages);
  const [at, setAt] = useState(() => suggestedPoint(task));
  const [gapText, setGapText] = useState(() => formatDuration(DAY_MINUTES));
  const [problem, setProblem] = useState<string | null>(null);

  async function submit() {
    const gap = parseDuration(gapText);
    if (gap === null || gap <= 0) {
      setProblem(m.badGap);
      return;
    }
    if (at <= task.start || at >= task.finish) {
      setProblem(m.badPoint);
      return;
    }
    setProblem(null);
    const result = await onSubmit(at, gap);
    if (result) onClose();
  }

  return (
    <Modal
      title={m.title}
      className="split-dialog"
      onClose={onClose}
      footer={
        <>
          <Button Icon={X} onClick={onClose}>{m.cancel}</Button>
          <Button variant="primary" Icon={Scissors} onClick={() => void submit()}>{m.split}</Button>
        </>
      }
    >
      <p className="meta">
        {m.summary(task.name, formatInstant(task.start), formatInstant(task.finish), formatDuration(task.duration))}
      </p>
      <label>{m.splitAt}</label>
      <DateTimeField
        ariaLabel={m.splitAtLabel}
        value={at}
        onCommit={(ms) => {
          if (ms != null) setAt(ms);
        }}
      />
      <label>{m.gap}</label>
      <input aria-label={m.gapLabel} value={gapText} onChange={(event) => setGapText(event.target.value)} />
      <p className="meta">
        {m.explanation}
      </p>
      {problem && <p className="meta bad">{problem}</p>}
    </Modal>
  );
}

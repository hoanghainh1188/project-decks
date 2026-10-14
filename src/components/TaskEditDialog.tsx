import { Check } from "lucide-react";

import { Button } from "./Button";
import { DateTimeField } from "./DateTimeField";
import { Modal } from "./Modal";
import { formatDuration, formatInstant, normalizePredecessorText, parseDuration } from "../format";
import { useMessages } from "../i18n";
import type { Command, Task } from "../types";
import { taskEditDialogMessages } from "./TaskEditDialog.i18n";

interface Props {
  task: Task;
  send: (command: Command) => void;
  onClose: () => void;
}

/** Same fields and commands as the task sheet row, for views without inline cells. */
export function TaskEditDialog({ task, send, onClose }: Props) {
  const m = useMessages(taskEditDialogMessages);
  return (
    <Modal
      title={`${task.wbs ? `${task.wbs} · ` : ""}${m.title}`}
      onClose={onClose}
      footer={<Button variant="primary" Icon={Check} onClick={onClose}>{m.close}</Button>}
    >
      <label>{m.name}</label>
      <input
        autoFocus
        defaultValue={task.name}
        key={`name-${task.uid}-${task.name}`}
        onKeyDown={(event) => event.key === "Enter" && event.currentTarget.blur()}
        onBlur={(event) => {
          const name = event.target.value.trim();
          if (name && name !== task.name) send({ type: "rename_task", uid: task.uid, name });
        }}
      />
      <label>{m.duration}</label>
      <input
        disabled={task.summary}
        defaultValue={formatDuration(task.duration)}
        key={`duration-${task.uid}-${task.duration}`}
        onKeyDown={(event) => event.key === "Enter" && event.currentTarget.blur()}
        onBlur={(event) => {
          const minutes = parseDuration(event.target.value);
          if (minutes !== null && minutes !== task.duration) send({ type: "set_duration", uid: task.uid, minutes });
        }}
      />
      <label>{m.start}</label>
      <DateTimeField
        disabled={task.summary}
        ariaLabel={m.start}
        value={task.start}
        onCommit={(start) => {
          if (start != null && start !== task.start) send({ type: "move_task", uid: task.uid, start });
        }}
      />
      <label>{m.finish}</label>
      <input readOnly value={formatInstant(task.finish)} />
      <label>{m.percentComplete}</label>
      <input
        disabled={task.summary}
        inputMode="numeric"
        defaultValue={String(Math.round(task.percent_complete))}
        key={`percent-${task.uid}-${task.percent_complete}`}
        onKeyDown={(event) => event.key === "Enter" && event.currentTarget.blur()}
        onBlur={(event) => {
          const percent = Number(event.target.value);
          if (Number.isFinite(percent) && percent !== Math.round(task.percent_complete)) {
            send({ type: "set_percent_complete", uid: task.uid, percent: Math.min(100, Math.max(0, percent)) });
          }
        }}
      />
      <label>{m.predecessors}</label>
      <input
        defaultValue={task.predecessors}
        key={`pred-${task.uid}-${task.predecessors}`}
        placeholder={m.predecessorsPlaceholder}
        onKeyDown={(event) => event.key === "Enter" && event.currentTarget.blur()}
        onBlur={(event) => {
          if (event.target.value !== task.predecessors) {
            send({ type: "set_predecessors", uid: task.uid, text: normalizePredecessorText(event.target.value) });
          }
        }}
      />
      {!task.summary && !task.milestone && (
        <>
          <label className="check">
            <input
              type="checkbox"
              checked={task.interrupts}
              aria-describedby={`edit-interrupts-note-${task.uid}`}
              onChange={(event) => send({ type: "set_interrupts", uid: task.uid, interrupts: event.target.checked })}
            />
            {m.interrupts}
          </label>
          <p className="check-note" id={`edit-interrupts-note-${task.uid}`}>{m.interruptsNote}</p>
        </>
      )}
    </Modal>
  );
}

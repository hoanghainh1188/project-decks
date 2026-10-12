import { DateTimeField } from "./DateTimeField";
import { Modal } from "./Modal";
import { formatDuration, formatInstant, parseDuration } from "../format";
import type { Command, Task } from "../types";

interface Props {
  task: Task;
  send: (command: Command) => void;
  onClose: () => void;
}

/** Same fields and commands as the task sheet row, for views without inline cells. */
export function TaskEditDialog({ task, send, onClose }: Props) {
  return (
    <Modal title={`${task.wbs ? `${task.wbs} · ` : ""}Sửa công việc`} onClose={onClose}>
      <label>Tên</label>
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
      <label>Thời lượng</label>
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
      <label>Bắt đầu</label>
      <DateTimeField
        disabled={task.summary}
        ariaLabel="Bắt đầu"
        value={task.start}
        onCommit={(start) => {
          if (start != null && start !== task.start) send({ type: "move_task", uid: task.uid, start });
        }}
      />
      <label>Kết thúc</label>
      <input readOnly value={formatInstant(task.finish)} />
      <label>% hoàn thành</label>
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
      <label>Tiền nhiệm</label>
      <input
        defaultValue={task.predecessors}
        key={`pred-${task.uid}-${task.predecessors}`}
        placeholder="VD: 2, 3SS+1d"
        onKeyDown={(event) => event.key === "Enter" && event.currentTarget.blur()}
        onBlur={(event) => {
          if (event.target.value !== task.predecessors) {
            send({ type: "set_predecessors", uid: task.uid, text: event.target.value });
          }
        }}
      />
      <button type="button" onClick={onClose}>Đóng</button>
    </Modal>
  );
}

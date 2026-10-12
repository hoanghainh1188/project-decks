import { CalendarPlus, Check, X } from "lucide-react";
import { useState } from "react";

import { appMessages } from "../App.i18n";
import { useMessages } from "../i18n";
import type { Command, Project, Snapshot } from "../types";
import { Button } from "./Button";
import { CalendarEditor } from "./CalendarEditor";
import { DateTimeField } from "./DateTimeField";
import { Modal } from "./Modal";

/**
 * Changes apply as they are made, so the Gantt previews them; Lưu keeps them and Hủy
 * (✕, Escape) undoes back to the state the dialog opened with.
 */
export function ProjectDialog({
  project,
  changed,
  onSave,
  onCancel,
  send,
  onCreateCalendar,
}: {
  project: Project;
  changed: boolean;
  onSave: () => void;
  onCancel: () => void;
  send: (command: Command) => Promise<Snapshot | void>;
  onCreateCalendar: () => Promise<number | null>;
}) {
  const m = useMessages(appMessages);
  const [editingId, setEditingId] = useState(project.calendar_id);
  const calendar =
    project.calendars.find((item) => item.id === editingId) ??
    project.calendars.find((item) => item.id === project.calendar_id) ??
    project.calendars[0];
  return (
    <Modal
      title={m.projectInfo}
      className="wide"
      onClose={onCancel}
      footer={
        <>
          <span className="meta dialog-foot-note">{changed ? m.previewing : ""}</span>
          <Button Icon={X} onClick={onCancel}>{m.cancel}</Button>
          <Button variant="primary" Icon={Check} onClick={onSave}>{m.save}</Button>
        </>
      }
    >
      <label>{m.name}</label>
      <input
        // Shows the name again after undo or redo inside the dialog.
        key={project.name}
        defaultValue={project.name}
        onBlur={(event) => {
          if (event.target.value !== project.name) void send({ type: "rename_project", name: event.target.value });
        }}
      />
      <label>{m.startDate}</label>
      <DateTimeField
        ariaLabel={m.startDateLabel}
        value={project.start}
        onCommit={(start) => {
          if (start != null) void send({ type: "set_project_start", start });
        }}
      />
      <label>{m.statusDate}</label>
      <DateTimeField
        optional
        ariaLabel={m.statusDate}
        value={project.status_date}
        onCommit={(date) => void send({ type: "set_status_date", date })}
      />
      {calendar && (
        <>
          <label>{m.editingCalendar}</label>
          <div className="row">
            <select value={calendar.id} onChange={(event) => setEditingId(Number(event.target.value))}>
              {project.calendars.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.name}
                  {item.id === project.calendar_id ? m.projectCalendarMark : ""}
                </option>
              ))}
            </select>
            <Button
              Icon={CalendarPlus}
              onClick={() => {
                void onCreateCalendar().then((id) => {
                  if (id != null) setEditingId(id);
                });
              }}
            >
              {m.newCalendarButton}
            </Button>
          </div>
          <CalendarEditor calendar={calendar} send={send} />
        </>
      )}
    </Modal>
  );
}

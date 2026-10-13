import { CalendarPlus, Check, X } from "lucide-react";
import { useState } from "react";

import { useMessages } from "../i18n";
import type { Command, Project, Snapshot } from "../types";
import { Button } from "./Button";
import { CalendarEditor } from "./CalendarEditor";
import { Modal } from "./Modal";
import { workingTimeMessages } from "./WorkingTimeDialog.i18n";

/**
 * Every calendar of the project, one at a time, like Project's "Change Working Time".
 * A transaction like Thông tin dự án: edits apply as they are made, Lưu keeps them and
 * Hủy (✕, Escape) undoes back to the state the dialog opened with.
 */
export function WorkingTimeDialog({
  project,
  initialId,
  changed,
  onSave,
  onCancel,
  send,
  onCreateCalendar,
}: {
  project: Project;
  /** The calendar shown first. */
  initialId: number;
  changed: boolean;
  onSave: () => void;
  onCancel: () => void;
  send: (command: Command) => Promise<Snapshot | void>;
  onCreateCalendar: () => Promise<number | null>;
}) {
  const m = useMessages(workingTimeMessages);
  const [calendarId, setCalendarId] = useState(initialId);
  const calendar =
    project.calendars.find((item) => item.id === calendarId) ??
    project.calendars.find((item) => item.id === project.calendar_id) ??
    project.calendars[0];
  return (
    <Modal
      title={m.title}
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
      <label htmlFor="working-time-calendar">{m.calendar}</label>
      <div className="row">
        <select
          id="working-time-calendar"
          value={calendar?.id}
          onChange={(event) => setCalendarId(Number(event.target.value))}
        >
          {project.calendars.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name}
              {item.id === project.calendar_id ? m.projectMark : ""}
            </option>
          ))}
        </select>
        <Button
          Icon={CalendarPlus}
          onClick={() => {
            void onCreateCalendar().then((id) => {
              if (id != null) setCalendarId(id);
            });
          }}
        >
          {m.newCalendar}
        </Button>
      </div>
      {calendar && <CalendarEditor key={calendar.id} calendar={calendar} calendars={project.calendars} send={send} />}
    </Modal>
  );
}

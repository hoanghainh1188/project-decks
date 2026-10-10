import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  applyCommand,
  errorMessage,
  exportMspdi,
  importMspdi,
  inTauri,
  loadState,
  newProject,
  onMenu,
  openProject,
  redo,
  saveProject,
  saveProjectAs,
  undo,
} from "./api";
import { GanttChart } from "./components/GanttChart";
import { ResourceSheet } from "./components/ResourceSheet";
import { TaskSheet } from "./components/TaskSheet";
import { TaskUsage } from "./components/TaskUsage";
import { formatDateKey, formatDuration, formatInstant, formatMoney, parseDuration, parseInstant } from "./format";
import type {
  Accrual,
  Command,
  ConstraintType,
  Project,
  Resource,
  Scale,
  Snapshot,
  Task,
  TaskType,
  ViewId,
} from "./types";
import "./styles.css";

const CONSTRAINTS: { id: ConstraintType; label: string }[] = [
  { id: "asap", label: "Sớm nhất" },
  { id: "alap", label: "Muộn nhất" },
  { id: "snet", label: "Bắt đầu không sớm hơn" },
  { id: "snlt", label: "Bắt đầu không muộn hơn" },
  { id: "fnet", label: "Kết thúc không sớm hơn" },
  { id: "fnlt", label: "Kết thúc không muộn hơn" },
  { id: "mso", label: "Phải bắt đầu vào" },
  { id: "mfo", label: "Phải kết thúc vào" },
];

export default function App() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [view, setView] = useState<ViewId>("gantt");
  const [scale, setScale] = useState<Scale>("day");
  const [selected, setSelected] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [infoOpen, setInfoOpen] = useState(false);
  const [baselineOpen, setBaselineOpen] = useState(false);
  const sheetRef = useRef<HTMLDivElement>(null);
  const ganttRef = useRef<HTMLDivElement>(null);
  const syncing = useRef(false);
  const lastAction = useRef(0);

  const run = useCallback(async (work: () => Promise<Snapshot | void>) => {
    if (!inTauri()) {
      setError("Mở ProjectDesk bằng pnpm tauri dev để dùng lịch và lưu tệp.");
      return;
    }
    try {
      const next = await work();
      if (next) setSnap(next);
      setError(null);
    } catch (err) {
      setError(errorMessage(err));
    }
  }, []);

  const send = useCallback((command: Command) => run(() => applyCommand(command)), [run]);

  const menuRef = useRef<(id: string) => void>(() => {});

  const project = snap?.project;
  const task = project?.tasks.find((item) => item.uid === selected) ?? null;

  const handleMenu = useCallback(
    (id: string) => {
      if (id === "new") void run(newProject);
      if (id === "open") void run(openProject);
      if (id === "save") void run(saveProject);
      if (id === "save-as") void run(saveProjectAs);
      if (id === "import-xml") void run(importMspdi);
      if (id === "export-xml") void run(async () => {
        await exportMspdi();
      });
      if (id === "undo") void run(undo);
      if (id === "redo") void run(redo);
      if (id === "view-gantt") setView("gantt");
      if (id === "view-tracking") setView("tracking");
      if (id === "view-resources") setView("resources");
      if (id === "view-usage") setView("usage");
      if (id === "scale-day") setScale("day");
      if (id === "scale-week") setScale("week");
      if (id === "scale-month") setScale("month");
      if (id === "project-info") setInfoOpen(true);
      if (id === "baseline") setBaselineOpen(true);
      if (id === "level") void send({ type: "level_resources" });
      if (id === "clear-level") void send({ type: "clear_leveling" });
      if (!task) return;
      if (id === "indent") void send({ type: "indent", uid: task.uid });
      if (id === "outdent") void send({ type: "outdent", uid: task.uid });
      if (id === "delete") void send({ type: "delete_task", uid: task.uid });
    },
    [run, send, task],
  );
  menuRef.current = handleMenu;

  useEffect(() => {
    if (!inTauri()) {
      setError("Mở ProjectDesk bằng pnpm tauri dev để dùng lịch và lưu tệp.");
      return;
    }
    void run(() => loadState());
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void onMenu((id) => {
      const now = Date.now();
      if (now - lastAction.current < 40) return;
      lastAction.current = now;
      menuRef.current(id);
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [run]);

  useEffect(() => {
    if (!project || project.tasks.length === 0) return;
    if (selected != null && project.tasks.some((item) => item.uid === selected)) return;
    setSelected(project.tasks[0].uid);
  }, [project, selected]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const key = event.key.toLowerCase();
      const meta = event.metaKey || event.ctrlKey;
      if (!meta) return;
      const id =
        key === "z" && event.shiftKey
          ? "redo"
          : key === "z"
            ? "undo"
            : key === "s" && event.shiftKey
              ? "save-as"
              : key === "s"
                ? "save"
                : key === "o"
                  ? "open"
                  : key === "n"
                    ? "new"
                    : key === "]"
                      ? "indent"
                      : key === "["
                        ? "outdent"
                        : "";
      if (!id) return;
      event.preventDefault();
      const now = Date.now();
      if (now - lastAction.current < 40) return;
      lastAction.current = now;
      handleMenu(id);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [handleMenu]);

  const criticalCount = useMemo(
    () => project?.tasks.filter((item) => item.critical && !item.summary).length ?? 0,
    [project],
  );
  const finish = project?.tasks.reduce((max, item) => Math.max(max, item.finish), 0) ?? 0;

  function syncScroll(source: "sheet" | "gantt") {
    if (syncing.current) return;
    const from = source === "sheet" ? sheetRef.current : ganttRef.current;
    const to = source === "sheet" ? ganttRef.current : sheetRef.current;
    if (!from || !to) return;
    syncing.current = true;
    to.scrollTop = from.scrollTop;
    syncing.current = false;
  }

  return (
    <div className="app">
      <header className="toolbar">
        <strong>ProjectDesk</strong>
        <button type="button" onClick={() => send({ type: "add_task", name: "", after_uid: selected })}>Thêm việc</button>
        <button type="button" onClick={() => send({ type: "add_milestone", name: "Mốc", after_uid: selected })}>Mốc</button>
        <button type="button" disabled={!task} onClick={() => task && send({ type: "indent", uid: task.uid })}>Thụt vào</button>
        <button type="button" disabled={!task} onClick={() => task && send({ type: "outdent", uid: task.uid })}>Thụt ra</button>
        <button type="button" disabled={!task} onClick={() => task && send({ type: "delete_task", uid: task.uid })}>Xóa</button>
        <button type="button" disabled={!task} onClick={() => linkAbove(project, task, send)}>Nối FS</button>
        <span className="spacer" />
        <div className="segmented">
          <button type="button" className={view === "gantt" ? "on" : ""} onClick={() => setView("gantt")}>Gantt</button>
          <button type="button" className={view === "tracking" ? "on" : ""} onClick={() => setView("tracking")}>Theo dõi</button>
          <button type="button" className={view === "resources" ? "on" : ""} onClick={() => setView("resources")}>Nguồn lực</button>
          <button type="button" className={view === "usage" ? "on" : ""} onClick={() => setView("usage")}>Sử dụng</button>
        </div>
        {(view === "gantt" || view === "tracking") && (
          <div className="segmented">
            <button type="button" className={scale === "day" ? "on" : ""} onClick={() => setScale("day")}>Ngày</button>
            <button type="button" className={scale === "week" ? "on" : ""} onClick={() => setScale("week")}>Tuần</button>
            <button type="button" className={scale === "month" ? "on" : ""} onClick={() => setScale("month")}>Tháng</button>
          </div>
        )}
      </header>
      {error && <div className="banner">{error}</div>}
      <main className="workspace">
        {project && (view === "gantt" || view === "tracking") && (
          <>
            <TaskSheet
              tasks={project.tasks}
              calendars={project.calendars}
              assignments={project.assignments}
              selected={selected}
              tracking={view === "tracking"}
              bodyRef={sheetRef}
              onSelect={setSelected}
              onRename={(uid, name) => send({ type: "rename_task", uid, name })}
              onDuration={(uid, minutes) => send({ type: "set_duration", uid, minutes })}
              onStart={(uid, start) => send({ type: "move_task", uid, start })}
              onPredecessors={(uid, text) => send({ type: "set_predecessors", uid, text })}
              onPercent={(uid, percent) => send({ type: "set_percent_complete", uid, percent })}
              onRemaining={(uid, minutes) => send({ type: "set_remaining_duration", uid, minutes })}
              onActualStart={(uid, start) => send({ type: "set_actual_start", uid, start })}
              onActualFinish={(uid, finish) => send({ type: "set_actual_finish", uid, finish })}
              onRemainingWork={(uid, minutes) => send({ type: "set_remaining_work", uid, minutes })}
              onOvertime={(uid, minutes) => send({ type: "set_overtime_work", uid, minutes })}
              onCalendar={(uid, calendarId) => send({ type: "set_task_calendar", uid, calendar_id: calendarId })}
              onScroll={() => syncScroll("sheet")}
            />
            <GanttChart
              project={project}
              scale={scale}
              selected={selected}
              tracking={view === "tracking"}
              bodyRef={ganttRef}
              onSelect={setSelected}
              onMove={(uid, start) => send({ type: "move_task", uid, start })}
              onResize={(uid, finish) => send({ type: "resize_task", uid, finish })}
              onLink={(from, to) => send({ type: "add_dependency", predecessor_uid: from, successor_uid: to, kind: "fs", lag: 0 })}
              onScroll={() => syncScroll("gantt")}
            />
          </>
        )}
        {project && view === "resources" && (
          <ResourceSheet
            resources={project.resources}
            calendars={project.calendars}
            overallocations={project.overallocations}
            onAdd={(kind) => send({ type: "add_resource", name: "", kind })}
            onChange={(resource) => send(resourceCommand(resource))}
            onDelete={(uid) => send({ type: "delete_resource", uid })}
          />
        )}
        {project && view === "usage" && <TaskUsage project={project} />}
      </main>
      {project && task && <Inspector project={project} task={task} send={send} />}
      <footer className="status">
        <span>{snap?.dirty ? "Chưa lưu" : "Đã lưu"}</span>
        <span>{snap?.path ?? "Chưa có tệp"}</span>
        <span>{project?.tasks.length ?? 0} công việc</span>
        <span>{criticalCount} việc găng</span>
        <span>{finish ? `Kết thúc ${formatInstant(finish)}` : ""}</span>
        <span>{project ? project.overallocations.length : 0} khoảng quá tải</span>
      </footer>
      {infoOpen && project && (
        <ProjectDialog
          project={project}
          onClose={() => setInfoOpen(false)}
          send={send}
        />
      )}
      {baselineOpen && (
        <BaselineDialog
          onClose={() => setBaselineOpen(false)}
          onSet={(number) => send({ type: "set_baseline", number })}
          onClear={(number) => send({ type: "clear_baseline", number })}
        />
      )}
    </div>
  );
}

function commitOptionalDate(
  text: string,
  current: number | null,
  send: (value: number | null) => void,
) {
  const trimmed = text.trim();
  if (!trimmed) {
    if (current != null) send(null);
    return;
  }
  const parsed = parseInstant(trimmed);
  if (parsed !== null && parsed !== current) send(parsed);
}

function linkAbove(
  project: Project | undefined,
  task: Task | null,
  send: (command: Command) => void,
) {
  if (!project || !task) return;
  const index = project.tasks.findIndex((item) => item.uid === task.uid);
  if (index <= 0) return;
  send({
    type: "add_dependency",
    predecessor_uid: project.tasks[index - 1].uid,
    successor_uid: task.uid,
    kind: "fs",
    lag: 0,
  });
}

function resourceCommand(resource: Resource): Command {
  return {
    type: "update_resource",
    uid: resource.uid,
    name: resource.name,
    kind: resource.kind,
    max_units: resource.max_units,
    calendar_id: resource.calendar_id,
    standard_rate: resource.standard_rate,
    overtime_rate: resource.overtime_rate,
    cost_per_use: resource.cost_per_use,
  };
}

function Inspector({
  project,
  task,
  send,
}: {
  project: Project;
  task: Task;
  send: (command: Command) => void;
}) {
  const needsDate = task.constraint_type !== "asap" && task.constraint_type !== "alap";
  const overtime = project.assignments
    .filter((item) => item.task_uid === task.uid)
    .reduce((sum, item) => sum + item.overtime_work, 0);
  return (
    <section className="inspector">
      <div>
        <label>Bắt đầu thực</label>
        <input
          disabled={task.summary}
          placeholder="dd/mm/yyyy hh:mm"
          defaultValue={task.actual_start ? formatInstant(task.actual_start) : ""}
          key={`as-${task.uid}-${task.actual_start ?? "none"}`}
          onBlur={(event) =>
            commitOptionalDate(event.target.value, task.actual_start, (start) =>
              send({ type: "set_actual_start", uid: task.uid, start }),
            )
          }
        />
      </div>
      <div>
        <label>Kết thúc thực</label>
        <input
          disabled={task.summary}
          placeholder="dd/mm/yyyy hh:mm"
          defaultValue={task.actual_finish ? formatInstant(task.actual_finish) : ""}
          key={`af-${task.uid}-${task.actual_finish ?? "none"}`}
          onBlur={(event) =>
            commitOptionalDate(event.target.value, task.actual_finish, (finish) =>
              send({ type: "set_actual_finish", uid: task.uid, finish }),
            )
          }
        />
      </div>
      <div>
        <label>Việc còn lại</label>
        <input
          disabled={task.summary}
          defaultValue={formatDuration(task.remaining_work)}
          key={`rw-${task.uid}-${task.remaining_work}`}
          onBlur={(event) => {
            const minutes = parseDuration(event.target.value);
            if (minutes !== null && minutes !== task.remaining_work) {
              send({ type: "set_remaining_work", uid: task.uid, minutes });
            }
          }}
        />
      </div>
      <div>
        <label>Giờ làm thêm</label>
        <input
          disabled={task.summary}
          defaultValue={formatDuration(overtime)}
          key={`ot-${task.uid}-${overtime}`}
          onBlur={(event) => {
            const minutes = parseDuration(event.target.value);
            if (minutes !== null && minutes !== overtime) {
              send({ type: "set_overtime_work", uid: task.uid, minutes });
            }
          }}
        />
      </div>
      <div>
        <label>Lịch công việc</label>
        <select
          value={task.calendar_id ?? ""}
          onChange={(event) =>
            send({
              type: "set_task_calendar",
              uid: task.uid,
              calendar_id: event.target.value ? Number(event.target.value) : null,
            })
          }
        >
          <option value="">Lịch dự án</option>
          {project.calendars.map((calendar) => (
            <option key={calendar.id} value={calendar.id}>{calendar.name}</option>
          ))}
        </select>
      </div>
      <div>
        <label>Ràng buộc</label>
        <select
          value={task.constraint_type}
          onChange={(event) =>
            send({
              type: "set_constraint",
              uid: task.uid,
              constraint_type: event.target.value as ConstraintType,
              date: task.constraint_date,
            })
          }
        >
          {CONSTRAINTS.map((item) => (
            <option key={item.id} value={item.id}>{item.label}</option>
          ))}
        </select>
      </div>
      {needsDate && (
        <div>
          <label>Ngày ràng buộc</label>
          <input
            defaultValue={task.constraint_date ? formatInstant(task.constraint_date) : ""}
            key={`${task.uid}-${task.constraint_date ?? 0}`}
            onBlur={(event) => {
              const date = parseInstant(event.target.value);
              send({
                type: "set_constraint",
                uid: task.uid,
                constraint_type: task.constraint_type,
                date,
              });
            }}
          />
        </div>
      )}
      <div>
        <label>Kiểu công việc</label>
        <select
          value={task.task_type}
          onChange={(event) =>
            send({
              type: "set_task_type",
              uid: task.uid,
              task_type: event.target.value as TaskType,
              effort_driven: task.effort_driven,
            })
          }
        >
          <option value="fixed_units">Đơn vị cố định</option>
          <option value="fixed_duration">Thời lượng cố định</option>
          <option value="fixed_work">Công việc cố định</option>
        </select>
      </div>
      <label className="check">
        <input
          type="checkbox"
          checked={task.effort_driven}
          onChange={(event) =>
            send({
              type: "set_task_type",
              uid: task.uid,
              task_type: task.task_type,
              effort_driven: event.target.checked,
            })
          }
        />
        Hướng theo nỗ lực
      </label>
      <label className="check">
        <input
          type="checkbox"
          checked={task.manual}
          onChange={(event) =>
            send({
              type: "set_manual",
              uid: task.uid,
              manual: event.target.checked,
              start: task.start,
              finish: task.finish,
            })
          }
        />
        Lên lịch tay
      </label>
      <div>
        <label>Ưu tiên</label>
        <input
          defaultValue={task.priority}
          key={`p-${task.uid}-${task.priority}`}
          onBlur={(event) => send({ type: "set_priority", uid: task.uid, priority: Number(event.target.value) || 0 })}
        />
      </div>
      <div>
        <label>Chi phí cố định</label>
        <input
          defaultValue={task.fixed_cost}
          key={`c-${task.uid}-${task.fixed_cost}`}
          onBlur={(event) =>
            send({
              type: "set_fixed_cost",
              uid: task.uid,
              cost: Number(event.target.value) || 0,
              accrual: task.fixed_cost_accrual,
            })
          }
        />
      </div>
      <div>
        <label>Cộng dồn</label>
        <select
          value={task.fixed_cost_accrual}
          onChange={(event) =>
            send({
              type: "set_fixed_cost",
              uid: task.uid,
              cost: task.fixed_cost,
              accrual: event.target.value as Accrual,
            })
          }
        >
          <option value="start">Đầu</option>
          <option value="prorated">Phân bổ</option>
          <option value="end">Cuối</option>
        </select>
      </div>
      <div>
        <label>Ghi chú</label>
        <input
          defaultValue={task.notes}
          key={`n-${task.uid}-${task.notes}`}
          onBlur={(event) => send({ type: "set_notes", uid: task.uid, notes: event.target.value })}
        />
      </div>
      <div className="assign">
        <label>Gán nguồn lực</label>
        <select
          defaultValue=""
          onChange={(event) => {
            const resource = Number(event.target.value);
            if (resource) send({ type: "assign_resource", task_uid: task.uid, resource_uid: resource, units: 1 });
            event.target.value = "";
          }}
        >
          <option value="">Chọn…</option>
          {project.resources.map((resource) => (
            <option key={resource.uid} value={resource.uid}>{resource.name}</option>
          ))}
        </select>
      </div>
      <p className="meta">
        Công việc {formatDuration(task.work)} · Chi phí {formatMoney(task.cost)} · Thực tế {formatMoney(task.actual_cost)} · Trễ cân bằng {formatDuration(task.leveling_delay)}
        {task.conflict ? " · Vi phạm ràng buộc" : ""}
      </p>
    </section>
  );
}

function ProjectDialog({
  project,
  onClose,
  send,
}: {
  project: Project;
  onClose: () => void;
  send: (command: Command) => void;
}) {
  const holidays = Object.entries(project.calendars.find((item) => item.id === project.calendar_id)?.exceptions ?? {})
    .filter(([, ranges]) => ranges.length === 0)
    .map(([date]) => date);
  return (
    <div className="modal">
      <form
        className="dialog"
        onSubmit={(event) => {
          event.preventDefault();
          onClose();
        }}
      >
        <h2>Thông tin dự án</h2>
        <label>Tên</label>
        <input
          defaultValue={project.name}
          onBlur={(event) => send({ type: "rename_project", name: event.target.value })}
        />
        <label>Ngày bắt đầu</label>
        <input
          defaultValue={formatInstant(project.start)}
          onBlur={(event) => {
            const start = parseInstant(event.target.value);
            if (start !== null) send({ type: "set_project_start", start });
          }}
        />
        <label>Ngày trạng thái</label>
        <input
          defaultValue={project.status_date ? formatInstant(project.status_date) : ""}
          onBlur={(event) => send({ type: "set_status_date", date: parseInstant(event.target.value) })}
        />
        <label>Ngày nghỉ (dd/mm/yyyy)</label>
        <input
          placeholder="02/09/2026"
          onKeyDown={(event) => {
            if (event.key !== "Enter") return;
            event.preventDefault();
            const date = formatDateKey(event.currentTarget.value);
            if (date) {
              send({ type: "add_holiday", date });
              event.currentTarget.value = "";
            }
          }}
        />
        <ul>
          {holidays.map((date) => (
            <li key={date}>
              {date}
              <button type="button" onClick={() => send({ type: "remove_holiday", date })}>Bỏ</button>
            </li>
          ))}
        </ul>
        <button type="submit">Đóng</button>
      </form>
    </div>
  );
}

function BaselineDialog({
  onClose,
  onSet,
  onClear,
}: {
  onClose: () => void;
  onSet: (number: number) => void;
  onClear: (number: number) => void;
}) {
  const [number, setNumber] = useState(0);
  return (
    <div className="modal">
      <div className="dialog">
        <h2>Đường cơ sở</h2>
        <label>Mốc 0–10</label>
        <input type="number" min={0} max={10} value={number} onChange={(event) => setNumber(Number(event.target.value))} />
        <div className="row">
          <button type="button" onClick={() => onSet(Math.min(10, Math.max(0, number)))}>Đặt</button>
          <button type="button" onClick={() => onClear(Math.min(10, Math.max(0, number)))}>Xóa</button>
          <button type="button" onClick={onClose}>Đóng</button>
        </div>
      </div>
    </div>
  );
}

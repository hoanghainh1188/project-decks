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
  savePdf,
  saveProject,
  saveProjectAs,
  undo,
} from "./api";
import { CalendarEditor } from "./components/CalendarEditor";
import { DateTimeField } from "./components/DateTimeField";
import { GanttChart } from "./components/GanttChart";
import { RecurringDialog } from "./components/RecurringDialog";
import { ResourceSheet } from "./components/ResourceSheet";
import { TaskSheet } from "./components/TaskSheet";
import { TaskUsage } from "./components/TaskUsage";
import { formatDuration, formatInstant, formatMoney, parseDuration } from "./format";
import { buildGanttPdf, pdfSuggestedName } from "./ganttPdf";
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
import { buildViewRows, type FilterMode, type GroupMode, type HighlightMode } from "./viewRows";
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
  const [calendarEditId, setCalendarEditId] = useState<number | null>(null);
  const [rowFilter, setRowFilter] = useState<FilterMode>("none");
  const [filterResource, setFilterResource] = useState<number | null>(null);
  const [rowGroup, setRowGroup] = useState<GroupMode>("none");
  const [rowHighlight, setRowHighlight] = useState<HighlightMode>("none");
  // null: closed; 0: create a new series; otherwise the summary UID being edited.
  const [seriesDialog, setSeriesDialog] = useState<number | null>(null);
  const sheetRef = useRef<HTMLDivElement>(null);
  const ganttRef = useRef<HTMLDivElement>(null);
  const exportPdfRef = useRef<() => void>(() => {});
  const syncing = useRef(false);
  const lastAction = useRef(0);
  const fileDialogBusyRef = useRef(false);
  const [fileDialogBusy, setFileDialogBusy] = useState(false);

  const run = useCallback(async (work: () => Promise<Snapshot | void>): Promise<Snapshot | void> => {
    if (!inTauri()) {
      setError("Mở ProjectDesk bằng pnpm tauri dev để dùng lịch và lưu tệp.");
      return;
    }
    try {
      const next = await work();
      if (next) setSnap(next);
      setError(null);
      return next;
    } catch (err) {
      setError(errorMessage(err));
    }
  }, []);

  const send = useCallback((command: Command) => run(() => applyCommand(command)), [run]);

  // One native file chooser at a time. A second click is ignored so it cannot
  // open another dialog while the first still owns the macOS run loop.
  const startFileDialog = useCallback((work: () => Promise<Snapshot | void>) => {
    if (fileDialogBusyRef.current) return;
    if (!inTauri()) {
      setError("Mở ProjectDesk bằng pnpm tauri dev để dùng lịch và lưu tệp.");
      return;
    }
    fileDialogBusyRef.current = true;
    setFileDialogBusy(true);
    void (async () => {
      try {
        await run(work);
      } finally {
        fileDialogBusyRef.current = false;
        setFileDialogBusy(false);
      }
    })();
  }, [run]);

  const createCalendar = useCallback(
    async (name: string, assign: (id: number) => Command | null, openEditor: boolean) => {
      const before = snap?.project.calendars.map((calendar) => calendar.id) ?? [];
      const result = await send({ type: "create_calendar", name });
      if (!result) return null;
      const created = result.project.calendars.find((calendar) => !before.includes(calendar.id));
      if (!created) return null;
      const command = assign(created.id);
      if (command) await send(command);
      if (openEditor) setCalendarEditId(created.id);
      return created.id;
    },
    [send, snap],
  );

  const menuRef = useRef<(id: string) => void>(() => {});

  const project = snap?.project;
  const task = project?.tasks.find((item) => item.uid === selected) ?? null;

  const handleMenu = useCallback(
    (id: string) => {
      if (id === "new") void run(newProject);
      if (id === "open") startFileDialog(openProject);
      if (id === "save") startFileDialog(saveProject);
      if (id === "save-as") startFileDialog(saveProjectAs);
      if (id === "import-xml") startFileDialog(importMspdi);
      if (id === "export-xml") startFileDialog(async () => {
        await exportMspdi();
      });
      if (id === "export-pdf") exportPdfRef.current();
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
      if (id === "recurring") setSeriesDialog(0);
      if (!task) return;
      if (id === "indent") void send({ type: "indent", uid: task.uid });
      if (id === "outdent") void send({ type: "outdent", uid: task.uid });
      if (id === "delete") void send({ type: "delete_task", uid: task.uid });
    },
    [run, send, startFileDialog, task],
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
    if (!project || filterResource == null) return;
    if (project.resources.some((resource) => resource.uid === filterResource)) return;
    setFilterResource(project.resources[0]?.uid ?? null);
  }, [project, filterResource]);

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
  const rows = useMemo(
    () => (project ? buildViewRows(project, rowFilter, filterResource, rowGroup, rowHighlight) : []),
    [project, rowFilter, filterResource, rowGroup, rowHighlight],
  );
  exportPdfRef.current = () => {
    if (!project) return;
    startFileDialog(async () => {
      const bytes = buildGanttPdf({
        project,
        rows,
        scale,
        selected,
        tracking: view === "tracking",
      });
      await savePdf(bytes, pdfSuggestedName(project.name));
    });
  };
  const finish = project?.tasks.reduce((max, item) => Math.max(max, item.finish), 0) ?? 0;
  const editingCalendar = project?.calendars.find((calendar) => calendar.id === calendarEditId) ?? null;

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
        <button type="button" disabled={!project} onClick={() => setSeriesDialog(0)}>Công việc lặp</button>
        <button type="button" disabled={!task} onClick={() => task && send({ type: "indent", uid: task.uid })}>Thụt vào</button>
        <button type="button" disabled={!task} onClick={() => task && send({ type: "outdent", uid: task.uid })}>Thụt ra</button>
        <button type="button" disabled={!task} onClick={() => task && send({ type: "delete_task", uid: task.uid })}>Xóa</button>
        <button type="button" disabled={!task} onClick={() => linkAbove(project, task, send)}>Nối FS</button>
        <button type="button" onClick={() => setInfoOpen(true)}>Lịch</button>
        <button type="button" disabled={!project || fileDialogBusy} onClick={() => exportPdfRef.current()}>Xuất PDF</button>
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
      {project && (view === "gantt" || view === "tracking") && (
        <div className="viewbar">
          <label className="view-control">
            Lọc
            <select
              aria-label="Lọc"
              value={rowFilter}
              onChange={(event) => {
                const next = event.target.value as FilterMode;
                setRowFilter(next);
                if (next === "resource" && filterResource == null) {
                  setFilterResource(project.resources[0]?.uid ?? null);
                }
              }}
            >
              <option value="none">Không lọc</option>
              <option value="critical">Đường găng</option>
              <option value="resource">Theo người</option>
              <option value="late">Trễ hạn</option>
            </select>
          </label>
          {rowFilter === "resource" && (
            <select
              aria-label="Người"
              value={filterResource ?? ""}
              onChange={(event) => setFilterResource(event.target.value ? Number(event.target.value) : null)}
            >
              {project.resources.length === 0 && <option value="">Chưa có nguồn lực</option>}
              {project.resources.map((resource) => (
                <option key={resource.uid} value={resource.uid}>
                  {resource.name.trim() || `NL ${resource.uid}`}
                </option>
              ))}
            </select>
          )}
          <label className="view-control">
            Nhóm
            <select
              aria-label="Nhóm"
              value={rowGroup}
              onChange={(event) => setRowGroup(event.target.value as GroupMode)}
            >
              <option value="none">Không nhóm</option>
              <option value="resource">Theo người</option>
              <option value="summary">Theo summary</option>
            </select>
          </label>
          <label className="view-control">
            Tô sáng
            <select
              aria-label="Tô sáng"
              value={rowHighlight}
              onChange={(event) => setRowHighlight(event.target.value as HighlightMode)}
            >
              <option value="none">Không</option>
              <option value="critical">Đường găng</option>
              <option value="late">Trễ hạn</option>
            </select>
          </label>
        </div>
      )}
      <main className="workspace">
        {project && (view === "gantt" || view === "tracking") && (
          <>
            <TaskSheet
              rows={rows}
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
              onEditSeries={(uid) => {
                setSelected(uid);
                setSeriesDialog(uid);
              }}
              onScroll={() => syncScroll("sheet")}
            />
            <GanttChart
              project={project}
              rows={rows}
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
            onEditCalendar={setCalendarEditId}
            onCreateCalendar={(resource) => {
              const name = resource.name.trim() ? `Lịch ${resource.name.trim()}` : "Lịch nguồn lực";
              void createCalendar(name, (id) => resourceCommand({ ...resource, calendar_id: id }), true);
            }}
          />
        )}
        {project && view === "usage" && <TaskUsage project={project} />}
      </main>
      {project && task && (
        <Inspector
          project={project}
          task={task}
          send={send}
          onEditSeries={setSeriesDialog}
          onEditCalendar={setCalendarEditId}
          onCreateCalendar={() => {
            const name = task.name.trim() ? `Lịch ${task.name.trim()}` : "Lịch công việc";
            void createCalendar(name, (id) => ({ type: "set_task_calendar", uid: task.uid, calendar_id: id }), true);
          }}
        />
      )}
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
          onCreateCalendar={() => createCalendar("Lịch mới", () => null, false)}
        />
      )}
      {editingCalendar && (
        <div className="modal">
          <div className="dialog wide">
            <h2>Sửa lịch</h2>
            <CalendarEditor calendar={editingCalendar} send={send} />
            <button type="button" onClick={() => setCalendarEditId(null)}>Đóng</button>
          </div>
        </div>
      )}
      {seriesDialog != null && project && (
        <RecurringDialog
          key={seriesDialog}
          project={project}
          initial={project.tasks.find((item) => item.uid === seriesDialog)?.recurrence?.spec ?? null}
          defaultStart={task && !task.summary ? task.start : project.start}
          onClose={() => setSeriesDialog(null)}
          onSubmit={(spec) => {
            const series = project.tasks.find((item) => item.uid === seriesDialog);
            return series?.recurrence
              ? send({ type: "update_recurring_task", uid: series.uid, spec })
              : send({ type: "create_recurring_task", after_uid: selected, spec });
          }}
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
  onEditSeries,
  onEditCalendar,
  onCreateCalendar,
}: {
  project: Project;
  task: Task;
  send: (command: Command) => void;
  onEditSeries: (uid: number) => void;
  onEditCalendar: (calendarId: number) => void;
  onCreateCalendar: () => void;
}) {
  const needsDate = task.constraint_type !== "asap" && task.constraint_type !== "alap";
  const overtime = project.assignments
    .filter((item) => item.task_uid === task.uid)
    .reduce((sum, item) => sum + item.overtime_work, 0);
  const series = task.series_uid != null ? project.tasks.find((item) => item.uid === task.series_uid) : null;
  return (
    <section className="inspector">
      {(task.recurrence || series) && (
        <div>
          <label>Công việc lặp</label>
          <button type="button" onClick={() => onEditSeries(task.recurrence ? task.uid : (series?.uid ?? task.uid))}>
            Sửa cả chuỗi…
          </button>
          {series && (
            <p className="meta">
              {task.detached ? "Lần này đã sửa riêng; sinh lại chuỗi sẽ giữ nguyên." : `Một lần của chuỗi “${series.name}”.`}
            </p>
          )}
        </div>
      )}
      <div>
        <label>Bắt đầu thực</label>
        <DateTimeField
          optional
          disabled={task.summary}
          ariaLabel="Bắt đầu thực"
          value={task.actual_start}
          onCommit={(start) => send({ type: "set_actual_start", uid: task.uid, start })}
        />
      </div>
      <div>
        <label>Kết thúc thực</label>
        <DateTimeField
          optional
          disabled={task.summary}
          ariaLabel="Kết thúc thực"
          value={task.actual_finish}
          onCommit={(finish) => send({ type: "set_actual_finish", uid: task.uid, finish })}
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
        <div className="row">
          <button
            type="button"
            disabled={task.summary || task.calendar_id == null}
            onClick={() => task.calendar_id != null && onEditCalendar(task.calendar_id)}
          >
            Sửa lịch
          </button>
          <button type="button" disabled={task.summary} onClick={onCreateCalendar}>
            Tạo lịch riêng
          </button>
        </div>
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
          <DateTimeField
            ariaLabel="Ngày ràng buộc"
            value={task.constraint_date}
            onCommit={(date) =>
              send({
                type: "set_constraint",
                uid: task.uid,
                constraint_type: task.constraint_type,
                date,
              })
            }
          />
        </div>
      )}
      <div>
        <label>Hạn chót</label>
        <DateTimeField
          optional
          ariaLabel="Hạn chót"
          value={task.deadline}
          onCommit={(date) => send({ type: "set_deadline", uid: task.uid, date })}
        />
      </div>
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
        <label>WBS</label>
        <input readOnly value={task.wbs} />
      </div>
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
      <div>
        <label>Công việc</label>
        <input
          disabled={task.summary}
          defaultValue={formatDuration(task.work)}
          key={`work-${task.uid}-${task.work}`}
          onBlur={(event) => {
            const minutes = parseDuration(event.target.value);
            if (minutes !== null && minutes !== task.work) {
              send({ type: "set_work", uid: task.uid, minutes });
            }
          }}
        />
      </div>
      <AssignmentEditor project={project} task={task} send={send} />
      <p className="meta">
        Chi phí {formatMoney(task.cost)} · Thực tế {formatMoney(task.actual_cost)} · Trễ cân bằng {formatDuration(task.leveling_delay)}
        {task.late ? " · Trễ hạn" : ""}
        {task.conflict ? " · Vi phạm ràng buộc" : ""}
      </p>
    </section>
  );
}

function AssignmentEditor({
  project,
  task,
  send,
}: {
  project: Project;
  task: Task;
  send: (command: Command) => void;
}) {
  const [units, setUnits] = useState("1");
  const rows = project.assignments.filter((item) => item.task_uid === task.uid);
  return (
    <div className="assign">
      <label>Gán nguồn lực</label>
      {rows.length > 0 && (
        <ul className="assign-list">
          {rows.map((assignment) => {
            const name =
              project.resources.find((resource) => resource.uid === assignment.resource_uid)?.name ??
              `NL ${assignment.resource_uid}`;
            return (
              <li key={assignment.uid}>
                <span>{name}</span>
                <label>
                  Đơn vị
                  <input
                    aria-label={`Đơn vị ${name}`}
                    disabled={task.summary}
                    defaultValue={String(assignment.units)}
                    key={`${assignment.uid}-${assignment.units}`}
                    onBlur={(event) => {
                      const next = Number(event.target.value);
                      if (Number.isFinite(next) && next !== assignment.units) {
                        send({
                          type: "update_assignment",
                          uid: assignment.uid,
                          units: next,
                          work: null,
                          overtime_work: null,
                        });
                      }
                    }}
                  />
                </label>
                <button
                  type="button"
                  disabled={task.summary}
                  onClick={() => send({ type: "remove_assignment", uid: assignment.uid })}
                >
                  Bỏ
                </button>
              </li>
            );
          })}
        </ul>
      )}
      <div className="assign-add">
        <label>
          Đơn vị
          <input
            aria-label="Đơn vị gán"
            disabled={task.summary}
            value={units}
            onChange={(event) => setUnits(event.target.value)}
          />
        </label>
        <select
          disabled={task.summary}
          defaultValue=""
          onChange={(event) => {
            const resource = Number(event.target.value);
            const parsed = Number(units);
            if (resource) {
              send({
                type: "assign_resource",
                task_uid: task.uid,
                resource_uid: resource,
                units: Number.isFinite(parsed) ? Math.max(0, parsed) : 1,
              });
            }
            event.target.value = "";
          }}
        >
          <option value="">Chọn…</option>
          {project.resources.map((resource) => (
            <option key={resource.uid} value={resource.uid}>{resource.name}</option>
          ))}
        </select>
      </div>
    </div>
  );
}

function ProjectDialog({
  project,
  onClose,
  send,
  onCreateCalendar,
}: {
  project: Project;
  onClose: () => void;
  send: (command: Command) => Promise<Snapshot | void>;
  onCreateCalendar: () => Promise<number | null>;
}) {
  const [editingId, setEditingId] = useState(project.calendar_id);
  const calendar =
    project.calendars.find((item) => item.id === editingId) ??
    project.calendars.find((item) => item.id === project.calendar_id) ??
    project.calendars[0];
  return (
    <div className="modal">
      <div className="dialog wide">
        <h2>Thông tin dự án</h2>
        <label>Tên</label>
        <input
          defaultValue={project.name}
          onBlur={(event) => send({ type: "rename_project", name: event.target.value })}
        />
        <label>Ngày bắt đầu</label>
        <DateTimeField
          ariaLabel="Ngày bắt đầu dự án"
          value={project.start}
          onCommit={(start) => {
            if (start != null) void send({ type: "set_project_start", start });
          }}
        />
        <label>Ngày trạng thái</label>
        <DateTimeField
          optional
          ariaLabel="Ngày trạng thái"
          value={project.status_date}
          onCommit={(date) => void send({ type: "set_status_date", date })}
        />
        {calendar && (
          <>
            <label>Lịch đang sửa</label>
            <div className="row">
              <select value={calendar.id} onChange={(event) => setEditingId(Number(event.target.value))}>
                {project.calendars.map((item) => (
                  <option key={item.id} value={item.id}>
                    {item.name}
                    {item.id === project.calendar_id ? " (dự án)" : ""}
                  </option>
                ))}
              </select>
              <button
                type="button"
                onClick={() => {
                  void onCreateCalendar().then((id) => {
                    if (id != null) setEditingId(id);
                  });
                }}
              >
                Tạo lịch
              </button>
            </div>
            <CalendarEditor calendar={calendar} send={send} />
          </>
        )}
        <button type="button" onClick={onClose}>Đóng</button>
      </div>
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

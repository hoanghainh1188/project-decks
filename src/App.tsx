import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  applyCommand,
  checkAutosave,
  compactHistory,
  errorMessage,
  exportMspdi,
  importMspdi,
  inTauri,
  loadMetrics,
  loadNetworkLayout,
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
import { EvmPanel } from "./components/EvmPanel";
import { GanttChart } from "./components/GanttChart";
import { Inspector } from "./components/inspector/Inspector";
import { Modal } from "./components/Modal";
import { NetworkDiagram } from "./components/NetworkDiagram";
import { RecurringDialog, recurringCommand } from "./components/RecurringDialog";
import { ResourceSheet } from "./components/ResourceSheet";
import { SplitDialog } from "./components/SplitDialog";
import { TaskEditDialog } from "./components/TaskEditDialog";
import { TaskSheet } from "./components/TaskSheet";
import { TaskUsage } from "./components/TaskUsage";
import { Toolbar } from "./components/Toolbar";
import { ViewBar } from "./components/ViewBar";
import { formatInstant } from "./format";
import { buildGanttPdf, pdfSuggestedName } from "./ganttPdf";
import { buildNetworkPdf } from "./networkPdf";
import type {
  Command,
  DepKind,
  NetworkLayout,
  Project,
  ProjectMetrics,
  Resource,
  Scale,
  Snapshot,
  Task,
  ViewId,
} from "./types";
import { projectFinish } from "./projectFinish";
import {
  commitFocusedField,
  isEditableTarget,
  runFieldAction,
  shortcutId,
  shouldRunAction,
  trackCommittedText,
} from "./shortcuts";
import { linkFromAbove, taskAbove, toolbarStates } from "./toolbarState";
import { buildViewRows, type FilterMode, type GroupMode, type HighlightMode } from "./viewRows";
import { fileName, windowTitle } from "./windowTitle";
import "./styles.css";

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
  // UID of the task the split dialog is open for.
  const [splitDialog, setSplitDialog] = useState<number | null>(null);
  const [evmOpen, setEvmOpen] = useState(false);
  const [metrics, setMetrics] = useState<ProjectMetrics | null>(null);
  const [networkSummaries, setNetworkSummaries] = useState(false);
  const [networkLayout, setNetworkLayout] = useState<NetworkLayout | null>(null);
  const [editUid, setEditUid] = useState<number | null>(null);
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
  const editTask = project?.tasks.find((item) => item.uid === editUid) ?? null;
  const closeEdit = useCallback(() => setEditUid(null), []);

  const handleMenu = useCallback(
    (id: string) => {
      if (id === "new") startFileDialog(newProject);
      if (id === "open") startFileDialog(openProject);
      if (id === "save" || id === "save-as") {
        commitFocusedField();
        startFileDialog(id === "save" ? saveProject : saveProjectAs);
      }
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
      if (id === "view-network") setView("network");
      if (id === "view-evm") {
        setView((current) => (current === "gantt" || current === "tracking" ? current : "gantt"));
        setEvmOpen(true);
      }
      if (id === "scale-day") setScale("day");
      if (id === "scale-week") setScale("week");
      if (id === "scale-month") setScale("month");
      if (id === "project-info") setInfoOpen(true);
      if (id === "baseline") setBaselineOpen(true);
      if (id === "level") void send({ type: "level_resources" });
      if (id === "clear-level") void send({ type: "clear_leveling" });
      if (id === "reschedule-uncompleted") void send({ type: "reschedule_uncompleted" });
      if (id === "compact-history") startFileDialog(compactHistory);
      if (id === "recurring") setSeriesDialog(0);
      if (!task) return;
      // Shortcuts follow the same rules as the toolbar buttons.
      const states = toolbarStates({ snap, task, view, fileDialogBusy });
      if (id === "split-task" && states.split.enabled) setSplitDialog(task.uid);
      if (id === "indent" && states.indent.enabled) void send({ type: "indent", uid: task.uid });
      if (id === "outdent" && states.outdent.enabled) void send({ type: "outdent", uid: task.uid });
      if (id === "delete" && states.remove.enabled) void send({ type: "delete_task", uid: task.uid });
    },
    [run, send, startFileDialog, task, snap, view, fileDialogBusy],
  );
  menuRef.current = handleMenu;

  // Shortcuts and menu accelerators can both fire for one key press; act once, either on
  // the project or on the focused text field.
  const dispatchAction = useCallback(
    (id: string, focused: EventTarget | null) => {
      const now = Date.now();
      if (now - lastAction.current < 40) return;
      lastAction.current = now;
      if (shouldRunAction(id, focused)) menuRef.current(id);
      else if (isEditableTarget(focused)) runFieldAction(id, focused);
    },
    [],
  );

  useEffect(() => trackCommittedText(document), []);

  const title = windowTitle(snap);
  useEffect(() => {
    document.title = title;
    if (!inTauri()) return;
    getCurrentWindow()
      .setTitle(title)
      .catch((err) => setError(errorMessage(err)));
  }, [title]);

  useEffect(() => {
    if (!inTauri()) {
      setError("Mở ProjectDesk bằng pnpm tauri dev để dùng lịch và lưu tệp.");
      return;
    }
    // After a crash the backend offers the autosaved project back in a native dialog.
    void run(() => loadState()).then(() =>
      startFileDialog(async () => (await checkAutosave()) ?? undefined),
    );
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void onMenu((id) => dispatchAction(id, document.activeElement)).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [run, dispatchAction, startFileDialog]);

  useEffect(() => {
    if (!project || project.tasks.length === 0) return;
    if (selected != null && project.tasks.some((item) => item.uid === selected)) return;
    setSelected(project.tasks[0].uid);
  }, [project, selected]);

  // EVM and burn-down numbers are computed by the engine; refresh them after every change.
  useEffect(() => {
    if (!evmOpen || !snap || !inTauri()) return;
    let cancelled = false;
    loadMetrics()
      .then((next) => {
        if (!cancelled) setMetrics(next);
      })
      .catch((err) => {
        if (!cancelled) setError(errorMessage(err));
      });
    return () => {
      cancelled = true;
    };
  }, [evmOpen, snap]);

  useEffect(() => {
    if (!project || filterResource == null) return;
    if (project.resources.some((resource) => resource.uid === filterResource)) return;
    setFilterResource(project.resources[0]?.uid ?? null);
  }, [project, filterResource]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const id = shortcutId(event);
      if (!id) return;
      event.preventDefault();
      dispatchAction(id, event.target);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dispatchAction]);

  const criticalCount = useMemo(
    () => project?.tasks.filter((item) => item.critical && !item.summary).length ?? 0,
    [project],
  );
  const rows = useMemo(
    () => (project ? buildViewRows(project, rowFilter, filterResource, rowGroup, rowHighlight) : []),
    [project, rowFilter, filterResource, rowGroup, rowHighlight],
  );
  // The network diagram ignores grouping; filters choose the boxes and highlight tints them.
  const networkRows = useMemo(
    () => (project && view === "network" ? buildViewRows(project, rowFilter, filterResource, "none", rowHighlight) : []),
    [project, view, rowFilter, filterResource, rowHighlight],
  );
  const networkVisible = useMemo(
    () => (rowFilter === "none" ? null : networkRows.flatMap((row) => (row.kind === "task" ? [row.task.uid] : []))),
    [networkRows, rowFilter],
  );
  const networkHighlighted = useMemo(
    () => new Set(networkRows.flatMap((row) => (row.kind === "task" && row.highlighted ? [row.task.uid] : []))),
    [networkRows],
  );

  // Layout comes from the engine, which also owns the schedule; refresh it after every change.
  useEffect(() => {
    if (view !== "network" || !snap || !inTauri()) return;
    let cancelled = false;
    loadNetworkLayout({ include_summary: networkSummaries, visible_uids: networkVisible })
      .then((next) => {
        if (!cancelled) setNetworkLayout(next);
      })
      .catch((err) => {
        if (!cancelled) setError(errorMessage(err));
      });
    return () => {
      cancelled = true;
    };
  }, [view, snap, networkSummaries, networkVisible]);

  exportPdfRef.current = () => {
    if (!project) return;
    if (view === "network") {
      const layout = networkLayout;
      if (!layout) return;
      startFileDialog(async () => {
        const bytes = buildNetworkPdf({ project, layout, highlighted: networkHighlighted });
        await savePdf(bytes, pdfSuggestedName(`${project.name.trim() || "Dự án"} - sơ đồ mạng`));
      });
      return;
    }
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
  const end = project ? projectFinish(project.tasks) : null;
  const splitTask = project?.tasks.find((item) => item.uid === splitDialog) ?? null;
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
      <Toolbar
        states={toolbarStates({ snap, task, view, fileDialogBusy })}
        evmOpen={evmOpen}
        onUndo={() => void run(undo)}
        onRedo={() => void run(redo)}
        onAddTask={() => void send({ type: "add_task", name: "", after_uid: selected })}
        onAddMilestone={() => void send({ type: "add_milestone", name: "Mốc", after_uid: selected })}
        onAddRecurring={() => setSeriesDialog(0)}
        onIndent={() => task && void send({ type: "indent", uid: task.uid })}
        onOutdent={() => task && void send({ type: "outdent", uid: task.uid })}
        onLink={(kind) => linkAbove(project, task, kind, send)}
        onUnlink={() => {
          const link = linkFromAbove(snap, task);
          if (link) void send({ type: "remove_dependency", uid: link.uid });
        }}
        onSplit={() => task && setSplitDialog(task.uid)}
        onDelete={() => task && void send({ type: "delete_task", uid: task.uid })}
        onProjectInfo={() => setInfoOpen(true)}
        onToggleEvm={() => {
          if (!evmOpen && view !== "gantt" && view !== "tracking") setView("gantt");
          setEvmOpen((open) => !open);
        }}
        onExportPdf={() => exportPdfRef.current()}
      />
      <ViewBar
        project={project}
        view={view}
        onView={setView}
        scale={scale}
        onScale={setScale}
        filter={rowFilter}
        onFilter={setRowFilter}
        filterResource={filterResource}
        onFilterResource={setFilterResource}
        group={rowGroup}
        onGroup={setRowGroup}
        highlight={rowHighlight}
        onHighlight={setRowHighlight}
        networkSummaries={networkSummaries}
        onNetworkSummaries={setNetworkSummaries}
      />
      {error && <div className="banner">{error}</div>}
      {/* EVM sits beside the Gantt and the inspector so its burn-down chart keeps the full height. */}
      <div className="work-area">
        <div className="work-column">
          <main className="workspace">
            {project && (view === "gantt" || view === "tracking") && (
              <>
                <TaskSheet
                  rows={rows}
                  calendars={project.calendars}
                  assignments={project.assignments}
                  resources={project.resources}
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
                  onEdit={setEditUid}
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
            {project && view === "network" && (
              <NetworkDiagram
                project={project}
                layout={networkLayout}
                selected={selected}
                highlighted={networkHighlighted}
                onSelect={setSelected}
                onEdit={(uid) => {
                  setSelected(uid);
                  setEditUid(uid);
                }}
              />
            )}
          </main>
          {project && task && (view === "gantt" || view === "tracking") && (
            <Inspector
              project={project}
              task={task}
              send={send}
              onEditSeries={setSeriesDialog}
              onSplit={setSplitDialog}
              onEditCalendar={setCalendarEditId}
              onCreateCalendar={() => {
                const name = task.name.trim() ? `Lịch ${task.name.trim()}` : "Lịch công việc";
                void createCalendar(name, (id) => ({ type: "set_task_calendar", uid: task.uid, calendar_id: id }), true);
              }}
            />
          )}
        </div>
        {project && evmOpen && (view === "gantt" || view === "tracking") && (
          <EvmPanel
            metrics={metrics}
            onOpenProjectInfo={() => setInfoOpen(true)}
            onOpenBaseline={() => setBaselineOpen(true)}
            onClose={() => setEvmOpen(false)}
          />
        )}
      </div>
      <footer className="status">
        <span>{snap?.dirty ? "Chưa lưu" : "Đã lưu"}</span>
        <span title={snap?.path ?? undefined}>{snap?.path ? fileName(snap.path) : "Chưa có tệp"}</span>
        <span>{project?.tasks.length ?? 0} công việc</span>
        <span>{criticalCount} việc găng</span>
        <span className="status-finish" title="Công việc kết thúc muộn nhất quyết định ngày kết thúc dự án">
          {end ? `Kết thúc ${formatInstant(end.finish)} · do “${end.driver}”` : ""}
        </span>
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
        <Modal title="Sửa lịch" className="wide" onClose={() => setCalendarEditId(null)}>
          <CalendarEditor calendar={editingCalendar} send={send} />
          <button type="button" onClick={() => setCalendarEditId(null)}>Đóng</button>
        </Modal>
      )}
      {seriesDialog != null && project && (
        <RecurringDialog
          key={seriesDialog}
          project={project}
          initial={project.tasks.find((item) => item.uid === seriesDialog)?.recurrence?.spec ?? null}
          defaultStart={task && !task.summary ? task.start : project.start}
          onClose={() => setSeriesDialog(null)}
          onSubmit={(spec) => send(recurringCommand(project.tasks, seriesDialog, selected, spec))}
        />
      )}
      {splitTask && (
        <SplitDialog
          key={splitTask.uid}
          task={splitTask}
          onClose={() => setSplitDialog(null)}
          onSubmit={(at, gap) => send({ type: "split_task", uid: splitTask.uid, at, gap })}
        />
      )}
      {editTask && <TaskEditDialog key={editTask.uid} task={editTask} send={send} onClose={closeEdit} />}
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
  kind: DepKind,
  send: (command: Command) => void,
) {
  const above = project ? taskAbove(project.tasks, task) : null;
  if (!above || !task) return;
  send({ type: "add_dependency", predecessor_uid: above.uid, successor_uid: task.uid, kind, lag: 0 });
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
    <Modal title="Thông tin dự án" className="wide" onClose={onClose}>
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
    </Modal>
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
    <Modal title="Đường cơ sở" onClose={onClose}>
      <label>Mốc 0–10</label>
      <input type="number" min={0} max={10} value={number} onChange={(event) => setNumber(Number(event.target.value))} />
      <div className="row">
        <button type="button" onClick={() => onSet(Math.min(10, Math.max(0, number)))}>Đặt</button>
        <button type="button" onClick={() => onClear(Math.min(10, Math.max(0, number)))}>Xóa</button>
        <button type="button" onClick={onClose}>Đóng</button>
      </div>
    </Modal>
  );
}

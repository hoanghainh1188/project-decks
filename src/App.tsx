import { getCurrentWindow } from "@tauri-apps/api/window";
import { Check } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  applyCommand,
  checkAutosave,
  errorMessage,
  inTauri,
  loadMetrics,
  loadNetworkLayout,
  loadState,
  onMenu,
  onOpenFile,
  openPending,
  redo,
  savePdf,
  setTheme,
  undo,
} from "./api";
import { appMessages } from "./App.i18n";
import { linkAbove, resourceCommand } from "./appCommands";
import { BaselineDialog } from "./components/BaselineDialog";
import { Button } from "./components/Button";
import { CalendarEditor } from "./components/CalendarEditor";
import { EvmPanel } from "./components/EvmPanel";
import { GanttChart } from "./components/GanttChart";
import { Inspector } from "./components/inspector/Inspector";
import { Modal } from "./components/Modal";
import { NetworkDiagram } from "./components/NetworkDiagram";
import { ProjectDialog } from "./components/ProjectDialog";
import { RecurringDialog, recurringCommand } from "./components/RecurringDialog";
import { ResourceSheet } from "./components/ResourceSheet";
import { SplitDialog } from "./components/SplitDialog";
import { StatusBar } from "./components/StatusBar";
import { TaskEditDialog } from "./components/TaskEditDialog";
import { TaskSheet } from "./components/TaskSheet";
import { TaskUsage } from "./components/TaskUsage";
import { Toolbar } from "./components/Toolbar";
import { VIEW_PANEL_ID, ViewBar, viewTabId } from "./components/ViewBar";
import { normalizePredecessorText } from "./format";
import { pick, useLang, useMessages, useSetLang } from "./i18n";
import { buildGanttPdf, pdfSuggestedName } from "./ganttPdf";
import { handleMenuAction } from "./menuActions";
import { buildNetworkPdf } from "./networkPdf";
import type { Command, NetworkLayout, ProjectMetrics, Scale, Snapshot, ViewId } from "./types";
import {
  commitFocusedField,
  isEditableTarget,
  runFieldAction,
  shortcutId,
  shouldRunAction,
  trackCommittedText,
} from "./shortcuts";
import { applyTheme, themeChoice, type ThemeChoice } from "./theme";
import { linkFromAbove, toolbarStates } from "./toolbarState";
import { undoTo } from "./undoTo";
import { useFileDialog } from "./useFileDialog";
import { buildViewRows, type FilterMode, type GroupMode, type HighlightMode } from "./viewRows";
import { windowTitle } from "./windowTitle";
import "./styles.css";

export default function App() {
  const m = useMessages(appMessages);
  const setLang = useSetLang();
  const lang = useLang();
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [view, setView] = useState<ViewId>("gantt");
  const [scale, setScale] = useState<Scale>("day");
  const [selected, setSelected] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Thông tin dự án is a transaction: the history cursor when it opened, so Hủy can
  // undo back to it. null while the dialog is closed.
  const [infoStart, setInfoStart] = useState<number | null>(null);
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

  // `null` from `work` means nothing happened (no queued file to open): the last error
  // stays on screen instead of being cleared by a call the user did not make.
  const run = useCallback(async (work: () => Promise<Snapshot | void | null>): Promise<Snapshot | void> => {
    if (!inTauri()) {
      setError(pick(appMessages).needsTauri);
      return;
    }
    try {
      const next = await work();
      if (next === null) return;
      if (next) setSnap(next);
      setError(null);
      return next;
    } catch (err) {
      setError(errorMessage(err));
    }
  }, []);

  const send = useCallback((command: Command) => run(() => applyCommand(command)), [run]);

  const { startFileDialog, openFileWhenFree, fileDialogBusy } = useFileDialog(run, setError);

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

  // Read the cursor through the queue: a cell committed by the click that opened the
  // dialog may still be in flight, and must count as before the dialog, not inside it.
  const openProjectInfo = useCallback(async () => {
    const state = await run(loadState);
    setInfoStart(state ? state.history_cursor : (snap?.history_cursor ?? 0));
  }, [run, snap]);

  // Undo every command applied since `cursor`. Queued edits (a field committed on
  // close) land first, because loadState waits for them.
  const rollbackTo = useCallback(
    (cursor: number) =>
      run(() => undoTo(cursor, { current: loadState, undo })),
    [run],
  );

  const menuRef = useRef<(id: string) => void>(() => {});

  const project = snap?.project;
  const task = project?.tasks.find((item) => item.uid === selected) ?? null;
  const editTask = project?.tasks.find((item) => item.uid === editUid) ?? null;
  const closeEdit = useCallback(() => setEditUid(null), []);

  // The theme switches at once; if the backend cannot store it, the previous one comes
  // back, so the screen, the menu check and settings.json agree.
  const changeTheme = useCallback((theme: ThemeChoice) => {
    const previous = themeChoice();
    applyTheme(theme);
    if (!inTauri()) return;
    setTheme(theme).catch((err) => {
      applyTheme(previous);
      setError(errorMessage(err));
    });
  }, []);

  const handleMenu = useCallback(
    (id: string) =>
      handleMenuAction(id, {
        snap,
        task,
        view,
        fileDialogBusy,
        infoStart,
        run,
        send,
        startFileDialog,
        exportPdf: () => exportPdfRef.current(),
        setView,
        setScale,
        setEvmOpen,
        openProjectInfo: () => void openProjectInfo(),
        openBaseline: () => setBaselineOpen(true),
        openRecurring: () => setSeriesDialog(0),
        openSplit: setSplitDialog,
        // A new, untouched project is renamed in the new language by the backend.
        changeLang: (next) =>
          void setLang(next)
            .then(() => run(loadState))
            .catch((err) => setError(errorMessage(err))),
        changeTheme,
      }),
    [run, send, startFileDialog, task, snap, view, fileDialogBusy, openProjectInfo, infoStart, setLang, changeTheme],
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
      setError(pick(appMessages).needsTauri);
      return;
    }
    // After a crash the backend offers the autosaved project back in a native dialog;
    // then a project the app was launched to open (a double-clicked file) opens.
    void run(() => loadState()).then(() =>
      startFileDialog(async () => {
        // A slot that cannot be restored must not keep the double-clicked file closed.
        const restored = await checkAutosave().catch((err: unknown) => {
          setError(errorMessage(err));
          return null;
        });
        return (await openPending()) ?? restored;
      }),
    );
    const stops: (() => void)[] = [];
    let cancelled = false;
    const keep = (stop: () => void) => {
      if (cancelled) stop();
      else stops.push(stop);
    };
    void onMenu((id) => dispatchAction(id, document.activeElement)).then(keep);
    void onOpenFile(openFileWhenFree).then(keep);
    return () => {
      cancelled = true;
      stops.forEach((stop) => stop());
    };
  }, [run, dispatchAction, startFileDialog, openFileWhenFree]);

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

  const rows = useMemo(
    () => (project ? buildViewRows(project, rowFilter, filterResource, rowGroup, rowHighlight) : []),
    // Group headers ("Chưa gán" / "Unassigned") are text, so they follow the language.
    [project, rowFilter, filterResource, rowGroup, rowHighlight, lang],
  );
  // The network diagram ignores grouping; filters choose the boxes and highlight tints them.
  const networkRows = useMemo(
    () => (project && view === "network" ? buildViewRows(project, rowFilter, filterResource, "none", rowHighlight) : []),
    [project, view, rowFilter, filterResource, rowHighlight, lang],
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
        await savePdf(bytes, pdfSuggestedName(m.networkPdfName(project.name.trim() || m.untitledProject)));
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
        onAddTask={() =>
          void send({ type: "add_task", name: m.newTask(project?.next_task_uid ?? 1), after_uid: selected })
        }
        onAddMilestone={() => void send({ type: "add_milestone", name: m.newMilestone, after_uid: selected })}
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
        onProjectInfo={() => void openProjectInfo()}
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
            {/* The panel the view tabs control; inside <main> so the main landmark stays. */}
            <div className="workspace-panel" id={VIEW_PANEL_ID} role="tabpanel" aria-labelledby={viewTabId(view)}>
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
                  onPredecessors={(uid, text) => send({ type: "set_predecessors", uid, text: normalizePredecessorText(text) })}
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
                onAdd={(kind) => send({ type: "add_resource", name: m.newResource(project.next_resource_uid), kind })}
                onChange={(resource) => send(resourceCommand(resource))}
                onDelete={(uid) => send({ type: "delete_resource", uid })}
                onEditCalendar={setCalendarEditId}
                onCreateCalendar={(resource) => {
                  const name = resource.name.trim() ? m.calendarFor(resource.name.trim()) : m.resourceCalendar;
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
            </div>
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
                const name = task.name.trim() ? m.calendarFor(task.name.trim()) : m.taskCalendar;
                void createCalendar(name, (id) => ({ type: "set_task_calendar", uid: task.uid, calendar_id: id }), true);
              }}
            />
          )}
        </div>
        {project && evmOpen && (view === "gantt" || view === "tracking") && (
          <EvmPanel
            metrics={metrics}
            onOpenProjectInfo={() => void openProjectInfo()}
            onOpenBaseline={() => setBaselineOpen(true)}
            onClose={() => setEvmOpen(false)}
          />
        )}
      </div>
      <StatusBar snap={snap} />
      {infoStart != null && project && (
        <ProjectDialog
          project={project}
          changed={(snap?.history_cursor ?? 0) !== infoStart}
          onSave={() => {
            commitFocusedField();
            setInfoStart(null);
          }}
          onCancel={() => {
            void rollbackTo(infoStart).then((state) => {
              // Close only once everything is undone; otherwise stay open so Hủy can be
              // tried again (or Lưu kept) instead of leaving the project half reverted.
              if (state && state.history_cursor <= infoStart) setInfoStart(null);
              else setError(m.rollbackFailed);
            });
          }}
          send={send}
          onCreateCalendar={() => createCalendar(m.newCalendar, () => null, false)}
        />
      )}
      {editingCalendar && (
        <Modal
          title={m.editCalendar}
          className="wide"
          onClose={() => setCalendarEditId(null)}
          footer={<Button variant="primary" Icon={Check} onClick={() => setCalendarEditId(null)}>{m.close}</Button>}
        >
          <CalendarEditor calendar={editingCalendar} send={send} />
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

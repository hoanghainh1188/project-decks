import type { Command, NetworkLayout, ProjectMetrics, Snapshot, Task, TaskClipboard } from "../types";
import { makeMetrics, makeSnapshot, makeTask } from "./fixtures";

export interface InvokeCall {
  cmd: string;
  args: Record<string, unknown> | undefined;
}

/**
 * In-memory stand-in for the Tauri commands in src/api.ts. It records every call and
 * applies a handful of commands so component flows can round-trip without the engine.
 */
class FakeBackend {
  calls: InvokeCall[] = [];
  snapshot: Snapshot = makeSnapshot();
  metrics: ProjectMetrics = makeMetrics();
  network: NetworkLayout = emptyNetwork();
  /** What `check_autosave` restores; null when there is no autosave. */
  autosave: Snapshot | null = null;
  /** The remembered interface language (`get_language`); null follows the system. */
  language: string | null = null;
  /** The remembered theme (`get_theme`). */
  theme = "system";
  private failures = new Map<string, unknown>();
  private holds = new Map<string, Promise<void>>();

  reset(snapshot: Snapshot = makeSnapshot(), metrics: ProjectMetrics = makeMetrics()): void {
    this.calls = [];
    this.snapshot = structuredClone(snapshot);
    this.metrics = structuredClone(metrics);
    this.network = emptyNetwork();
    this.failures.clear();
    this.holds.clear();
    this.autosave = null;
    this.language = null;
    this.theme = "system";
  }

  /** The next call to `cmd` waits until the returned function runs, like a slow Rust command. */
  holdNext(cmd: string): () => void {
    let release = (): void => {};
    this.holds.set(cmd, new Promise<void>((resolve) => (release = resolve)));
    return () => release();
  }

  /** The next call to `cmd` rejects with `error`, like a Rust `Err(String)`. */
  failNext(cmd: string, error: unknown): void {
    this.failures.set(cmd, error);
  }

  commands(): Command[] {
    return this.calls
      .filter((call) => call.cmd === "apply_command")
      .map((call) => call.args?.command as Command);
  }

  async invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
    this.calls.push({ cmd, args });
    const hold = this.holds.get(cmd);
    if (hold) {
      this.holds.delete(cmd);
      await hold;
    }
    if (this.failures.has(cmd)) {
      const error = this.failures.get(cmd);
      this.failures.delete(cmd);
      throw error;
    }
    // Window plugin calls such as setting the title.
    if (cmd.startsWith("plugin:")) return undefined;
    switch (cmd) {
      case "open_pending":
        // Nothing queued, as when the app is opened normally.
        return null;
      case "open_recent":
      case "project_state":
      case "new_project":
      case "open_project":
      case "save_project":
      case "save_project_as":
      case "import_mspdi":
      case "compact_history":
      case "undo":
      case "redo":
        return structuredClone(this.snapshot);
      case "project_metrics":
        return structuredClone(this.metrics);
      case "network_layout":
        return structuredClone(this.network);
      case "export_mspdi":
      case "save_pdf":
        return undefined;
      case "check_autosave":
        if (this.autosave) this.snapshot = structuredClone(this.autosave);
        return this.autosave ? structuredClone(this.autosave) : null;
      case "get_theme":
        return this.theme;
      case "set_theme":
        this.theme = args?.theme as string;
        return null;
      case "get_language":
        return this.language;
      case "set_language":
        if (args?.remember) this.language = args.language as string;
        return undefined;
      case "copy_tasks":
        return this.copy(args?.uids as number[]);
      case "apply_command":
        this.apply(args?.command as Command);
        return structuredClone(this.snapshot);
      default:
        throw `Fake backend: lệnh ${cmd} chưa hỗ trợ`;
    }
  }

  /** A clipboard with the names and durations of `uids`, in sheet order. */
  private copy(uids: number[]): TaskClipboard {
    const tasks = this.snapshot.project.tasks.filter((task: Task) => uids.includes(task.uid));
    return {
      tasks: tasks.map((task) => ({
        name: task.name,
        level: 0,
        milestone: task.milestone,
        manual: task.manual,
        duration: task.duration,
        work: task.work,
        start: task.start,
        finish: task.finish,
        notes: task.notes,
        priority: task.priority,
        calendar_id: task.calendar_id,
        calendar_name: null,
        constraint_type: task.constraint_type,
        constraint_date: task.constraint_date,
        deadline: task.deadline,
        task_type: task.task_type,
        effort_driven: task.effort_driven,
        fixed_cost: task.fixed_cost,
        fixed_cost_accrual: task.fixed_cost_accrual,
        splits: task.splits,
        assignments: [],
      })),
      links: [],
    };
  }

  private apply(command: Command): void {
    const project = this.snapshot.project;
    this.snapshot.dirty = true;
    this.snapshot.can_undo = true;
    switch (command.type) {
      case "rename_project":
        project.name = command.name;
        return;
      case "set_status_date":
        project.status_date = command.date;
        return;
      case "rename_task": {
        const task = project.tasks.find((item) => item.uid === command.uid);
        if (!task) throw `Không có công việc ${command.uid}`;
        task.name = command.name;
        return;
      }
      case "create_recurring_task": {
        const uid = Math.max(0, ...project.tasks.map((task) => task.uid)) + 1;
        project.tasks.push(
          makeTask({
            uid,
            name: command.spec.name,
            summary: true,
            start: command.spec.start,
            recurrence: { spec: command.spec, skipped: [] },
          }),
        );
        return;
      }
      case "update_recurring_task": {
        const task = project.tasks.find((item) => item.uid === command.uid);
        if (!task?.recurrence) throw `Không có chuỗi lặp ${command.uid}`;
        task.name = command.spec.name;
        task.recurrence = { ...task.recurrence, spec: command.spec };
        return;
      }
      case "paste_tasks": {
        // Only names, durations and order: enough for the UI flows; the engine does the rest.
        const at = command.after_uid == null ? project.tasks.length : project.tasks.findIndex((item) => item.uid === command.after_uid) + 1;
        let uid = Math.max(0, ...project.tasks.map((item) => item.uid));
        const pasted = command.clipboard.tasks.map((copy) => makeTask({ uid: ++uid, name: copy.name, duration: copy.duration }));
        project.tasks.splice(at, 0, ...pasted);
        return;
      }
      case "fill_down": {
        const source = project.tasks.find((item) => item.uid === command.from_uid);
        if (!source) throw `Không có công việc ${command.from_uid}`;
        for (const task of project.tasks.filter((item) => command.to_uids.includes(item.uid))) {
          if (command.field === "name") task.name = source.name;
          if (command.field === "duration") task.duration = source.duration;
        }
        return;
      }
      case "create_calendar": {
        const id = Math.max(0, ...project.calendars.map((item) => item.id)) + 1;
        const base = project.calendars.find((item) => item.id === command.base_id);
        const source = base ?? project.calendars[0];
        project.calendars.push({
          ...structuredClone(source),
          id,
          name: command.name,
          base: base ? { id: base.id, week: base.week.map(() => null), exceptions: {}, work_weeks: [] } : null,
        });
        return;
      }
      case "set_project_calendar":
        project.calendar_id = command.calendar_id;
        return;
      case "set_calendar_base": {
        const calendar = project.calendars.find((item) => item.id === command.calendar_id);
        if (!calendar) throw `Không có lịch ${command.calendar_id}`;
        calendar.base =
          command.base_id == null
            ? null
            : { id: command.base_id, week: calendar.week.map(() => null), exceptions: {}, work_weeks: [] };
        return;
      }
      case "set_calendar_work_week": {
        const calendar = project.calendars.find((item) => item.id === command.calendar_id);
        if (!calendar) throw `Không có lịch ${command.calendar_id}`;
        const week = { name: command.name, from: command.from, to: command.to, week: command.week };
        calendar.work_weeks = [...calendar.work_weeks.filter((item) => item.from !== command.from), week].sort((a, b) =>
          a.from.localeCompare(b.from),
        );
        return;
      }
      case "clear_calendar_work_week": {
        const calendar = project.calendars.find((item) => item.id === command.calendar_id);
        if (!calendar) throw `Không có lịch ${command.calendar_id}`;
        calendar.work_weeks = calendar.work_weeks.filter((item) => item.from !== command.from);
        return;
      }
      case "set_assignment_actual_cost": {
        const assignment = project.assignments.find((item) => item.uid === command.uid);
        if (!assignment) throw `Không có phân công ${command.uid}`;
        assignment.manual_actual_cost = command.cost;
        if (project.editable_actual_costs && command.cost != null) assignment.actual_cost = command.cost;
        return;
      }
      case "set_editable_actual_costs":
        project.editable_actual_costs = command.editable;
        return;
      case "set_task_actual_cost": {
        const task = project.tasks.find((item) => item.uid === command.uid);
        if (!task) throw `Không có công việc ${command.uid}`;
        task.manual_actual_cost = command.cost;
        if (project.editable_actual_costs && command.cost != null) task.actual_cost = command.cost;
        return;
      }
      default:
        // Other commands are only recorded; tests assert on the payload.
        return;
    }
  }
}

function emptyNetwork(): NetworkLayout {
  return { nodes: [], edges: [], columns: 0, width: 0, height: 0, crossings: 0 };
}

export const fakeBackend = new FakeBackend();

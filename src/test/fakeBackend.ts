import type { Command, NetworkLayout, ProjectMetrics, Snapshot } from "../types";
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
  private failures = new Map<string, unknown>();

  reset(snapshot: Snapshot = makeSnapshot(), metrics: ProjectMetrics = makeMetrics()): void {
    this.calls = [];
    this.snapshot = structuredClone(snapshot);
    this.metrics = structuredClone(metrics);
    this.network = emptyNetwork();
    this.failures.clear();
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
    if (this.failures.has(cmd)) {
      const error = this.failures.get(cmd);
      this.failures.delete(cmd);
      throw error;
    }
    switch (cmd) {
      case "project_state":
      case "new_project":
      case "open_project":
      case "save_project":
      case "save_project_as":
      case "import_mspdi":
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
      case "apply_command":
        this.apply(args?.command as Command);
        return structuredClone(this.snapshot);
      default:
        throw `Fake backend: lệnh ${cmd} chưa hỗ trợ`;
    }
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

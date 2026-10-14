import { Check, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import { checkImport, errorMessage, readTableSheet } from "../api";
import { useMessages } from "../i18n";
import { guessColumns, RESOURCE_COLUMNS, TASK_COLUMNS, type BareUnit, type ImportSetup, type ImportTarget } from "../importColumns";
import { convertTable, engineIssues, type ImportIssue } from "../importConvert";
import type { Command, ImportSheet, Project, Snapshot, Task } from "../types";
import { Button } from "./Button";
import { importDialogMessages } from "./ImportDialog.i18n";
import { Modal } from "./Modal";

/** Lines shown in the preview, and problems listed, before the rest are counted. */
const PREVIEW_LINES = 20;
const LISTED_PROBLEMS = 50;
/** Rows offered as the header row. */
const HEADER_CHOICES = 10;

function setupFor(sheet: ImportSheet, target: ImportTarget, headerRow: number, project: Project, previous?: ImportSetup): ImportSetup {
  const headers = sheet.rows[headerRow] ?? [];
  return {
    target,
    headerRow,
    columns: guessColumns(headers, target, project.fields),
    bareWork: previous?.bareWork ?? "hours",
    bareDuration: previous?.bareDuration ?? "days",
  };
}

/**
 * Tệp → Nhập bảng: map the columns of a CSV or sheet to fields, see the rows as they
 * will be read, and import them in one `import_rows` (one undo step). Cells are converted
 * here; the engine's own checks (`check_import`) run on every change once every cell
 * reads, and both kinds of problem are shown at the file's line. Not a transaction:
 * nothing changes until Nhập.
 */
export function ImportDialog({
  project,
  initial,
  afterTask,
  error,
  send,
  onClose,
}: {
  project: Project;
  /** The table `pick_table` read. */
  initial: ImportSheet;
  /** Where tasks go: after this task's subtree, or at the end when null. */
  afterTask: Task | null;
  /** The last command the engine refused. */
  error: string | null;
  send: (command: Command) => Promise<Snapshot | void>;
  onClose: () => void;
}) {
  const m = useMessages(importDialogMessages);
  const [sheet, setSheet] = useState(initial);
  const [setup, setSetup] = useState<ImportSetup>(() => setupFor(initial, "tasks", 0, project));
  const [engine, setEngine] = useState<{ problems: Parameters<typeof engineIssues>[0]; ready: boolean }>({ problems: [], ready: false });
  const [readError, setReadError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const sending = useRef(false);
  // Only the sheet asked for last is shown, however the reads finish.
  const sheetRequest = useRef(0);

  const converted = useMemo(() => convertTable(sheet, setup, project), [sheet, setup, project]);
  const rowCount = setup.target === "tasks" ? converted.tasks.length : converted.resources.length;
  const afterUid = setup.target === "tasks" ? (afterTask?.uid ?? null) : null;
  const resources = setup.target === "resources" ? converted.resources : [];
  const tasks = setup.target === "tasks" ? converted.tasks : [];
  const readable = converted.issues.length === 0 && rowCount > 0;

  // Ask the engine on every change once every cell reads; only the latest answer counts.
  useEffect(() => {
    if (!readable) {
      setEngine({ problems: [], ready: false });
      return;
    }
    let current = true;
    setEngine({ problems: [], ready: false });
    setReadError(null);
    checkImport(afterUid, resources, tasks)
      .then((problems) => current && setEngine({ problems, ready: true }))
      .catch((err) => current && setReadError(errorMessage(err)));
    return () => {
      current = false;
    };
    // `resources` and `tasks` are rebuilt with `converted`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [converted, readable, afterUid]);

  const issues: ImportIssue[] = useMemo(
    () => (readable ? engineIssues(engine.problems, converted, setup, project.fields) : converted.issues),
    [readable, engine.problems, converted, setup, project.fields],
  );
  const marked = useMemo(() => new Map(issues.map((issue) => [`${issue.line}:${issue.column}`, issue.message])), [issues]);
  const headers = sheet.rows[setup.headerRow] ?? [];
  const canImport = readable && engine.ready && issues.length === 0 && !busy;

  const columnName = (column: number | null) => {
    if (column == null) return null;
    return headers[column]?.trim() || m.columnOf(column + 1, "");
  };

  async function changeSheet(name: string) {
    const request = ++sheetRequest.current;
    try {
      const next = await readTableSheet(name);
      if (request !== sheetRequest.current) return;
      setReadError(null);
      setSheet(next);
      setSetup((current) => setupFor(next, current.target, 0, project, current));
    } catch (err) {
      if (request === sheetRequest.current) setReadError(errorMessage(err));
    }
  }

  async function submit() {
    if (sending.current || !canImport) return;
    sending.current = true;
    setBusy(true);
    const done = await send({ type: "import_rows", after_uid: afterUid, resources, tasks });
    sending.current = false;
    setBusy(false);
    if (done) onClose();
  }

  const close = () => {
    if (!sending.current) onClose();
  };

  const status = error ?? readError;
  const note = status ? (
    <span className="dialog-foot-note bad" role="alert">
      {status}
    </span>
  ) : (
    <span className="meta dialog-foot-note" role="status">
      {readable && !engine.ready ? m.checking : canImport ? m.ready(rowCount) : ""}
    </span>
  );
  const footer = (
    <>
      {note}
      <Button Icon={X} disabled={busy} onClick={close}>
        {m.cancel}
      </Button>
      <Button variant="primary" Icon={Check} disabled={!canImport} onClick={() => void submit()}>
        {m.importN(rowCount)}
      </Button>
    </>
  );

  const kinds = setup.target === "tasks" ? TASK_COLUMNS : RESOURCE_COLUMNS;
  const data = sheet.rows.map((cells, index) => ({ cells, line: sheet.lines[index] ?? index + 1, index })).filter((row) => row.index > setup.headerRow);
  const shown = data.slice(0, PREVIEW_LINES);
  const uses = (kind: string) => setup.columns.includes(kind);

  return (
    <Modal title={m.title} className="wide import-dialog" onClose={close} footer={footer}>
      <div className="import-setup">
        {sheet.sheets.length > 1 && (
          <label>
            {m.sheet}{" "}
            <select aria-label={m.sheet} value={sheet.sheet ?? ""} onChange={(event) => void changeSheet(event.target.value)}>
              {sheet.sheets.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </select>
          </label>
        )}
        <label>
          {m.headerRow}{" "}
          <select
            aria-label={m.headerRow}
            value={setup.headerRow}
            onChange={(event) => setSetup((current) => setupFor(sheet, current.target, Number(event.target.value), project, current))}
          >
            {sheet.rows.slice(0, HEADER_CHOICES).map((cells, index) => (
              <option key={index} value={index}>
                {m.lineN(sheet.lines[index] ?? index + 1)}
                {cells[0] ? `: ${cells[0]}` : ""}
              </option>
            ))}
          </select>
        </label>
        <fieldset className="import-target">
          <legend>{m.target}</legend>
          {(["tasks", "resources"] as const).map((target) => (
            <label key={target}>
              <input
                type="radio"
                name="import-target"
                checked={setup.target === target}
                onChange={() => setSetup((current) => setupFor(sheet, target, current.headerRow, project, current))}
              />{" "}
              {m[target]}
            </label>
          ))}
        </fieldset>
        {setup.target === "tasks" && uses("work") && (
          <BareUnitChoice label={m.bareWork} value={setup.bareWork} onChange={(bareWork) => setSetup({ ...setup, bareWork })} />
        )}
        {setup.target === "tasks" && uses("duration") && (
          <BareUnitChoice label={m.bareDuration} value={setup.bareDuration} onChange={(bareDuration) => setSetup({ ...setup, bareDuration })} />
        )}
      </div>
      <p className="meta">
        {setup.target === "tasks"
          ? `${afterTask ? m.afterTask(afterTask.name) : m.atEnd} ${m.peopleByName}`
          : m.keepPeople}
      </p>
      <div className="suggestion-wrap import-preview">
        <table className="suggestions" aria-label={m.preview}>
          <thead>
            <tr>
              <th scope="col">{m.line}</th>
              {setup.columns.map((kind, column) => (
                <th scope="col" key={column}>
                  <select
                    aria-label={m.columnOf(column + 1, headers[column] ?? "")}
                    value={kind}
                    onChange={(event) =>
                      setSetup({ ...setup, columns: setup.columns.map((item, index) => (index === column ? event.target.value : item)) })
                    }
                  >
                    <option value="skip">{m.skip}</option>
                    {kinds.map((item) => (
                      <option key={item} value={item}>
                        {m[item]}
                      </option>
                    ))}
                    {setup.target === "tasks" &&
                      project.fields.map((field) => (
                        <option key={field.id} value={`field:${field.id}`}>
                          {m.field(field.name)}
                        </option>
                      ))}
                  </select>
                  <span className="import-header">{headers[column]}</span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {shown.map(({ cells, line }) => (
              <tr key={line}>
                <th scope="row">{line}</th>
                {setup.columns.map((kind, column) => {
                  const problem = marked.get(`${line}:${column}`);
                  return (
                    <td key={column} className={problem ? "bad" : kind === "skip" ? "off" : undefined} title={problem}>
                      {cells[column]}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {data.length === 0 ? (
        <p className="meta">{m.noRows}</p>
      ) : data.length > shown.length ? (
        <p className="meta">{m.previewMore(shown.length, data.length)}</p>
      ) : null}
      {issues.length > 0 && (
        <section>
          <h3 className="dialog-subhead">{m.problems}</h3>
          <ul aria-label={m.problems} className="import-problems">
            {issues.slice(0, LISTED_PROBLEMS).map((issue, index) => (
              <li key={index}>{m.problemAt(issue.line, columnName(issue.column), issue.message)}</li>
            ))}
            {issues.length > LISTED_PROBLEMS && <li>{m.moreProblems(issues.length - LISTED_PROBLEMS)}</li>}
          </ul>
        </section>
      )}
    </Modal>
  );
}

function BareUnitChoice({ label, value, onChange }: { label: string; value: BareUnit; onChange: (value: BareUnit) => void }) {
  const m = useMessages(importDialogMessages);
  return (
    <label>
      {label}{" "}
      <select aria-label={label} value={value} onChange={(event) => onChange(event.target.value as BareUnit)}>
        <option value="hours">{m.hours}</option>
        <option value="days">{m.days}</option>
      </select>
    </label>
  );
}


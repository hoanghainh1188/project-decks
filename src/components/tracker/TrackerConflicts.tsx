import { useMessages } from "../../i18n";
import type { Project, PullConflict, Side } from "../../types";
import { trackerDialogMessages } from "./TrackerDialog.i18n";

/** The key a choice is kept under. */
export function choiceKey(conflict: Pick<PullConflict, "key" | "field">): string {
  return `${conflict.key}\u0000${conflict.field}`;
}

/** Rows listed before the rest are counted. */
const LISTED = 200;

/**
 * Fields changed at the source and in Sheafly since the last sync, to different values.
 * Nothing moves until each has a choice: take the source's value, or keep Sheafly's.
 */
export function TrackerConflicts({
  project,
  conflicts,
  choices,
  onChoose,
  onChooseAll,
}: {
  project: Project;
  conflicts: PullConflict[];
  choices: Record<string, Side>;
  onChoose: (conflict: PullConflict, side: Side) => void;
  onChooseAll: (side: Side) => void;
}) {
  const m = useMessages(trackerDialogMessages);
  const open = conflicts.filter((conflict) => !choices[choiceKey(conflict)]).length;

  const fieldName = (field: string) => {
    if (field.startsWith("field:")) {
      const id = Number(field.slice("field:".length));
      return project.fields.find((item) => item.id === id)?.name ?? field;
    }
    const known = { name: m.fieldName, work: m.fieldWork, start: m.fieldStart, due: m.fieldDue, status: m.fieldStatus, assignee: m.fieldAssignee } as Record<string, string>;
    return known[field] ?? field;
  };
  const show = (field: string, value: string | null): string => {
    if (value == null) return "—";
    if (field === "work") return m.hours(Number(value) / 60);
    if (field === "status") {
      return ({ not_started: m.notStarted, in_progress: m.inProgress, done: m.done } as Record<string, string>)[value] ?? value;
    }
    return value;
  };

  return (
    <section className="tracker-conflicts">
      <h3 className="dialog-subhead">{m.conflicts(open)}</h3>
      <p className="meta">{m.conflictsHint}</p>
      <div className="tracker-conflict-bulk">
        <button type="button" className="btn btn-secondary" onClick={() => onChooseAll("sheafly")}>
          {m.keepAllSheafly}
        </button>
        <button type="button" className="btn btn-secondary" onClick={() => onChooseAll("source")}>
          {m.takeAllSource}
        </button>
      </div>
      <div className="suggestion-wrap">
        <table className="suggestions" aria-label={m.conflictsTable}>
          <thead>
            <tr>
              <th scope="col">{m.key}</th>
              <th scope="col">{m.field}</th>
              <th scope="col">{m.lastSync}</th>
              <th scope="col">{m.atSource}</th>
              <th scope="col">{m.inSheafly}</th>
              <th scope="col">{m.keep}</th>
            </tr>
          </thead>
          <tbody>
            {conflicts.slice(0, LISTED).map((conflict) => {
              const id = choiceKey(conflict);
              const label = `${conflict.key} ${fieldName(conflict.field)}`;
              return (
                <tr key={id}>
                  <th scope="row" title={conflict.name}>
                    {conflict.key}
                  </th>
                  <td>{fieldName(conflict.field)}</td>
                  <td>{conflict.base == null ? m.unknownBase : show(conflict.field, conflict.base)}</td>
                  <td>{show(conflict.field, conflict.source)}</td>
                  <td>{show(conflict.field, conflict.local)}</td>
                  <td>
                    {(["source", "sheafly"] as const).map((side) => (
                      <label key={side}>
                        <input
                          type="radio"
                          name={`conflict-${id}`}
                          aria-label={`${label}: ${side === "source" ? m.takeSource : m.keepSheafly}`}
                          checked={choices[id] === side}
                          onChange={() => onChoose(conflict, side)}
                        />{" "}
                        {side === "source" ? m.takeSource : m.keepSheafly}{" "}
                      </label>
                    ))}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {conflicts.length > LISTED && <p className="meta">{m.moreRows(conflicts.length - LISTED)}</p>}
      </div>
    </section>
  );
}

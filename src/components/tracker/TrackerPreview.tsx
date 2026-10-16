import { useMemo } from "react";

import { useMessages } from "../../i18n";
import type { Project, PullPreview, PullRowDiff } from "../../types";
import { trackerDialogMessages } from "./TrackerDialog.i18n";
import { unmatchedEmails } from "./trackerSource";

/** Rows listed before the rest are counted. */
const LISTED = 200;

/** Counts of the rows by what pulling does to them. */
export function counts(preview: PullPreview) {
  const by = (kind: PullRowDiff["kind"]) => preview.changes.filter((row) => row.kind === kind).length;
  return { added: by("add"), changed: by("change"), same: by("same"), gone: preview.gone.length };
}

/** What pulling would do: counts, the tasks that change, the ones gone, and what needs attention. */
export function TrackerPreview({
  project,
  preview,
  onMatchEmail,
}: {
  project: Project;
  preview: PullPreview;
  /** Give the person `uid` the email of issues no one matched. */
  onMatchEmail: (uid: number, email: string) => void;
}) {
  const m = useMessages(trackerDialogMessages);
  const { added, changed, same, gone } = counts(preview);
  const listed = useMemo(() => preview.changes.filter((row) => row.kind !== "same"), [preview.changes]);
  const emails = useMemo(() => unmatchedEmails(preview.problems, preview.rows), [preview.problems, preview.rows]);
  const people = project.resources.filter((resource) => resource.kind === "person" && !resource.email);
  const warned = (kind: "no_email" | "skipped") => preview.warnings.filter((warning) => warning.kind === kind).length;
  const otherProblems = preview.problems.filter((problem) => problem.column !== "assignee");
  return (
    <div className="tracker-preview">
      <p role="status">{m.summary(added, changed, same, gone)}</p>
      <p className="meta">{m.onlySource}</p>
      {listed.length === 0 && preview.gone.length === 0 && preview.problems.length === 0 && <p className="meta">{m.nothing}</p>}
      {emails.length > 0 && (
        <section>
          <h3 className="dialog-subhead">{m.unmatched}</h3>
          <p className="meta">{m.unmatchedHint}</p>
          <ul className="tracker-emails">
            {emails.map((email) => (
              <li key={email}>
                <span>{email}</span>{" "}
                <select
                  aria-label={m.personForEmail(email)}
                  value=""
                  onChange={(event) => event.target.value && onMatchEmail(Number(event.target.value), email)}
                >
                  <option value="">{m.pickPerson}</option>
                  {people.map((person) => (
                    <option key={person.uid} value={person.uid}>
                      {person.name}
                    </option>
                  ))}
                </select>
              </li>
            ))}
          </ul>
        </section>
      )}
      {otherProblems.length > 0 && (
        <section>
          <h3 className="dialog-subhead">{m.problems}</h3>
          <ul aria-label={m.problems} className="import-problems">
            {otherProblems.slice(0, LISTED).map((problem, index) => (
              <li key={index}>{m.problemAt(problem.key, problem.message)}</li>
            ))}
            {otherProblems.length > LISTED && <li>{m.moreRows(otherProblems.length - LISTED)}</li>}
          </ul>
        </section>
      )}
      {(warned("no_email") > 0 || warned("skipped") > 0) && (
        <ul className="meta tracker-warnings">
          {warned("no_email") > 0 && <li>{m.noEmail(warned("no_email"))}</li>}
          {warned("skipped") > 0 && <li>{m.skipped(warned("skipped"))}</li>}
        </ul>
      )}
      {listed.length > 0 && (
        <div className="suggestion-wrap">
          <table className="suggestions" aria-label={m.changesTable}>
            <thead>
              <tr>
                <th scope="col">{m.key}</th>
                <th scope="col">{m.name}</th>
                <th scope="col">{m.change}</th>
              </tr>
            </thead>
            <tbody>
              {listed.slice(0, LISTED).map((row) => (
                <tr key={row.key}>
                  <th scope="row">{row.key}</th>
                  <td>{row.name}</td>
                  <td>{row.kind === "add" ? m.added : row.fields.map((field) => m[`field_${field}`]).join(", ")}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {listed.length > LISTED && <p className="meta">{m.moreRows(listed.length - LISTED)}</p>}
        </div>
      )}
      {preview.gone.length > 0 && (
        <section>
          <h3 className="dialog-subhead">{m.goneTitle}</h3>
          <ul aria-label={m.goneTitle} className="import-problems">
            {preview.gone.slice(0, LISTED).map((task) => (
              <li key={task.uid}>
                {task.key}: {task.name}
              </li>
            ))}
            {preview.gone.length > LISTED && <li>{m.moreRows(preview.gone.length - LISTED)}</li>}
          </ul>
        </section>
      )}
    </div>
  );
}

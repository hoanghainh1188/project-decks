import { useMessages } from "../../i18n";
import type { PushPreview, PushSkip } from "../../types";
import { trackerPushMessages } from "./TrackerPushDialog.i18n";

/** Rows listed before the rest are counted. */
const LISTED = 200;

/** What a push would write, issue by issue, and what it leaves out. */
export function TrackerPushPreview({ preview }: { preview: PushPreview }) {
  const m = useMessages(trackerPushMessages);
  const { changes, skipped, missing } = preview.plan;
  const reason = (item: PushSkip): string => {
    if (item.reason === "many_assignees") return m.skipManyAssignees;
    if (item.reason === "no_email") return m.skipNoEmail;
    const names = { start: m.pushFieldStart, due: m.pushFieldDue, assignee: m.pushFieldAssignee } as Record<string, string>;
    return m.skipSourceChanged(names[item.field ?? ""] ?? item.field ?? "");
  };
  return (
    <div className="tracker-preview">
      {changes.length === 0 ? (
        <p className="meta">{m.nothing}</p>
      ) : (
        <>
          <p className="meta">{m.willWrite}</p>
          <div className="suggestion-wrap">
            <table className="suggestions" aria-label={m.changesTable}>
              <thead>
                <tr>
                  <th scope="col">{m.key}</th>
                  <th scope="col">{m.name}</th>
                  <th scope="col">{m.startCol}</th>
                  <th scope="col">{m.dueCol}</th>
                  <th scope="col">{m.assigneeCol}</th>
                </tr>
              </thead>
              <tbody>
                {changes.slice(0, LISTED).map((change) => (
                  <tr key={change.key}>
                    <th scope="row">{change.key}</th>
                    <td>{change.name}</td>
                    <td>{change.start ? m.change(change.start.from, change.start.to) : ""}</td>
                    <td>{change.due ? m.change(change.due.from, change.due.to) : ""}</td>
                    <td>{change.assignee ? m.change(change.assignee.from, change.assignee.to) : ""}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {changes.length > LISTED && <p className="meta">{m.moreRows(changes.length - LISTED)}</p>}
          </div>
        </>
      )}
      {preview.unknown_assignees.length > 0 && (
        <p className="bad" role="alert">
          {m.unknown(preview.unknown_assignees.join(", "))}
        </p>
      )}
      {skipped.length > 0 && (
        <section>
          <h3 className="dialog-subhead">{m.skippedTitle}</h3>
          <ul aria-label={m.skippedTitle} className="import-problems">
            {skipped.slice(0, LISTED).map((item, index) => (
              <li key={`${item.key}-${index}`}>
                {item.key}: {reason(item)}
              </li>
            ))}
            {skipped.length > LISTED && <li>{m.moreRows(skipped.length - LISTED)}</li>}
          </ul>
        </section>
      )}
      {missing.length > 0 && <p className="meta">{m.missingTitle(missing.length)}</p>}
    </div>
  );
}

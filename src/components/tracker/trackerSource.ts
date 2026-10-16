import { DEFAULT_SYNC_SETTINGS, type PullProblem, type Source, type StatusClass, type StatusMapping, type TrackerKind } from "../../types";

/** What the form holds; the text fields are as typed. */
export interface SourceForm {
  kind: TrackerKind;
  site: string;
  projectKey: string;
  filter: string;
  statuses: StatusMapping[];
  /** The email of the account, for a tracker that signs in with one; kept with the credentials, not in the source. */
  account: string;
}

export const DEFAULT_HOURS_PER_POINT = 4;

/** The trackers the app can pull from. */
export const TRACKER_KINDS: TrackerKind[] = ["backlog", "jira"];

/** Whether the tracker signs in with an account (an email) and a token. */
export function needsAccount(kind: TrackerKind): boolean {
  return kind === "jira";
}

export function formFor(source: Source | null): SourceForm {
  return {
    kind: source?.kind ?? "backlog",
    account: "",
    site: source?.site ?? "",
    projectKey: source?.project_key ?? "",
    filter: source?.filter ?? "",
    statuses: source?.status_map ?? [],
  };
}

/** The source the form describes, tidied the way the engine tidies it. */
export function sourceOf(form: SourceForm, current: Source | null): Source {
  return {
    kind: form.kind,
    site: form.site.trim().toLowerCase(),
    project_key: form.projectKey.trim(),
    filter: form.filter.trim(),
    hours_per_point: current?.hours_per_point ?? DEFAULT_HOURS_PER_POINT,
    status_map: form.statuses
      .map((item) => ({ status: item.status.trim(), class: item.class }))
      .filter((item) => item.status !== ""),
    // Set in the sync settings, not in this form: keep what the project has.
    sync: current?.sync ?? DEFAULT_SYNC_SETTINGS,
  };
}

export function sameSource(a: Source | null, b: Source): boolean {
  if (!a) return false;
  return JSON.stringify(sourceOf(formFor(a), a)) === JSON.stringify(b);
}

/** A form with enough to try a connection. */
export function complete(source: Source): boolean {
  return source.site !== "" && source.project_key !== "";
}

export const STATUS_CLASSES: StatusClass[] = ["not_started", "in_progress", "done"];

/** Distinct emails of the issues whose assignee no one in the project has, by problem. */
export function unmatchedEmails(problems: PullProblem[], rows: { assignee_email?: string | null }[]): string[] {
  const emails = new Set<string>();
  for (const problem of problems) {
    if (problem.column !== "assignee" || problem.row == null) continue;
    const email = rows[problem.row]?.assignee_email?.trim();
    if (email) emails.add(email);
  }
  return [...emails];
}

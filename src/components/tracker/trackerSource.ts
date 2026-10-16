import type { PullProblem, Source, StatusClass, StatusMapping } from "../../types";

/** What the form holds; the text fields are as typed. */
export interface SourceForm {
  site: string;
  projectKey: string;
  filter: string;
  statuses: StatusMapping[];
}

export const DEFAULT_HOURS_PER_POINT = 4;

export function formFor(source: Source | null): SourceForm {
  return {
    site: source?.site ?? "",
    projectKey: source?.project_key ?? "",
    filter: source?.filter ?? "",
    statuses: source?.status_map ?? [],
  };
}

/** The source the form describes, tidied the way the engine tidies it. */
export function sourceOf(form: SourceForm, current: Source | null): Source {
  return {
    kind: "backlog",
    site: form.site.trim().toLowerCase(),
    project_key: form.projectKey.trim(),
    filter: form.filter.trim(),
    hours_per_point: current?.hours_per_point ?? DEFAULT_HOURS_PER_POINT,
    status_map: form.statuses
      .map((item) => ({ status: item.status.trim(), class: item.class }))
      .filter((item) => item.status !== ""),
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

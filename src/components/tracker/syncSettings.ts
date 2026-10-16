import type { OnConflict, Project, SyncDirection, SyncSettings } from "../../types";

/** The keys of the built-in fields that sync, in the order the dialog lists them. */
export const BUILT_IN = ["name", "work", "start", "due", "status", "assignee"] as const;
/** The days Sheafly computes, pushed to fields of their own. */
export const PLANNED = ["planned_start", "planned_finish"] as const;

/** What a row can do. */
export interface RowKind {
  /** Directions offered. */
  directions: SyncDirection[];
  /** The direction when the source's settings say nothing. */
  defaultDirection: SyncDirection;
  /** Whether the row has a field at the source to read that can be changed. */
  readsFrom: boolean;
  /** Whether the row has a field at the source to write to. */
  writesTo: boolean;
  /** Whether a conflict can arise (pulled fields). */
  conflicts: boolean;
}

const BOTH: SyncDirection[] = ["both", "pull", "push", "off"];
const PULL_OR_OFF: SyncDirection[] = ["pull", "off"];

export function kindOf(field: string): RowKind {
  switch (field) {
    case "start":
    case "due":
      return { directions: BOTH, defaultDirection: "both", readsFrom: true, writesTo: false, conflicts: true };
    case "assignee":
      return { directions: BOTH, defaultDirection: "both", readsFrom: false, writesTo: false, conflicts: true };
    case "work":
      return { directions: PULL_OR_OFF, defaultDirection: "pull", readsFrom: true, writesTo: false, conflicts: true };
    case "planned_start":
    case "planned_finish":
      return { directions: ["push", "off"], defaultDirection: "push", readsFrom: false, writesTo: true, conflicts: false };
    case "name":
    case "status":
    case "fields":
      return { directions: PULL_OR_OFF, defaultDirection: "pull", readsFrom: false, writesTo: false, conflicts: true };
    default:
      // A custom field of the project (`field:<id>`): read from a field of another name if asked.
      return { directions: PULL_OR_OFF, defaultDirection: "pull", readsFrom: true, writesTo: false, conflicts: true };
  }
}

/** One row of the dialog. */
export interface Row {
  field: string;
  direction: SyncDirection;
  onConflict: OnConflict;
  sourceField: string;
  pushField: string;
}

export function defaultRow(field: string): Row {
  return { field, direction: kindOf(field).defaultDirection, onConflict: "ask", sourceField: "", pushField: "" };
}

/** The keys the dialog lists for a project: built-in, the planned days, all custom fields, each custom field. */
export function fieldKeys(project: Project): string[] {
  return [...BUILT_IN, ...PLANNED, "fields", ...project.fields.map((field) => `field:${field.id}`)];
}

/** The rows for the project's saved settings. */
export function toRows(settings: SyncSettings, project: Project): Row[] {
  return fieldKeys(project).map((field) => {
    const policy = settings.fields.find((item) => item.field === field);
    if (!policy) return defaultRow(field);
    const kind = kindOf(field);
    // A direction the row cannot show means what the row's default means (both pulls).
    const direction = kind.directions.includes(policy.direction) ? policy.direction : kind.defaultDirection;
    return {
      field,
      direction,
      onConflict: policy.on_conflict,
      sourceField: policy.source_field ?? "",
      pushField: policy.push_field ?? "",
    };
  });
}

export interface Constants {
  inProgressPercent: number;
  startHour: number;
  dueHour: number;
}

/** The settings the rows and constants describe; a row at its defaults is left out. */
export function toSettings(rows: Row[], constants: Constants): SyncSettings {
  const fields = rows
    .filter((row) => {
      const base = defaultRow(row.field);
      return row.direction !== base.direction || row.onConflict !== "ask" || row.sourceField.trim() !== "" || row.pushField.trim() !== "";
    })
    .map((row) => ({
      field: row.field,
      direction: row.direction,
      on_conflict: kindOf(row.field).conflicts ? row.onConflict : ("ask" as OnConflict),
      source_field: row.sourceField.trim() || null,
      push_field: row.pushField.trim() || null,
    }));
  return {
    fields,
    in_progress_percent: constants.inProgressPercent,
    start_hour: constants.startHour,
    due_hour: constants.dueHour,
  };
}

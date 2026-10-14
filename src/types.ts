export type ConstraintType =
  | "asap"
  | "alap"
  | "snet"
  | "snlt"
  | "fnet"
  | "fnlt"
  | "mso"
  | "mfo";

export type DepKind = "fs" | "ss" | "ff" | "sf";
export type TaskType = "fixed_units" | "fixed_duration" | "fixed_work";
export type Accrual = "start" | "prorated" | "end";
export type ResourceKind = "person" | "machine";

export interface TimeRange {
  start_minute: number;
  end_minute: number;
}

/** Hours that replace the default week on every day of `[from, to]` ("YYYY-MM-DD"). */
export interface WorkWeek {
  name: string;
  from: string;
  to: string;
  week: TimeRange[][];
}

/** What a calendar based on calendar `id` sets itself; the rest comes from the base. */
export interface CalendarBase {
  id: number;
  /** A weekday set on this calendar, or null to take the base's. */
  week: (TimeRange[] | null)[];
  exceptions: Record<string, TimeRange[]>;
  work_weeks: WorkWeek[];
}

export interface Calendar {
  id: number;
  name: string;
  /** The merged hours: for a based calendar, its own over its base's. */
  week: TimeRange[][];
  exceptions: Record<string, TimeRange[]>;
  /** Ordered by `from`, never overlapping. */
  work_weeks: WorkWeek[];
  base: CalendarBase | null;
}

export interface Baseline {
  number: number;
  start: number;
  finish: number;
  work: number;
  cost: number;
  /** Segments of a split task when the baseline was set; empty otherwise. */
  segments: Segment[];
}

/** Interruption of a task's work; both values are working minutes. */
export interface Split {
  offset: number;
  gap: number;
  /** Made by leveling to let an urgent task in; omitted when false. */
  leveling?: boolean;
}

/** One scheduled piece of a split task, as instants. */
export interface Segment {
  start: number;
  finish: number;
}

export interface Task {
  uid: number;
  id: number;
  name: string;
  wbs: string;
  outline_level: number;
  parent_uid: number | null;
  notes: string;
  priority: number;
  milestone: boolean;
  summary: boolean;
  manual: boolean;
  duration: number;
  constraint_type: ConstraintType;
  constraint_date: number | null;
  calendar_id: number | null;
  task_type: TaskType;
  effort_driven: boolean;
  percent_complete: number;
  actual_start: number | null;
  actual_finish: number | null;
  remaining_duration: number;
  remaining_work: number;
  fixed_cost: number;
  fixed_cost_accrual: Accrual;
  leveling_delay: number;
  /** Urgent: when leveling, it goes ahead of work in progress ranked below it on the same people. */
  interrupts: boolean;
  start: number;
  finish: number;
  early_start: number;
  early_finish: number;
  late_start: number;
  late_finish: number;
  total_slack: number;
  free_slack: number;
  critical: boolean;
  units: number;
  work: number;
  actual_work: number;
  cost: number;
  actual_cost: number;
  remaining_cost: number;
  /** Hand-entered actual cost; counts only while `Project.editable_actual_costs` is on. */
  manual_actual_cost: number | null;
  conflict: boolean;
  predecessors: string;
  baselines: Baseline[];
  deadline: number | null;
  late: boolean;
  recurrence: Recurrence | null;
  series_uid: number | null;
  occurrence_date: number | null;
  detached: boolean;
  /** Custom field values by field id (JSON object keys are strings); no key, no value. */
  values: Record<string, FieldValue>;
  splits: Split[];
  /** Computed by the engine; empty when the task is not split. */
  segments: Segment[];
}

/** Mirror of `fields::FieldKind`. A choice is one of `options`, in the user's order. */
export type FieldKind = { type: "text" } | { type: "number" } | { type: "choice"; options: string[] };

/** Mirror of `level_order::Criterion`: what one key of the leveling order compares. */
export type Criterion =
  | { type: "start" }
  | { type: "priority" }
  | { type: "slack" }
  | { type: "deadline" }
  | { type: "duration" }
  | { type: "id" }
  | { type: "field"; id: number };

/** Mirror of `level_order::SortKey`. */
export interface SortKey {
  criterion: Criterion;
  descending: boolean;
}

/** Mirror of `fields::FieldDef`: a custom task field the project declares. */
export interface FieldDef {
  id: number;
  name: string;
  kind: FieldKind;
}

/** Mirror of `fields::FieldValue`: a number for number fields, text for the others. */
export type FieldValue = number | string;

/** Weekdays use 0 = Monday … 6 = Sunday, like calendar weeks. Ordinals are 1–4, or 5 for the last one. */
export type RecurrencePattern =
  | { kind: "daily"; every: number }
  | { kind: "calendar_daily"; every: number }
  | { kind: "weekly"; every: number; weekdays: number[] }
  | { kind: "monthly"; every: number; day: number }
  | { kind: "monthly_nth"; every: number; ordinal: number; weekday: number }
  | { kind: "yearly"; every: number; month: number; day: number }
  | { kind: "yearly_nth"; every: number; month: number; ordinal: number; weekday: number };

/** Units are 1 for 100 %. */
export interface SeriesResource {
  uid: number;
  units: number;
}

export type RecurrenceEnd = { kind: "count"; count: number } | { kind: "until"; date: number };

export interface RecurrenceSpec {
  name: string;
  duration: number;
  start: number;
  pattern: RecurrencePattern;
  end: RecurrenceEnd;
  calendar_id: number | null;
  /** Only in series saved before per-resource units; read as units 1. */
  resource_uids?: number[];
  resources: SeriesResource[];
  /** Never sets the project finish or the critical path. Absent in older series. */
  background?: boolean;
}

export interface Recurrence {
  spec: RecurrenceSpec;
  skipped: number[];
}

export interface Dependency {
  uid: number;
  predecessor_uid: number;
  successor_uid: number;
  kind: DepKind;
  lag: number;
}

export interface Resource {
  uid: number;
  name: string;
  kind: ResourceKind;
  max_units: number;
  calendar_id: number | null;
  standard_rate: number;
  overtime_rate: number;
  cost_per_use: number;
  /** Days with other units than `max_units`, in date order. */
  availability: AvailabilityPeriod[];
  /** Ids of the project's skills this resource has, ascending. */
  skills: number[];
}

/** Mirror of `skills::SkillDef`. */
export interface SkillDef {
  id: number;
  name: string;
}

/** Mirror of `skills::Condition`: the task's value of `field` is one of `values`. */
export interface Condition {
  field: number;
  values: FieldValue[];
}

/** Mirror of `skills::RuleTarget`: one person, or everyone with a skill. */
export type RuleTarget = { type: "resource"; uid: number } | { type: "skill"; id: number };

/**
 * Mirror of `skills::AssignmentRule`: all conditions must hold (none matches every task);
 * the first rule that matches a task is used.
 */
export interface AssignmentRule {
  conditions: Condition[];
  target: RuleTarget;
  units: number;
}

/** Mirror of `suggest::Pick`: one person offered for a task, with estimated dates. */
export interface Pick {
  resource_uid: number;
  start: number;
  finish: number;
}

/** Mirror of `suggest::NoPick`: why a task has no one to offer. */
export type NoPick = "no_rule" | "no_one_with_skill" | "no_room";

/** Mirror of `suggest::Suggestion`: the people offered for one unassigned task, best first. */
export interface Suggestion {
  task_uid: number;
  /** Index of the rule that matched. */
  rule: number | null;
  /** Units the assignment would have: the rule's. */
  units: number;
  picks: Pick[];
  reason: NoPick | null;
}

/** Mirror of `skills::TaskAssignment`, one row of `assign_tasks`. */
export interface TaskAssignment {
  task_uid: number;
  resource_uid: number;
  units: number;
}

/** Mirror of `command::LevelingDelay`. */
export interface LevelingDelay {
  uid: number;
  minutes: number;
}

/** Mirror of `command::LevelingSplits`: all the leveling splits of one task. */
export interface LevelingSplits {
  uid: number;
  splits: Split[];
}

/** Mirror of `plan::PlanMove`: a task whose dates the plan changes. */
export interface PlanMove {
  uid: number;
  start_before: number;
  finish_before: number;
  start: number;
  finish: number;
}

/** Mirror of `plan::PlanPreview`, what `preview_plan` returns and `apply_plan` takes back. */
export interface PlanPreview {
  assignments: TaskAssignment[];
  delays: LevelingDelay[];
  splits: LevelingSplits[];
  moves: PlanMove[];
  late: number[];
  overallocated: number[];
}

/** Mirror of `import::ImportResource`, one person or machine of `import_rows`. */
export interface ImportResource {
  name: string;
  kind: ResourceKind;
  max_units?: number | null;
  standard_rate?: number | null;
  skills?: string[];
}

/** Mirror of `import::ImportAssignee`: a person by name, letter case aside. */
export interface ImportAssignee {
  name: string;
  units: number;
}

/**
 * Mirror of `import::ImportTask`, one task of `import_rows`. `work` and `duration` are
 * working minutes (a task with `work` is Fixed Work); instants are project wall-clock
 * milliseconds; `values` pairs a field name with its value; `after` lists rows of the same
 * import (0 for the first task) this one waits for.
 */
export interface ImportTask {
  name: string;
  level?: number;
  work?: number | null;
  duration?: number | null;
  priority?: number | null;
  deadline?: number | null;
  start_no_earlier?: number | null;
  values?: [string, FieldValue][];
  assignees?: ImportAssignee[];
  after?: number[];
}

/** Mirror of `import::ImportColumn` (serde's external tagging). */
export type ImportColumn =
  | "row"
  | "name"
  | "kind"
  | "max_units"
  | "rate"
  | "skills"
  | "level"
  | "work"
  | "duration"
  | "priority"
  | "deadline"
  | "start_no_earlier"
  | "assignees"
  | "after"
  | { field: string };

/** One problem `check_import` found; `row` counts from 0 within its table, null is the whole import. */
export interface ImportProblem {
  table: "resources" | "tasks";
  row: number | null;
  column: ImportColumn;
  message: string;
}

/** A table read for import (`pick_table`, `read_table_sheet`): every cell as text. */
export interface ImportSheet {
  sheets: string[];
  sheet: string | null;
  rows: string[][];
  /** The file's line or row number (from 1) of each row; blank rows are left out of `rows`. */
  lines: number[];
}

/**
 * Mirror of `availability::AvailabilityPeriod`: `units` on every day from `from` to `to`
 * (both "YYYY-MM-DD", both included); `null` is no limit on that side.
 */
export interface AvailabilityPeriod {
  from: string | null;
  to: string | null;
  units: number;
}

export interface Assignment {
  uid: number;
  task_uid: number;
  resource_uid: number;
  units: number;
  work: number;
  overtime_work: number;
  actual_work: number;
  remaining_work: number;
  cost: number;
  actual_cost: number;
  /** Hand-entered actual cost; counts only while costs are editable and the task has no entry. */
  manual_actual_cost: number | null;
}

export interface Overallocation {
  resource_uid: number;
  date: string;
  units: number;
  max_units: number;
}

export interface UsageDay {
  date: string;
  hours: number;
}

export interface UsageRow {
  task_uid: number;
  assignment_uid: number | null;
  resource_uid: number | null;
  label: string;
  days: UsageDay[];
}

export interface Project {
  name: string;
  start: number;
  status_date: number | null;
  calendar_id: number;
  calendars: Calendar[];
  tasks: Task[];
  dependencies: Dependency[];
  resources: Resource[];
  assignments: Assignment[];
  /** UIDs the engine gives the next task and resource; the UI names new items with them. */
  next_task_uid: number;
  next_resource_uid: number;
  overallocations: Overallocation[];
  usage: UsageRow[];
  /** Actual costs are entered by hand instead of computed from progress. */
  editable_actual_costs: boolean;
  /** Custom task fields, in the order they were added. */
  fields: FieldDef[];
  /** The order leveling places tasks in; empty for the default (start, priority, slack). */
  leveling_order: SortKey[];
  /** Skills resources can have, in the order they were added. */
  skills: SkillDef[];
  next_skill_id: number;
  /** Ordered rules from custom field values to a person or a skill. */
  assignment_rules: AssignmentRule[];
}

export interface Snapshot {
  project: Project;
  path: string | null;
  dirty: boolean;
  can_undo: boolean;
  can_redo: boolean;
  /** Commands applied and not undone; a dialog records it to roll back to on Hủy. */
  history_cursor: number;
}

export type Command =
  | { type: "rename_project"; name: string }
  | { type: "set_project_start"; start: number }
  | { type: "set_status_date"; date: number | null }
  | { type: "add_task"; name: string; after_uid: number | null }
  | { type: "add_milestone"; name: string; after_uid: number | null }
  | { type: "delete_task"; uid: number }
  | { type: "rename_task"; uid: number; name: string }
  | { type: "set_duration"; uid: number; minutes: number }
  | { type: "set_manual"; uid: number; manual: boolean; start: number | null; finish: number | null }
  | { type: "set_constraint"; uid: number; constraint_type: ConstraintType; date: number | null }
  | { type: "indent"; uid: number }
  | { type: "outdent"; uid: number }
  | { type: "set_predecessors"; uid: number; text: string }
  | { type: "add_dependency"; predecessor_uid: number; successor_uid: number; kind: DepKind; lag: number }
  | { type: "remove_dependency"; uid: number }
  | { type: "set_notes"; uid: number; notes: string }
  | { type: "set_priority"; uid: number; priority: number }
  | { type: "set_task_calendar"; uid: number; calendar_id: number | null }
  | { type: "add_holiday"; date: string }
  | { type: "remove_holiday"; date: string }
  | { type: "create_calendar"; name: string; base_id: number | null }
  | { type: "set_calendar_base"; calendar_id: number; base_id: number | null }
  | { type: "set_project_calendar"; calendar_id: number }
  | { type: "rename_calendar"; calendar_id: number; name: string }
  | { type: "set_calendar_week"; calendar_id: number; week: TimeRange[][] }
  | { type: "set_calendar_work_week"; calendar_id: number; name: string; from: string; to: string; week: TimeRange[][] }
  | { type: "clear_calendar_work_week"; calendar_id: number; from: string }
  | {
      type: "set_calendar_exceptions";
      calendar_id: number;
      from: string;
      to: string;
      ranges: TimeRange[];
    }
  | { type: "clear_calendar_exceptions"; calendar_id: number; from: string; to: string }
  | { type: "move_task"; uid: number; start: number }
  | { type: "resize_task"; uid: number; finish: number }
  | { type: "add_resource"; name: string; kind: ResourceKind }
  | {
      type: "update_resource";
      uid: number;
      name: string;
      kind: ResourceKind;
      max_units: number;
      calendar_id: number | null;
      standard_rate: number;
      overtime_rate: number;
      cost_per_use: number;
    }
  | { type: "delete_resource"; uid: number }
  | { type: "assign_resource"; task_uid: number; resource_uid: number; units: number }
  | { type: "update_assignment"; uid: number; units: number | null; work: number | null; overtime_work: number | null }
  | { type: "remove_assignment"; uid: number }
  | { type: "set_task_type"; uid: number; task_type: TaskType; effort_driven: boolean }
  | { type: "set_work"; uid: number; minutes: number }
  | { type: "set_percent_complete"; uid: number; percent: number }
  | { type: "set_actual_start"; uid: number; start: number | null }
  | { type: "set_actual_finish"; uid: number; finish: number | null }
  | { type: "set_remaining_duration"; uid: number; minutes: number }
  | { type: "set_remaining_work"; uid: number; minutes: number }
  | { type: "set_overtime_work"; uid: number; minutes: number }
  | { type: "set_fixed_cost"; uid: number; cost: number; accrual: Accrual }
  | { type: "set_baseline"; number: number }
  | { type: "clear_baseline"; number: number }
  | { type: "level_resources" }
  | {
      type: "apply_leveling";
      delays: LevelingDelay[];
      /** Tasks whose leveling splits changed, each with all of them. */
      splits?: LevelingSplits[];
    }
  | { type: "clear_leveling" }
  | { type: "reschedule_uncompleted" }
  | { type: "set_leveling_delay"; uid: number; minutes: number }
  | { type: "set_interrupts"; uid: number; interrupts: boolean }
  | { type: "set_deadline"; uid: number; date: number | null }
  | { type: "set_editable_actual_costs"; editable: boolean }
  | { type: "set_task_actual_cost"; uid: number; cost: number | null }
  | { type: "set_assignment_actual_cost"; uid: number; cost: number | null }
  | { type: "create_recurring_task"; after_uid: number | null; spec: RecurrenceSpec }
  | { type: "update_recurring_task"; uid: number; spec: RecurrenceSpec }
  | { type: "split_task"; uid: number; at: number; gap: number }
  | { type: "remove_split"; uid: number; index: number | null }
  | { type: "move_segment"; uid: number; index: number; start: number }
  | { type: "paste_tasks"; after_uid: number | null; clipboard: TaskClipboard }
  | { type: "fill_down"; from_uid: number; to_uids: number[]; field: FillField }
  | { type: "add_field"; name: string; kind: FieldKind }
  | {
      type: "update_field";
      id: number;
      name: string;
      kind: FieldKind;
      /** Choices renamed in place, so tasks keep them. */
      renamed_options: { from: string; to: string }[];
    }
  | { type: "delete_field"; id: number }
  | { type: "set_field_value"; uid: number; field_id: number; value: FieldValue | null }
  | { type: "set_leveling_order"; order: SortKey[] }
  | { type: "set_resource_availability"; uid: number; periods: AvailabilityPeriod[] }
  | { type: "add_skill"; name: string }
  | { type: "rename_skill"; id: number; name: string }
  | { type: "delete_skill"; id: number }
  | { type: "set_resource_skills"; uid: number; skills: number[] }
  | { type: "set_assignment_rules"; rules: AssignmentRule[] }
  | { type: "assign_tasks"; assignments: TaskAssignment[] }
  | { type: "import_rows"; after_uid: number | null; resources: ImportResource[]; tasks: ImportTask[] }
  | { type: "apply_plan"; assignments: TaskAssignment[]; delays: LevelingDelay[]; splits: LevelingSplits[] };

/** Mirror of `clipboard::TaskCopy`: one copied task, its level relative to the copy. */
export interface TaskCopy {
  name: string;
  level: number;
  milestone: boolean;
  manual: boolean;
  duration: number;
  work: number;
  start: number;
  finish: number;
  notes: string;
  priority: number;
  calendar_id: number | null;
  /** Matches the calendar by name when pasting into another project. */
  calendar_name: string | null;
  constraint_type: ConstraintType;
  constraint_date: number | null;
  deadline: number | null;
  task_type: TaskType;
  effort_driven: boolean;
  fixed_cost: number;
  fixed_cost_accrual: Accrual;
  splits: Split[];
  assignments: { resource_uid: number; resource_name: string; units: number; work: number }[];
  /** Custom field values by field name, as `[name, value]` pairs. */
  values: [string, FieldValue][];
}

/** Mirror of `clipboard::TaskClipboard`; links point at positions in `tasks`. */
export interface TaskClipboard {
  tasks: TaskCopy[];
  links: { from: number; to: number; kind: DepKind; lag: number }[];
}

/** Mirror of `clipboard::FillField`: the columns fill-down copies; `{ field }` is a custom field. */
export type FillField =
  | "name"
  | "duration"
  | "start"
  | "predecessors"
  | "percent_complete"
  | "calendar"
  | "interrupts"
  | { field: number };

export type ViewId = "gantt" | "tracking" | "resources" | "usage" | "network";

export interface NetworkOptions {
  include_summary: boolean;
  visible_uids: number[] | null;
}

// Network diagram geometry comes from the engine in pixels at zoom 1.
export interface NetworkNode {
  uid: number;
  column: number;
  row: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface NetworkPoint {
  x: number;
  y: number;
}

export interface NetworkEdge {
  /** The link drawn; a link to a hidden summary can be drawn as several edges. */
  uid: number;
  /** Boxes joined: a task inside a hidden summary, or a recurring series' box. */
  from_uid: number;
  to_uid: number;
  kind: DepKind;
  lag: number;
  points: NetworkPoint[];
}

export interface NetworkLayout {
  nodes: NetworkNode[];
  edges: NetworkEdge[];
  columns: number;
  width: number;
  height: number;
  crossings: number;
}

// EVM values come from the engine; null means undefined because a denominator was not above zero.
export interface EvmMetrics {
  bac: number;
  pv: number;
  ev: number;
  ac: number;
  sv: number;
  cv: number;
  spi: number | null;
  cpi: number | null;
  eac: number | null;
  etc: number | null;
  vac: number | null;
}

export interface EvmReport {
  status_date: number | null;
  has_baseline: boolean;
  metrics: EvmMetrics | null;
}

export interface BurndownPoint {
  date: number;
  remaining_work: number;
}

export interface BurndownSnapshot {
  date: number;
  remaining_work: number;
  ev: number;
  ac: number;
}

export interface BurndownReport {
  status_date: number | null;
  baseline_start: number | null;
  baseline_finish: number | null;
  baseline_work: number;
  ideal: BurndownPoint[];
  /** Baseline 0 work still planned over time; flat across split gaps. */
  planned: BurndownPoint[];
  actual: BurndownSnapshot[];
  current_remaining_work: number;
}

/** Cumulative values at one instant; EV and AC are null after the status date. */
export interface SCurvePoint {
  date: number;
  pv: number;
  ev: number | null;
  ac: number | null;
}

export interface SCurveReport {
  status_date: number | null;
  baseline_finish: number | null;
  step: "day" | "week";
  points: SCurvePoint[];
}

export interface BaselineStatistics {
  start: number;
  finish: number;
  duration: number;
  work: number;
  cost: number;
}

/** Totals for the Thống kê tab of Thông tin dự án, computed by the engine. */
export interface ProjectStatistics {
  start: number;
  finish: number;
  duration: number;
  work: number;
  actual_work: number;
  remaining_work: number;
  cost: number;
  actual_cost: number;
  remaining_cost: number;
  percent_complete: number;
  baseline: BaselineStatistics | null;
}

export interface ProjectMetrics {
  evm: EvmReport;
  burndown: BurndownReport;
  s_curve: SCurveReport;
  statistics: ProjectStatistics;
}
export type Scale = "day" | "week" | "month";

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

export interface Calendar {
  id: number;
  name: string;
  week: TimeRange[][];
  exceptions: Record<string, TimeRange[]>;
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
  conflict: boolean;
  predecessors: string;
  baselines: Baseline[];
  deadline: number | null;
  late: boolean;
  recurrence: Recurrence | null;
  series_uid: number | null;
  occurrence_date: number | null;
  detached: boolean;
  splits: Split[];
  /** Computed by the engine; empty when the task is not split. */
  segments: Segment[];
}

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
  overallocations: Overallocation[];
  usage: UsageRow[];
}

export interface Snapshot {
  project: Project;
  path: string | null;
  dirty: boolean;
  can_undo: boolean;
  can_redo: boolean;
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
  | { type: "create_calendar"; name: string }
  | { type: "rename_calendar"; calendar_id: number; name: string }
  | { type: "set_calendar_week"; calendar_id: number; week: TimeRange[][] }
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
  | { type: "clear_leveling" }
  | { type: "reschedule_uncompleted" }
  | { type: "set_leveling_delay"; uid: number; minutes: number }
  | { type: "set_deadline"; uid: number; date: number | null }
  | { type: "create_recurring_task"; after_uid: number | null; spec: RecurrenceSpec }
  | { type: "update_recurring_task"; uid: number; spec: RecurrenceSpec }
  | { type: "split_task"; uid: number; at: number; gap: number }
  | { type: "remove_split"; uid: number; index: number | null };

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
  uid: number;
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

export interface ProjectMetrics {
  evm: EvmReport;
  burndown: BurndownReport;
  s_curve: SCurveReport;
}
export type Scale = "day" | "week" | "month";

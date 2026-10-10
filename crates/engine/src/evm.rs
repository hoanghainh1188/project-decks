//! Earned value (EVM) and burn-down figures.
//!
//! Every figure sums leaf tasks only, so a summary never counts its children twice.
//! Planned and earned values read baseline 0; actual cost and remaining work read the
//! current schedule. The UI renders these numbers and does not recompute them.
//!
//! The engine stores no timephased actuals, only actual start/finish, % complete and
//! the actual cost phase 3 derives from them (labour at resource rates plus fixed cost,
//! following the task accrual). Actuals are therefore phased by assuming recorded
//! progress was made evenly in working time from the actual start up to the progress
//! point: p % complete is reached after p % of the duration on the task calendar, or at
//! the actual finish once the task is finished. Actual cost is spread over that window
//! with the task accrual, the same way PV is spread over the baseline window.

use chrono::Datelike;
use serde::{Deserialize, Serialize};

use crate::calendar::Calendar;
use crate::model::{Accrual, Baseline, Project, Task};
use crate::timeutil::{civil, compose};

const DAY_MS: i64 = 86_400_000;
/// Projects spanning more calendar days than this are sampled weekly.
pub const DAILY_CURVE_MAX_DAYS: i64 = 92;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvmMetrics {
    /// Budget at completion: total baseline 0 cost.
    pub bac: f64,
    /// Planned value (BCWS) at the status date.
    pub pv: f64,
    /// Earned value (BCWP): % complete × baseline 0 cost.
    pub ev: f64,
    /// Actual cost (ACWP) incurred up to the status date.
    pub ac: f64,
    pub sv: f64,
    pub cv: f64,
    /// Undefined when PV is not above zero.
    pub spi: Option<f64>,
    /// Undefined when AC is not above zero.
    pub cpi: Option<f64>,
    /// BAC / CPI; undefined when CPI is undefined or not above zero.
    pub eac: Option<f64>,
    pub etc: Option<f64>,
    pub vac: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvmReport {
    pub status_date: Option<i64>,
    /// At least one leaf task has baseline 0.
    pub has_baseline: bool,
    /// Present only when there is both a status date and a baseline 0.
    pub metrics: Option<EvmMetrics>,
}

/// One burn-down sample. `date` is local midnight of the sampled day.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BurndownSnapshot {
    pub date: i64,
    /// Working minutes.
    pub remaining_work: i64,
    pub ev: f64,
    pub ac: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BurndownPoint {
    pub date: i64,
    /// Working minutes.
    pub remaining_work: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BurndownReport {
    pub status_date: Option<i64>,
    pub baseline_start: Option<i64>,
    pub baseline_finish: Option<i64>,
    /// Total baseline 0 work of leaf tasks, in working minutes.
    pub baseline_work: i64,
    /// Straight line from (baseline start, baseline work) to (baseline finish, 0); empty without baseline 0.
    pub ideal: Vec<BurndownPoint>,
    /// Stored snapshots up to the status date, oldest first.
    pub actual: Vec<BurndownSnapshot>,
    /// Remaining work of the current schedule, in working minutes.
    pub current_remaining_work: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveStep {
    Day,
    Week,
}

/// Cumulative values at one instant. EV and AC exist only up to the status date.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SCurvePoint {
    pub date: i64,
    pub pv: f64,
    pub ev: Option<f64>,
    pub ac: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SCurveReport {
    pub status_date: Option<i64>,
    pub baseline_finish: Option<i64>,
    pub step: CurveStep,
    /// Samples at local midnight every day or week, plus the status date and the
    /// baseline finish; empty without baseline 0.
    pub points: Vec<SCurvePoint>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectMetrics {
    pub evm: EvmReport,
    pub burndown: BurndownReport,
    pub s_curve: SCurveReport,
}

fn leaves(project: &Project) -> impl Iterator<Item = &Task> {
    project.tasks.iter().filter(|task| !task.summary)
}

fn baseline_zero(task: &Task) -> Option<&Baseline> {
    task.baselines.iter().find(|baseline| baseline.number == 0)
}

pub fn has_baseline(project: &Project) -> bool {
    leaves(project).any(|task| baseline_zero(task).is_some())
}

/// Share of an amount accrued by `at` over the window `[start, finish]`.
/// Prorated accrual follows working minutes on `calendar`; a zero-length window
/// (milestone) accrues everything once its finish is reached.
fn accrued_fraction(calendar: &Calendar, accrual: Accrual, start: i64, finish: i64, at: i64) -> f64 {
    match accrual {
        Accrual::Start => {
            if at >= start {
                1.0
            } else {
                0.0
            }
        }
        Accrual::End => {
            if at >= finish {
                1.0
            } else {
                0.0
            }
        }
        Accrual::Prorated => {
            let total = calendar.working_between(start, finish);
            if total <= 0 {
                return if at >= finish { 1.0 } else { 0.0 };
            }
            let until = at.clamp(start, finish);
            let elapsed = calendar.working_between(start, until);
            (elapsed as f64 / total as f64).clamp(0.0, 1.0)
        }
    }
}

/// Share of the baseline cost planned to be spent by `status`, following the task accrual.
pub fn planned_fraction(project: &Project, task: &Task, baseline: &Baseline, status: i64) -> f64 {
    accrued_fraction(
        project.task_calendar(task),
        task.fixed_cost_accrual,
        baseline.start,
        baseline.finish,
        status,
    )
}

/// Working-time window over which the recorded progress of `task` is assumed to have
/// happened (see the module notes). `None` when the task has neither progress nor cost.
pub fn progress_window(project: &Project, task: &Task) -> Option<(i64, i64)> {
    let percent = task.percent_complete.clamp(0.0, 100.0);
    if percent <= 0.0 && task.actual_cost <= 0.0 {
        return None;
    }
    let start = task.actual_start.unwrap_or(task.start);
    let finish = match task.actual_finish {
        Some(finish) if percent >= 100.0 => finish,
        _ => {
            let done = (task.duration.max(0) as f64 * percent / 100.0).round() as i64;
            project.task_calendar(task).add_working(start, done)
        }
    };
    Some((start, finish.max(start)))
}

/// Actual cost of `task` incurred by `at`: its current actual cost spread over the
/// progress window with the task accrual.
pub fn actual_cost_at(project: &Project, task: &Task, at: i64) -> f64 {
    let Some((start, finish)) = progress_window(project, task) else {
        return 0.0;
    };
    let calendar = project.task_calendar(task);
    task.actual_cost * accrued_fraction(calendar, task.fixed_cost_accrual, start, finish, at)
}

/// Earned value of `task` by `at` for the S-curve. Earning follows progress, not the
/// cost accrual, and the window is cut at the status date so the curve reaches the
/// reported EV (% complete × baseline cost) on that date.
fn earned_at(project: &Project, task: &Task, baseline: &Baseline, at: i64, status: i64) -> f64 {
    let percent = task.percent_complete.clamp(0.0, 100.0);
    if percent <= 0.0 {
        return 0.0;
    }
    let Some((start, finish)) = progress_window(project, task) else {
        return 0.0;
    };
    let (start, finish) = (start.min(status), finish.min(status));
    let calendar = project.task_calendar(task);
    let fraction = accrued_fraction(calendar, Accrual::Prorated, start, finish, at);
    baseline.cost * percent / 100.0 * fraction
}

fn positive_ratio(numerator: f64, denominator: f64) -> Option<f64> {
    (denominator > 0.0).then(|| numerator / denominator)
}

/// EVM figures as of `status`, whether or not the project stores that status date.
pub fn evm_at(project: &Project, status: i64) -> EvmMetrics {
    let mut bac = 0.0;
    let mut pv = 0.0;
    let mut ev = 0.0;
    let mut ac = 0.0;
    for task in leaves(project) {
        ac += actual_cost_at(project, task, status);
        let Some(baseline) = baseline_zero(task) else {
            continue;
        };
        bac += baseline.cost;
        pv += baseline.cost * planned_fraction(project, task, baseline, status);
        ev += baseline.cost * task.percent_complete.clamp(0.0, 100.0) / 100.0;
    }
    let spi = positive_ratio(ev, pv);
    let cpi = positive_ratio(ev, ac);
    let eac = cpi.and_then(|cpi| positive_ratio(bac, cpi));
    EvmMetrics {
        bac,
        pv,
        ev,
        ac,
        sv: ev - pv,
        cv: ev - ac,
        spi,
        cpi,
        eac,
        etc: eac.map(|eac| eac - ac),
        vac: eac.map(|eac| bac - eac),
    }
}

pub fn evm(project: &Project) -> EvmReport {
    let has_baseline = has_baseline(project);
    let metrics = match project.status_date {
        Some(status) if has_baseline => Some(evm_at(project, status)),
        _ => None,
    };
    EvmReport {
        status_date: project.status_date,
        has_baseline,
        metrics,
    }
}

pub fn day_start(instant: i64) -> i64 {
    compose(civil(instant).0, 0)
}

fn earned_value(project: &Project) -> f64 {
    leaves(project)
        .filter_map(|task| {
            baseline_zero(task)
                .map(|baseline| baseline.cost * task.percent_complete.clamp(0.0, 100.0) / 100.0)
        })
        .sum()
}

pub fn remaining_work(project: &Project) -> i64 {
    leaves(project).map(|task| task.remaining_work.max(0)).sum()
}

/// Sample the project for the day containing `date`; AC is the cost incurred by `date`.
pub fn take_snapshot(project: &Project, date: i64) -> BurndownSnapshot {
    BurndownSnapshot {
        date: day_start(date),
        remaining_work: remaining_work(project),
        ev: earned_value(project),
        ac: leaves(project).map(|task| actual_cost_at(project, task, date)).sum(),
    }
}

/// Insert keeping date order; a snapshot for an existing day replaces it.
pub fn record_snapshot(snapshots: &mut Vec<BurndownSnapshot>, snapshot: BurndownSnapshot) {
    match snapshots.binary_search_by_key(&snapshot.date, |existing| existing.date) {
        Ok(index) => snapshots[index] = snapshot,
        Err(index) => snapshots.insert(index, snapshot),
    }
}

pub fn burndown(project: &Project, snapshots: &[BurndownSnapshot]) -> BurndownReport {
    let mut baseline_start: Option<i64> = None;
    let mut baseline_finish: Option<i64> = None;
    let mut baseline_work = 0i64;
    for baseline in leaves(project).filter_map(baseline_zero) {
        baseline_start = Some(baseline_start.map_or(baseline.start, |s| s.min(baseline.start)));
        baseline_finish = Some(baseline_finish.map_or(baseline.finish, |f| f.max(baseline.finish)));
        baseline_work += baseline.work.max(0);
    }
    let ideal = match (baseline_start, baseline_finish) {
        (Some(start), Some(finish)) => vec![
            BurndownPoint {
                date: start,
                remaining_work: baseline_work,
            },
            BurndownPoint {
                date: finish,
                remaining_work: 0,
            },
        ],
        _ => Vec::new(),
    };
    let mut actual: Vec<BurndownSnapshot> = snapshots
        .iter()
        .filter(|snapshot| {
            project
                .status_date
                .is_none_or(|status| snapshot.date <= status)
        })
        .cloned()
        .collect();
    actual.sort_by_key(|snapshot| snapshot.date);
    BurndownReport {
        status_date: project.status_date,
        baseline_start,
        baseline_finish,
        baseline_work,
        ideal,
        actual,
        current_remaining_work: remaining_work(project),
    }
}

fn curve_instants(start: i64, end: i64, step: CurveStep, extra: &[i64]) -> Vec<i64> {
    let mut first = day_start(start);
    let step_ms = match step {
        CurveStep::Day => DAY_MS,
        CurveStep::Week => {
            let weekday = i64::from(civil(first).0.weekday().num_days_from_monday());
            first -= weekday * DAY_MS;
            7 * DAY_MS
        }
    };
    let mut instants = Vec::new();
    let mut cursor = first;
    while cursor < end {
        instants.push(cursor);
        cursor += step_ms;
    }
    instants.push(end);
    instants.extend_from_slice(extra);
    instants.sort_unstable();
    instants.dedup();
    instants
}

/// Cumulative PV through the baseline finish, and EV and AC up to the status date.
pub fn s_curve(project: &Project) -> SCurveReport {
    let status = project.status_date;
    let mut baseline_start: Option<i64> = None;
    let mut baseline_finish: Option<i64> = None;
    for baseline in leaves(project).filter_map(baseline_zero) {
        baseline_start = Some(baseline_start.map_or(baseline.start, |s| s.min(baseline.start)));
        baseline_finish = Some(baseline_finish.map_or(baseline.finish, |f| f.max(baseline.finish)));
    }
    let (Some(baseline_start), Some(finish)) = (baseline_start, baseline_finish) else {
        return SCurveReport {
            status_date: status,
            baseline_finish: None,
            step: CurveStep::Day,
            points: Vec::new(),
        };
    };
    let mut start = baseline_start;
    if let Some(status) = status {
        for (window_start, _) in leaves(project).filter_map(|task| progress_window(project, task)) {
            start = start.min(window_start.min(status));
        }
    }
    let end = status.map_or(finish, |status| status.max(finish));
    let span_days = (day_start(end) - day_start(start)) / DAY_MS;
    let step = if span_days <= DAILY_CURVE_MAX_DAYS {
        CurveStep::Day
    } else {
        CurveStep::Week
    };
    let extra: Vec<i64> = status.into_iter().chain([finish]).collect();
    let points = curve_instants(start, end, step, &extra)
        .into_iter()
        .map(|at| {
            let mut pv = 0.0;
            for task in leaves(project) {
                if let Some(baseline) = baseline_zero(task) {
                    pv += baseline.cost * planned_fraction(project, task, baseline, at);
                }
            }
            let (ev, ac) = match status {
                Some(status) if at <= status => {
                    let mut ev = 0.0;
                    let mut ac = 0.0;
                    for task in leaves(project) {
                        ac += actual_cost_at(project, task, at);
                        if let Some(baseline) = baseline_zero(task) {
                            ev += earned_at(project, task, baseline, at, status);
                        }
                    }
                    (Some(ev), Some(ac))
                }
                _ => (None, None),
            };
            SCurvePoint { date: at, pv, ev, ac }
        })
        .collect();
    SCurveReport {
        status_date: status,
        baseline_finish: Some(finish),
        step,
        points,
    }
}

pub fn metrics(project: &Project, snapshots: &[BurndownSnapshot]) -> ProjectMetrics {
    ProjectMetrics {
        evm: evm(project),
        burndown: burndown(project, snapshots),
        s_curve: s_curve(project),
    }
}

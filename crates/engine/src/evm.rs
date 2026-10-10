//! Earned value (EVM) and burn-down figures.
//!
//! Every figure sums leaf tasks only, so a summary never counts its children twice.
//! Planned and earned values read baseline 0; actual cost and remaining work read the
//! current schedule. The UI renders these numbers and does not recompute them.

use serde::{Deserialize, Serialize};

use crate::model::{Accrual, Baseline, Project, Task};
use crate::timeutil::{civil, compose};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvmMetrics {
    /// Budget at completion: total baseline 0 cost.
    pub bac: f64,
    /// Planned value (BCWS) at the status date.
    pub pv: f64,
    /// Earned value (BCWP): % complete × baseline 0 cost.
    pub ev: f64,
    /// Actual cost (ACWP).
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectMetrics {
    pub evm: EvmReport,
    pub burndown: BurndownReport,
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

/// Share of the baseline cost planned to be spent by `status`, following the task accrual.
pub fn planned_fraction(project: &Project, task: &Task, baseline: &Baseline, status: i64) -> f64 {
    match task.fixed_cost_accrual {
        Accrual::Start => {
            if status >= baseline.start {
                1.0
            } else {
                0.0
            }
        }
        Accrual::End => {
            if status >= baseline.finish {
                1.0
            } else {
                0.0
            }
        }
        Accrual::Prorated => {
            let calendar = project.task_calendar(task);
            let total = calendar.working_between(baseline.start, baseline.finish);
            if total <= 0 {
                // Zero-length baseline (milestone): all of it is planned once its finish is reached.
                return if status >= baseline.finish { 1.0 } else { 0.0 };
            }
            let until = status.clamp(baseline.start, baseline.finish);
            let elapsed = calendar.working_between(baseline.start, until);
            (elapsed as f64 / total as f64).clamp(0.0, 1.0)
        }
    }
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
        ac += task.actual_cost;
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

/// Sample the project for the day containing `date`.
pub fn take_snapshot(project: &Project, date: i64) -> BurndownSnapshot {
    BurndownSnapshot {
        date: day_start(date),
        remaining_work: remaining_work(project),
        ev: earned_value(project),
        ac: leaves(project).map(|task| task.actual_cost).sum(),
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

pub fn metrics(project: &Project, snapshots: &[BurndownSnapshot]) -> ProjectMetrics {
    ProjectMetrics {
        evm: evm(project),
        burndown: burndown(project, snapshots),
    }
}

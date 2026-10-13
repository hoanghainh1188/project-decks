//! Project statistics for the "Thống kê" tab of Project Information, as Microsoft
//! Project shows them: dates, duration, work and cost, current against baseline 0.

use serde::{Deserialize, Serialize};

use crate::model::{Project, Task};
use crate::tracking;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectStatistics {
    /// Earliest start and latest finish of the tasks; the project start when empty.
    pub start: i64,
    pub finish: i64,
    /// Working minutes between start and finish on the project calendar.
    pub duration: i64,
    pub work: i64,
    pub actual_work: i64,
    pub remaining_work: i64,
    pub cost: f64,
    pub actual_cost: f64,
    pub remaining_cost: f64,
    /// Done share of the work, or of the duration when nothing has work.
    pub percent_complete: f64,
    /// Baseline 0 totals, when any task has one.
    pub baseline: Option<BaselineStatistics>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BaselineStatistics {
    pub start: i64,
    pub finish: i64,
    pub duration: i64,
    pub work: i64,
    pub cost: f64,
}

pub fn statistics(project: &Project) -> ProjectStatistics {
    let leaves: Vec<&Task> = project.tasks.iter().filter(|task| !task.summary).collect();
    // Dates as the schedule's own horizon: background series count only when nothing
    // else is left.
    let background = crate::schedule::background_tasks(project);
    let foreground: Vec<&Task> = project
        .tasks
        .iter()
        .zip(&background)
        .filter(|(task, background)| !task.summary && !**background)
        .map(|(task, _)| task)
        .collect();
    let dated = if foreground.is_empty() {
        &leaves
    } else {
        &foreground
    };
    // Top-level rows already hold their subtree's totals, a summary's fixed cost included.
    let top: Vec<&Task> = project
        .tasks
        .iter()
        .filter(|task| task.parent_uid.is_none())
        .collect();
    let start = dated
        .iter()
        .map(|task| task.start)
        .min()
        .unwrap_or(project.start);
    let finish = dated.iter().map(|task| task.finish).max().unwrap_or(start);
    let calendar = project.calendar();
    let work: i64 = top.iter().map(|task| task.work).sum();
    let actual_work: i64 = top.iter().map(|task| task.actual_work).sum();
    let percent_complete = if work > 0 {
        actual_work as f64 / work as f64 * 100.0
    } else {
        let total: i64 = leaves.iter().map(|task| task.duration.max(0)).sum();
        let done: i64 = leaves
            .iter()
            .map(|task| {
                if tracking::is_complete(task) {
                    task.duration.max(0)
                } else {
                    tracking::done_minutes(task)
                }
            })
            .sum();
        if total > 0 {
            done as f64 / total as f64 * 100.0
        } else {
            0.0
        }
    };
    let zero = |task: &&Task| {
        task.baselines
            .iter()
            .find(|baseline| baseline.number == 0)
            .cloned()
    };
    // Dates from the tasks, totals from the top rows like the current ones, so a
    // summary's fixed cost is on both sides.
    let dated_baselines: Vec<_> = dated.iter().filter_map(zero).collect();
    let top_baselines: Vec<_> = top.iter().filter_map(zero).collect();
    let baseline = (!dated_baselines.is_empty()).then(|| {
        let start = dated_baselines
            .iter()
            .map(|baseline| baseline.start)
            .min()
            .unwrap_or(start);
        let finish = dated_baselines
            .iter()
            .map(|baseline| baseline.finish)
            .max()
            .unwrap_or(start);
        BaselineStatistics {
            start,
            finish,
            duration: calendar.working_between(start, finish).max(0),
            work: top_baselines.iter().map(|baseline| baseline.work).sum(),
            cost: top_baselines.iter().map(|baseline| baseline.cost).sum(),
        }
    });
    ProjectStatistics {
        start,
        finish,
        duration: calendar.working_between(start, finish).max(0),
        work,
        actual_work,
        remaining_work: top.iter().map(|task| task.remaining_work).sum(),
        cost: top.iter().map(|task| task.cost).sum(),
        actual_cost: top.iter().map(|task| task.actual_cost).sum(),
        remaining_cost: top.iter().map(|task| task.remaining_cost).sum(),
        percent_complete: percent_complete.clamp(0.0, 100.0),
        baseline,
    }
}

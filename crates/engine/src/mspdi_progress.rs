//! MSPDI deadline and progress fields: `Deadline`, `ActualDuration`, `ActualWork`,
//! `RemainingWork`, `Resume`, `Stop`, and timephased actual work (`TimephasedData`
//! type 2).
//!
//! When reading, the file's actual dates and remaining amounts win over its percent
//! complete, as in Microsoft Project: an actual finish completes the task, and the done
//! share comes from actual/remaining duration, else actual/remaining work, else the
//! actual work found in the timephased data.

use crate::calendar::Calendar;
use crate::model::{Project, Task};
use crate::split;
use crate::timeutil::{format_duration_iso, format_iso, parse_duration_iso, parse_iso};
use crate::tracking;

/// Progress fields read from one `<Task>`, beyond those `mspdi.rs` already reads.
#[derive(Clone, Default)]
pub(crate) struct TaskProgress {
    pub deadline: Option<i64>,
    pub actual_duration: Option<i64>,
    pub actual_work: Option<i64>,
    pub actual_cost: Option<f64>,
    pub remaining_work: Option<i64>,
}

impl TaskProgress {
    /// Read a `<Task>` child element; false when the element is not one of ours.
    pub fn read(&mut self, name: &str, value: &str) -> bool {
        match name {
            "Deadline" => self.deadline = parse_iso(value),
            "ActualDuration" => self.actual_duration = parse_duration_iso(value),
            "ActualWork" => self.actual_work = parse_duration_iso(value),
            "ActualCost" => {
                self.actual_cost = value.parse().ok().filter(|cost: &f64| cost.is_finite());
            }
            "RemainingWork" => self.remaining_work = parse_duration_iso(value),
            _ => return false,
        }
        true
    }
}

/// Actual work found in an assignment's type 2 timephased data, per task.
#[derive(Clone, Copy, Default)]
pub(crate) struct TimephasedActual {
    pub first_start: Option<i64>,
    pub minutes: i64,
}

impl TimephasedActual {
    pub fn add(&mut self, start: i64, minutes: i64) {
        if minutes <= 0 {
            return;
        }
        self.minutes += minutes;
        self.first_start = Some(self.first_start.map_or(start, |first| first.min(start)));
    }
}

/// The done share in percent, from what the file says about the task, or `None` when
/// it only gives the percent complete. `duration` is the task's `Duration`, `work` the
/// task work known so far, `remaining_duration` its `RemainingDuration`.
pub(crate) fn percent_from_file(
    progress: &TaskProgress,
    timephased: Option<TimephasedActual>,
    duration: i64,
    remaining_duration: Option<i64>,
    actual_finish: Option<i64>,
) -> Option<f64> {
    if actual_finish.is_some() {
        return Some(100.0);
    }
    let share = |done: i64, left: i64| {
        let total = done + left;
        (total > 0).then(|| (done as f64 / total as f64 * 100.0).clamp(0.0, 100.0))
    };
    match (progress.actual_duration, remaining_duration) {
        (Some(done), Some(left)) => return share(done, left),
        (None, Some(left)) if duration > 0 && left <= duration => {
            return share(duration - left, left)
        }
        _ => {}
    }
    if let (Some(done), Some(left)) = (progress.actual_work, progress.remaining_work) {
        return share(done, left);
    }
    let actual = timephased.filter(|actual| actual.minutes > 0)?;
    let left = progress.remaining_work?;
    share(actual.minutes, left)
}

/// Done working minutes of the duration.
fn done_of(task: &Task) -> i64 {
    if tracking::is_complete(task) {
        task.duration.max(0)
    } else {
        tracking::done_minutes(task)
    }
}

/// `Stop` then `Resume`, written after `Duration` as the schema orders them: where the
/// done work ends and where the rest goes on. Only for a task in progress.
pub(crate) fn write_stop_resume(xml: &mut String, project: &Project, task: &Task) {
    if task.summary || task.actual_start.is_none() || tracking::is_complete(task) {
        return;
    }
    let cal = project.task_calendar(task);
    let done = done_of(task);
    el(xml, "Stop", &format_iso(stop_at(cal, task, done)));
    el(
        xml,
        "Resume",
        &format_iso(split::resume_at(cal, task, done)),
    );
}

/// `Deadline`, written after `ConstraintDate`.
pub(crate) fn write_deadline(xml: &mut String, task: &Task) {
    if let Some(deadline) = task.deadline {
        el(xml, "Deadline", &format_iso(deadline));
    }
}

/// The actual and remaining amounts, written after `RemainingDuration`.
pub(crate) fn write_task_progress(xml: &mut String, task: &Task) {
    el(xml, "ActualDuration", &format_duration_iso(done_of(task)));
    el(xml, "ActualWork", &format_duration_iso(task.actual_work));
    el(
        xml,
        "RemainingWork",
        &format_duration_iso(task.remaining_work),
    );
}

/// The instant the first `done` working minutes of the task end.
fn stop_at(cal: &Calendar, task: &Task, done: i64) -> i64 {
    if done <= 0 {
        return task.actual_start.unwrap_or(task.start);
    }
    let mut left = done;
    for (start, finish) in split::work_intervals(task) {
        let length = cal.working_between(start, finish).max(0);
        if left <= length {
            return cal.add_working(start, left);
        }
        left -= length;
    }
    task.finish
}

/// Type 2 (actual work) periods of a started task, one per worked interval up to the
/// stop date, sharing `actual_work` by working time.
pub(crate) fn write_actual_timephased(
    xml: &mut String,
    project: &Project,
    task: &Task,
    assignment_uid: u32,
    actual_work: i64,
) {
    if task.summary || task.actual_start.is_none() || actual_work <= 0 {
        return;
    }
    let cal = project.task_calendar(task);
    let done = done_of(task);
    let stop = if tracking::is_complete(task) {
        task.finish
    } else {
        stop_at(cal, task, done)
    };
    let pieces: Vec<(i64, i64, i64)> = split::work_intervals(task)
        .into_iter()
        .filter_map(|(start, finish)| {
            let finish = finish.min(stop);
            let minutes = cal.working_between(start, finish);
            (minutes > 0).then_some((start, finish, minutes))
        })
        .collect();
    let total: i64 = pieces.iter().map(|piece| piece.2).sum();
    if pieces.is_empty() || total <= 0 {
        // A milestone or a task done in no working time: one period at its start.
        let start = task.actual_start.unwrap_or(task.start);
        write_period(xml, assignment_uid, start, start.max(stop), actual_work);
        return;
    }
    let mut assigned = 0i64;
    for (index, (start, finish, minutes)) in pieces.iter().enumerate() {
        let value = if index + 1 == pieces.len() {
            actual_work - assigned
        } else {
            (actual_work as f64 * *minutes as f64 / total as f64).round() as i64
        };
        assigned += value;
        write_period(xml, assignment_uid, *start, *finish, value.max(0));
    }
}

fn write_period(xml: &mut String, assignment_uid: u32, start: i64, finish: i64, minutes: i64) {
    xml.push_str("<TimephasedData>\n");
    el(xml, "Type", "2");
    el(xml, "UID", &assignment_uid.to_string());
    el(xml, "Start", &format_iso(start));
    el(xml, "Finish", &format_iso(finish));
    el(xml, "Unit", "2");
    el(xml, "Value", &format_duration_iso(minutes));
    xml.push_str("</TimephasedData>\n");
}

fn el(xml: &mut String, name: &str, value: &str) {
    xml.push('<');
    xml.push_str(name);
    xml.push('>');
    xml.push_str(value);
    xml.push_str("</");
    xml.push_str(name);
    xml.push_str(">\n");
}

/// What a `<Task>` said about its progress, kept until its assignments are read.
pub(crate) struct FileProgress {
    pub progress: TaskProgress,
    pub duration: i64,
    pub remaining_duration: Option<i64>,
    pub actual_start: Option<i64>,
    pub actual_finish: Option<i64>,
    pub start: Option<i64>,
    pub percent: f64,
    /// The file has a `PercentComplete`.
    pub percent_given: bool,
}

impl FileProgress {
    /// Actual dates and remaining amounts win over the percent complete (Phase 13).
    /// Returns the file's percent and the one used when the file gave one and they
    /// differ by more than a point.
    pub fn apply(&self, task: &mut Task, actual: Option<TimephasedActual>) -> Option<(f64, f64)> {
        if task.summary {
            return None;
        }
        let percent = percent_from_file(
            &self.progress,
            actual,
            self.duration,
            self.remaining_duration,
            self.actual_finish,
        )?;
        // Keep the file's percent when it agrees within a minute of rounding.
        let minute = if self.duration > 0 {
            100.0 / self.duration as f64
        } else {
            0.0
        };
        let percent = if (percent - self.percent).abs() <= minute && percent < 100.0 {
            self.percent
        } else {
            percent
        };
        task.percent_complete = percent;
        let differs = (self.percent_given && (percent - self.percent).abs() > 1.0)
            .then_some((self.percent, percent));
        if percent <= 0.0 {
            // Started only if the file says so.
            task.actual_start = self.actual_start;
            task.actual_finish = None;
            return differs;
        }
        if task.actual_start.is_none() {
            let first = actual.and_then(|actual| actual.first_start);
            // Without a start in the file the task has no date yet: leave it unstarted
            // in dates and let the percent stand.
            task.actual_start = first.or(self.start);
        }
        if percent < 100.0 {
            task.actual_finish = None;
        } else if task.actual_finish.is_none() {
            task.actual_finish = Some(task.finish);
        }
        differs
    }
}

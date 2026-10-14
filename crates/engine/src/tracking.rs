//! Actual progress, the way Microsoft Project treats it.
//!
//! An actual start pins the start, whatever the links and constraints say, and an actual
//! finish pins the finish. The remaining work runs straight on from the work done, so an
//! in-progress task keeps one bar from its actual start. A complete task is never
//! critical, and a task is behind the status date only when less of it is done than
//! its schedule says should be by then.
//!
//! "% complete" is the done share of the duration. Remaining duration and remaining
//! work edits keep that done part and change only what is left.

use crate::calendar::Calendar;
use crate::error::EngineError;
use crate::model::{ConstraintType, Project, Task};
use crate::split::{self, Split};
use crate::work::{self, Changed};

/// Finished: an actual finish, or 100 % done.
pub fn is_complete(task: &Task) -> bool {
    task.actual_finish.is_some() || task.percent_complete >= 100.0
}

/// Working minutes of the duration already done.
pub fn done_minutes(task: &Task) -> i64 {
    let percent = task.percent_complete.clamp(0.0, 100.0);
    (task.duration.max(0) as f64 * percent / 100.0).round() as i64
}

/// Start and finish fixed by actual dates, or `None` when the task has not started.
/// Without an actual finish the work runs on from the actual start, gaps included.
pub fn pinned_dates(task: &Task, cal: &Calendar) -> Option<(i64, i64)> {
    let start = task.actual_start?;
    let finish = match task.actual_finish {
        Some(finish) => finish.max(start),
        None => {
            let span = split::span(task);
            if span <= 0 {
                start
            } else {
                cal.add_working(start, span)
            }
        }
    };
    Some((start, finish))
}

/// Less is done than the schedule says should be by `status`: the work scheduled before
/// the status date is more than the work done. A future task is never behind.
pub fn behind_schedule(task: &Task, cal: &Calendar, status: i64) -> bool {
    if task.summary || is_complete(task) || task.start >= status {
        return false;
    }
    if task.duration <= 0 {
        // A milestone due before the status date and not reached.
        return true;
    }
    let expected = split::working_within(cal, &split::work_intervals(task), task.start, status);
    done_minutes(task) < expected
}

pub(crate) fn set_percent_complete(
    project: &mut Project,
    uid: u32,
    percent: f64,
) -> Result<(), EngineError> {
    let task = project.task_mut(uid).ok_or(EngineError::TaskNotFound)?;
    if task.summary {
        return Ok(());
    }
    let percent = percent.clamp(0.0, 100.0);
    task.percent_complete = percent;
    if percent <= 0.0 {
        task.actual_start = None;
        task.actual_finish = None;
    } else if task.actual_start.is_none() {
        task.actual_start = Some(task.start);
    }
    task.actual_finish = if percent >= 100.0 {
        Some(task.finish)
    } else {
        None
    };
    Ok(())
}

pub(crate) fn set_actual_start(
    project: &mut Project,
    uid: u32,
    start: Option<i64>,
) -> Result<(), EngineError> {
    let task = project.task(uid).ok_or(EngineError::TaskNotFound)?;
    if task.summary {
        return Ok(());
    }
    let cal = project.task_calendar(task).clone();
    let task = project.task_mut(uid).ok_or(EngineError::TaskNotFound)?;
    if let (Some(start), Some(_)) = (start, task.actual_finish) {
        // As in Project: a finished task keeps its actual duration and moves its finish.
        let span = split::span(task);
        task.actual_start = Some(start);
        task.actual_finish = Some(if span > 0 {
            cal.add_working(start, span)
        } else {
            start
        });
        return Ok(());
    }
    task.actual_start = start;
    if start.is_some() && task.percent_complete <= 0.0 {
        task.percent_complete = 1.0;
    }
    if start.is_none() {
        task.percent_complete = 0.0;
        task.actual_finish = None;
    }
    Ok(())
}

/// An actual finish completes the task, and the duration becomes the working time
/// between the actual dates, so the bar ends on the date entered.
pub(crate) fn set_actual_finish(
    project: &mut Project,
    uid: u32,
    finish: Option<i64>,
) -> Result<(), EngineError> {
    let task = project.task(uid).ok_or(EngineError::TaskNotFound)?;
    if task.summary {
        return Ok(());
    }
    let Some(finish) = finish else {
        let task = project.task_mut(uid).ok_or(EngineError::TaskNotFound)?;
        task.actual_finish = None;
        if task.percent_complete >= 100.0 {
            task.percent_complete = 0.0;
        }
        return Ok(());
    };
    let start = task.actual_start.unwrap_or(task.start);
    fit_duration(project, uid, start, finish)
}

/// Record both actual dates of a finished task and size the duration to the working
/// time between them, less any split gaps. A milestone keeps no duration.
fn fit_duration(
    project: &mut Project,
    uid: u32,
    start: i64,
    finish: i64,
) -> Result<(), EngineError> {
    let task = project.task(uid).ok_or(EngineError::TaskNotFound)?;
    let worked = project.task_calendar(task).working_between(start, finish);
    if finish < start || (worked <= 0 && !task.milestone) {
        return Err(EngineError::ActualFinishBeforeStart);
    }
    let task = project.task_mut(uid).ok_or(EngineError::TaskNotFound)?;
    task.actual_start = Some(start);
    task.actual_finish = Some(finish);
    task.percent_complete = 100.0;
    if task.milestone {
        return Ok(());
    }
    split::normalize(&mut task.splits, worked);
    task.duration = (worked - split::total_gap(&task.splits)).max(1);
    work::apply_change(project, uid, Changed::Duration);
    Ok(())
}

/// Keep the done part: the duration becomes done + `minutes`. Before the task starts
/// this simply sets the duration.
pub(crate) fn set_remaining_duration(
    project: &mut Project,
    uid: u32,
    minutes: i64,
) -> Result<(), EngineError> {
    let task = project.task(uid).ok_or(EngineError::TaskNotFound)?;
    if task.summary {
        return Ok(());
    }
    let cal = project.task_calendar(task).clone();
    let remaining = minutes.max(0);
    let task = project.task_mut(uid).ok_or(EngineError::TaskNotFound)?;
    if task.actual_start.is_none() && task.percent_complete <= 0.0 {
        task.duration = remaining;
        task.percent_complete = 0.0;
    } else {
        let done = done_minutes(task);
        task.duration = done + remaining;
        settle_progress(task, &cal, done, task.duration, remaining);
    }
    task.milestone = task.duration == 0;
    work::apply_change(project, uid, Changed::Duration);
    Ok(())
}

/// Keep the done work: the work becomes done + `minutes`. Before the task starts this
/// simply sets the work.
pub(crate) fn set_remaining_work(
    project: &mut Project,
    uid: u32,
    minutes: i64,
    set_work: impl Fn(&mut Project, u32, i64) -> Result<(), EngineError>,
) -> Result<(), EngineError> {
    let task = project.task(uid).ok_or(EngineError::TaskNotFound)?;
    if task.summary {
        return Ok(());
    }
    let cal = project.task_calendar(task).clone();
    let remaining = minutes.max(0);
    if task.work <= 0 || (task.actual_start.is_none() && task.percent_complete <= 0.0) {
        set_work(project, uid, remaining)?;
        if let Some(task) = project.task_mut(uid) {
            task.percent_complete = 0.0;
        }
        return Ok(());
    }
    let percent = task.percent_complete.clamp(0.0, 100.0);
    let done_work = (task.work as f64 * percent / 100.0).round() as i64;
    set_work(project, uid, done_work + remaining)?;
    let task = project.task_mut(uid).ok_or(EngineError::TaskNotFound)?;
    settle_progress(task, &cal, done_work, done_work + remaining, remaining);
    Ok(())
}

/// Percent from `done` out of `total`; nothing remaining completes the task on the date
/// its work runs out.
fn settle_progress(task: &mut Task, cal: &Calendar, done: i64, total: i64, remaining: i64) {
    task.percent_complete = if total > 0 {
        (done as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        100.0
    };
    if remaining > 0 {
        task.actual_finish = None;
        return;
    }
    task.percent_complete = 100.0;
    let start = *task.actual_start.get_or_insert(task.start);
    let span = split::span(task);
    task.actual_finish = Some(if span > 0 {
        cal.add_working(start, span)
    } else {
        start
    });
}

/// "Reschedule uncompleted work to start after the status date", for the whole project.
///
/// A started task whose remaining work should already be under way gets a split at its
/// progress point, so the rest resumes at the status date. A task that should have
/// started but has not gets a start-no-earlier-than constraint on the status date.
/// Manual tasks, finished tasks, tasks with a firmer constraint, and recurring
/// occurrences that have not started stay as they are. The status date is part of
/// the project, so replaying the command gives the same splits.
pub(crate) fn reschedule_uncompleted(project: &mut Project) -> Result<(), EngineError> {
    let status = project.status_date.ok_or(EngineError::NoStatusDate)?;
    for index in 0..project.tasks.len() {
        let task = &project.tasks[index];
        if task.summary || task.manual || is_complete(task) {
            continue;
        }
        let cal = project.task_calendar(task);
        let resume = cal.next_work_start(status);
        if task.actual_start.is_some() {
            let done = done_minutes(task);
            if done <= 0 || done >= task.duration {
                continue;
            }
            // Measured without leveling's own pauses, which Clear Leveling can take back.
            let mut own = task.clone();
            own.splits.retain(|split| !split.leveling);
            let gap = cal.working_between(split::resume_at(cal, &own, done), resume);
            if gap > 0 {
                let task = &mut project.tasks[index];
                task.splits.push(Split { offset: done, gap, leveling: false });
                split::normalize(&mut task.splits, task.duration);
            }
        } else if task.series_uid.is_none() && task.start < status && waits_for(task, resume) {
            let task = &mut project.tasks[index];
            task.constraint_type = ConstraintType::Snet;
            task.constraint_date = Some(resume);
        }
    }
    Ok(())
}

/// Whether a start-no-earlier-than `resume` only tightens the task's own constraint.
fn waits_for(task: &Task, resume: i64) -> bool {
    match task.constraint_type {
        ConstraintType::Asap => true,
        ConstraintType::Snet => task.constraint_date.is_none_or(|date| date < resume),
        _ => false,
    }
}

use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::error::EngineError;
use crate::model::{ConstraintType, Overallocation, Project, Task};
use crate::schedule::schedule;
use crate::split;
use crate::timeutil::{civil, compose, date_key};

/// Resource units scheduled on a day, compared with max units.
pub fn publish_overallocations(project: &mut Project) {
    project.overallocations = find_overallocations(project);
}

pub fn find_overallocations(project: &Project) -> Vec<Overallocation> {
    let mut totals: BTreeMap<(u32, String), f64> = BTreeMap::new();
    for assignment in &project.assignments {
        let Some(task) = project.task(assignment.task_uid) else {
            continue;
        };
        if task.summary || task.finish <= task.start || assignment.units <= 0.0 {
            continue;
        }
        let Some(resource) = project.resources.iter().find(|r| r.uid == assignment.resource_uid)
        else {
            continue;
        };
        let task_cal = project.task_calendar(task);
        let (mut date, _) = civil(task.start);
        let (end_date, _) = civil(task.finish);
        for _ in 0..3660 {
            let minutes = overlap_minutes(task_cal, task, date);
            if minutes > 0 {
                *totals
                    .entry((resource.uid, date_key(date)))
                    .or_insert(0.0) += assignment.units;
            }
            if date >= end_date {
                break;
            }
            date = match date.succ_opt() {
                Some(next) => next,
                None => break,
            };
        }
    }

    let mut rows = Vec::new();
    for ((resource_uid, date), units) in totals {
        let Some(resource) = project.resources.iter().find(|r| r.uid == resource_uid) else {
            continue;
        };
        let resource_cal = project.resource_calendar(resource);
        let Some(naive) = crate::timeutil::parse_date_key(&date) else {
            continue;
        };
        let available = if day_has_work(resource_cal, naive) {
            resource.max_units
        } else {
            0.0
        };
        if units > available + 0.0001 {
            rows.push(Overallocation {
                resource_uid,
                date,
                units,
                max_units: available,
            });
        }
    }
    rows
}

fn day_has_work(cal: &crate::calendar::Calendar, date: NaiveDate) -> bool {
    !cal.ranges_on(date).is_empty()
}

/// Working minutes the task does on `date`; split gaps do not count.
fn overlap_minutes(cal: &crate::calendar::Calendar, task: &Task, date: NaiveDate) -> i64 {
    let day_start = compose(date, 0);
    let day_end = date
        .succ_opt()
        .map(|next| compose(next, 0))
        .unwrap_or(day_start);
    split::working_within(cal, &split::work_intervals(task), day_start, day_end)
}

/// Heuristic leveling: delay lower-priority tasks first, then tasks with more total slack.
/// Writes the result into `leveling_delay`. Leveling never splits a task; existing
/// splits move with the task.
pub fn level_project(project: &mut Project) -> Result<(), EngineError> {
    for task in &mut project.tasks {
        if task.manual
            || matches!(
                task.constraint_type,
                ConstraintType::Mso | ConstraintType::Mfo
            )
        {
            continue;
        }
        task.leveling_delay = 0;
    }
    schedule(project)?;
    let guard = project.tasks.len().max(1) * 48;
    for _ in 0..guard {
        let overs = find_overallocations(project);
        let Some(hit) = overs.iter().min_by(|a, b| a.date.cmp(&b.date)) else {
            break;
        };
        let Some(victim) = pick_victim(project, hit) else {
            break;
        };
        let Some(date) = crate::timeutil::parse_date_key(&hit.date) else {
            break;
        };
        let task = project.task(victim).unwrap();
        let cal = project.task_calendar(task).clone();
        let current = task.start;
        let next_day = date.succ_opt().unwrap_or(date);
        let target = cal.next_work_start(compose(next_day, 0));
        let mut bump = cal.working_between(current, target).max(0);
        if bump <= 0 {
            bump = 8 * 60;
        }
        let task = project.task_mut(victim).unwrap();
        task.leveling_delay += bump;
        schedule(project)?;
    }
    Ok(())
}

fn pick_victim(project: &Project, hit: &Overallocation) -> Option<u32> {
    let mut best: Option<(i32, i64, i64)> = None;
    for assignment in project
        .assignments
        .iter()
        .filter(|a| a.resource_uid == hit.resource_uid && a.units > 0.0)
    {
        let Some(task) = project.task(assignment.task_uid) else {
            continue;
        };
        if task.summary
            || task.manual
            || task.priority >= 1000
            || matches!(
                task.constraint_type,
                ConstraintType::Mso | ConstraintType::Mfo
            )
        {
            continue;
        }
        let cal = project.task_calendar(task);
        let Some(date) = crate::timeutil::parse_date_key(&hit.date) else {
            continue;
        };
        if overlap_minutes(cal, task, date) <= 0 {
            continue;
        }
        // Lower priority, then larger slack, then higher uid.
        let key = (task.priority, -task.total_slack, -(task.uid as i64));
        let replace = match best {
            None => true,
            Some(current) => key < current,
        };
        if replace {
            best = Some(key);
        }
    }
    best.map(|(_, _, uid)| (-uid) as u32)
}

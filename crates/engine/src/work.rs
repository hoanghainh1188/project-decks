use crate::model::{Accrual, Assignment, Project, TaskType, UsageDay, UsageRow};
use crate::timeutil::{civil, compose, date_key};

#[derive(Clone, Copy, Debug)]
pub enum Changed {
    Duration,
    Units,
    Work,
}

fn scale(duration: i64, units: f64) -> i64 {
    (duration as f64 * units).round().max(0.0) as i64
}

/// Keep `work = duration × units` according to the task type.
pub fn apply_change(project: &mut Project, task_uid: u32, changed: Changed) {
    let Some(index) = project.index_of(task_uid) else {
        return;
    };
    if project.tasks[index].summary {
        return;
    }
    if project.tasks[index].task_type == TaskType::FixedWork {
        project.tasks[index].effort_driven = true;
    }
    let assignment_indexes: Vec<usize> = project
        .assignments
        .iter()
        .enumerate()
        .filter(|(_, a)| a.task_uid == task_uid)
        .map(|(i, _)| i)
        .collect();

    if assignment_indexes.is_empty() {
        let task = &mut project.tasks[index];
        if task.units <= 0.0 {
            task.units = 1.0;
        }
        match task.task_type {
            TaskType::FixedUnits => match changed {
                Changed::Duration => task.work = scale(task.duration, task.units),
                Changed::Work => {
                    task.duration = (task.work as f64 / task.units).round().max(0.0) as i64;
                }
                Changed::Units => {
                    if task.effort_driven && task.work > 0 {
                        task.duration = (task.work as f64 / task.units).round().max(0.0) as i64;
                    } else {
                        task.work = scale(task.duration, task.units);
                    }
                }
            },
            TaskType::FixedDuration => match changed {
                Changed::Duration | Changed::Units => {
                    task.work = scale(task.duration, task.units);
                }
                Changed::Work => {
                    if task.duration > 0 {
                        task.units = task.work as f64 / task.duration as f64;
                    }
                }
            },
            TaskType::FixedWork => match changed {
                Changed::Work | Changed::Units => {
                    task.duration = (task.work as f64 / task.units).round().max(0.0) as i64;
                }
                Changed::Duration => {
                    if task.duration > 0 {
                        task.units = task.work as f64 / task.duration as f64;
                    }
                }
            },
        }
        task.milestone = task.duration == 0;
        return;
    }

    let task_type = project.tasks[index].task_type;
    let effort_driven = project.tasks[index].effort_driven;
    let mut duration = project.tasks[index].duration;
    let units: f64 = assignment_indexes
        .iter()
        .map(|i| project.assignments[*i].units)
        .sum();
    let mut work: i64 = assignment_indexes
        .iter()
        .map(|i| project.assignments[*i].work)
        .sum();

    match task_type {
        TaskType::FixedUnits => match changed {
            Changed::Duration => {
                for i in &assignment_indexes {
                    let units = project.assignments[*i].units;
                    project.assignments[*i].work = scale(duration, units);
                }
                work = assignment_indexes
                    .iter()
                    .map(|i| project.assignments[*i].work)
                    .sum();
            }
            Changed::Work => {
                if units > 0.0 {
                    duration = (work as f64 / units).round().max(0.0) as i64;
                }
                for i in &assignment_indexes {
                    let units = project.assignments[*i].units;
                    project.assignments[*i].work = scale(duration, units);
                }
                work = assignment_indexes
                    .iter()
                    .map(|i| project.assignments[*i].work)
                    .sum();
            }
            Changed::Units => {
                if effort_driven && work > 0 && units > 0.0 {
                    duration = (work as f64 / units).round().max(0.0) as i64;
                }
                for i in &assignment_indexes {
                    let units = project.assignments[*i].units;
                    project.assignments[*i].work = scale(duration, units);
                }
                work = assignment_indexes
                    .iter()
                    .map(|i| project.assignments[*i].work)
                    .sum();
            }
        },
        TaskType::FixedDuration => match changed {
            Changed::Duration | Changed::Units => {
                for i in &assignment_indexes {
                    let units = project.assignments[*i].units;
                    project.assignments[*i].work = scale(duration, units);
                }
                work = assignment_indexes
                    .iter()
                    .map(|i| project.assignments[*i].work)
                    .sum();
            }
            Changed::Work => {
                if duration > 0 && units > 0.0 {
                    // Keep each assignment's share of the new total work via units.
                    let target = work as f64;
                    let current_units = units;
                    for i in &assignment_indexes {
                        let share = project.assignments[*i].units / current_units;
                        project.assignments[*i].units = if duration > 0 {
                            (target * share) / duration as f64
                        } else {
                            project.assignments[*i].units
                        };
                        project.assignments[*i].work =
                            scale(duration, project.assignments[*i].units);
                    }
                    work = assignment_indexes
                        .iter()
                        .map(|i| project.assignments[*i].work)
                        .sum();
                }
            }
        },
        TaskType::FixedWork => match changed {
            Changed::Units | Changed::Work => {
                if units > 0.0 {
                    duration = (work as f64 / units).round().max(0.0) as i64;
                }
                for i in &assignment_indexes {
                    let units = project.assignments[*i].units;
                    project.assignments[*i].work = scale(duration, units);
                }
                work = assignment_indexes
                    .iter()
                    .map(|i| project.assignments[*i].work)
                    .sum();
            }
            Changed::Duration => {
                if duration > 0 && work > 0 {
                    let new_units = work as f64 / duration as f64;
                    let old_units = units.max(0.0001);
                    for i in &assignment_indexes {
                        let share = project.assignments[*i].units / old_units;
                        project.assignments[*i].units = new_units * share;
                        project.assignments[*i].work =
                            scale(duration, project.assignments[*i].units);
                    }
                    work = assignment_indexes
                        .iter()
                        .map(|i| project.assignments[*i].work)
                        .sum();
                }
            }
        },
    }

    let task = &mut project.tasks[index];
    task.duration = duration.max(0);
    task.milestone = task.duration == 0;
    task.units = assignment_indexes
        .iter()
        .map(|i| project.assignments[*i].units)
        .sum();
    task.work = work;
}

pub fn add_assignment(project: &mut Project, task_uid: u32, resource_uid: u32, units: f64) -> u32 {
    if let Some(existing) = project
        .assignments
        .iter_mut()
        .find(|a| a.task_uid == task_uid && a.resource_uid == resource_uid)
    {
        existing.units = units.max(0.0);
        let uid = existing.uid;
        apply_change(project, task_uid, Changed::Units);
        return uid;
    }
    let had = project.assignments.iter().any(|a| a.task_uid == task_uid);
    let uid = project.alloc_assignment_uid();
    let carried_work = if had {
        0
    } else {
        project.task(task_uid).map(|t| t.work).unwrap_or(0)
    };
    project.assignments.push(Assignment {
        uid,
        task_uid,
        resource_uid,
        units: units.max(0.0),
        work: if had { 0 } else { carried_work },
        overtime_work: 0,
        actual_work: 0,
        remaining_work: 0,
        cost: 0.0,
        actual_cost: 0.0,
    });
    apply_change(project, task_uid, Changed::Units);
    uid
}

pub fn publish_costs_and_usage(project: &mut Project) {
    rollup_summary_work(project);
    refresh_tracking(project);
    compute_costs(project);
    project.usage = build_usage(project);
}

fn rollup_summary_work(project: &mut Project) {
    for index in (0..project.tasks.len()).rev() {
        if !project.tasks[index].summary {
            continue;
        }
        let level = project.tasks[index].outline_level;
        let mut work = 0i64;
        let mut actual = 0i64;
        let mut remaining = 0i64;
        let mut cost = 0.0;
        let mut actual_cost = 0.0;
        for j in index + 1..project.tasks.len() {
            if project.tasks[j].outline_level <= level {
                break;
            }
            if project.tasks[j].outline_level == level + 1 {
                work += project.tasks[j].work;
                actual += project.tasks[j].actual_work;
                remaining += project.tasks[j].remaining_work;
                cost += project.tasks[j].cost;
                actual_cost += project.tasks[j].actual_cost;
            }
        }
        let task = &mut project.tasks[index];
        task.work = work;
        task.actual_work = actual;
        task.remaining_work = remaining;
        task.cost = cost;
        task.actual_cost = actual_cost;
        task.remaining_cost = (cost - actual_cost).max(0.0);
        if work > 0 {
            task.percent_complete = (actual as f64 / work as f64 * 100.0).clamp(0.0, 100.0);
        }
    }
}

fn refresh_tracking(project: &mut Project) {
    let percents: Vec<(u32, f64, i64, i64)> = project
        .tasks
        .iter()
        .filter(|t| !t.summary)
        .map(|t| (t.uid, t.percent_complete.clamp(0.0, 100.0), t.duration, t.work))
        .collect();
    for (uid, pct, duration, work) in percents {
        let fraction = pct / 100.0;
        let remaining_duration = (duration as f64 * (1.0 - fraction)).round().max(0.0) as i64;
        let actual_work = (work as f64 * fraction).round().max(0.0) as i64;
        let remaining_work = (work - actual_work).max(0);
        if let Some(task) = project.task_mut(uid) {
            task.percent_complete = pct;
            task.remaining_duration = remaining_duration;
            task.actual_work = actual_work;
            task.remaining_work = remaining_work;
            if pct <= 0.0 {
                task.actual_start = None;
                task.actual_finish = None;
            } else if task.actual_start.is_none() {
                task.actual_start = Some(task.start);
            }
            if pct >= 100.0 {
                task.actual_finish = task.actual_finish.or(Some(task.finish));
                task.remaining_duration = 0;
                task.remaining_work = 0;
                task.actual_work = work;
            } else {
                task.actual_finish = None;
            }
        }
        for assignment in project
            .assignments
            .iter_mut()
            .filter(|a| a.task_uid == uid)
        {
            assignment.actual_work = (assignment.work as f64 * fraction).round().max(0.0) as i64;
            assignment.remaining_work = (assignment.work - assignment.actual_work).max(0);
            if pct >= 100.0 {
                assignment.actual_work = assignment.work;
                assignment.remaining_work = 0;
            }
        }
    }
}

fn compute_costs(project: &mut Project) {
    let rates: Vec<(u32, f64, f64, f64)> = project
        .resources
        .iter()
        .map(|r| (r.uid, r.standard_rate, r.overtime_rate, r.cost_per_use))
        .collect();
    for assignment in &mut project.assignments {
        let Some((_, standard, overtime, per_use)) =
            rates.iter().find(|(uid, _, _, _)| *uid == assignment.resource_uid)
        else {
            continue;
        };
        let overtime_work = assignment.overtime_work.clamp(0, assignment.work);
        let regular = assignment.work - overtime_work;
        assignment.cost = regular as f64 / 60.0 * standard
            + overtime_work as f64 / 60.0 * overtime
            + per_use;
    }
    let task_uids: Vec<u32> = project
        .tasks
        .iter()
        .filter(|t| !t.summary)
        .map(|t| t.uid)
        .collect();
    for uid in task_uids {
        let labor: f64 = project
            .assignments
            .iter()
            .filter(|a| a.task_uid == uid)
            .map(|a| a.cost)
            .sum();
        let task = project.task(uid).unwrap();
        let total = labor + task.fixed_cost;
        let pct = task.percent_complete.clamp(0.0, 100.0) / 100.0;
        let actual = match task.fixed_cost_accrual {
            Accrual::Prorated => total * pct,
            Accrual::Start => {
                if pct > 0.0 {
                    total
                } else {
                    0.0
                }
            }
            Accrual::End => {
                if pct >= 1.0 {
                    total
                } else {
                    0.0
                }
            }
        };
        let actual_labor = match task.fixed_cost_accrual {
            Accrual::Prorated => labor * pct,
            Accrual::Start => {
                if pct > 0.0 {
                    labor
                } else {
                    0.0
                }
            }
            Accrual::End => {
                if pct >= 1.0 {
                    labor
                } else {
                    0.0
                }
            }
        };
        for assignment in project.assignments.iter_mut().filter(|a| a.task_uid == uid) {
            if labor > 0.0 {
                assignment.actual_cost = actual_labor * (assignment.cost / labor);
            } else {
                assignment.actual_cost = 0.0;
            }
        }
        let task = project.task_mut(uid).unwrap();
        task.cost = total;
        task.actual_cost = actual;
        task.remaining_cost = (total - actual).max(0.0);
    }
}

fn build_usage(project: &Project) -> Vec<UsageRow> {
    let mut rows = Vec::new();
    for task in &project.tasks {
        if task.summary {
            continue;
        }
        let cal = project.task_calendar(task);
        let assignments: Vec<&Assignment> = project
            .assignments
            .iter()
            .filter(|a| a.task_uid == task.uid)
            .collect();
        let mut task_days: Vec<UsageDay> = Vec::new();
        if task.finish > task.start {
            let (mut date, _) = civil(task.start);
            let (end_date, _) = civil(task.finish);
            for _ in 0..3660 {
                let day_start = compose(date, 0);
                let next = date.succ_opt().unwrap_or(date);
                let day_end = compose(next, 0);
                let overlap_start = task.start.max(day_start);
                let overlap_end = task.finish.min(day_end);
                let minutes = if overlap_end > overlap_start {
                    cal.working_between(overlap_start, overlap_end).max(0)
                } else {
                    0
                };
                if minutes > 0 {
                    let hours = if assignments.is_empty() {
                        minutes as f64 / 60.0
                    } else {
                        assignments
                            .iter()
                            .map(|a| minutes as f64 / 60.0 * a.units)
                            .sum()
                    };
                    task_days.push(UsageDay {
                        date: date_key(date),
                        hours,
                    });
                }
                if date >= end_date {
                    break;
                }
                date = next;
            }
        }
        rows.push(UsageRow {
            task_uid: task.uid,
            assignment_uid: None,
            resource_uid: None,
            label: task.name.clone(),
            days: task_days.clone(),
        });
        for assignment in assignments {
            let resource_name = project
                .resources
                .iter()
                .find(|r| r.uid == assignment.resource_uid)
                .map(|r| r.name.clone())
                .unwrap_or_else(|| format!("NL {}", assignment.resource_uid));
            let days = task_days
                .iter()
                .map(|day| UsageDay {
                    date: day.date.clone(),
                    hours: {
                        // Rebuild from duration hours × units. Task row hours already include units,
                        // so recover duration hours by dividing back out when possible.
                        let duration_hours: f64 = {
                            let total_units: f64 = project
                                .assignments
                                .iter()
                                .filter(|a| a.task_uid == task.uid)
                                .map(|a| a.units)
                                .sum();
                            if total_units > 0.0 {
                                day.hours / total_units
                            } else {
                                0.0
                            }
                        };
                        duration_hours * assignment.units
                    },
                })
                .collect();
            rows.push(UsageRow {
                task_uid: task.uid,
                assignment_uid: Some(assignment.uid),
                resource_uid: Some(assignment.resource_uid),
                label: resource_name,
                days,
            });
        }
    }
    rows
}

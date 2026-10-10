use serde::{Deserialize, Serialize};

use crate::calendar::{Calendar, TimeRange};
use crate::error::EngineError;
use crate::level::level_project;
use crate::model::{
    Accrual, Baseline, ConstraintType, DepKind, Dependency, Project, Resource, ResourceKind, Task,
    TaskType,
};
use crate::schedule::normalize_outline;
use crate::timeutil::{parse_date_key, MINUTES_PER_DAY, MINUTES_PER_HOUR};
use crate::work::{self, Changed};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    RenameProject {
        name: String,
    },
    SetProjectStart {
        start: i64,
    },
    SetStatusDate {
        date: Option<i64>,
    },
    AddTask {
        name: String,
        after_uid: Option<u32>,
    },
    AddMilestone {
        name: String,
        after_uid: Option<u32>,
    },
    DeleteTask {
        uid: u32,
    },
    RenameTask {
        uid: u32,
        name: String,
    },
    SetDuration {
        uid: u32,
        minutes: i64,
    },
    SetManual {
        uid: u32,
        manual: bool,
        start: Option<i64>,
        finish: Option<i64>,
    },
    SetConstraint {
        uid: u32,
        constraint_type: ConstraintType,
        date: Option<i64>,
    },
    Indent {
        uid: u32,
    },
    Outdent {
        uid: u32,
    },
    SetPredecessors {
        uid: u32,
        text: String,
    },
    AddDependency {
        predecessor_uid: u32,
        successor_uid: u32,
        kind: DepKind,
        lag: i64,
    },
    RemoveDependency {
        uid: u32,
    },
    SetNotes {
        uid: u32,
        notes: String,
    },
    SetPriority {
        uid: u32,
        priority: i32,
    },
    SetTaskCalendar {
        uid: u32,
        calendar_id: Option<u32>,
    },
    AddHoliday {
        date: String,
    },
    RemoveHoliday {
        date: String,
    },
    CreateCalendar {
        name: String,
    },
    RenameCalendar {
        calendar_id: u32,
        name: String,
    },
    SetCalendarWeek {
        calendar_id: u32,
        week: Vec<Vec<TimeRange>>,
    },
    SetCalendarExceptions {
        calendar_id: u32,
        from: String,
        to: String,
        ranges: Vec<TimeRange>,
    },
    ClearCalendarExceptions {
        calendar_id: u32,
        from: String,
        to: String,
    },
    MoveTask {
        uid: u32,
        start: i64,
    },
    ResizeTask {
        uid: u32,
        finish: i64,
    },
    AddResource {
        name: String,
        kind: ResourceKind,
    },
    UpdateResource {
        uid: u32,
        name: String,
        kind: ResourceKind,
        max_units: f64,
        calendar_id: Option<u32>,
        standard_rate: f64,
        overtime_rate: f64,
        cost_per_use: f64,
    },
    DeleteResource {
        uid: u32,
    },
    AssignResource {
        task_uid: u32,
        resource_uid: u32,
        units: f64,
    },
    UpdateAssignment {
        uid: u32,
        units: Option<f64>,
        work: Option<i64>,
        overtime_work: Option<i64>,
    },
    RemoveAssignment {
        uid: u32,
    },
    SetTaskType {
        uid: u32,
        task_type: TaskType,
        effort_driven: bool,
    },
    SetWork {
        uid: u32,
        minutes: i64,
    },
    SetPercentComplete {
        uid: u32,
        percent: f64,
    },
    SetActualStart {
        uid: u32,
        start: Option<i64>,
    },
    SetActualFinish {
        uid: u32,
        finish: Option<i64>,
    },
    SetRemainingDuration {
        uid: u32,
        minutes: i64,
    },
    SetRemainingWork {
        uid: u32,
        minutes: i64,
    },
    SetOvertimeWork {
        uid: u32,
        minutes: i64,
    },
    SetFixedCost {
        uid: u32,
        cost: f64,
        accrual: Accrual,
    },
    SetBaseline {
        number: u8,
    },
    ClearBaseline {
        number: u8,
    },
    LevelResources,
    ClearLeveling,
    SetLevelingDelay {
        uid: u32,
        minutes: i64,
    },
    SetDeadline {
        uid: u32,
        date: Option<i64>,
    },
}

pub fn apply_one(project: &mut Project, command: &Command) -> Result<(), EngineError> {
    match command {
        Command::RenameProject { name } => project.name = name.clone(),
        Command::SetProjectStart { start } => {
            project.start = project.calendar().next_work_start(*start);
        }
        Command::SetStatusDate { date } => project.status_date = *date,
        Command::AddTask { name, after_uid } => {
            let label = if name.trim().is_empty() {
                format!("Công việc {}", project.next_task_uid)
            } else {
                name.clone()
            };
            let uid = project.alloc_task_uid();
            insert_task(project, *after_uid, Task::new(uid, label))?;
        }
        Command::AddMilestone { name, after_uid } => {
            let label = if name.trim().is_empty() {
                "Mốc".into()
            } else {
                name.clone()
            };
            let uid = project.alloc_task_uid();
            let mut task = Task::new(uid, label);
            task.duration = 0;
            task.work = 0;
            task.remaining_duration = 0;
            task.remaining_work = 0;
            task.milestone = true;
            insert_task(project, *after_uid, task)?;
        }
        Command::DeleteTask { uid } => delete_task(project, *uid)?,
        Command::RenameTask { uid, name } => {
            project.task_mut(*uid).ok_or(EngineError::TaskNotFound)?.name = name.clone();
        }
        Command::SetDuration { uid, minutes } => {
            let task = project.task_mut(*uid).ok_or(EngineError::TaskNotFound)?;
            if task.summary {
                return Ok(());
            }
            task.duration = (*minutes).max(0);
            task.milestone = task.duration == 0;
            work::apply_change(project, *uid, Changed::Duration);
        }
        Command::SetManual {
            uid,
            manual,
            start,
            finish,
        } => {
            let task = project.task_mut(*uid).ok_or(EngineError::TaskNotFound)?;
            if task.summary {
                return Ok(());
            }
            task.manual = *manual;
            if *manual {
                if let Some(start) = start {
                    task.start = *start;
                }
                if let Some(finish) = finish {
                    task.finish = *finish;
                }
            }
        }
        Command::SetConstraint {
            uid,
            constraint_type,
            date,
        } => {
            let task = project.task_mut(*uid).ok_or(EngineError::TaskNotFound)?;
            task.constraint_type = *constraint_type;
            task.constraint_date = *date;
        }
        Command::Indent { uid } => indent(project, *uid)?,
        Command::Outdent { uid } => outdent(project, *uid)?,
        Command::SetPredecessors { uid, text } => set_predecessors(project, *uid, text)?,
        Command::AddDependency {
            predecessor_uid,
            successor_uid,
            kind,
            lag,
        } => {
            if predecessor_uid == successor_uid {
                return Err(EngineError::DependencyCycle);
            }
            if project.task(*predecessor_uid).is_none() || project.task(*successor_uid).is_none() {
                return Err(EngineError::TaskNotFound);
            }
            if let Some(existing) = project.dependencies.iter_mut().find(|dep| {
                dep.predecessor_uid == *predecessor_uid && dep.successor_uid == *successor_uid
            }) {
                existing.kind = *kind;
                existing.lag = *lag;
            } else {
                let uid = project.alloc_dependency_uid();
                project.dependencies.push(Dependency {
                    uid,
                    predecessor_uid: *predecessor_uid,
                    successor_uid: *successor_uid,
                    kind: *kind,
                    lag: *lag,
                });
            }
        }
        Command::RemoveDependency { uid } => {
            project.dependencies.retain(|dep| dep.uid != *uid);
        }
        Command::SetNotes { uid, notes } => {
            project.task_mut(*uid).ok_or(EngineError::TaskNotFound)?.notes = notes.clone();
        }
        Command::SetPriority { uid, priority } => {
            let task = project.task_mut(*uid).ok_or(EngineError::TaskNotFound)?;
            task.priority = (*priority).clamp(0, 1000);
        }
        Command::SetTaskCalendar { uid, calendar_id } => {
            if let Some(id) = calendar_id {
                if project.calendar_by_id(*id).is_none() {
                    return Err(EngineError::Store("Không có lịch này".into()));
                }
            }
            project
                .task_mut(*uid)
                .ok_or(EngineError::TaskNotFound)?
                .calendar_id = *calendar_id;
        }
        Command::AddHoliday { date } => {
            let naive = parse_date_key(date).ok_or_else(|| EngineError::Store(date.clone()))?;
            project.calendar_mut().set_day_off(naive);
        }
        Command::RemoveHoliday { date } => {
            let naive = parse_date_key(date).ok_or_else(|| EngineError::Store(date.clone()))?;
            project.calendar_mut().clear_exception(naive);
        }
        Command::CreateCalendar { name } => {
            let uid = project.alloc_calendar_uid();
            let mut calendar = Calendar::standard();
            calendar.id = uid;
            let label = name.trim();
            calendar.name = if label.is_empty() {
                format!("Lịch {uid}")
            } else {
                label.to_string()
            };
            project.calendars.push(calendar);
        }
        Command::RenameCalendar { calendar_id, name } => {
            let label = name.trim();
            if label.is_empty() {
                return Err(EngineError::Store("Tên lịch trống".into()));
            }
            project
                .calendar_mut_by_id(*calendar_id)
                .ok_or_else(|| EngineError::Store("Không có lịch này".into()))?
                .name = label.to_string();
        }
        Command::SetCalendarWeek { calendar_id, week } => {
            if week.len() != 7 {
                return Err(EngineError::Store("Tuần làm việc phải có 7 ngày".into()));
            }
            let mut days: [Vec<TimeRange>; 7] = Default::default();
            for (index, day) in week.iter().enumerate() {
                days[index] = normalize_ranges(day)?;
            }
            let calendar = project
                .calendar_mut_by_id(*calendar_id)
                .ok_or_else(|| EngineError::Store("Không có lịch này".into()))?;
            calendar.week = days;
            if !calendar.has_working_time() {
                return Err(EngineError::NoWorkingTime);
            }
        }
        Command::SetCalendarExceptions {
            calendar_id,
            from,
            to,
            ranges,
        } => {
            let from_date = parse_date_key(from)
                .ok_or_else(|| EngineError::Store("Ngày không hợp lệ".into()))?;
            let to_date =
                parse_date_key(to).ok_or_else(|| EngineError::Store("Ngày không hợp lệ".into()))?;
            let ranges = normalize_ranges(ranges)?;
            let calendar = project
                .calendar_mut_by_id(*calendar_id)
                .ok_or_else(|| EngineError::Store("Không có lịch này".into()))?;
            if !calendar.set_exception_span(from_date, to_date, ranges) {
                return Err(EngineError::Store("Khoảng ngày quá dài".into()));
            }
            if !calendar.has_working_time() {
                return Err(EngineError::NoWorkingTime);
            }
        }
        Command::ClearCalendarExceptions {
            calendar_id,
            from,
            to,
        } => {
            let from_date = parse_date_key(from)
                .ok_or_else(|| EngineError::Store("Ngày không hợp lệ".into()))?;
            let to_date =
                parse_date_key(to).ok_or_else(|| EngineError::Store("Ngày không hợp lệ".into()))?;
            let calendar = project
                .calendar_mut_by_id(*calendar_id)
                .ok_or_else(|| EngineError::Store("Không có lịch này".into()))?;
            if !calendar.clear_exception_span(from_date, to_date) {
                return Err(EngineError::Store("Khoảng ngày quá dài".into()));
            }
            if !calendar.has_working_time() {
                return Err(EngineError::NoWorkingTime);
            }
        }
        Command::MoveTask { uid, start } => move_task(project, *uid, *start)?,
        Command::ResizeTask { uid, finish } => resize_task(project, *uid, *finish)?,
        Command::AddResource { name, kind } => {
            let uid = project.alloc_resource_uid();
            let label = if name.trim().is_empty() {
                format!("Nguồn lực {uid}")
            } else {
                name.clone()
            };
            project.resources.push(Resource::new(uid, label, *kind));
        }
        Command::UpdateResource {
            uid,
            name,
            kind,
            max_units,
            calendar_id,
            standard_rate,
            overtime_rate,
            cost_per_use,
        } => {
            if let Some(id) = calendar_id {
                if project.calendar_by_id(*id).is_none() {
                    return Err(EngineError::Store("Không có lịch này".into()));
                }
            }
            let resource = project
                .resources
                .iter_mut()
                .find(|resource| resource.uid == *uid)
                .ok_or(EngineError::ResourceNotFound)?;
            resource.name = name.clone();
            resource.kind = *kind;
            resource.max_units = (*max_units).max(0.0);
            resource.calendar_id = *calendar_id;
            resource.standard_rate = *standard_rate;
            resource.overtime_rate = *overtime_rate;
            resource.cost_per_use = *cost_per_use;
        }
        Command::DeleteResource { uid } => {
            project.resources.retain(|resource| resource.uid != *uid);
            let assignment_uids: Vec<u32> = project
                .assignments
                .iter()
                .filter(|assignment| assignment.resource_uid == *uid)
                .map(|assignment| assignment.uid)
                .collect();
            for assignment_uid in assignment_uids {
                work::remove_assignment(project, assignment_uid);
            }
        }
        Command::AssignResource {
            task_uid,
            resource_uid,
            units,
        } => {
            if project.task(*task_uid).is_none() {
                return Err(EngineError::TaskNotFound);
            }
            if project
                .resources
                .iter()
                .all(|resource| resource.uid != *resource_uid)
            {
                return Err(EngineError::ResourceNotFound);
            }
            work::add_assignment(project, *task_uid, *resource_uid, *units);
        }
        Command::UpdateAssignment {
            uid,
            units,
            work,
            overtime_work,
        } => {
            let task_uid = project
                .assignments
                .iter()
                .find(|assignment| assignment.uid == *uid)
                .map(|assignment| assignment.task_uid)
                .ok_or(EngineError::AssignmentNotFound)?;
            if let Some(overtime_work) = overtime_work {
                if let Some(assignment) = project
                    .assignments
                    .iter_mut()
                    .find(|assignment| assignment.uid == *uid)
                {
                    assignment.overtime_work = (*overtime_work).max(0);
                }
            }
            if let Some(units) = units {
                if let Some(assignment) = project
                    .assignments
                    .iter_mut()
                    .find(|assignment| assignment.uid == *uid)
                {
                    assignment.units = (*units).max(0.0);
                }
                work::apply_change(project, task_uid, Changed::Units);
            }
            if let Some(work_minutes) = work {
                if let Some(assignment) = project
                    .assignments
                    .iter_mut()
                    .find(|assignment| assignment.uid == *uid)
                {
                    assignment.work = (*work_minutes).max(0);
                }
                work::apply_change(project, task_uid, Changed::Work);
            }
        }
        Command::RemoveAssignment { uid } => {
            if !work::remove_assignment(project, *uid) {
                return Err(EngineError::AssignmentNotFound);
            }
        }
        Command::SetTaskType {
            uid,
            task_type,
            effort_driven,
        } => {
            let task = project.task_mut(*uid).ok_or(EngineError::TaskNotFound)?;
            task.task_type = *task_type;
            task.effort_driven = if *task_type == TaskType::FixedWork {
                true
            } else {
                *effort_driven
            };
        }
        Command::SetWork { uid, minutes } => set_work(project, *uid, *minutes)?,
        Command::SetPercentComplete { uid, percent } => {
            let task = project.task_mut(*uid).ok_or(EngineError::TaskNotFound)?;
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
            if percent >= 100.0 {
                task.actual_finish = Some(task.finish);
            } else {
                task.actual_finish = None;
            }
        }
        Command::SetActualStart { uid, start } => {
            let task = project.task_mut(*uid).ok_or(EngineError::TaskNotFound)?;
            task.actual_start = *start;
            if start.is_some() && task.percent_complete <= 0.0 {
                task.percent_complete = 1.0;
            }
            if start.is_none() {
                task.percent_complete = 0.0;
                task.actual_finish = None;
            }
        }
        Command::SetActualFinish { uid, finish } => {
            let task = project.task_mut(*uid).ok_or(EngineError::TaskNotFound)?;
            task.actual_finish = *finish;
            if finish.is_some() {
                task.percent_complete = 100.0;
                if task.actual_start.is_none() {
                    task.actual_start = Some(task.start);
                }
            } else if task.percent_complete >= 100.0 {
                task.percent_complete = 0.0;
            }
        }
        Command::SetRemainingDuration { uid, minutes } => {
            let task = project.task_mut(*uid).ok_or(EngineError::TaskNotFound)?;
            if task.summary {
                return Ok(());
            }
            let remaining = (*minutes).max(0);
            if task.duration <= 0 {
                task.duration = remaining;
                task.percent_complete = 0.0;
            } else if remaining >= task.duration {
                task.duration = remaining;
                task.percent_complete = 0.0;
                task.actual_start = None;
                task.actual_finish = None;
            } else {
                task.percent_complete =
                    (task.duration - remaining) as f64 / task.duration as f64 * 100.0;
                if task.percent_complete >= 100.0 {
                    task.actual_finish = Some(task.finish);
                } else {
                    task.actual_finish = None;
                }
            }
            task.milestone = task.duration == 0;
            work::apply_change(project, *uid, Changed::Duration);
        }
        Command::SetRemainingWork { uid, minutes } => {
            set_remaining_work(project, *uid, *minutes)?;
        }
        Command::SetOvertimeWork { uid, minutes } => {
            set_overtime_work(project, *uid, *minutes)?;
        }
        Command::SetFixedCost { uid, cost, accrual } => {
            let task = project.task_mut(*uid).ok_or(EngineError::TaskNotFound)?;
            task.fixed_cost = *cost;
            task.fixed_cost_accrual = *accrual;
        }
        Command::SetBaseline { number } => {
            let number = (*number).min(10);
            for task in &mut project.tasks {
                task.baselines.retain(|baseline| baseline.number != number);
                task.baselines.push(Baseline {
                    number,
                    start: task.start,
                    finish: task.finish,
                    work: task.work,
                    cost: task.cost,
                });
            }
        }
        Command::ClearBaseline { number } => {
            let number = (*number).min(10);
            for task in &mut project.tasks {
                task.baselines.retain(|baseline| baseline.number != number);
            }
        }
        Command::LevelResources => level_project(project)?,
        Command::ClearLeveling => {
            for task in &mut project.tasks {
                task.leveling_delay = 0;
            }
        }
        Command::SetLevelingDelay { uid, minutes } => {
            let task = project.task_mut(*uid).ok_or(EngineError::TaskNotFound)?;
            task.leveling_delay = (*minutes).max(0);
        }
        Command::SetDeadline { uid, date } => {
            let task = project.task_mut(*uid).ok_or(EngineError::TaskNotFound)?;
            task.deadline = *date;
        }
    }
    Ok(())
}

fn insert_task(project: &mut Project, after_uid: Option<u32>, mut task: Task) -> Result<(), EngineError> {
    normalize_outline(project);
    let (index, level) = if let Some(uid) = after_uid {
        let position = project.index_of(uid).ok_or(EngineError::TaskNotFound)?;
        let level = project.tasks[position].outline_level;
        (subtree_end(&project.tasks, position), level)
    } else if let Some(last) = project.tasks.last() {
        (project.tasks.len(), last.outline_level)
    } else {
        (0, 1)
    };
    task.outline_level = level;
    task.start = project.start;
    task.finish = project.start;
    project.tasks.insert(index, task);
    Ok(())
}

fn delete_task(project: &mut Project, uid: u32) -> Result<(), EngineError> {
    normalize_outline(project);
    let index = project.index_of(uid).ok_or(EngineError::TaskNotFound)?;
    let end = subtree_end(&project.tasks, index);
    let uids: Vec<u32> = project.tasks[index..end].iter().map(|task| task.uid).collect();
    project.tasks.drain(index..end);
    project.dependencies.retain(|dep| {
        !uids.contains(&dep.predecessor_uid) && !uids.contains(&dep.successor_uid)
    });
    project
        .assignments
        .retain(|assignment| !uids.contains(&assignment.task_uid));
    Ok(())
}

fn indent(project: &mut Project, uid: u32) -> Result<(), EngineError> {
    normalize_outline(project);
    let index = project.index_of(uid).ok_or(EngineError::TaskNotFound)?;
    if index == 0 {
        return Ok(());
    }
    let max_level = project.tasks[index - 1].outline_level + 1;
    if project.tasks[index].outline_level >= max_level {
        return Ok(());
    }
    let end = subtree_end(&project.tasks, index);
    for task in &mut project.tasks[index..end] {
        task.outline_level += 1;
    }
    Ok(())
}

fn outdent(project: &mut Project, uid: u32) -> Result<(), EngineError> {
    normalize_outline(project);
    let index = project.index_of(uid).ok_or(EngineError::TaskNotFound)?;
    if project.tasks[index].outline_level <= 1 {
        return Ok(());
    }
    let end = subtree_end(&project.tasks, index);
    for task in &mut project.tasks[index..end] {
        task.outline_level = task.outline_level.saturating_sub(1).max(1);
    }
    Ok(())
}

fn subtree_end(tasks: &[Task], index: usize) -> usize {
    let level = tasks[index].outline_level;
    let mut end = index + 1;
    while end < tasks.len() && tasks[end].outline_level > level {
        end += 1;
    }
    end
}

fn move_task(project: &mut Project, uid: u32, start: i64) -> Result<(), EngineError> {
    let manual = project.task(uid).ok_or(EngineError::TaskNotFound)?.manual;
    let summary = project.task(uid).unwrap().summary;
    if summary {
        return Ok(());
    }
    if manual {
        let duration = project.task(uid).unwrap().duration;
        let milestone = project.task(uid).unwrap().milestone;
        let cal = project.task_calendar(project.task(uid).unwrap()).clone();
        let start = cal.next_work_start(start);
        let finish = if milestone || duration <= 0 {
            start
        } else {
            cal.add_working(start, duration)
        };
        let task = project.task_mut(uid).unwrap();
        task.start = start;
        task.finish = finish;
    } else {
        let task = project.task_mut(uid).unwrap();
        task.constraint_type = ConstraintType::Snet;
        task.constraint_date = Some(start);
    }
    Ok(())
}

fn resize_task(project: &mut Project, uid: u32, finish: i64) -> Result<(), EngineError> {
    let task = project.task(uid).ok_or(EngineError::TaskNotFound)?;
    if task.summary {
        return Ok(());
    }
    let cal = project.task_calendar(task).clone();
    let start = task.start;
    let minutes = cal.working_between(start, finish).max(0);
    let task = project.task_mut(uid).unwrap();
    task.duration = minutes;
    task.milestone = minutes == 0;
    if task.manual {
        task.finish = finish.max(start);
    }
    work::apply_change(project, uid, Changed::Duration);
    Ok(())
}

fn set_work(project: &mut Project, uid: u32, minutes: i64) -> Result<(), EngineError> {
    let task = project.task(uid).ok_or(EngineError::TaskNotFound)?;
    if task.summary {
        return Ok(());
    }
    let minutes = minutes.max(0);
    let indexes: Vec<usize> = project
        .assignments
        .iter()
        .enumerate()
        .filter(|(_, assignment)| assignment.task_uid == uid)
        .map(|(index, _)| index)
        .collect();
    if indexes.is_empty() {
        project.task_mut(uid).unwrap().work = minutes;
    } else {
        let old: i64 = indexes.iter().map(|i| project.assignments[*i].work).sum();
        if old > 0 {
            for index in &indexes {
                let share = project.assignments[*index].work as f64 / old as f64;
                project.assignments[*index].work = (share * minutes as f64).round().max(0.0) as i64;
            }
        } else if let Some(index) = indexes.first() {
            project.assignments[*index].work = minutes;
        }
    }
    work::apply_change(project, uid, Changed::Work);
    Ok(())
}

fn set_overtime_work(project: &mut Project, uid: u32, minutes: i64) -> Result<(), EngineError> {
    let task = project.task(uid).ok_or(EngineError::TaskNotFound)?;
    if task.summary {
        return Ok(());
    }
    let minutes = minutes.max(0);
    let indexes: Vec<usize> = project
        .assignments
        .iter()
        .enumerate()
        .filter(|(_, assignment)| assignment.task_uid == uid)
        .map(|(index, _)| index)
        .collect();
    if indexes.is_empty() {
        return Err(EngineError::Store(
            "Hãy gán nguồn lực trước khi nhập giờ làm thêm".into(),
        ));
    }
    let old: i64 = indexes.iter().map(|i| project.assignments[*i].work).sum();
    if old <= 0 {
        project.assignments[indexes[0]].overtime_work = minutes;
        for index in indexes.iter().skip(1) {
            project.assignments[*index].overtime_work = 0;
        }
    } else {
        let mut assigned = 0i64;
        for (offset, index) in indexes.iter().enumerate() {
            let piece = if offset + 1 == indexes.len() {
                minutes - assigned
            } else {
                let share = (project.assignments[*index].work as f64 / old as f64 * minutes as f64)
                    .round() as i64;
                assigned += share;
                share
            };
            project.assignments[*index].overtime_work = piece.max(0);
        }
    }
    for index in indexes {
        let work = project.assignments[index].work.max(0);
        if project.assignments[index].overtime_work > work {
            project.assignments[index].overtime_work = work;
        }
    }
    Ok(())
}

fn set_remaining_work(project: &mut Project, uid: u32, minutes: i64) -> Result<(), EngineError> {
    let task = project.task(uid).ok_or(EngineError::TaskNotFound)?;
    if task.summary {
        return Ok(());
    }
    let remaining = minutes.max(0);
    if task.work <= 0 {
        set_work(project, uid, remaining)?;
        if let Some(task) = project.task_mut(uid) {
            task.percent_complete = 0.0;
        }
        return Ok(());
    }
    if remaining >= task.work {
        set_work(project, uid, remaining)?;
        if let Some(task) = project.task_mut(uid) {
            task.percent_complete = 0.0;
            task.actual_start = None;
            task.actual_finish = None;
        }
    } else {
        let work = task.work;
        let task = project.task_mut(uid).unwrap();
        task.percent_complete = (work - remaining) as f64 / work as f64 * 100.0;
        if task.percent_complete >= 100.0 {
            task.actual_finish = Some(task.finish);
        } else {
            task.actual_finish = None;
        }
    }
    Ok(())
}

pub fn parse_predecessor_text(text: &str) -> Result<Vec<(u32, DepKind, i64)>, EngineError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let mut links = Vec::new();
    for token in trimmed.split([',', ';']) {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        links.push(parse_predecessor_token(token)?);
    }
    Ok(links)
}

fn parse_predecessor_token(token: &str) -> Result<(u32, DepKind, i64), EngineError> {
    let mut chars = token.chars().peekable();
    let mut id_text = String::new();
    while matches!(chars.peek(), Some(ch) if ch.is_ascii_digit()) {
        id_text.push(chars.next().unwrap());
    }
    if id_text.is_empty() {
        return Err(EngineError::BadPredecessor(token.into()));
    }
    let id: u32 = id_text
        .parse()
        .map_err(|_| EngineError::BadPredecessor(token.into()))?;
    let rest: String = chars.collect();
    let rest = rest.trim();
    if rest.is_empty() {
        return Ok((id, DepKind::Fs, 0));
    }
    let kind = rest
        .get(..2)
        .and_then(DepKind::parse)
        .unwrap_or(DepKind::Fs);
    let after = if rest.get(..2).and_then(DepKind::parse).is_some() {
        rest[2..].trim()
    } else {
        rest
    };
    let lag = parse_lag(after).map_err(|_| EngineError::BadPredecessor(token.into()))?;
    Ok((id, kind, lag))
}

fn parse_lag(text: &str) -> Result<i64, ()> {
    let text = text.trim().replace(',', ".");
    if text.is_empty() {
        return Ok(0);
    }
    let sign = if text.starts_with('-') { -1i64 } else { 1 };
    let text = text.trim_start_matches(['+', '-']).trim();
    let mut number = String::new();
    let mut unit = String::new();
    for ch in text.chars() {
        if (ch.is_ascii_digit() || ch == '.') && unit.is_empty() {
            number.push(ch);
        } else if ch.is_whitespace() {
            continue;
        } else {
            unit.push(ch);
        }
    }
    let value: f64 = number.parse().map_err(|_| ())?;
    let minutes = match unit.to_lowercase().as_str() {
        "" | "d" | "day" | "days" | "ngày" | "ngay" => value * MINUTES_PER_DAY as f64,
        "h" | "hr" | "hour" | "hours" | "giờ" | "gio" => value * MINUTES_PER_HOUR as f64,
        "m" | "min" | "phút" | "phut" => value,
        _ => return Err(()),
    };
    Ok(sign * minutes.round() as i64)
}

fn set_predecessors(project: &mut Project, uid: u32, text: &str) -> Result<(), EngineError> {
    if project.task(uid).is_none() {
        return Err(EngineError::TaskNotFound);
    }
    normalize_outline(project);
    let links = parse_predecessor_text(text)?;
    project.dependencies.retain(|dep| dep.successor_uid != uid);
    for (id, kind, lag) in links {
        let predecessor_uid = project
            .tasks
            .iter()
            .find(|task| task.id == id)
            .map(|task| task.uid)
            .ok_or_else(|| EngineError::BadPredecessor(id.to_string()))?;
        if predecessor_uid == uid {
            return Err(EngineError::DependencyCycle);
        }
        let dep_uid = project.alloc_dependency_uid();
        project.dependencies.push(Dependency {
            uid: dep_uid,
            predecessor_uid,
            successor_uid: uid,
            kind,
            lag,
        });
    }
    Ok(())
}

fn normalize_ranges(ranges: &[TimeRange]) -> Result<Vec<TimeRange>, EngineError> {
    let mut ordered = ranges.to_vec();
    ordered.sort_by_key(|range| (range.start_minute, range.end_minute));
    let mut last_end = 0u16;
    for range in &ordered {
        if range.end_minute > 24 * 60 || range.start_minute >= range.end_minute {
            return Err(EngineError::Store("Khoảng giờ không hợp lệ".into()));
        }
        if range.start_minute < last_end {
            return Err(EngineError::Store("Các khoảng giờ bị chồng nhau".into()));
        }
        last_end = range.end_minute;
    }
    Ok(ordered)
}

use std::collections::BTreeMap;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::calendar::{Calendar, TimeRange};
use crate::error::EngineError;
use crate::model::{
    Accrual, Assignment, Baseline, ConstraintType, DepKind, Dependency, Project, Resource,
    ResourceKind, Task, TaskType,
};
use crate::schedule::schedule;
use crate::timeutil::{
    format_duration_iso, format_iso, parse_duration_iso, parse_iso,
};

/// Write MSPDI XML for tasks, dependencies, calendars, resources, assignments, and baselines.
pub fn write_mspdi(project: &Project) -> Result<String, EngineError> {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Project xmlns=\"http://schemas.microsoft.com/project\">\n",
    );
    el(&mut xml, "Name", &escape(&project.name));
    el(&mut xml, "StartDate", &format_iso(project.start));
    if let Some(finish) = project.tasks.iter().map(|task| task.finish).max() {
        el(&mut xml, "FinishDate", &format_iso(finish));
    }
    el(&mut xml, "CalendarUID", &project.calendar_id.to_string());
    if let Some(status) = project.status_date {
        el(&mut xml, "StatusDate", &format_iso(status));
    }
    xml.push_str("<Calendars>\n");
    for calendar in &project.calendars {
        write_calendar(&mut xml, calendar);
    }
    xml.push_str("</Calendars>\n<Tasks>\n");
    for task in &project.tasks {
        write_task(&mut xml, project, task);
    }
    xml.push_str("</Tasks>\n<Resources>\n");
    for resource in &project.resources {
        write_resource(&mut xml, resource);
    }
    xml.push_str("</Resources>\n<Assignments>\n");
    for assignment in &project.assignments {
        write_assignment(&mut xml, assignment);
    }
    xml.push_str("</Assignments>\n</Project>\n");
    Ok(xml)
}

pub fn read_mspdi(xml: &str) -> Result<Project, EngineError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<String> = Vec::new();
    let mut text = String::new();
    let mut project = Project::new("Dự án", 0);
    project.calendars.clear();
    project.tasks.clear();
    let mut calendar: Option<CalendarBuilder> = None;
    let mut weekday: Option<WeekdayBuilder> = None;
    let mut working: Option<TimeRange> = None;
    let mut exception: Option<ExceptionBuilder> = None;
    let mut task: Option<TaskBuilder> = None;
    let mut link: Option<LinkBuilder> = None;
    let mut baseline: Option<Baseline> = None;
    let mut resource: Option<ResourceBuilder> = None;
    let mut assignment: Option<AssignmentBuilder> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                stack.push(local_name(event.name().as_ref()));
                text.clear();
                match stack.last().map(String::as_str) {
                    Some("Calendar") => calendar = Some(CalendarBuilder::default()),
                    Some("WeekDay") => weekday = Some(WeekdayBuilder::default()),
                    Some("WorkingTime") => {
                        working = Some(TimeRange {
                            start_minute: 0,
                            end_minute: 0,
                        })
                    }
                    Some("Exception") => exception = Some(ExceptionBuilder::default()),
                    Some("Task") => task = Some(TaskBuilder::default()),
                    Some("PredecessorLink") => link = Some(LinkBuilder::default()),
                    Some("Baseline") => {
                        baseline = Some(Baseline {
                            number: 0,
                            start: 0,
                            finish: 0,
                            work: 0,
                            cost: 0.0,
                        })
                    }
                    Some("Resource") => resource = Some(ResourceBuilder::default()),
                    Some("Assignment") => assignment = Some(AssignmentBuilder::default()),
                    _ => {}
                }
            }
            Ok(Event::Empty(event)) => {
                let _ = local_name(event.name().as_ref());
            }
            Ok(Event::Text(event)) => {
                text = event.unescape().unwrap_or_default().into_owned();
            }
            Ok(Event::End(event)) => {
                let name = local_name(event.name().as_ref());
                let parent = stack.iter().rev().nth(1).map(String::as_str);
                apply_text(
                    &name,
                    parent,
                    &text,
                    &mut project,
                    calendar.as_mut(),
                    weekday.as_mut(),
                    working.as_mut(),
                    exception.as_mut(),
                    task.as_mut(),
                    link.as_mut(),
                    baseline.as_mut(),
                    resource.as_mut(),
                    assignment.as_mut(),
                );
                match name.as_str() {
                    "WorkingTime" => {
                        if let (Some(day), Some(range)) = (weekday.as_mut(), working.take()) {
                            if range.end_minute > range.start_minute {
                                day.ranges.push(range);
                            }
                        }
                    }
                    "WeekDay" => {
                        if let (Some(cal), Some(day)) = (calendar.as_mut(), weekday.take()) {
                            if let Some(index) = day_type_index(day.day_type) {
                                cal.week[index] = if day.working { day.ranges } else { Vec::new() };
                            }
                        }
                    }
                    "Exception" => {
                        if let (Some(cal), Some(item)) = (calendar.as_mut(), exception.take()) {
                            if let Some(date) = item.from.as_deref().and_then(parse_iso).map(|ms| {
                                crate::timeutil::date_key(crate::timeutil::civil(ms).0)
                            }) {
                                let ranges = if item.working { item.ranges.clone() } else { Vec::new() };
                                cal.exceptions.insert(date, ranges);
                            }
                        }
                    }
                    "Calendar" => {
                        if let Some(builder) = calendar.take() {
                            project.calendars.push(builder.finish());
                        }
                    }
                    "PredecessorLink" => {
                        if let (Some(task), Some(link)) = (task.as_mut(), link.take()) {
                            task.links.push(link);
                        }
                    }
                    "Baseline" => {
                        if let (Some(task), Some(baseline)) = (task.as_mut(), baseline.take()) {
                            task.baselines.push(baseline);
                        }
                    }
                    "Task" => {
                        if let Some(builder) = task.take() {
                            if !builder.is_null && builder.uid != 0 {
                                let mut next_dep = project.next_dependency_uid;
                                let (parsed, deps) = builder.into_task_and_links(&mut next_dep);
                                project.next_dependency_uid = next_dep;
                                project.tasks.push(parsed);
                                project.dependencies.extend(deps);
                            }
                        }
                    }
                    "Resource" => {
                        if let Some(builder) = resource.take() {
                            if builder.uid != 0 {
                                project.resources.push(builder.finish());
                            }
                        }
                    }
                    "Assignment" => {
                        if let Some(builder) = assignment.take() {
                            if builder.task_uid != 0 && builder.resource_uid != 0 {
                                project.assignments.push(builder.finish());
                            }
                        }
                    }
                    _ => {}
                }
                if stack.last().map(String::as_str) == Some(name.as_str()) {
                    stack.pop();
                }
                text.clear();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(EngineError::Xml(err.to_string())),
            _ => {}
        }
    }

    if project.calendars.is_empty() {
        project.calendars.push(Calendar::standard());
    }
    if project.calendar_id == 0 {
        project.calendar_id = project.calendars[0].id;
    }
    if project.calendar_by_id(project.calendar_id).is_none() {
        project.calendar_id = project.calendars[0].id;
    }
    project.next_task_uid = project.tasks.iter().map(|task| task.uid).max().unwrap_or(0) + 1;
    project.next_resource_uid = project
        .resources
        .iter()
        .map(|resource| resource.uid)
        .max()
        .unwrap_or(0)
        + 1;
    project.next_assignment_uid = project
        .assignments
        .iter()
        .map(|assignment| assignment.uid)
        .max()
        .unwrap_or(0)
        + 1;
    project.next_dependency_uid = project
        .dependencies
        .iter()
        .map(|dep| dep.uid)
        .max()
        .unwrap_or(0)
        + 1;
    project.next_calendar_uid = project.calendars.iter().map(|calendar| calendar.id).max().unwrap_or(0) + 1;
    for task in &mut project.tasks {
        let work: i64 = project
            .assignments
            .iter()
            .filter(|assignment| assignment.task_uid == task.uid)
            .map(|assignment| assignment.work)
            .sum();
        let units: f64 = project
            .assignments
            .iter()
            .filter(|assignment| assignment.task_uid == task.uid)
            .map(|assignment| assignment.units)
            .sum();
        if work > 0 {
            task.work = work;
        }
        if units > 0.0 {
            task.units = units;
        }
    }
    project.start = project.calendar().next_work_start(project.start);
    schedule(&mut project)?;
    Ok(project)
}

fn apply_text(
    name: &str,
    parent: Option<&str>,
    text: &str,
    project: &mut Project,
    calendar: Option<&mut CalendarBuilder>,
    weekday: Option<&mut WeekdayBuilder>,
    working: Option<&mut TimeRange>,
    exception: Option<&mut ExceptionBuilder>,
    task: Option<&mut TaskBuilder>,
    link: Option<&mut LinkBuilder>,
    baseline: Option<&mut Baseline>,
    resource: Option<&mut ResourceBuilder>,
    assignment: Option<&mut AssignmentBuilder>,
) {
    let value = text.trim();
    if value.is_empty() {
        return;
    }
    match (parent, name) {
        (Some("Project"), "Name") => project.name = value.to_string(),
        (Some("Project"), "StartDate") => {
            if let Some(ms) = parse_iso(value) {
                project.start = ms;
            }
        }
        (Some("Project"), "CalendarUID") => {
            if let Ok(id) = value.parse() {
                project.calendar_id = id;
            }
        }
        (Some("Project"), "StatusDate") => project.status_date = parse_iso(value),
        (Some("Calendar"), "UID") => {
            if let Some(calendar) = calendar {
                calendar.id = value.parse().unwrap_or(calendar.id);
            }
        }
        (Some("Calendar"), "Name") => {
            if let Some(calendar) = calendar {
                calendar.name = value.to_string();
            }
        }
        (Some("WeekDay"), "DayType") => {
            if let Some(day) = weekday {
                day.day_type = value.parse().unwrap_or(0);
            }
        }
        (Some("WeekDay"), "DayWorking") => {
            if let Some(day) = weekday {
                day.working = value != "0";
            }
        }
        (Some("WorkingTime"), "FromTime") => {
            if let Some(range) = working {
                range.start_minute = parse_clock(value);
            }
        }
        (Some("WorkingTime"), "ToTime") => {
            if let Some(range) = working {
                range.end_minute = parse_clock(value);
            }
        }
        (Some("Exception"), "DayWorking") => {
            if let Some(item) = exception {
                item.working = value != "0";
            }
        }
        (Some("TimePeriod"), "FromDate") => {
            if let Some(item) = exception {
                item.from = Some(value.to_string());
            }
        }
        (Some("Task"), "UID") => {
            if let Some(task) = task {
                task.uid = value.parse().unwrap_or(0);
            }
        }
        (Some("Task"), "Name") => {
            if let Some(task) = task {
                task.name = value.to_string();
            }
        }
        (Some("Task"), "OutlineLevel") => {
            if let Some(task) = task {
                task.outline_level = value.parse().unwrap_or(1);
            }
        }
        (Some("Task"), "Duration") => {
            if let Some(task) = task {
                task.duration = parse_duration_iso(value).unwrap_or(0);
            }
        }
        (Some("Task"), "Milestone") => {
            if let Some(task) = task {
                task.milestone = value == "1";
            }
        }
        (Some("Task"), "Manual") => {
            if let Some(task) = task {
                task.manual = value == "1";
            }
        }
        (Some("Task"), "Notes") => {
            if let Some(task) = task {
                task.notes = value.to_string();
            }
        }
        (Some("Task"), "Priority") => {
            if let Some(task) = task {
                task.priority = value.parse().unwrap_or(500);
            }
        }
        (Some("Task"), "ConstraintType") => {
            if let Some(task) = task {
                task.constraint = value.parse().ok().and_then(constraint_from_xml);
            }
        }
        (Some("Task"), "ConstraintDate") => {
            if let Some(task) = task {
                task.constraint_date = parse_iso(value);
            }
        }
        (Some("Task"), "CalendarUID") => {
            if let Some(task) = task {
                task.calendar_id = value.parse().ok();
            }
        }
        (Some("Task"), "Type") => {
            if let Some(task) = task {
                task.task_type = match value {
                    "1" => TaskType::FixedDuration,
                    "2" => TaskType::FixedWork,
                    _ => TaskType::FixedUnits,
                };
            }
        }
        (Some("Task"), "EffortDriven") => {
            if let Some(task) = task {
                task.effort_driven = value != "0";
            }
        }
        (Some("Task"), "PercentComplete") => {
            if let Some(task) = task {
                task.percent = value.parse().unwrap_or(0.0);
            }
        }
        (Some("Task"), "ActualStart") => {
            if let Some(task) = task {
                task.actual_start = parse_iso(value);
            }
        }
        (Some("Task"), "ActualFinish") => {
            if let Some(task) = task {
                task.actual_finish = parse_iso(value);
            }
        }
        (Some("Task"), "RemainingDuration") => {
            if let Some(task) = task {
                task.remaining_duration = parse_duration_iso(value);
            }
        }
        (Some("Task"), "Start") => {
            if let Some(task) = task {
                task.start = parse_iso(value);
            }
        }
        (Some("Task"), "Finish") => {
            if let Some(task) = task {
                task.finish = parse_iso(value);
            }
        }
        (Some("Task"), "FixedCost") => {
            if let Some(task) = task {
                task.fixed_cost = value.parse().unwrap_or(0.0);
            }
        }
        (Some("Task"), "FixedCostAccrual") => {
            if let Some(task) = task {
                task.accrual = match value {
                    "1" => Accrual::Start,
                    "3" => Accrual::End,
                    _ => Accrual::Prorated,
                };
            }
        }
        (Some("Task"), "LevelingDelay") => {
            if let Some(task) = task {
                let tenths: i64 = value.parse().unwrap_or(0);
                task.leveling_delay = tenths / 10;
            }
        }
        (Some("Task"), "IsNull") => {
            if let Some(task) = task {
                task.is_null = value == "1";
            }
        }
        (Some("PredecessorLink"), "PredecessorUID") => {
            if let Some(link) = link {
                link.predecessor = value.parse().unwrap_or(0);
            }
        }
        (Some("PredecessorLink"), "Type") => {
            if let Some(link) = link {
                link.kind = match value {
                    "0" => DepKind::Ff,
                    "2" => DepKind::Ss,
                    "3" => DepKind::Sf,
                    _ => DepKind::Fs,
                };
            }
        }
        (Some("PredecessorLink"), "LinkLag") => {
            if let Some(link) = link {
                let tenths: i64 = value.parse().unwrap_or(0);
                link.lag = tenths / 10;
            }
        }
        (Some("Baseline"), "Number") => {
            if let Some(baseline) = baseline {
                baseline.number = value.parse().unwrap_or(0);
            }
        }
        (Some("Baseline"), "Start") => {
            if let Some(baseline) = baseline {
                baseline.start = parse_iso(value).unwrap_or(0);
            }
        }
        (Some("Baseline"), "Finish") => {
            if let Some(baseline) = baseline {
                baseline.finish = parse_iso(value).unwrap_or(0);
            }
        }
        (Some("Baseline"), "Work") => {
            if let Some(baseline) = baseline {
                baseline.work = parse_duration_iso(value).unwrap_or(0);
            }
        }
        (Some("Baseline"), "Cost") => {
            if let Some(baseline) = baseline {
                baseline.cost = value.parse().unwrap_or(0.0);
            }
        }
        (Some("Resource"), "UID") => {
            if let Some(resource) = resource {
                resource.uid = value.parse().unwrap_or(0);
            }
        }
        (Some("Resource"), "Name") => {
            if let Some(resource) = resource {
                resource.name = value.to_string();
            }
        }
        (Some("Resource"), "Group") => {
            if let Some(resource) = resource {
                resource.group = value.to_string();
            }
        }
        (Some("Resource"), "MaxUnits") => {
            if let Some(resource) = resource {
                resource.max_units = value.parse().unwrap_or(1.0);
            }
        }
        (Some("Resource"), "StandardRate") => {
            if let Some(resource) = resource {
                resource.standard_rate = value.parse().unwrap_or(0.0);
            }
        }
        (Some("Resource"), "OvertimeRate") => {
            if let Some(resource) = resource {
                resource.overtime_rate = value.parse().unwrap_or(0.0);
            }
        }
        (Some("Resource"), "CostPerUse") => {
            if let Some(resource) = resource {
                resource.cost_per_use = value.parse().unwrap_or(0.0);
            }
        }
        (Some("Resource"), "CalendarUID") => {
            if let Some(resource) = resource {
                resource.calendar_id = value.parse().ok();
            }
        }
        (Some("Assignment"), "UID") => {
            if let Some(assignment) = assignment {
                assignment.uid = value.parse().unwrap_or(0);
            }
        }
        (Some("Assignment"), "TaskUID") => {
            if let Some(assignment) = assignment {
                assignment.task_uid = value.parse().unwrap_or(0);
            }
        }
        (Some("Assignment"), "ResourceUID") => {
            if let Some(assignment) = assignment {
                assignment.resource_uid = value.parse().unwrap_or(0);
            }
        }
        (Some("Assignment"), "Units") => {
            if let Some(assignment) = assignment {
                assignment.units = value.parse().unwrap_or(1.0);
            }
        }
        (Some("Assignment"), "Work") => {
            if let Some(assignment) = assignment {
                assignment.work = parse_duration_iso(value).unwrap_or(0);
            }
        }
        (Some("Assignment"), "OvertimeWork") => {
            if let Some(assignment) = assignment {
                assignment.overtime_work = parse_duration_iso(value).unwrap_or(0);
            }
        }
        _ => {}
    }
}

fn write_calendar(xml: &mut String, calendar: &Calendar) {
    xml.push_str("<Calendar>\n");
    el(xml, "UID", &calendar.id.to_string());
    el(xml, "Name", &escape(&calendar.name));
    el(xml, "IsBaseCalendar", "1");
    xml.push_str("<WeekDays>\n");
    for index in 0..7 {
        let day_type = index_to_day_type(index);
        xml.push_str("<WeekDay>\n");
        el(xml, "DayType", &day_type.to_string());
        let ranges = &calendar.week[index];
        el(xml, "DayWorking", if ranges.is_empty() { "0" } else { "1" });
        if !ranges.is_empty() {
            xml.push_str("<WorkingTimes>\n");
            for range in ranges {
                xml.push_str("<WorkingTime>\n");
                el(xml, "FromTime", &format_clock(range.start_minute));
                el(xml, "ToTime", &format_clock(range.end_minute));
                xml.push_str("</WorkingTime>\n");
            }
            xml.push_str("</WorkingTimes>\n");
        }
        xml.push_str("</WeekDay>\n");
    }
    xml.push_str("</WeekDays>\n<Exceptions>\n");
    for (date, ranges) in &calendar.exceptions {
        xml.push_str("<Exception>\n<EnteredByOccurrences>0</EnteredByOccurrences>\n<TimePeriod>\n");
        el(xml, "FromDate", &format!("{date}T00:00:00"));
        el(xml, "ToDate", &format!("{date}T23:59:00"));
        xml.push_str("</TimePeriod>\n");
        el(xml, "Name", "Ngoại lệ");
        el(xml, "DayWorking", if ranges.is_empty() { "0" } else { "1" });
        xml.push_str("</Exception>\n");
    }
    xml.push_str("</Exceptions>\n</Calendar>\n");
}

fn write_task(xml: &mut String, project: &Project, task: &Task) {
    xml.push_str("<Task>\n");
    el(xml, "UID", &task.uid.to_string());
    el(xml, "ID", &task.id.to_string());
    el(xml, "Name", &escape(&task.name));
    el(xml, "Type", &task_type_xml(task.task_type).to_string());
    el(xml, "IsNull", "0");
    el(xml, "WBS", &escape(&task.wbs));
    el(xml, "OutlineNumber", &escape(&task.wbs));
    el(xml, "OutlineLevel", &task.outline_level.to_string());
    el(xml, "Start", &format_iso(task.start));
    el(xml, "Finish", &format_iso(task.finish));
    el(xml, "Duration", &format_duration_iso(task.duration));
    el(xml, "Manual", if task.manual { "1" } else { "0" });
    el(xml, "Milestone", if task.milestone { "1" } else { "0" });
    el(xml, "Summary", if task.summary { "1" } else { "0" });
    el(xml, "Critical", if task.critical { "1" } else { "0" });
    el(xml, "EffortDriven", if task.effort_driven { "1" } else { "0" });
    el(xml, "Priority", &task.priority.to_string());
    el(xml, "Notes", &escape(&task.notes));
    el(
        xml,
        "ConstraintType",
        &constraint_to_xml(task.constraint_type).to_string(),
    );
    if let Some(date) = task.constraint_date {
        el(xml, "ConstraintDate", &format_iso(date));
    }
    if let Some(calendar_id) = task.calendar_id {
        el(xml, "CalendarUID", &calendar_id.to_string());
    }
    el(xml, "PercentComplete", &format_number(task.percent_complete));
    if let Some(start) = task.actual_start {
        el(xml, "ActualStart", &format_iso(start));
    }
    if let Some(finish) = task.actual_finish {
        el(xml, "ActualFinish", &format_iso(finish));
    }
    el(
        xml,
        "RemainingDuration",
        &format_duration_iso(task.remaining_duration),
    );
    el(xml, "FixedCost", &format_number(task.fixed_cost));
    el(
        xml,
        "FixedCostAccrual",
        &match task.fixed_cost_accrual {
            Accrual::Start => "1",
            Accrual::Prorated => "2",
            Accrual::End => "3",
        },
    );
    el(xml, "LevelingDelay", &(task.leveling_delay * 10).to_string());
    for dep in project
        .dependencies
        .iter()
        .filter(|dep| dep.successor_uid == task.uid)
    {
        xml.push_str("<PredecessorLink>\n");
        el(xml, "PredecessorUID", &dep.predecessor_uid.to_string());
        el(xml, "Type", &dep_type_xml(dep.kind).to_string());
        el(xml, "LinkLag", &(dep.lag * 10).to_string());
        el(xml, "LagFormat", "3");
        xml.push_str("</PredecessorLink>\n");
    }
    for baseline in &task.baselines {
        xml.push_str("<Baseline>\n");
        el(xml, "Number", &baseline.number.to_string());
        el(xml, "Start", &format_iso(baseline.start));
        el(xml, "Finish", &format_iso(baseline.finish));
        el(xml, "Work", &format_duration_iso(baseline.work));
        el(xml, "Cost", &format_number(baseline.cost));
        xml.push_str("</Baseline>\n");
    }
    xml.push_str("</Task>\n");
}

fn write_resource(xml: &mut String, resource: &Resource) {
    xml.push_str("<Resource>\n");
    el(xml, "UID", &resource.uid.to_string());
    el(xml, "Name", &escape(&resource.name));
    el(xml, "Type", "1");
    el(
        xml,
        "Group",
        match resource.kind {
            ResourceKind::Person => "Người",
            ResourceKind::Machine => "Máy",
        },
    );
    el(xml, "MaxUnits", &format_number(resource.max_units));
    el(xml, "StandardRate", &format_number(resource.standard_rate));
    el(xml, "OvertimeRate", &format_number(resource.overtime_rate));
    el(xml, "CostPerUse", &format_number(resource.cost_per_use));
    if let Some(calendar_id) = resource.calendar_id {
        el(xml, "CalendarUID", &calendar_id.to_string());
    }
    xml.push_str("</Resource>\n");
}

fn write_assignment(xml: &mut String, assignment: &Assignment) {
    xml.push_str("<Assignment>\n");
    el(xml, "UID", &assignment.uid.to_string());
    el(xml, "TaskUID", &assignment.task_uid.to_string());
    el(xml, "ResourceUID", &assignment.resource_uid.to_string());
    el(xml, "Units", &format_number(assignment.units));
    el(xml, "Work", &format_duration_iso(assignment.work));
    el(
        xml,
        "OvertimeWork",
        &format_duration_iso(assignment.overtime_work),
    );
    el(xml, "Cost", &format_number(assignment.cost));
    xml.push_str("</Assignment>\n");
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

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

fn format_clock(minute: u16) -> String {
    format!("{:02}:{:02}:00", minute / 60, minute % 60)
}

fn parse_clock(text: &str) -> u16 {
    let mut parts = text.split(':');
    let hour: u16 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let minute: u16 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    hour * 60 + minute
}

fn local_name(qname: &[u8]) -> String {
    let text = String::from_utf8_lossy(qname);
    match text.rfind('}') {
        Some(index) => text[index + 1..].to_string(),
        None => text.to_string(),
    }
}

fn index_to_day_type(index: usize) -> u8 {
    match index {
        0 => 2,
        1 => 3,
        2 => 4,
        3 => 5,
        4 => 6,
        5 => 7,
        _ => 1,
    }
}

fn day_type_index(day_type: u8) -> Option<usize> {
    match day_type {
        1 => Some(6),
        2 => Some(0),
        3 => Some(1),
        4 => Some(2),
        5 => Some(3),
        6 => Some(4),
        7 => Some(5),
        _ => None,
    }
}

fn constraint_to_xml(constraint: ConstraintType) -> i32 {
    match constraint {
        ConstraintType::Asap => 0,
        ConstraintType::Alap => 1,
        ConstraintType::Mso => 2,
        ConstraintType::Mfo => 3,
        ConstraintType::Snet => 4,
        ConstraintType::Snlt => 5,
        ConstraintType::Fnet => 6,
        ConstraintType::Fnlt => 7,
    }
}

fn constraint_from_xml(value: i32) -> Option<ConstraintType> {
    Some(match value {
        0 => ConstraintType::Asap,
        1 => ConstraintType::Alap,
        2 => ConstraintType::Mso,
        3 => ConstraintType::Mfo,
        4 => ConstraintType::Snet,
        5 => ConstraintType::Snlt,
        6 => ConstraintType::Fnet,
        7 => ConstraintType::Fnlt,
        _ => return None,
    })
}

fn task_type_xml(task_type: TaskType) -> i32 {
    match task_type {
        TaskType::FixedUnits => 0,
        TaskType::FixedDuration => 1,
        TaskType::FixedWork => 2,
    }
}

fn dep_type_xml(kind: DepKind) -> i32 {
    match kind {
        DepKind::Ff => 0,
        DepKind::Fs => 1,
        DepKind::Ss => 2,
        DepKind::Sf => 3,
    }
}

#[derive(Default)]
struct CalendarBuilder {
    id: u32,
    name: String,
    week: [Vec<TimeRange>; 7],
    exceptions: BTreeMap<String, Vec<TimeRange>>,
}

impl CalendarBuilder {
    fn finish(self) -> Calendar {
        Calendar {
            id: if self.id == 0 { 1 } else { self.id },
            name: if self.name.is_empty() {
                "Chuẩn".into()
            } else {
                self.name
            },
            week: self.week,
            exceptions: self.exceptions,
        }
    }
}

#[derive(Default)]
struct WeekdayBuilder {
    day_type: u8,
    working: bool,
    ranges: Vec<TimeRange>,
}

#[derive(Default)]
struct ExceptionBuilder {
    working: bool,
    from: Option<String>,
    ranges: Vec<TimeRange>,
}

#[derive(Clone, Default)]
struct LinkBuilder {
    predecessor: u32,
    kind: DepKind,
    lag: i64,
}

impl Default for DepKind {
    fn default() -> Self {
        Self::Fs
    }
}

#[derive(Default)]
struct TaskBuilder {
    uid: u32,
    name: String,
    outline_level: u32,
    duration: i64,
    milestone: bool,
    manual: bool,
    notes: String,
    priority: i32,
    constraint: Option<ConstraintType>,
    constraint_date: Option<i64>,
    calendar_id: Option<u32>,
    task_type: TaskType,
    effort_driven: bool,
    percent: f64,
    actual_start: Option<i64>,
    actual_finish: Option<i64>,
    remaining_duration: Option<i64>,
    start: Option<i64>,
    finish: Option<i64>,
    fixed_cost: f64,
    accrual: Accrual,
    leveling_delay: i64,
    is_null: bool,
    links: Vec<LinkBuilder>,
    baselines: Vec<Baseline>,
}

impl TaskBuilder {
    fn finish(self) -> Task {
        let name = if self.name.is_empty() {
            "Công việc".to_string()
        } else {
            self.name.clone()
        };
        let mut task = Task::new(self.uid, name);
        task.outline_level = self.outline_level.max(1);
        task.duration = if self.milestone { 0 } else { self.duration };
        task.milestone = self.milestone || task.duration == 0;
        task.manual = self.manual;
        task.notes = self.notes;
        task.priority = if self.priority == 0 { 500 } else { self.priority };
        if let Some(constraint) = self.constraint {
            task.constraint_type = constraint;
        }
        task.constraint_date = self.constraint_date;
        task.calendar_id = self.calendar_id;
        task.task_type = self.task_type;
        task.effort_driven = self.effort_driven || self.task_type == TaskType::FixedWork;
        task.percent_complete = self.percent;
        task.actual_start = self.actual_start;
        task.actual_finish = self.actual_finish;
        task.remaining_duration = self.remaining_duration.unwrap_or(task.duration);
        if let Some(start) = self.start {
            task.start = start;
        }
        if let Some(finish) = self.finish {
            task.finish = finish;
        }
        task.fixed_cost = self.fixed_cost;
        task.fixed_cost_accrual = self.accrual;
        task.leveling_delay = self.leveling_delay;
        task.work = task.duration;
        task.baselines = self.baselines;
        let dependencies = self.links;
        // Dependencies are attached by the caller via a side channel: stash them in notes? No.
        // The builder is converted in read_mspdi before dependencies exist on the task.
        // Store links in the task by pushing after finish — handled below by returning links separately.
        let _ = dependencies;
        task
    }
}

#[derive(Default)]
struct ResourceBuilder {
    uid: u32,
    name: String,
    group: String,
    max_units: f64,
    standard_rate: f64,
    overtime_rate: f64,
    cost_per_use: f64,
    calendar_id: Option<u32>,
}

impl ResourceBuilder {
    fn finish(self) -> Resource {
        let kind = if self.group.contains("Máy") || self.group.eq_ignore_ascii_case("machine") {
            ResourceKind::Machine
        } else {
            ResourceKind::Person
        };
        Resource {
            uid: self.uid,
            name: if self.name.is_empty() {
                format!("Nguồn lực {}", self.uid)
            } else {
                self.name
            },
            kind,
            max_units: if self.max_units <= 0.0 { 1.0 } else { self.max_units },
            calendar_id: self.calendar_id,
            standard_rate: self.standard_rate,
            overtime_rate: self.overtime_rate,
            cost_per_use: self.cost_per_use,
        }
    }
}

#[derive(Default)]
struct AssignmentBuilder {
    uid: u32,
    task_uid: u32,
    resource_uid: u32,
    units: f64,
    work: i64,
    overtime_work: i64,
}

impl AssignmentBuilder {
    fn finish(self) -> Assignment {
        Assignment {
            uid: self.uid,
            task_uid: self.task_uid,
            resource_uid: self.resource_uid,
            units: if self.units <= 0.0 { 1.0 } else { self.units },
            work: self.work,
            overtime_work: self.overtime_work,
            actual_work: 0,
            remaining_work: self.work,
            cost: 0.0,
            actual_cost: 0.0,
        }
    }
}

// The task builder drops links. Re-read them by keeping a parallel list.
// `read_mspdi` pushes `builder.finish()` and loses links. Fix that in `Task` conversion
// by writing dependencies from a thread-local? Better: change the push site.
// This helper is used by a second pass stored on the builder via a public field consumed
// before finish. See `take_links` below.

impl TaskBuilder {
    fn into_task_and_links(self, next_dep: &mut u32) -> (Task, Vec<Dependency>) {
        let links = self.links.clone();
        let uid = self.uid;
        let task = self.finish();
        let mut dependencies = Vec::new();
        for link in links {
            if link.predecessor == 0 {
                continue;
            }
            let dep_uid = *next_dep;
            *next_dep += 1;
            dependencies.push(Dependency {
                uid: dep_uid,
                predecessor_uid: link.predecessor,
                successor_uid: uid,
                kind: link.kind,
                lag: link.lag,
            });
        }
        (task, dependencies)
    }
}


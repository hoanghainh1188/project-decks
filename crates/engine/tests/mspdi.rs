//! MSPDI round trips: export, import, and compare field by field, plus a handwritten
//! fixture (not from Microsoft Project) for the progress fields.

use chrono::Datelike;

use sheafly_engine::command::Command;
use sheafly_engine::model::{ResourceKind, Task};
use sheafly_engine::mspdi::{read_mspdi, write_mspdi};
use sheafly_engine::schedule::schedule;
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{Document, Project};

const DAY: i64 = 480;

/// 2026-01-05 is a Monday; `day` 0 is that Monday.
fn at(day: u32, hour: u32, minute: u32) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(i64::from(day)))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, minute)
}

fn doc_with(days: &[i64]) -> Document {
    let mut project = Project::new("MSPDI", at(0, 8, 0));
    for (index, days) in days.iter().enumerate() {
        let mut task = Task::new(index as u32 + 1, format!("Việc {}", index + 1));
        task.duration = days * DAY;
        task.work = days * DAY;
        task.remaining_duration = days * DAY;
        task.remaining_work = days * DAY;
        project.tasks.push(task);
    }
    project.next_task_uid = days.len() as u32 + 1;
    schedule(&mut project).unwrap();
    Document::from_project(project)
}

fn round_trip(project: &Project) -> (String, Project) {
    let xml = write_mspdi(project).unwrap();
    (xml.clone(), read_mspdi(&xml).unwrap())
}

fn assert_same_progress(before: &Task, after: &Task) {
    let name = &before.name;
    assert_eq!(after.start, before.start, "{name}: start");
    assert_eq!(after.finish, before.finish, "{name}: finish");
    assert_eq!(after.duration, before.duration, "{name}: duration");
    assert_eq!(after.deadline, before.deadline, "{name}: deadline");
    assert_eq!(
        after.actual_start, before.actual_start,
        "{name}: actual start"
    );
    assert_eq!(
        after.actual_finish, before.actual_finish,
        "{name}: actual finish"
    );
    assert!(
        (after.percent_complete - before.percent_complete).abs() < 1e-6,
        "{name}: percent {} vs {}",
        after.percent_complete,
        before.percent_complete
    );
    assert_eq!(
        after.remaining_duration, before.remaining_duration,
        "{name}: remaining duration"
    );
    assert_eq!(after.work, before.work, "{name}: work");
    assert_eq!(after.actual_work, before.actual_work, "{name}: actual work");
    assert_eq!(
        after.remaining_work, before.remaining_work,
        "{name}: remaining work"
    );
    assert_eq!(after.late, before.late, "{name}: late");
    assert_eq!(after.splits, before.splits, "{name}: splits");
}

fn element<'a>(xml: &'a str, task_name: &str, field: &str) -> Option<&'a str> {
    let at = xml.find(&format!("<Name>{task_name}</Name>"))?;
    let task = &xml[at..at + xml[at..].find("</Task>")?];
    let open = format!("<{field}>");
    let start = task.find(&open)? + open.len();
    Some(&task[start..start + task[start..].find('<')?])
}

#[test]
fn deadline_and_progress_survive_a_round_trip() {
    let mut doc = doc_with(&[2, 4, 3, 5, 4]);
    doc.apply(Command::AddResource {
        name: "An".into(),
        kind: ResourceKind::Person,
    })
    .unwrap();
    for task_uid in [2, 3] {
        doc.apply(Command::AssignResource {
            task_uid,
            resource_uid: 1,
            units: 1.0,
        })
        .unwrap();
    }
    // 1: finished; 2: half done with a resource; 3: split and started;
    // 4: deadline only; 5: deadline and 25 % done.
    doc.apply(Command::SetActualFinish {
        uid: 1,
        finish: Some(at(1, 17, 0)),
    })
    .unwrap();
    doc.apply(Command::SetPercentComplete {
        uid: 2,
        percent: 50.0,
    })
    .unwrap();
    doc.apply(Command::SplitTask {
        uid: 3,
        at: at(1, 8, 0),
        gap: DAY,
    })
    .unwrap();
    doc.apply(Command::SetPercentComplete {
        uid: 3,
        percent: 25.0,
    })
    .unwrap();
    doc.apply(Command::SetDeadline {
        uid: 4,
        date: Some(at(11, 17, 0)),
    })
    .unwrap();
    doc.apply(Command::SetDeadline {
        uid: 5,
        date: Some(at(1, 17, 0)),
    })
    .unwrap();
    doc.apply(Command::SetPercentComplete {
        uid: 5,
        percent: 25.0,
    })
    .unwrap();

    let (xml, back) = round_trip(&doc.project);
    for before in &doc.project.tasks {
        assert_same_progress(before, back.task(before.uid).unwrap());
    }
    assert!(back.task(5).unwrap().late, "past its deadline");

    // The written fields themselves.
    assert_eq!(
        element(&xml, "Việc 4", "Deadline"),
        Some("2026-01-16T17:00:00")
    );
    assert_eq!(element(&xml, "Việc 2", "ActualDuration"), Some("PT16H0M0S"));
    assert_eq!(element(&xml, "Việc 2", "ActualWork"), Some("PT16H0M0S"));
    assert_eq!(element(&xml, "Việc 2", "RemainingWork"), Some("PT16H0M0S"));
    assert_eq!(element(&xml, "Việc 2", "Stop"), Some("2026-01-06T17:00:00"));
    assert_eq!(
        element(&xml, "Việc 2", "Resume"),
        Some("2026-01-07T08:00:00")
    );
    assert_eq!(
        element(&xml, "Việc 1", "Resume"),
        None,
        "finished: no resume"
    );
    assert_eq!(
        element(&xml, "Việc 4", "Stop"),
        None,
        "not started: no stop"
    );
    // Split task 3 (3 days, gap of a day after day 1): 25 % of 24 h is 6 h, all before
    // the split, so it stops on Monday 15:00 and resumes there.
    assert_eq!(element(&xml, "Việc 3", "Stop"), Some("2026-01-05T15:00:00"));
    assert_eq!(
        element(&xml, "Việc 3", "Resume"),
        Some("2026-01-05T15:00:00")
    );
}

#[test]
fn started_assignments_write_timephased_actual_work_per_segment() {
    let mut doc = doc_with(&[3]);
    doc.apply(Command::AddResource {
        name: "An".into(),
        kind: ResourceKind::Person,
    })
    .unwrap();
    doc.apply(Command::AssignResource {
        task_uid: 1,
        resource_uid: 1,
        units: 1.0,
    })
    .unwrap();
    doc.apply(Command::SplitTask {
        uid: 1,
        at: at(1, 8, 0),
        gap: DAY,
    })
    .unwrap();
    // 2 of 3 days done: day 0, then the second piece's first day (Wednesday).
    doc.apply(Command::SetPercentComplete {
        uid: 1,
        percent: 200.0 / 3.0,
    })
    .unwrap();

    let (xml, back) = round_trip(&doc.project);
    let actual: Vec<&str> = xml
        .split("<TimephasedData>")
        .skip(1)
        .filter(|period| period.contains("<Type>2</Type>"))
        .collect();
    assert_eq!(actual.len(), 2, "one actual period per worked segment");
    assert!(actual[0].contains("<Start>2026-01-05T08:00:00</Start>"));
    assert!(actual[0].contains("<Value>PT8H0M0S</Value>"));
    assert!(actual[1].contains("<Start>2026-01-07T08:00:00</Start>"));
    assert!(actual[1].contains("<Finish>2026-01-07T17:00:00</Finish>"));
    assert!(actual[1].contains("<Value>PT8H0M0S</Value>"));
    assert_same_progress(doc.project.task(1).unwrap(), back.task(1).unwrap());
    assert_eq!(
        back.assignments[0].actual_work,
        doc.project.assignments[0].actual_work
    );

    // Not started: no actual periods at all.
    let (xml, _) = round_trip(&doc_with(&[2]).project);
    assert!(!xml.contains("<Type>2</Type>"));
}

#[test]
fn an_unstarted_project_round_trips_without_progress() {
    let doc = doc_with(&[1, 2]);
    let (_, back) = round_trip(&doc.project);
    for before in &doc.project.tasks {
        let after = back.task(before.uid).unwrap();
        assert_same_progress(before, after);
        assert_eq!(after.actual_start, None);
    }
}

#[test]
fn handwritten_file_lets_actuals_and_remaining_win_over_percent() {
    let xml = include_str!("fixtures/progress.xml");
    let project = read_mspdi(xml).unwrap();
    let task = |uid: u32| project.task(uid).unwrap();

    // An actual finish completes the task whatever PercentComplete says.
    assert_eq!(task(1).percent_complete, 100.0);
    assert_eq!(task(1).actual_finish, Some(at(1, 17, 0)));

    // ActualDuration / RemainingDuration: 16 h of 32 h.
    assert!((task(2).percent_complete - 50.0).abs() < 1e-6);
    assert_eq!(task(2).actual_start, Some(at(0, 8, 0)));
    assert_eq!(task(2).actual_finish, None);
    assert_eq!(task(2).remaining_duration, 2 * DAY);

    // ActualWork / RemainingWork, without actual dates: started where scheduled.
    assert!((task(3).percent_complete - 25.0).abs() < 1e-6);
    assert_eq!(task(3).actual_start, Some(task(3).start));

    // Deadline, and nothing started.
    assert_eq!(task(4).deadline, Some(at(11, 17, 0)));
    assert_eq!(task(4).actual_start, None);
    assert_eq!(task(4).percent_complete, 0.0);

    // Only timephased actual work: 8 h done, 8 h left, started on Tuesday, which
    // pins the start even though the task would otherwise start on Monday.
    assert!((task(5).percent_complete - 50.0).abs() < 1e-6);
    assert_eq!(task(5).actual_start, Some(at(1, 8, 0)));
    assert_eq!(task(5).start, at(1, 8, 0));
    assert_eq!(task(5).actual_work, DAY);

    // And the result itself round-trips.
    let (_, back) = round_trip(&project);
    for before in &project.tasks {
        assert_same_progress(before, back.task(before.uid).unwrap());
    }
}

#[test]
fn stop_comes_before_resume_and_deadline_after_the_constraint_date() {
    let mut doc = doc_with(&[4]);
    doc.apply(Command::SetDeadline {
        uid: 1,
        date: Some(at(9, 17, 0)),
    })
    .unwrap();
    doc.apply(Command::SetPercentComplete {
        uid: 1,
        percent: 50.5,
    })
    .unwrap();
    let (xml, back) = round_trip(&doc.project);
    let pos = |tag: &str| xml.find(tag).unwrap();
    assert!(pos("<Duration>") < pos("<Stop>"));
    assert!(pos("<Stop>") < pos("<Resume>"));
    assert!(pos("<ConstraintType>") < pos("<Deadline>"));
    assert!(pos("<Deadline>") < pos("<PercentComplete>"));
    // Rounded actual/remaining minutes do not nudge the percent.
    assert_eq!(back.task(1).unwrap().percent_complete, 50.5);
}

fn one_task(fields: &str, assignment: &str) -> String {
    format!(
        "<Project xmlns=\"http://schemas.microsoft.com/project\"><StartDate>2026-01-05T08:00:00</StartDate>\
         <Tasks><Task><UID>1</UID><Name>T</Name><OutlineLevel>1</OutlineLevel>{fields}</Task></Tasks>\
         <Resources><Resource><UID>1</UID><Name>An</Name></Resource></Resources>\
         <Assignments>{assignment}</Assignments></Project>"
    )
}

#[test]
fn progress_without_a_start_in_the_file_does_not_pin_the_task_to_1970() {
    let xml = one_task(
        "<Duration>PT16H0M0S</Duration><ActualDuration>PT8H0M0S</ActualDuration>\
         <RemainingDuration>PT8H0M0S</RemainingDuration>",
        "",
    );
    let project = read_mspdi(&xml).unwrap();
    let task = project.task(1).unwrap();
    assert!((task.percent_complete - 50.0).abs() < 1e-6);
    assert_eq!(task.start, at(0, 8, 0));
    assert_ne!(task.actual_start, Some(0));
}

#[test]
fn a_day_without_actual_work_is_not_a_split_when_planned_work_is_given() {
    let period = |kind: u32, day: u32, hours: u32| {
        format!(
            "<TimephasedData><Type>{kind}</Type><UID>1</UID>\
             <Start>2026-01-{:02}T08:00:00</Start><Finish>2026-01-{:02}T17:00:00</Finish>\
             <Unit>2</Unit><Value>PT{hours}H0M0S</Value></TimephasedData>",
            5 + day,
            5 + day
        )
    };
    let assignment = format!(
        "<Assignment><UID>1</UID><TaskUID>1</TaskUID><ResourceUID>1</ResourceUID>\
         <Units>1</Units><Work>PT24H0M0S</Work>{}{}{}{}</Assignment>",
        period(1, 0, 24),
        period(2, 0, 8),
        period(2, 1, 0),
        period(2, 2, 8),
    );
    let xml = one_task(
        "<Start>2026-01-05T08:00:00</Start><Duration>PT24H0M0S</Duration>\
         <ActualStart>2026-01-05T08:00:00</ActualStart><RemainingWork>PT8H0M0S</RemainingWork>",
        &assignment,
    );
    let project = read_mspdi(&xml).unwrap();
    let task = project.task(1).unwrap();
    assert!(task.splits.is_empty());
    assert_eq!(task.finish, at(2, 17, 0));
}

fn with_costs() -> Document {
    let mut doc = doc_with(&[2, 4]);
    doc.apply(Command::AddResource {
        name: "An".into(),
        kind: ResourceKind::Person,
    })
    .unwrap();
    doc.apply(Command::UpdateResource {
        uid: 1,
        name: "An".into(),
        kind: ResourceKind::Person,
        max_units: 1.0,
        calendar_id: None,
        standard_rate: 100.0,
        overtime_rate: 150.0,
        cost_per_use: 0.0,
    })
    .unwrap();
    doc.apply(Command::AssignResource {
        task_uid: 1,
        resource_uid: 1,
        units: 1.0,
    })
    .unwrap();
    doc.apply(Command::SetFixedCost {
        uid: 2,
        cost: 1000.0,
        accrual: sheafly_engine::model::Accrual::Start,
    })
    .unwrap();
    doc.apply(Command::SetPercentComplete {
        uid: 1,
        percent: 50.0,
    })
    .unwrap();
    doc
}

#[test]
fn computed_costs_are_written_and_the_flag_is_off() {
    let doc = with_costs();
    let (xml, back) = round_trip(&doc.project);
    assert!(xml.contains("<EditableActualCosts>0</EditableActualCosts>"));
    // Task 1: 16 h at 100 = 1,600; half done.
    assert_eq!(element(&xml, "Việc 1", "Cost"), Some("1600"));
    assert_eq!(element(&xml, "Việc 1", "ActualCost"), Some("800"));
    assert_eq!(element(&xml, "Việc 1", "RemainingCost"), Some("800"));
    for task in &doc.project.tasks {
        let after = back.task(task.uid).unwrap();
        assert_eq!(after.cost, task.cost, "{}: cost", task.name);
        assert_eq!(
            after.actual_cost, task.actual_cost,
            "{}: actual cost",
            task.name
        );
        assert_eq!(after.manual_actual_cost, None);
    }
    assert!(!back.editable_actual_costs);
}

#[test]
fn hand_entered_actual_costs_round_trip_and_feed_the_totals() {
    let mut doc = with_costs();
    doc.apply(Command::SetEditableActualCosts { editable: true })
        .unwrap();
    doc.apply(Command::SetTaskActualCost {
        uid: 1,
        cost: Some(1234.0),
    })
    .unwrap();
    doc.apply(Command::SetTaskActualCost {
        uid: 2,
        cost: Some(250.0),
    })
    .unwrap();
    assert_eq!(doc.project.task(1).unwrap().actual_cost, 1234.0);
    assert_eq!(doc.project.task(1).unwrap().remaining_cost, 366.0);

    let (xml, back) = round_trip(&doc.project);
    assert!(xml.contains("<EditableActualCosts>1</EditableActualCosts>"));
    assert!(back.editable_actual_costs);
    for task in &doc.project.tasks {
        let after = back.task(task.uid).unwrap();
        // A file does not say whether a task's cost was entered on the task or on its
        // assignments; with assignments the entry comes back on them (numbers equal).
        let assigned = back.assignments.iter().any(|a| a.task_uid == task.uid);
        if !assigned {
            assert_eq!(after.manual_actual_cost, task.manual_actual_cost, "{}", task.name);
        }
        assert_eq!(
            after.actual_cost, task.actual_cost,
            "{}: actual cost",
            task.name
        );
        assert_eq!(
            after.remaining_cost, task.remaining_cost,
            "{}: remaining",
            task.name
        );
    }
    // The assignment carries the labour part of the entered cost.
    assert_eq!(
        back.assignments[0].actual_cost,
        doc.project.assignments[0].actual_cost
    );

    // Turning the switch off brings the computed numbers back; the entries are kept.
    doc.apply(Command::SetEditableActualCosts { editable: false })
        .unwrap();
    assert_eq!(doc.project.task(1).unwrap().actual_cost, 800.0);
    assert_eq!(
        doc.project.task(1).unwrap().manual_actual_cost,
        Some(1234.0)
    );
}

#[test]
fn entered_costs_spread_over_the_progress_and_unstarted_ones_land_on_the_status_date() {
    use sheafly_engine::evm::evm_at;
    let mut doc = with_costs();
    doc.apply(Command::SetStatusDate {
        date: Some(at(3, 8, 0)),
    })
    .unwrap();
    doc.apply(Command::SetEditableActualCosts { editable: true })
        .unwrap();
    doc.apply(Command::SetTaskActualCost {
        uid: 2,
        cost: Some(500.0),
    })
    .unwrap();
    // Task 2 has no progress: nothing before the status date, all of it from then on.
    assert_eq!(evm_at(&doc.project, at(2, 8, 0)).ac, 800.0);
    assert_eq!(evm_at(&doc.project, at(3, 8, 0)).ac, 1300.0);
}

#[test]
fn a_file_that_computes_its_own_actual_costs_gives_no_entries() {
    let xml = one_task(
        "<Duration>PT8H0M0S</Duration><ActualCost>777</ActualCost>",
        "",
    );
    let project = read_mspdi(&xml).unwrap();
    assert!(!project.editable_actual_costs);
    assert_eq!(project.task(1).unwrap().manual_actual_cost, None);
    let editable = xml.replace(
        "<StartDate>",
        "<EditableActualCosts>1</EditableActualCosts><StartDate>",
    );
    let project = read_mspdi(&editable).unwrap();
    assert!(project.editable_actual_costs);
    assert_eq!(project.task(1).unwrap().manual_actual_cost, Some(777.0));
    assert_eq!(project.task(1).unwrap().actual_cost, 777.0);
}

#[test]
fn infinite_or_summary_actual_costs_in_a_file_are_not_kept() {
    let xml = one_task(
        "<Duration>PT8H0M0S</Duration><ActualCost>1e999</ActualCost>",
        "",
    )
    .replace(
        "<StartDate>",
        "<EditableActualCosts>1</EditableActualCosts><StartDate>",
    );
    let project = read_mspdi(&xml).unwrap();
    assert_eq!(project.task(1).unwrap().manual_actual_cost, None);
    assert!(project.task(1).unwrap().actual_cost.is_finite());

    let summary = "<Task><UID>2</UID><Name>S</Name><OutlineLevel>1</OutlineLevel><Summary>1</Summary><ActualCost>999</ActualCost></Task>\
                   <Task><UID>3</UID><Name>L</Name><OutlineLevel>2</OutlineLevel><Duration>PT8H0M0S</Duration></Task>";
    let xml = format!(
        "<Project xmlns=\"http://schemas.microsoft.com/project\"><EditableActualCosts>1</EditableActualCosts>\
         <StartDate>2026-01-05T08:00:00</StartDate><Tasks>{summary}</Tasks></Project>"
    );
    let project = read_mspdi(&xml).unwrap();
    assert_eq!(project.task(2).unwrap().manual_actual_cost, None);
}

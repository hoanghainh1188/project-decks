//! Phase 13: actual progress drives the schedule the way Microsoft Project does.
//! Dates are worked out by hand on the default calendar (Mon–Fri, 08–12 and 13–17).

use chrono::Datelike;

use projectdesk_engine::command::Command;
use projectdesk_engine::model::{ConstraintType, DepKind, Dependency, ResourceKind, Task};
use projectdesk_engine::schedule::schedule;
use projectdesk_engine::split::Segment;
use projectdesk_engine::timeutil::timestamp;
use projectdesk_engine::{Document, Project};

const DAY: i64 = 480;

/// 2026-01-05 is a Monday; `day` 0 is that Monday.
fn at(day: u32, hour: u32, minute: u32) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(i64::from(day)))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, minute)
}

/// Tasks named by their duration in days, linked finish-to-start.
fn doc_with(days: &[i64], links: &[(u32, u32)]) -> Document {
    let mut project = Project::new("Theo dõi", at(0, 8, 0));
    for (index, days) in days.iter().enumerate() {
        let mut task = Task::new(index as u32 + 1, format!("Việc {}", index + 1));
        task.duration = days * DAY;
        task.work = days * DAY;
        task.milestone = *days == 0;
        project.tasks.push(task);
    }
    project.next_task_uid = days.len() as u32 + 1;
    for (index, (pred, succ)) in links.iter().enumerate() {
        project.dependencies.push(Dependency {
            uid: index as u32 + 1,
            predecessor_uid: *pred,
            successor_uid: *succ,
            kind: DepKind::Fs,
            lag: 0,
        });
    }
    project.next_dependency_uid = links.len() as u32 + 1;
    schedule(&mut project).unwrap();
    Document::from_project(project)
}

fn task(doc: &Document, uid: u32) -> &Task {
    doc.project.task(uid).unwrap()
}

fn dates(doc: &Document, uid: u32) -> (i64, i64) {
    (task(doc, uid).start, task(doc, uid).finish)
}

fn actual_start(doc: &mut Document, uid: u32, start: i64) {
    doc.apply(Command::SetActualStart {
        uid,
        start: Some(start),
    })
    .unwrap();
}

#[test]
fn a_late_actual_start_pushes_the_successor() {
    let mut doc = doc_with(&[2, 1], &[(1, 2)]);
    actual_start(&mut doc, 1, at(1, 8, 0));
    assert_eq!(dates(&doc, 1), (at(1, 8, 0), at(2, 17, 0)));
    assert_eq!(dates(&doc, 2), (at(3, 8, 0), at(3, 17, 0)));
}

#[test]
fn an_actual_start_ignores_links_and_constraints() {
    let mut doc = doc_with(&[2, 1], &[(1, 2)]);
    actual_start(&mut doc, 2, at(0, 13, 0));
    assert_eq!(dates(&doc, 2), (at(0, 13, 0), at(1, 12, 0)));

    doc.apply(Command::SetConstraint {
        uid: 1,
        constraint_type: ConstraintType::Snet,
        date: Some(at(7, 8, 0)),
    })
    .unwrap();
    actual_start(&mut doc, 1, at(0, 8, 0));
    assert_eq!(dates(&doc, 1), (at(0, 8, 0), at(1, 17, 0)));
}

#[test]
fn the_remaining_work_runs_on_from_the_work_done() {
    let mut doc = doc_with(&[4], &[]);
    actual_start(&mut doc, 1, at(0, 8, 0));
    doc.apply(Command::SetPercentComplete {
        uid: 1,
        percent: 50.0,
    })
    .unwrap();
    assert_eq!(dates(&doc, 1), (at(0, 8, 0), at(3, 17, 0)));

    // Two days done, three left: five days from the actual start, 40 % complete.
    doc.apply(Command::SetRemainingDuration {
        uid: 1,
        minutes: 3 * DAY,
    })
    .unwrap();
    let updated = task(&doc, 1);
    assert_eq!(updated.duration, 5 * DAY);
    assert!((updated.percent_complete - 40.0).abs() < 1e-9);
    assert_eq!(updated.actual_start, Some(at(0, 8, 0)), "progress is kept");
    assert_eq!(dates(&doc, 1), (at(0, 8, 0), at(4, 17, 0)));
}

#[test]
fn no_remaining_duration_completes_on_the_date_the_work_runs_out() {
    let mut doc = doc_with(&[4], &[]);
    actual_start(&mut doc, 1, at(0, 8, 0));
    doc.apply(Command::SetPercentComplete {
        uid: 1,
        percent: 25.0,
    })
    .unwrap();
    doc.apply(Command::SetRemainingDuration { uid: 1, minutes: 0 })
        .unwrap();
    let done = task(&doc, 1);
    assert_eq!(done.duration, DAY);
    assert_eq!(done.percent_complete, 100.0);
    assert_eq!(done.actual_finish, Some(at(0, 17, 0)));
}

#[test]
fn before_the_start_remaining_duration_is_the_duration() {
    let mut doc = doc_with(&[4], &[]);
    doc.apply(Command::SetRemainingDuration {
        uid: 1,
        minutes: 2 * DAY,
    })
    .unwrap();
    assert_eq!(task(&doc, 1).duration, 2 * DAY);
    assert_eq!(task(&doc, 1).actual_start, None);
}

#[test]
fn remaining_work_keeps_the_work_done() {
    let mut doc = doc_with(&[2], &[]);
    actual_start(&mut doc, 1, at(0, 8, 0));
    doc.apply(Command::SetPercentComplete {
        uid: 1,
        percent: 50.0,
    })
    .unwrap();
    doc.apply(Command::SetRemainingWork {
        uid: 1,
        minutes: 120,
    })
    .unwrap();
    let updated = task(&doc, 1);
    assert_eq!(updated.work, 600);
    assert!((updated.percent_complete - 80.0).abs() < 1e-9);
    assert_eq!(updated.actual_start, Some(at(0, 8, 0)));
}

#[test]
fn an_actual_finish_sets_the_duration_and_drives_the_successor() {
    let mut doc = doc_with(&[2, 1], &[(1, 2)]);
    actual_start(&mut doc, 1, at(0, 8, 0));
    doc.apply(Command::SetActualFinish {
        uid: 1,
        finish: Some(at(0, 17, 0)),
    })
    .unwrap();
    let done = task(&doc, 1);
    assert_eq!(done.duration, DAY);
    assert_eq!(done.percent_complete, 100.0);
    assert_eq!(dates(&doc, 1), (at(0, 8, 0), at(0, 17, 0)));
    assert_eq!(dates(&doc, 2), (at(1, 8, 0), at(1, 17, 0)));
}

#[test]
fn finished_work_is_never_critical() {
    let mut doc = doc_with(&[1, 1, 1], &[(1, 2), (2, 3)]);
    assert!(doc.project.tasks.iter().all(|task| task.critical));
    doc.apply(Command::SetPercentComplete {
        uid: 1,
        percent: 100.0,
    })
    .unwrap();
    assert!(!task(&doc, 1).critical);
    assert!(task(&doc, 2).critical && task(&doc, 3).critical);

    // A group whose tasks are all done is not critical either.
    let mut doc = doc_with(&[0, 1, 1], &[(2, 3)]);
    doc.apply(Command::Indent { uid: 2 }).unwrap();
    doc.apply(Command::Indent { uid: 3 }).unwrap();
    assert!(task(&doc, 1).summary && task(&doc, 1).critical);
    doc.apply(Command::SetPercentComplete {
        uid: 2,
        percent: 100.0,
    })
    .unwrap();
    doc.apply(Command::SetPercentComplete {
        uid: 3,
        percent: 100.0,
    })
    .unwrap();
    assert!(!task(&doc, 1).critical);
}

#[test]
fn a_started_task_gets_slack_from_its_finish() {
    // B (1 day) has two days of slack beside A (3 days); starting B keeps that slack.
    let mut doc = doc_with(&[3, 1, 0], &[(1, 3), (2, 3)]);
    assert_eq!(task(&doc, 2).total_slack, 2 * DAY);
    actual_start(&mut doc, 2, at(0, 8, 0));
    assert_eq!(task(&doc, 2).total_slack, 2 * DAY);
    assert!(!task(&doc, 2).critical);
}

#[test]
fn actual_dates_beat_manual_dates() {
    let mut doc = doc_with(&[2], &[]);
    doc.apply(Command::SetManual {
        uid: 1,
        manual: true,
        start: Some(at(7, 8, 0)),
        finish: Some(at(8, 17, 0)),
    })
    .unwrap();
    assert_eq!(dates(&doc, 1), (at(7, 8, 0), at(8, 17, 0)));
    actual_start(&mut doc, 1, at(0, 8, 0));
    assert_eq!(dates(&doc, 1), (at(0, 8, 0), at(1, 17, 0)));
}

#[test]
fn a_started_split_task_keeps_its_gap() {
    let mut doc = doc_with(&[2], &[]);
    doc.apply(Command::SplitTask {
        uid: 1,
        at: at(1, 8, 0),
        gap: DAY,
    })
    .unwrap();
    actual_start(&mut doc, 1, at(1, 8, 0));
    let started = task(&doc, 1);
    assert_eq!(started.finish, at(3, 17, 0));
    assert_eq!(
        started.segments,
        vec![
            Segment {
                start: at(1, 8, 0),
                finish: at(1, 17, 0)
            },
            Segment {
                start: at(3, 8, 0),
                finish: at(3, 17, 0)
            },
        ]
    );
}

#[test]
fn leveling_moves_the_task_that_has_not_started() {
    let mut doc = doc_with(&[1, 1], &[]);
    doc.apply(Command::AddResource {
        name: "An".into(),
        kind: ResourceKind::Person,
    })
    .unwrap();
    for uid in [1, 2] {
        doc.apply(Command::AssignResource {
            task_uid: uid,
            resource_uid: 1,
            units: 1.0,
        })
        .unwrap();
    }
    // Task 1 would normally move first (lower priority), but it has started.
    doc.apply(Command::SetPriority {
        uid: 1,
        priority: 100,
    })
    .unwrap();
    actual_start(&mut doc, 1, at(0, 8, 0));
    doc.apply(Command::LevelResources).unwrap();
    assert_eq!(dates(&doc, 1), (at(0, 8, 0), at(0, 17, 0)));
    assert_eq!(dates(&doc, 2), (at(1, 8, 0), at(1, 17, 0)));
    assert_eq!(task(&doc, 1).leveling_delay, 0);
}

#[test]
fn late_means_behind_where_the_schedule_says_the_task_should_be() {
    let mut doc = doc_with(&[2], &[]);
    actual_start(&mut doc, 1, at(0, 8, 0));
    doc.apply(Command::SetPercentComplete {
        uid: 1,
        percent: 50.0,
    })
    .unwrap();
    doc.apply(Command::SetStatusDate {
        date: Some(at(1, 8, 0)),
    })
    .unwrap();
    assert!(!task(&doc, 1).late, "one day done by the end of day one");
    doc.apply(Command::SetStatusDate {
        date: Some(at(1, 13, 0)),
    })
    .unwrap();
    assert!(task(&doc, 1).late, "half of day two is due too");
    doc.apply(Command::SetStatusDate {
        date: Some(at(0, 8, 0)),
    })
    .unwrap();
    assert!(!task(&doc, 1).late);
}

#[test]
fn an_actual_finish_before_the_actual_start_is_refused() {
    let mut doc = doc_with(&[2], &[]);
    actual_start(&mut doc, 1, at(1, 8, 0));
    let before = task(&doc, 1).clone();
    let error = doc
        .apply(Command::SetActualFinish {
            uid: 1,
            finish: Some(at(0, 17, 0)),
        })
        .unwrap_err();
    assert!(
        error.to_string().contains("phải sau ngày bắt đầu thực tế"),
        "{error}"
    );
    assert_eq!(task(&doc, 1), &before);
}

#[test]
fn moving_the_start_of_a_finished_task_keeps_its_duration() {
    let mut doc = doc_with(&[2, 1], &[(1, 2)]);
    actual_start(&mut doc, 1, at(0, 8, 0));
    doc.apply(Command::SetActualFinish {
        uid: 1,
        finish: Some(at(1, 17, 0)),
    })
    .unwrap();
    actual_start(&mut doc, 1, at(1, 8, 0));
    assert_eq!(task(&doc, 1).duration, 2 * DAY);
    assert_eq!(task(&doc, 1).actual_finish, Some(at(2, 17, 0)));
    assert_eq!(dates(&doc, 1), (at(1, 8, 0), at(2, 17, 0)));
    assert_eq!(dates(&doc, 2), (at(3, 8, 0), at(3, 17, 0)));
}

#[test]
fn a_summary_takes_actual_dates_and_lateness_from_its_tasks() {
    let mut doc = doc_with(&[0, 1, 1], &[(2, 3)]);
    doc.apply(Command::Indent { uid: 2 }).unwrap();
    doc.apply(Command::Indent { uid: 3 }).unwrap();
    actual_start(&mut doc, 1, at(5, 8, 0));
    assert_eq!(
        task(&doc, 1).actual_start,
        None,
        "actual dates belong to tasks"
    );

    doc.apply(Command::SetStatusDate {
        date: Some(at(3, 8, 0)),
    })
    .unwrap();
    assert!(task(&doc, 1).late, "its tasks are behind");
    doc.apply(Command::SetPercentComplete {
        uid: 2,
        percent: 100.0,
    })
    .unwrap();
    doc.apply(Command::SetPercentComplete {
        uid: 3,
        percent: 100.0,
    })
    .unwrap();
    assert!(!task(&doc, 1).late, "every task in it is done");
}

#[test]
fn imported_progress_without_actual_dates_starts_where_it_was_scheduled() {
    use projectdesk_engine::mspdi::{read_mspdi, write_mspdi};

    let mut doc = doc_with(&[2, 1], &[(1, 2)]);
    actual_start(&mut doc, 1, at(0, 8, 0));
    doc.apply(Command::SetPercentComplete {
        uid: 1,
        percent: 50.0,
    })
    .unwrap();
    doc.apply(Command::SetPercentComplete {
        uid: 2,
        percent: 100.0,
    })
    .unwrap();
    let xml = write_mspdi(&doc.project).unwrap();
    let stripped: String = xml
        .lines()
        .filter(|line| !line.contains("<ActualStart>") && !line.contains("<ActualFinish>"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(stripped, xml, "the export carried actual dates to strip");

    let imported = read_mspdi(&stripped).unwrap();
    let first = imported
        .tasks
        .iter()
        .find(|task| task.name == "Việc 1")
        .unwrap();
    let second = imported
        .tasks
        .iter()
        .find(|task| task.name == "Việc 2")
        .unwrap();
    assert_eq!(first.actual_start, Some(at(0, 8, 0)));
    assert_eq!(first.actual_finish, None);
    assert_eq!(second.actual_finish, Some(at(2, 17, 0)));
}

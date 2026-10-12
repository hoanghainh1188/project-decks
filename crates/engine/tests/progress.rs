//! Phase 13, part 2: rescheduling unfinished work, actual cost on split tasks, and
//! progress on recurring occurrences. Dates are worked out by hand on the default
//! calendar (Mon–Fri, 08–12 and 13–17).

use chrono::Datelike;

use projectdesk_engine::command::Command;
use projectdesk_engine::document::replay;
use projectdesk_engine::evm::actual_cost_at;
use projectdesk_engine::model::{Accrual, ConstraintType, DepKind, Dependency, Task};
use projectdesk_engine::recurrence::{RecurrenceEnd, RecurrencePattern, RecurrenceSpec};
use projectdesk_engine::schedule::schedule;
use projectdesk_engine::split::{Segment, Split};
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

fn doc_with(days: &[i64], links: &[(u32, u32)]) -> Document {
    let mut project = Project::new("Tiến độ", at(0, 8, 0));
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

fn status(doc: &mut Document, date: i64) {
    doc.apply(Command::SetStatusDate { date: Some(date) })
        .unwrap();
}

fn json(project: &Project) -> serde_json::Value {
    serde_json::to_value(project).unwrap()
}

#[test]
fn unfinished_work_of_a_started_task_resumes_at_the_status_date() {
    // Four days from Monday, one day done; the status date is Wednesday morning.
    let mut doc = doc_with(&[4, 1], &[(1, 2)]);
    doc.apply(Command::SetActualStart {
        uid: 1,
        start: Some(at(0, 8, 0)),
    })
    .unwrap();
    doc.apply(Command::SetPercentComplete {
        uid: 1,
        percent: 25.0,
    })
    .unwrap();
    status(&mut doc, at(2, 8, 0));
    assert!(task(&doc, 1).late);

    doc.apply(Command::RescheduleUncompleted).unwrap();

    let moved = task(&doc, 1);
    assert_eq!(
        moved.splits,
        vec![Split {
            offset: DAY,
            gap: DAY
        }]
    );
    assert_eq!(
        moved.segments,
        vec![
            Segment {
                start: at(0, 8, 0),
                finish: at(0, 17, 0)
            },
            Segment {
                start: at(2, 8, 0),
                finish: at(4, 17, 0)
            },
        ]
    );
    assert!(!moved.late, "nothing more is due by the status date");
    assert_eq!(task(&doc, 2).start, at(7, 8, 0));
    assert_eq!(
        json(&doc.project),
        json(&replay(&doc.base, &doc.commands).unwrap()),
        "replaying gives the same split"
    );
    doc.undo().unwrap();
    assert!(task(&doc, 1).splits.is_empty());
}

#[test]
fn a_task_that_should_have_started_waits_for_the_status_date() {
    let mut doc = doc_with(&[1, 1], &[]);
    status(&mut doc, at(2, 13, 0));
    doc.apply(Command::RescheduleUncompleted).unwrap();
    for uid in [1, 2] {
        let waiting = task(&doc, uid);
        assert_eq!(waiting.constraint_type, ConstraintType::Snet);
        assert_eq!(waiting.constraint_date, Some(at(2, 13, 0)));
        assert_eq!(waiting.start, at(2, 13, 0));
    }
}

#[test]
fn rescheduling_leaves_finished_future_manual_and_pinned_work_alone() {
    let mut doc = doc_with(&[1, 1, 1, 1, 1], &[]);
    doc.apply(Command::SetPercentComplete {
        uid: 1,
        percent: 100.0,
    })
    .unwrap();
    doc.apply(Command::SetConstraint {
        uid: 2,
        constraint_type: ConstraintType::Snet,
        date: Some(at(9, 8, 0)),
    })
    .unwrap();
    doc.apply(Command::SetManual {
        uid: 3,
        manual: true,
        start: None,
        finish: None,
    })
    .unwrap();
    doc.apply(Command::SetConstraint {
        uid: 4,
        constraint_type: ConstraintType::Mso,
        date: Some(at(0, 8, 0)),
    })
    .unwrap();
    status(&mut doc, at(2, 8, 0));
    let before = doc.project.clone();

    doc.apply(Command::RescheduleUncompleted).unwrap();

    for uid in [1, 2, 3, 4] {
        assert_eq!(task(&doc, uid), before.task(uid).unwrap(), "task {uid}");
    }
    assert_eq!(task(&doc, 5).constraint_date, Some(at(2, 8, 0)));
}

#[test]
fn rescheduling_needs_a_status_date() {
    let mut doc = doc_with(&[1], &[]);
    let error = doc.apply(Command::RescheduleUncompleted).unwrap_err();
    assert!(error.to_string().contains("ngày trạng thái"), "{error}");
}

#[test]
fn actual_cost_of_a_split_task_is_spent_only_in_its_segments() {
    // Two days split after the first by a one-day gap; 75 % done means Monday and
    // Wednesday morning. 720 of fixed cost, prorated, so 540 is actual.
    let mut doc = doc_with(&[2], &[]);
    doc.apply(Command::SplitTask {
        uid: 1,
        at: at(1, 8, 0),
        gap: DAY,
    })
    .unwrap();
    doc.apply(Command::SetFixedCost {
        uid: 1,
        cost: 720.0,
        accrual: Accrual::Prorated,
    })
    .unwrap();
    doc.apply(Command::SetActualStart {
        uid: 1,
        start: Some(at(0, 8, 0)),
    })
    .unwrap();
    doc.apply(Command::SetPercentComplete {
        uid: 1,
        percent: 75.0,
    })
    .unwrap();
    let done = task(&doc, 1);
    assert!((done.actual_cost - 540.0).abs() < 1e-6);

    let cost_at = |instant| actual_cost_at(&doc.project, task(&doc, 1), instant);
    assert!(
        (cost_at(at(0, 17, 0)) - 360.0).abs() < 1e-6,
        "{}",
        cost_at(at(0, 17, 0))
    );
    assert!(
        (cost_at(at(1, 17, 0)) - 360.0).abs() < 1e-6,
        "nothing in the gap"
    );
    assert!((cost_at(at(2, 10, 0)) - 450.0).abs() < 1e-6);
    assert!((cost_at(at(2, 12, 0)) - 540.0).abs() < 1e-6);
}

fn weekly_meeting(background: bool) -> RecurrenceSpec {
    RecurrenceSpec {
        name: "Họp tuần".into(),
        duration: 60,
        start: at(0, 8, 0),
        pattern: RecurrencePattern::Weekly {
            every: 1,
            weekdays: vec![0],
        },
        end: RecurrenceEnd::Count { count: 6 },
        calendar_id: None,
        resource_uids: Vec::new(),
        resources: Vec::new(),
        background,
    }
}

/// One five-day task, then a weekly meeting series that runs past it.
fn with_meetings(background: bool) -> (Document, u32) {
    let mut doc = doc_with(&[5], &[]);
    doc.apply(Command::CreateRecurringTask {
        after_uid: Some(1),
        spec: weekly_meeting(background),
    })
    .unwrap();
    let summary = doc
        .project
        .tasks
        .iter()
        .find(|task| task.recurrence.is_some())
        .unwrap()
        .uid;
    (doc, summary)
}

fn occurrences(doc: &Document, summary: u32) -> Vec<&Task> {
    doc.project
        .tasks
        .iter()
        .filter(|task| task.series_uid == Some(summary))
        .collect()
}

#[test]
fn progress_on_an_occurrence_keeps_it_in_its_series() {
    let (mut doc, summary) = with_meetings(false);
    let first = occurrences(&doc, summary)[0].uid;
    doc.apply(Command::SetPercentComplete {
        uid: first,
        percent: 100.0,
    })
    .unwrap();
    assert!(!task(&doc, first).detached);

    // Regenerating with a longer meeting keeps the one that took place as it was.
    let mut longer = weekly_meeting(false);
    longer.duration = 90;
    doc.apply(Command::UpdateRecurringTask {
        uid: summary,
        spec: longer,
    })
    .unwrap();
    let meetings = occurrences(&doc, summary);
    assert_eq!(meetings.len(), 6);
    let held = task(&doc, first);
    assert_eq!((held.duration, held.percent_complete), (60, 100.0));
    assert!(meetings
        .iter()
        .filter(|task| task.uid != first)
        .all(|task| task.duration == 90));
}

#[test]
fn a_background_series_does_not_set_the_critical_path() {
    let (doc, summary) = with_meetings(false);
    assert!(!task(&doc, 1).critical, "the meetings outlast the task");
    assert!(occurrences(&doc, summary).last().unwrap().critical);

    let (doc, summary) = with_meetings(true);
    assert!(task(&doc, 1).critical, "the task sets the finish again");
    assert!(occurrences(&doc, summary).iter().all(|task| !task.critical));
    assert!(!task(&doc, summary).critical);
}

#[test]
fn a_background_series_inside_a_phase_leaves_the_phase_off_the_critical_path() {
    // Task 1 (ten days) sets the finish; phase 2 holds a one-day task and the meetings.
    let mut doc = doc_with(&[10, 0, 1], &[]);
    doc.apply(Command::Indent { uid: 3 }).unwrap();
    doc.apply(Command::CreateRecurringTask {
        after_uid: Some(3),
        spec: weekly_meeting(true),
    })
    .unwrap();
    let summary = doc
        .project
        .tasks
        .iter()
        .find(|task| task.recurrence.is_some())
        .unwrap()
        .uid;
    assert_eq!(
        task(&doc, summary).parent_uid,
        Some(2),
        "the series sits in the phase"
    );
    assert!(task(&doc, 1).critical);
    assert!(!task(&doc, 2).critical, "{:?}", task(&doc, 2).total_slack);
    assert!(!task(&doc, 3).critical);
}

#[test]
fn changing_the_length_of_an_unstarted_occurrence_still_detaches_it() {
    let (mut doc, summary) = with_meetings(false);
    let first = occurrences(&doc, summary)[0].uid;
    doc.apply(Command::SetRemainingDuration {
        uid: first,
        minutes: 30,
    })
    .unwrap();
    assert!(task(&doc, first).detached);
}

#[test]
fn deleting_a_series_keeps_the_meetings_that_took_place() {
    let (mut doc, summary) = with_meetings(false);
    let held = occurrences(&doc, summary)[0].uid;
    doc.apply(Command::SetPercentComplete {
        uid: held,
        percent: 100.0,
    })
    .unwrap();
    doc.apply(Command::DeleteTask { uid: summary }).unwrap();
    let kept = task(&doc, held);
    assert_eq!((kept.series_uid, kept.percent_complete), (None, 100.0));
    assert_eq!(
        doc.project.tasks.len(),
        2,
        "the task and the meeting that took place"
    );
}

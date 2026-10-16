//! Project statistics and choosing the project calendar.

use sheafly_engine::calendar::TimeRange;
use sheafly_engine::command::Command;
use sheafly_engine::statistics::statistics;
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::Document;

const DAY: i64 = 480;

fn two_tasks() -> Document {
    let mut doc = Document::new_default();
    // A fixed Monday: the default project starts today, and a start late in the week
    // sends the tasks over a weekend.
    doc.apply(Command::SetProjectStart {
        start: timestamp(2026, 1, 5, 8, 0),
    })
    .unwrap();
    doc.apply(Command::SetDuration {
        uid: 1,
        minutes: 2 * DAY,
    })
    .unwrap();
    doc.apply(Command::AddTask {
        name: "B".into(),
        after_uid: Some(1),
    })
    .unwrap();
    doc.apply(Command::SetFixedCost {
        uid: 2,
        cost: 300.0,
        accrual: sheafly_engine::model::Accrual::Prorated,
    })
    .unwrap();
    doc
}

#[test]
fn statistics_sum_the_project_and_compare_with_the_baseline() {
    let mut doc = two_tasks();
    let before = statistics(&doc.project);
    assert_eq!(before.start, doc.project.start);
    assert_eq!(
        before.duration,
        2 * DAY,
        "both tasks start with the project"
    );
    assert_eq!(before.work, 3 * DAY);
    assert_eq!(before.cost, 300.0);
    assert_eq!(before.baseline, None);
    doc.apply(Command::SetBaseline { number: 0 }).unwrap();
    doc.apply(Command::SetPercentComplete {
        uid: 2,
        percent: 50.0,
    })
    .unwrap();
    doc.apply(Command::SetDuration {
        uid: 1,
        minutes: 4 * DAY,
    })
    .unwrap();
    let after = statistics(&doc.project);
    assert_eq!(after.duration, 4 * DAY);
    assert_eq!(after.actual_cost, 150.0);
    assert_eq!(after.remaining_cost, 150.0);
    assert!((after.percent_complete - 240.0 / (5.0 * 480.0) * 100.0).abs() < 1e-9);
    let baseline = after.baseline.clone().unwrap();
    assert_eq!(
        (baseline.duration, baseline.work, baseline.cost),
        (2 * DAY, 3 * DAY, 300.0)
    );
    // The metrics the app reads carry the same numbers.
    assert_eq!(doc.metrics().statistics, after);
}

#[test]
fn the_project_calendar_can_be_changed_and_must_have_working_time() {
    let mut doc = two_tasks();
    doc.apply(Command::CreateCalendar {
        name: "Ca dài".into(),
        base_id: None,
    })
    .unwrap();
    let long = vec![TimeRange::new(6 * 60, 18 * 60)];
    let week = vec![
        long.clone(),
        long.clone(),
        long.clone(),
        long.clone(),
        long,
        vec![],
        vec![],
    ];
    doc.apply(Command::SetCalendarWeek {
        calendar_id: 2,
        week,
    })
    .unwrap();
    doc.apply(Command::SetProjectCalendar { calendar_id: 2 })
        .unwrap();
    assert_eq!(doc.project.calendar_id, 2);
    // 960 minutes now fit in a 12-hour day and a third.
    let task = doc.project.task(1).unwrap();
    assert_eq!(task.finish - task.start, (24 + 4) * 3_600_000);
    assert!(doc
        .apply(Command::SetProjectCalendar { calendar_id: 9 })
        .is_err());
    doc.undo().unwrap();
    assert_eq!(doc.project.calendar_id, 1);
}

#[test]
fn a_summary_fixed_cost_counts_on_both_sides_of_the_baseline() {
    let mut doc = two_tasks();
    doc.apply(Command::AddTask {
        name: "Con".into(),
        after_uid: Some(2),
    })
    .unwrap();
    doc.apply(Command::Indent { uid: 3 }).unwrap();
    doc.apply(Command::SetFixedCost {
        uid: 2,
        cost: 700.0,
        accrual: sheafly_engine::model::Accrual::End,
    })
    .unwrap();
    doc.apply(Command::SetBaseline { number: 0 }).unwrap();
    let stats = statistics(&doc.project);
    let baseline = stats.baseline.unwrap();
    assert_eq!(
        baseline.cost, stats.cost,
        "nothing changed since the baseline"
    );
    assert_eq!(baseline.work, stats.work);
}

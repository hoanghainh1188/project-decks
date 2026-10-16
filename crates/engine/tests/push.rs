//! Pushing dates back to an external task tracker (phase 19, pha 2): `push::plan` lists
//! what a push would write, `RecordPush` remembers the days that went through, and a pull
//! that finds one of those days at the source reads nothing from it.

use chrono::Datelike;

use sheafly_engine::command::Command;
use sheafly_engine::model::{ConstraintType, ResourceKind, Task};
use sheafly_engine::pull::{diff, PullField, PullKind, PullTask};
use sheafly_engine::push::{plan, PushOptions, PushSkipReason, PushedEntry};
use sheafly_engine::store::{load, save};
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{Document, EngineError, Invalid, Project};

const HOUR: i64 = 60;

fn at(day: u32, hour: u32) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(i64::from(day)))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, 0)
}

/// An (an@example.com) and Bình (binh@example.com).
fn doc() -> Document {
    let mut doc = Document::from_project(Project::new("Đẩy", at(0, 8)));
    for (name, email) in [("An", "an@example.com"), ("Bình", "binh@example.com")] {
        doc.apply(Command::AddResource { name: name.into(), kind: ResourceKind::Person }).unwrap();
        let uid = doc.project.resources.last().unwrap().uid;
        doc.apply(Command::SetResourceEmail { uid, email: email.into() }).unwrap();
    }
    doc
}

fn issue(key: &str, name: &str) -> PullTask {
    PullTask { key: key.into(), name: name.into(), ..PullTask::default() }
}

/// P-1 is two days of work (Mon 5 and Tue 6 January) for An; P-2 is one hour for Bình.
fn pulled() -> (Document, Vec<PullTask>) {
    let mut doc = doc();
    let rows = vec![
        PullTask { work: Some(16 * HOUR), assignee_email: Some("an@example.com".into()), ..issue("P-1", "Hai ngày") },
        PullTask { work: Some(HOUR), assignee_email: Some("binh@example.com".into()), ..issue("P-2", "Một giờ") },
    ];
    doc.apply(Command::ApplyPull { rows: rows.clone(), pulled_at: 1, resolutions: None }).unwrap();
    (doc, rows)
}

fn by_key<'a>(project: &'a Project, key: &str) -> &'a Task {
    project.tasks.iter().find(|task| task.external.as_ref().is_some_and(|link| link.key == key)).unwrap()
}

fn dates() -> PushOptions {
    PushOptions { start: true, due: true, ..PushOptions::default() }
}

#[test]
fn empty_fields_at_the_source_are_filled_with_the_computed_days() {
    let (doc, rows) = pulled();
    let result = plan(&doc.project, &rows, dates());
    assert_eq!(result.changes.len(), 2);
    let first = &result.changes[0];
    assert_eq!((first.key.as_str(), first.name.as_str()), ("P-1", "Hai ngày"));
    let start = first.start.as_ref().unwrap();
    let due = first.due.as_ref().unwrap();
    assert_eq!((start.from.as_deref(), start.to.as_str()), (None, "2026-01-05"));
    assert_eq!((due.from.as_deref(), due.to.as_str()), (None, "2026-01-06"));
    assert!(first.assignee.is_none(), "the assignee is not pushed unless asked");
    assert_eq!(result.changes[1].due.as_ref().unwrap().to, "2026-01-05");
}

#[test]
fn nothing_is_pushed_when_no_field_is_chosen() {
    let (doc, rows) = pulled();
    assert!(plan(&doc.project, &rows, PushOptions::default()).is_empty());
    let only_start = plan(&doc.project, &rows, PushOptions { start: true, ..PushOptions::default() });
    assert!(only_start.changes.iter().all(|change| change.due.is_none() && change.start.is_some()));
}

#[test]
fn a_value_at_the_source_is_replaced_only_with_overwrite_and_equal_days_are_left() {
    let (doc, mut rows) = pulled();
    // P-1: another start, the same due day at another hour. P-2: both set elsewhere.
    rows[0].start = Some(at(3, 8));
    rows[0].due = Some(at(1, 9));
    rows[1].start = Some(at(0, 17));
    rows[1].due = Some(at(4, 17));

    let keep = plan(&doc.project, &rows, dates());
    assert!(keep.is_empty(), "without overwrite nothing set at the source is touched");

    let over = plan(&doc.project, &rows, PushOptions { overwrite: true, ..dates() });
    assert_eq!(over.changes.len(), 2);
    let first = &over.changes[0];
    assert_eq!(first.start.as_ref().unwrap().from.as_deref(), Some("2026-01-08"));
    assert_eq!(first.start.as_ref().unwrap().to, "2026-01-05");
    assert!(first.due.is_none(), "the same day at another hour is the same date");
    let second = &over.changes[1];
    assert!(second.start.is_none(), "2026-01-05 either way");
    assert_eq!(second.due.as_ref().unwrap().to, "2026-01-05");
}

#[test]
fn the_assignee_is_pushed_for_a_task_with_one_person_with_an_email() {
    let (doc, mut rows) = pulled();
    let options = PushOptions { assignee: true, ..PushOptions::default() };
    // At the source nobody is on P-1 and someone else is on P-2.
    rows[0].assignee_email = None;
    rows[1].assignee_email = Some("khac@example.com".into());
    let result = plan(&doc.project, &rows, options);
    assert_eq!(result.changes.len(), 1, "P-2 already has an assignee and overwrite is off");
    let change = result.changes[0].assignee.as_ref().unwrap();
    assert_eq!((change.from.as_deref(), change.to.as_str()), (None, "an@example.com"));

    let over = plan(&doc.project, &rows, PushOptions { overwrite: true, ..options });
    assert_eq!(over.changes.len(), 2);
    assert_eq!(over.changes[1].assignee.as_ref().unwrap().from.as_deref(), Some("khac@example.com"));
}

#[test]
fn the_same_person_at_the_source_is_left_whatever_the_letter_case() {
    let (doc, mut rows) = pulled();
    rows[0].assignee_email = Some("AN@Example.com".into());
    let result = plan(&doc.project, &rows, PushOptions { assignee: true, overwrite: true, ..PushOptions::default() });
    assert!(result.changes.iter().all(|change| change.key != "P-1"));
}

#[test]
fn tasks_with_many_people_or_a_person_without_email_are_skipped_with_a_reason() {
    let (mut doc, rows) = pulled();
    let first = by_key(&doc.project, "P-1").uid;
    let binh = doc.project.resources[1].uid;
    doc.apply(Command::AssignResource { task_uid: first, resource_uid: binh, units: 1.0 }).unwrap();
    let second = by_key(&doc.project, "P-2").uid;
    doc.apply(Command::SetResourceEmail { uid: binh, email: String::new() }).unwrap();
    doc.apply(Command::AssignResource { task_uid: second, resource_uid: doc.project.resources[0].uid, units: 1.0 }).unwrap();
    let options = PushOptions { assignee: true, ..PushOptions::default() };
    let result = plan(&doc.project, &rows.iter().cloned().map(|mut row| { row.assignee_email = None; row }).collect::<Vec<_>>(), options);
    let reasons: Vec<(&str, PushSkipReason)> = result.skipped.iter().map(|skip| (skip.key.as_str(), skip.reason)).collect();
    assert_eq!(reasons, vec![("P-1", PushSkipReason::ManyAssignees), ("P-2", PushSkipReason::ManyAssignees)]);

    // One person with no email.
    let (mut doc, rows) = pulled();
    let binh = doc.project.resources[1].uid;
    doc.apply(Command::SetResourceEmail { uid: binh, email: String::new() }).unwrap();
    let rows: Vec<PullTask> = rows.into_iter().map(|mut row| { row.assignee_email = None; row }).collect();
    let result = plan(&doc.project, &rows, options);
    assert_eq!(result.skipped.len(), 1);
    assert_eq!((result.skipped[0].key.as_str(), result.skipped[0].reason), ("P-2", PushSkipReason::NoEmail));
}

#[test]
fn linked_tasks_the_source_did_not_return_are_missing_and_gone_ones_are_ignored() {
    let (mut doc, rows) = pulled();
    let short = vec![rows[0].clone()];
    let result = plan(&doc.project, &short, dates());
    assert_eq!(result.missing.len(), 1);
    assert_eq!(result.missing[0].key, "P-2");
    assert_eq!(result.changes.len(), 1);

    // A pull without P-2 marks it gone; after that a push does not even list it.
    doc.apply(Command::ApplyPull { rows: short.clone(), pulled_at: 2, resolutions: None }).unwrap();
    let result = plan(&doc.project, &short, dates());
    assert!(result.missing.is_empty());
}

#[test]
fn record_push_remembers_the_days_in_one_undo_step() {
    let (mut doc, _) = pulled();
    let before = doc.project.clone();
    doc.apply(Command::RecordPush {
        entries: vec![PushedEntry { key: "P-1".into(), start: Some("2026-01-05".into()), due: Some("2026-01-06".into()), assignee: None }],
    })
    .unwrap();
    let link = by_key(&doc.project, "P-1").external.clone().unwrap();
    assert_eq!(link.pushed_start.as_deref(), Some("2026-01-05"));
    assert_eq!(link.pushed_due.as_deref(), Some("2026-01-06"));
    assert!(by_key(&doc.project, "P-2").external.as_ref().unwrap().pushed_start.is_none());
    doc.undo().unwrap();
    assert_eq!(doc.project, before);
}

#[test]
fn record_push_refuses_bad_days_and_entries_that_match_nothing() {
    let (mut doc, _) = pulled();
    let bad = doc.apply(Command::RecordPush {
        entries: vec![PushedEntry { key: "P-1".into(), start: Some("05/01/2026".into()), due: None, assignee: None }],
    });
    assert_eq!(bad.unwrap_err(), EngineError::Invalid(Invalid::PushDate));
    for entries in [
        vec![],
        vec![PushedEntry { key: "NOPE-1".into(), start: Some("2026-01-05".into()), due: None, assignee: None }],
        vec![PushedEntry { key: "P-1".into(), start: None, due: None, assignee: None }],
    ] {
        let err = doc.apply(Command::RecordPush { entries }).unwrap_err();
        assert_eq!(err, EngineError::Invalid(Invalid::NothingToRecord));
    }
}

/// The source now holds the days a push wrote.
fn echoed(rows: &[PullTask]) -> Vec<PullTask> {
    let mut rows = rows.to_vec();
    rows[0].start = Some(at(0, 8));
    rows[0].due = Some(at(1, 17));
    rows
}

#[test]
fn a_pull_does_not_read_back_the_days_it_pushed() {
    let (mut doc, rows) = pulled();
    let rows = echoed(&rows);

    // Without the record, the pushed days would come back as a constraint and a deadline.
    let unknown = diff(&doc.project, &rows, 5);
    assert_eq!(unknown.rows[0].fields, vec![PullField::Start, PullField::Due]);

    doc.apply(Command::RecordPush {
        entries: vec![PushedEntry { key: "P-1".into(), start: Some("2026-01-05".into()), due: Some("2026-01-06".into()), assignee: None }],
    })
    .unwrap();
    let known = diff(&doc.project, &rows, 5);
    assert!(known.errors.is_empty());
    assert_eq!(known.rows[0].kind, PullKind::Same);
    let err = doc.apply(Command::ApplyPull { rows, pulled_at: 5, resolutions: None }).unwrap_err();
    assert_eq!(err, EngineError::Invalid(Invalid::NothingToPull));
    let task = by_key(&doc.project, "P-1");
    assert_eq!(task.constraint_type, ConstraintType::Asap);
    assert_eq!(task.deadline, None);
}

#[test]
fn a_different_day_at_the_source_is_a_real_change_and_applies() {
    let (mut doc, rows) = pulled();
    doc.apply(Command::RecordPush {
        entries: vec![PushedEntry { key: "P-1".into(), start: Some("2026-01-05".into()), due: Some("2026-01-06".into()), assignee: None }],
    })
    .unwrap();
    let mut rows = echoed(&rows);
    rows[0].start = Some(at(2, 8));
    rows[0].due = Some(at(1, 17));
    doc.apply(Command::ApplyPull { rows, pulled_at: 6, resolutions: None }).unwrap();
    let task = by_key(&doc.project, "P-1");
    assert_eq!(task.constraint_type, ConstraintType::Snet);
    assert_eq!(task.constraint_date, Some(at(2, 8)));
    assert_eq!(task.deadline, None, "the due day is still the pushed one");
}

#[test]
fn the_pushed_days_survive_a_file() {
    let (mut doc, _) = pulled();
    doc.apply(Command::RecordPush {
        entries: vec![PushedEntry { key: "P-2".into(), start: None, due: Some("2026-01-05".into()), assignee: None }],
    })
    .unwrap();
    let dir = std::env::temp_dir().join(format!("sheafly-push-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("push.sheafly").to_string_lossy().to_string();
    save(&doc, &path).unwrap();
    let opened = load(&path).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(opened.project, doc.project);
    assert_eq!(by_key(&opened.project, "P-2").external.as_ref().unwrap().pushed_due.as_deref(), Some("2026-01-05"));
}

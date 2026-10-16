//! Planned days (phase 19, phase 4B): the start and finish Sheafly computes can be pushed to
//! fields of their own (`planned_start`, `planned_finish`) instead of the tracker's start
//! and due dates, so the deadline the source's due date stands for is left alone.

use chrono::Datelike;

use sheafly_engine::command::Command;
use sheafly_engine::model::{ResourceKind, Task};
use sheafly_engine::pull::PullTask;
use sheafly_engine::push::{plan, PushOptions, PushSkipReason, PushedEntry};
use sheafly_engine::source::{Direction, FieldPolicy, OnConflict, Source, SyncSettings, TrackerKind};
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

fn planned(field: &str, direction: Direction, push_field: Option<&str>) -> FieldPolicy {
    FieldPolicy {
        field: field.into(),
        direction,
        on_conflict: OnConflict::Ask,
        source_field: None,
        push_field: push_field.map(str::to_string),
    }
}

fn source(fields: Vec<FieldPolicy>) -> Source {
    Source {
        kind: TrackerKind::Backlog,
        site: "team.backlog.com".into(),
        project_key: "PROJ".into(),
        filter: String::new(),
        hours_per_point: 4.0,
        status_map: vec![],
        sync: SyncSettings { fields, ..SyncSettings::default() },
    }
}

/// P-1 is two days of work (Monday 5 and Tuesday 6 January), pulled three-way so it has a base.
fn doc(fields: Vec<FieldPolicy>) -> Document {
    let mut doc = Document::from_project(Project::new("Dự kiến", at(0, 8)));
    doc.apply(Command::AddResource { name: "An".into(), kind: ResourceKind::Person }).unwrap();
    doc.apply(Command::SetSource { source: Some(source(fields)) }).unwrap();
    let row = PullTask { key: "P-1".into(), name: "Việc".into(), work: Some(16 * HOUR), ..PullTask::default() };
    doc.apply(Command::ApplyPull { rows: vec![row], pulled_at: 1, resolutions: Some(vec![]) }).unwrap();
    doc
}

fn row() -> PullTask {
    PullTask { key: "P-1".into(), name: "Việc".into(), ..PullTask::default() }
}

fn dates() -> PushOptions {
    PushOptions { start: true, due: true, ..PushOptions::default() }
}

fn task(doc: &Document) -> &Task {
    doc.project.tasks.iter().find(|task| task.external.is_some()).unwrap()
}

#[test]
fn without_planned_settings_the_days_go_where_they_always_did() {
    let doc = doc(vec![]);
    let change = &plan(&doc.project, &[row()], dates()).changes[0];
    assert_eq!((change.start_field.as_deref(), change.due_field.as_deref()), (None, None));
}

#[test]
fn planned_days_name_their_own_fields_and_compare_with_those() {
    let doc = doc(vec![
        planned("planned_start", Direction::Both, Some("custom:Bắt đầu dự kiến")),
        planned("planned_finish", Direction::Both, Some("custom:Kết thúc dự kiến")),
    ]);
    // The tracker's own start and due dates are full and different; they are not the target.
    let mut at_source = row();
    at_source.start = Some(at(5, 8));
    at_source.due = Some(at(9, 17));
    let change = &plan(&doc.project, &[at_source.clone()], dates()).changes[0];
    assert_eq!(change.start_field.as_deref(), Some("custom:Bắt đầu dự kiến"));
    assert_eq!(change.due_field.as_deref(), Some("custom:Kết thúc dự kiến"));
    assert_eq!(change.start.as_ref().unwrap().from, None);
    assert_eq!(change.start.as_ref().unwrap().to, "2026-01-05");
    assert_eq!(change.due.as_ref().unwrap().to, "2026-01-06");

    // What the planned fields hold now is what the push compares with.
    at_source.planned_start = Some(at(0, 8));
    at_source.planned_finish = Some(at(3, 17));
    let change = &plan(&doc.project, &[at_source.clone()], PushOptions { overwrite: true, ..dates() }).changes[0];
    assert!(change.start.is_none(), "the planned start already matches");
    assert_eq!(change.due.as_ref().unwrap().from.as_deref(), Some("2026-01-08"));
    // Without overwrite a planned field that already has a value is left.
    assert!(plan(&doc.project, &[at_source], dates()).is_empty());
}

#[test]
fn the_direction_of_a_planned_field_governs_its_push_not_the_plain_one() {
    // Start is pull-only, but the planned start (its own field) may be pushed.
    let doc = doc(vec![
        planned("start", Direction::Pull, None),
        planned("planned_start", Direction::Push, Some("custom:Bắt đầu dự kiến")),
        planned("planned_finish", Direction::Off, Some("custom:Kết thúc dự kiến")),
    ]);
    let change = &plan(&doc.project, &[row()], dates()).changes[0];
    assert!(change.start.is_some() && change.due.is_none());
    // Without a planned setting the plain direction applies as before.
    let plain = self::doc(vec![planned("start", Direction::Pull, None)]);
    let result = plan(&plain.project, &[row()], dates());
    assert!(result.changes[0].start.is_none() && result.changes[0].due.is_some());
}

#[test]
fn recorded_planned_days_let_the_next_push_see_that_the_source_changed_them() {
    let mut doc = doc(vec![
        planned("planned_start", Direction::Both, Some("custom:Bắt đầu dự kiến")),
        planned("planned_finish", Direction::Both, Some("custom:Kết thúc dự kiến")),
    ]);
    doc.apply(Command::RecordPush {
        entries: vec![PushedEntry {
            key: "P-1".into(),
            start: None,
            due: None,
            assignee: None,
            planned_start: Some("2026-01-05".into()),
            planned_finish: Some("2026-01-06".into()),
        }],
    })
    .unwrap();
    let base = task(&doc).external.as_ref().unwrap().base.as_ref().unwrap();
    assert_eq!(base.source.get("planned_start").map(String::as_str), Some("2026-01-05"));
    assert_eq!(base.source.get("planned_finish").map(String::as_str), Some("2026-01-06"));

    // Sheafly now plans a later finish; the source still holds what we wrote: push it.
    let mut at_source = row();
    at_source.planned_start = Some(at(0, 8));
    at_source.planned_finish = Some(at(1, 17));
    doc.apply(Command::SetConstraint {
        uid: task(&doc).uid,
        constraint_type: sheafly_engine::model::ConstraintType::Snet,
        date: Some(at(2, 8)),
    })
    .unwrap();
    let options = PushOptions { overwrite: true, ..dates() };
    let change = &plan(&doc.project, &[at_source.clone()], options).changes[0];
    assert_eq!(change.start.as_ref().unwrap().to, "2026-01-07");

    // Somebody edits the planned finish at the source in between: leave it for a pull.
    at_source.planned_finish = Some(at(6, 17));
    let result = plan(&doc.project, &[at_source], options);
    assert!(result.changes[0].due.is_none());
    let skip = result.skipped.iter().find(|skip| skip.reason == PushSkipReason::SourceChanged).unwrap();
    assert_eq!(skip.field.as_deref(), Some("planned_finish"));
}

#[test]
fn a_planned_day_alone_is_enough_to_record_and_a_bad_one_is_refused() {
    let mut doc = doc(vec![]);
    let entry = |planned_start: Option<&str>| PushedEntry {
        key: "P-1".into(),
        start: None,
        due: None,
        assignee: None,
        planned_start: planned_start.map(str::to_string),
        planned_finish: None,
    };
    doc.apply(Command::RecordPush { entries: vec![entry(Some("2026-01-05"))] }).unwrap();
    let err = doc.apply(Command::RecordPush { entries: vec![entry(Some("05/01"))] }).unwrap_err();
    assert_eq!(err, EngineError::Invalid(Invalid::PushDate));
}

#[test]
fn the_planned_keys_are_valid_in_the_settings() {
    let mut doc = doc(vec![]);
    doc.apply(Command::SetSource {
        source: Some(source(vec![planned("planned_start", Direction::Push, Some("custom:A")), planned("planned_finish", Direction::Both, None)])),
    })
    .unwrap();
}

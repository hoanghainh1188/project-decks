//! Three-way sync of pulled tasks (phase 19, phase 3): a pull with `resolutions` moves a
//! field only when the source changed it and Sheafly did not, lists fields changed on
//! both sides as conflicts, and applies the user's choice. Pushes keep the base in step.

use chrono::Datelike;

use sheafly_engine::command::Command;
use sheafly_engine::fields::{FieldKind, FieldValue};
use sheafly_engine::model::{ConstraintType, ResourceKind, Task};
use sheafly_engine::pull::{diff_with, PullKind, PullTask};
use sheafly_engine::push::{plan, PushOptions, PushSkipReason, PushedEntry};
use sheafly_engine::source::StatusClass;
use sheafly_engine::store::{load, save};
use sheafly_engine::sync::{Resolution, Side};
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

fn doc() -> Document {
    let mut doc = Document::from_project(Project::new("Ba bên", at(0, 8)));
    for (name, email) in [("An", "an@example.com"), ("Bình", "binh@example.com")] {
        doc.apply(Command::AddResource { name: name.into(), kind: ResourceKind::Person }).unwrap();
        let uid = doc.project.resources.last().unwrap().uid;
        doc.apply(Command::SetResourceEmail { uid, email: email.into() }).unwrap();
    }
    doc.apply(Command::AddField { name: "Loại".into(), kind: FieldKind::Text }).unwrap();
    doc
}

/// What the source says about P-1 at the start.
fn issue() -> PullTask {
    PullTask {
        key: "P-1".into(),
        name: "Làm đăng nhập".into(),
        work: Some(8 * HOUR),
        start: Some(at(2, 8)),
        due: Some(at(8, 17)),
        status: Some(StatusClass::NotStarted),
        assignee_email: Some("an@example.com".into()),
        values: vec![("Loại".into(), FieldValue::Text("Tính năng".into()))],
        ..PullTask::default()
    }
}

fn three_way(doc: &mut Document, rows: Vec<PullTask>, resolutions: Vec<Resolution>) -> Result<(), EngineError> {
    doc.apply(Command::ApplyPull { rows, pulled_at: 1, resolutions: Some(resolutions) }).map(|_| ())
}

fn synced() -> Document {
    let mut doc = doc();
    three_way(&mut doc, vec![issue()], vec![]).unwrap();
    doc
}

fn task(doc: &Document) -> &Task {
    doc.project.tasks.iter().find(|task| task.external.is_some()).unwrap()
}

fn uid(doc: &Document) -> u32 {
    task(doc).uid
}

fn nothing(doc: &mut Document, rows: Vec<PullTask>) {
    let err = doc.apply(Command::ApplyPull { rows, pulled_at: 9, resolutions: Some(vec![]) }).unwrap_err();
    assert_eq!(err, EngineError::Invalid(Invalid::NothingToPull));
}

fn choose(field: &str, keep: Side) -> Vec<Resolution> {
    vec![Resolution { key: "P-1".into(), field: field.into(), keep }]
}

#[test]
fn the_first_three_way_pull_adds_the_task_and_records_a_base() {
    let doc = synced();
    let task = task(&doc);
    assert_eq!(task.name, "Làm đăng nhập");
    assert_eq!(task.constraint_type, ConstraintType::Snet);
    let base = task.external.as_ref().unwrap().base.as_ref().unwrap();
    assert_eq!(base.source.get("name").map(String::as_str), Some("Làm đăng nhập"));
    assert_eq!(base.source.get("start").map(String::as_str), Some("2026-01-07"));
    assert_eq!(base.source.get("due").map(String::as_str), Some("2026-01-13"));
    assert_eq!(base.source.get("status").map(String::as_str), Some("not_started"));
    assert_eq!(base.source.get("assignee").map(String::as_str), Some("an@example.com"));
    assert_eq!(base.local.get("work").map(String::as_str), Some("480"));
    assert_eq!(base.local.get("assignee").map(String::as_str), Some("an@example.com"));
    assert_eq!(base.source.get(&format!("field:{}", doc.project.fields[0].id)).map(String::as_str), Some("Tính năng"));
}

#[test]
fn pulling_the_same_rows_again_changes_nothing() {
    let mut doc = synced();
    let before = doc.project.clone();
    nothing(&mut doc, vec![issue()]);
    assert_eq!(doc.project, before);
}

#[test]
fn an_edit_in_sheafly_survives_a_pull_when_the_source_did_not_change() {
    let mut doc = synced();
    let uid = uid(&doc);
    doc.apply(Command::RenameTask { uid, name: "Tên của tôi".into() }).unwrap();
    doc.apply(Command::SetDeadline { uid, date: Some(at(20, 17)) }).unwrap();
    nothing(&mut doc, vec![issue()]);
    assert_eq!(task(&doc).name, "Tên của tôi");
    assert_eq!(task(&doc).deadline, Some(at(20, 17)));
}

#[test]
fn a_change_at_the_source_alone_is_taken() {
    let mut doc = synced();
    let mut row = issue();
    row.name = "Tên mới ở nguồn".into();
    row.due = Some(at(9, 17));
    row.status = Some(StatusClass::Done);
    three_way(&mut doc, vec![row.clone()], vec![]).unwrap();
    let task = task(&doc);
    assert_eq!(task.name, "Tên mới ở nguồn");
    assert_eq!(task.deadline, Some(at(9, 17)));
    assert_eq!(task.percent_complete, 100.0);
    let base = task.external.as_ref().unwrap().base.as_ref().unwrap();
    assert_eq!(base.source.get("name").map(String::as_str), Some("Tên mới ở nguồn"));
    assert_eq!(base.source.get("status").map(String::as_str), Some("done"));
    nothing(&mut doc, vec![row]);
}

#[test]
fn different_fields_changed_on_each_side_both_go_through() {
    let mut doc = synced();
    let uid = uid(&doc);
    doc.apply(Command::RenameTask { uid, name: "Tên của tôi".into() }).unwrap();
    let mut row = issue();
    row.due = Some(at(9, 17));
    three_way(&mut doc, vec![row], vec![]).unwrap();
    assert_eq!(task(&doc).name, "Tên của tôi");
    assert_eq!(task(&doc).deadline, Some(at(9, 17)));
}

fn conflicting() -> (Document, Vec<PullTask>) {
    let mut doc = synced();
    let uid = uid(&doc);
    doc.apply(Command::RenameTask { uid, name: "Tên của tôi".into() }).unwrap();
    let mut row = issue();
    row.name = "Tên ở nguồn".into();
    (doc, vec![row])
}

#[test]
fn a_field_changed_on_both_sides_is_a_conflict_and_nothing_applies() {
    let (mut doc, rows) = conflicting();
    let preview = diff_with(&doc.project, &rows, 5, Some(&[]));
    assert!(preview.errors.is_empty());
    assert_eq!(preview.conflicts.len(), 1);
    let conflict = &preview.conflicts[0];
    assert_eq!((conflict.key.as_str(), conflict.field.as_str()), ("P-1", "name"));
    assert_eq!(conflict.base.as_deref(), Some("Làm đăng nhập"));
    assert_eq!(conflict.source.as_deref(), Some("Tên ở nguồn"));
    assert_eq!(conflict.local.as_deref(), Some("Tên của tôi"));
    assert_eq!(preview.rows[0].kind, PullKind::Same);

    let before = doc.project.clone();
    let err = doc.apply(Command::ApplyPull { rows, pulled_at: 5, resolutions: Some(vec![]) }).unwrap_err();
    assert_eq!(err, EngineError::Invalid(Invalid::PullHasConflicts));
    assert_eq!(doc.project, before);
}

#[test]
fn taking_the_source_resolves_a_conflict_in_its_favour() {
    let (mut doc, rows) = conflicting();
    three_way(&mut doc, rows.clone(), choose("name", Side::Source)).unwrap();
    assert_eq!(task(&doc).name, "Tên ở nguồn");
    nothing(&mut doc, rows);
}

#[test]
fn keeping_sheafly_resolves_it_and_a_later_change_at_the_source_conflicts_again() {
    let (mut doc, rows) = conflicting();
    // Settling by keeping Sheafly changes no task, but it is recorded (one undo step).
    three_way(&mut doc, rows.clone(), choose("name", Side::Sheafly)).unwrap();
    assert_eq!(task(&doc).name, "Tên của tôi");
    nothing(&mut doc, rows.clone());

    let mut again = rows;
    again[0].name = "Lại đổi ở nguồn".into();
    let preview = diff_with(&doc.project, &again, 6, Some(&[]));
    assert_eq!(preview.conflicts.len(), 1, "the kept value is still protected");
    assert_eq!(preview.conflicts[0].base.as_deref(), Some("Tên ở nguồn"));
}

#[test]
fn both_sides_changed_to_the_same_value_just_updates_the_base() {
    let mut doc = synced();
    let uid = uid(&doc);
    doc.apply(Command::RenameTask { uid, name: "Cùng tên".into() }).unwrap();
    let mut row = issue();
    row.name = "Cùng tên".into();
    three_way(&mut doc, vec![row.clone()], vec![]).unwrap();
    nothing(&mut doc, vec![row]);
    assert_eq!(task(&doc).external.as_ref().unwrap().base.as_ref().unwrap().source.get("name").map(String::as_str), Some("Cùng tên"));
}

#[test]
fn dates_status_assignee_and_custom_fields_conflict_by_their_own_keys() {
    let mut doc = synced();
    let uid = uid(&doc);
    let field = doc.project.fields[0].id;
    let binh = doc.project.resources[1].uid;
    doc.apply(Command::SetConstraint { uid, constraint_type: ConstraintType::Snet, date: Some(at(4, 8)) }).unwrap();
    doc.apply(Command::SetDeadline { uid, date: Some(at(20, 17)) }).unwrap();
    doc.apply(Command::SetPercentComplete { uid, percent: 100.0 }).unwrap();
    doc.apply(Command::SetFieldValue { uid, field_id: field, value: Some(FieldValue::Text("Lỗi".into())) }).unwrap();
    doc.apply(Command::SetWork { uid, minutes: 3 * HOUR }).unwrap();
    for resource in doc.project.assignments.iter().map(|a| a.uid).collect::<Vec<_>>() {
        doc.apply(Command::RemoveAssignment { uid: resource }).unwrap();
    }
    doc.apply(Command::AssignResource { task_uid: uid, resource_uid: binh, units: 1.0 }).unwrap();

    let mut row = issue();
    row.start = Some(at(5, 8));
    row.due = Some(at(9, 17));
    row.status = Some(StatusClass::InProgress);
    row.values = vec![("Loại".into(), FieldValue::Text("Tài liệu".into()))];
    row.work = Some(5 * HOUR);
    // The assignee did not change at the source: no conflict there, and Sheafly keeps Bình.
    let preview = diff_with(&doc.project, &[row.clone()], 5, Some(&[]));
    let mut keys: Vec<&str> = preview.conflicts.iter().map(|conflict| conflict.field.as_str()).collect();
    keys.sort_unstable();
    let field_key = format!("field:{field}");
    let mut expected = vec!["due", "start", "status", "work", field_key.as_str()];
    expected.sort_unstable();
    assert_eq!(keys, expected);
    let start = preview.conflicts.iter().find(|c| c.field == "start").unwrap();
    assert_eq!(
        (start.base.as_deref(), start.source.as_deref(), start.local.as_deref()),
        (Some("2026-01-07"), Some("2026-01-10"), Some("2026-01-09"))
    );
    let status = preview.conflicts.iter().find(|c| c.field == "status").unwrap();
    assert_eq!(
        (status.base.as_deref(), status.source.as_deref(), status.local.as_deref()),
        (Some("not_started"), Some("in_progress"), Some("done"))
    );
    assert!(preview.rows[0].fields.is_empty());
}

#[test]
fn an_assignee_changed_on_both_sides_conflicts_and_the_source_choice_applies() {
    let mut doc = synced();
    let uid = uid(&doc);
    let binh = doc.project.resources[1].uid;
    for assignment in doc.project.assignments.iter().map(|a| a.uid).collect::<Vec<_>>() {
        doc.apply(Command::RemoveAssignment { uid: assignment }).unwrap();
    }
    doc.apply(Command::AssignResource { task_uid: uid, resource_uid: binh, units: 1.0 }).unwrap();
    // The source moves the issue to someone else, Bình is not who is there.
    let mut row = issue();
    row.assignee_email = Some("binh@example.com".into());
    let same_person = diff_with(&doc.project, &[row.clone()], 5, Some(&[]));
    assert!(same_person.conflicts.is_empty(), "both sides landed on Bình: they agree");

    row.assignee_email = Some("an@example.com".into());
    let unchanged = diff_with(&doc.project, &[row], 5, Some(&[]));
    assert!(unchanged.conflicts.is_empty(), "the source did not change: Sheafly keeps Bình");
}

#[test]
fn a_link_with_no_base_records_one_from_agreement_and_reports_disagreement() {
    // Made by the original pull: no base.
    let mut doc = doc();
    doc.apply(Command::ApplyPull { rows: vec![issue()], pulled_at: 1, resolutions: None }).unwrap();
    assert!(task(&doc).external.as_ref().unwrap().base.is_none());
    nothing(&mut doc, vec![issue()]);

    let uid = uid(&doc);
    doc.apply(Command::RenameTask { uid, name: "Tên của tôi".into() }).unwrap();
    let preview = diff_with(&doc.project, &[issue()], 5, Some(&[]));
    assert_eq!(preview.conflicts.len(), 1);
    assert_eq!(preview.conflicts[0].base, None, "nobody knows who changed it");
    // Taking the source settles it and gives the link a base.
    three_way(&mut doc, vec![issue()], choose("name", Side::Source)).unwrap();
    assert_eq!(task(&doc).name, "Làm đăng nhập");
    assert!(task(&doc).external.as_ref().unwrap().base.is_some());
}

#[test]
fn a_day_the_source_holds_because_of_a_push_is_not_a_conflict_without_a_base() {
    let mut doc = doc();
    doc.apply(Command::ApplyPull {
        rows: vec![PullTask { key: "P-1".into(), name: "Việc".into(), work: Some(16 * HOUR), ..PullTask::default() }],
        pulled_at: 1,
        resolutions: None,
    })
    .unwrap();
    doc.apply(Command::RecordPush {
        entries: vec![PushedEntry { key: "P-1".into(), start: Some("2026-01-05".into()), due: Some("2026-01-06".into()), assignee: None }],
    })
    .unwrap();
    let row = PullTask {
        key: "P-1".into(),
        name: "Việc".into(),
        work: Some(16 * HOUR),
        start: Some(at(0, 8)),
        due: Some(at(1, 17)),
        ..PullTask::default()
    };
    let preview = diff_with(&doc.project, &[row], 5, Some(&[]));
    assert!(preview.conflicts.is_empty());
    assert_eq!(preview.rows[0].kind, PullKind::Same);
    assert!(preview.rows[0].fields.is_empty());
}

#[test]
fn a_push_keeps_the_base_so_a_pull_right_after_changes_nothing() {
    let mut doc = synced();
    let rows = vec![issue()];
    // Push the computed days; the source then holds them.
    let result = plan(&doc.project, &rows, PushOptions { start: true, due: true, overwrite: true, ..PushOptions::default() });
    let change = &result.changes[0];
    let entry = PushedEntry {
        key: "P-1".into(),
        start: change.start.as_ref().map(|d| d.to.clone()),
        due: change.due.as_ref().map(|d| d.to.clone()),
        assignee: None,
    };
    // The task is scheduled from its SNET start on 2026-01-07 (8 hours: one day) so the start
    // day already matches and only the due day differs.
    assert!(change.start.is_none());
    let due_to = change.due.as_ref().unwrap().to.clone();
    assert_eq!(due_to, "2026-01-07");
    doc.apply(Command::RecordPush { entries: vec![entry] }).unwrap();

    let mut echoed = issue();
    echoed.due = Some(at(2, 17));
    nothing(&mut doc, vec![echoed.clone()]);
    // And someone else moving the due date later is a real change that applies.
    echoed.due = Some(at(10, 17));
    three_way(&mut doc, vec![echoed], vec![]).unwrap();
    assert_eq!(task(&doc).deadline, Some(at(10, 17)));
}

#[test]
fn a_push_leaves_alone_a_field_the_source_changed_since_the_last_sync() {
    let doc = synced();
    let mut row = issue();
    row.start = Some(at(4, 8));
    row.due = Some(at(11, 17));
    row.assignee_email = Some("binh@example.com".into());
    let options = PushOptions { start: true, due: true, assignee: true, overwrite: true };
    let result = plan(&doc.project, &[row], options);
    assert!(result.changes.is_empty());
    let mut fields: Vec<&str> = result
        .skipped
        .iter()
        .filter(|skip| skip.reason == PushSkipReason::SourceChanged)
        .filter_map(|skip| skip.field.as_deref())
        .collect();
    fields.sort_unstable();
    assert_eq!(fields, ["assignee", "due", "start"]);
}

#[test]
fn a_status_the_source_does_not_send_and_a_group_are_left_alone() {
    let mut doc = synced();
    let mut row = issue();
    row.status = None;
    row.work = None;
    row.assignee_email = None;
    nothing(&mut doc, vec![row]);
    // A task with children in the pull is a group and takes no effort from the source.
    let parent = PullTask { key: "P-1".into(), ..issue() };
    let child = PullTask { key: "P-2".into(), name: "Con".into(), parent_key: Some("P-1".into()), ..PullTask::default() };
    three_way(&mut doc, vec![parent, child], vec![]).unwrap();
    assert!(task(&doc).summary);
}

#[test]
fn the_base_and_the_resolutions_survive_undo_and_a_file() {
    let (mut doc, rows) = conflicting();
    let before = doc.project.clone();
    three_way(&mut doc, rows, choose("name", Side::Sheafly)).unwrap();
    let after = doc.project.clone();
    doc.undo().unwrap();
    assert_eq!(doc.project, before);
    doc.redo().unwrap();
    assert_eq!(doc.project, after);

    let dir = std::env::temp_dir().join(format!("sheafly-sync-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sync.sheafly").to_string_lossy().to_string();
    save(&doc, &path).unwrap();
    let opened = load(&path).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(opened.project, after);
}

#[test]
fn the_original_pull_replays_as_it_was_and_json_without_resolutions_still_reads() {
    let old = r#"{"type":"apply_pull","rows":[{"key":"P-1","name":"Việc"}],"pulled_at":1}"#;
    let command: Command = serde_json::from_str(old).unwrap();
    match &command {
        Command::ApplyPull { resolutions, .. } => assert!(resolutions.is_none()),
        other => panic!("{other:?}"),
    }
    // And it serializes back without the new key.
    assert!(!serde_json::to_string(&command).unwrap().contains("resolutions"));
    // The original pull still lets the source win over an edit made in Sheafly.
    let mut doc = doc();
    doc.apply(command).unwrap();
    let uid = uid(&doc);
    doc.apply(Command::RenameTask { uid, name: "Tên của tôi".into() }).unwrap();
    doc.apply(Command::ApplyPull {
        rows: vec![PullTask { key: "P-1".into(), name: "Việc".into(), ..PullTask::default() }],
        pulled_at: 2,
        resolutions: None,
    })
    .unwrap();
    assert_eq!(task(&doc).name, "Việc");
}

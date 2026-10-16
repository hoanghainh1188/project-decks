//! Configurable sync (phase 19, phase 4A): a source's `SyncSettings` set which way each
//! field syncs, what happens by default when a field changed on both sides, and the
//! percent an in-progress issue gets. Defaults leave everything as it was.

use chrono::Datelike;

use sheafly_engine::command::Command;
use sheafly_engine::fields::{FieldKind, FieldValue};
use sheafly_engine::model::{ConstraintType, ResourceKind, Task};
use sheafly_engine::pull::{diff_with, PullTask};
use sheafly_engine::push::{plan, PushOptions};
use sheafly_engine::source::{
    Direction, FieldPolicy, OnConflict, Source, StatusClass, SyncSettings, TrackerKind,
};
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

fn policy(field: &str, direction: Direction, on_conflict: OnConflict) -> FieldPolicy {
    FieldPolicy { field: field.into(), direction, on_conflict, source_field: None, push_field: None }
}

fn source(sync: SyncSettings) -> Source {
    Source {
        kind: TrackerKind::Backlog,
        site: "team.backlog.com".into(),
        project_key: "PROJ".into(),
        filter: String::new(),
        hours_per_point: 4.0,
        status_map: vec![],
        sync,
    }
}

fn with(fields: Vec<FieldPolicy>) -> SyncSettings {
    SyncSettings { fields, ..SyncSettings::default() }
}

fn doc(sync: SyncSettings) -> Document {
    let mut doc = Document::from_project(Project::new("Cấu hình", at(0, 8)));
    doc.apply(Command::AddResource { name: "An".into(), kind: ResourceKind::Person }).unwrap();
    let uid = doc.project.resources[0].uid;
    doc.apply(Command::SetResourceEmail { uid, email: "an@example.com".into() }).unwrap();
    doc.apply(Command::AddField { name: "Loại".into(), kind: FieldKind::Text }).unwrap();
    doc.apply(Command::SetSource { source: Some(source(sync)) }).unwrap();
    doc
}

fn issue() -> PullTask {
    PullTask {
        key: "P-1".into(),
        name: "Làm đăng nhập".into(),
        work: Some(8 * HOUR),
        start: Some(at(2, 8)),
        due: Some(at(8, 17)),
        status: Some(StatusClass::InProgress),
        assignee_email: Some("an@example.com".into()),
        values: vec![("Loại".into(), FieldValue::Text("Tính năng".into()))],
        ..PullTask::default()
    }
}

fn pull(doc: &mut Document, rows: Vec<PullTask>, resolutions: Vec<Resolution>) -> Result<(), EngineError> {
    doc.apply(Command::ApplyPull { rows, pulled_at: 1, resolutions: Some(resolutions) }).map(|_| ())
}

fn task(doc: &Document) -> &Task {
    doc.project.tasks.iter().find(|task| task.external.is_some()).unwrap()
}

#[test]
fn the_defaults_are_what_the_sync_always_did() {
    let settings = SyncSettings::default();
    assert_eq!((settings.in_progress_percent, settings.start_hour, settings.due_hour), (50, 8, 17));
    assert!(settings.fields.is_empty());
    assert_eq!(settings.direction("due"), Direction::Both);
    assert_eq!(settings.on_conflict("field:3"), OnConflict::Ask);
    // A source written before the settings existed reads with the defaults.
    let old = r#"{"kind":"backlog","site":"team.backlog.com","project_key":"PROJ"}"#;
    let source: Source = serde_json::from_str(old).unwrap();
    assert_eq!(source.sync, SyncSettings::default());
}

#[test]
fn a_specific_field_policy_beats_the_one_for_all_custom_fields() {
    let settings = with(vec![
        policy("fields", Direction::Pull, OnConflict::Source),
        policy("field:2", Direction::Off, OnConflict::Sheafly),
    ]);
    assert_eq!(settings.direction("field:1"), Direction::Pull);
    assert_eq!(settings.on_conflict("field:1"), OnConflict::Source);
    assert_eq!(settings.direction("field:2"), Direction::Off);
    assert_eq!(settings.on_conflict("field:2"), OnConflict::Sheafly);
    // "fields" does not apply to the built-in fields.
    assert_eq!(settings.direction("name"), Direction::Both);
}

#[test]
fn the_settings_are_checked_when_the_source_is_saved() {
    let mut doc = doc(SyncSettings::default());
    let bad: Vec<SyncSettings> = vec![
        with(vec![policy("nonsense", Direction::Both, OnConflict::Ask)]),
        with(vec![policy("field:x", Direction::Both, OnConflict::Ask)]),
        with(vec![policy("due", Direction::Both, OnConflict::Ask), policy("due", Direction::Off, OnConflict::Ask)]),
        with((0..101).map(|i| policy(&format!("field:{i}"), Direction::Both, OnConflict::Ask)).collect()),
        SyncSettings { in_progress_percent: 0, ..SyncSettings::default() },
        SyncSettings { in_progress_percent: 100, ..SyncSettings::default() },
        SyncSettings { start_hour: 24, ..SyncSettings::default() },
        SyncSettings { due_hour: 24, ..SyncSettings::default() },
        with(vec![FieldPolicy { source_field: Some("x".repeat(101)), ..policy("due", Direction::Both, OnConflict::Ask) }]),
    ];
    for sync in bad {
        let err = doc.apply(Command::SetSource { source: Some(source(sync)) }).unwrap_err();
        assert!(
            matches!(err, EngineError::Invalid(Invalid::SyncPolicy | Invalid::SyncSetting)),
            "{err:?}"
        );
    }
    // Blank field references are tidied away.
    let tidy = with(vec![FieldPolicy {
        source_field: Some("  ".into()),
        push_field: Some(" customField_7 ".into()),
        ..policy("due", Direction::Push, OnConflict::Sheafly)
    }]);
    doc.apply(Command::SetSource { source: Some(source(tidy)) }).unwrap();
    let kept = &doc.project.source.as_ref().unwrap().sync.fields[0];
    assert_eq!((kept.source_field.as_deref(), kept.push_field.as_deref()), (None, Some("customField_7")));
}

#[test]
fn deleting_a_custom_field_drops_its_policy() {
    let mut doc = doc(SyncSettings::default());
    let id = doc.project.fields[0].id;
    let keep = policy("due", Direction::Pull, OnConflict::Ask);
    let gone = policy(&format!("field:{id}"), Direction::Off, OnConflict::Ask);
    doc.apply(Command::SetSource { source: Some(source(with(vec![keep.clone(), gone]))) }).unwrap();
    doc.apply(Command::DeleteField { id }).unwrap();
    assert_eq!(doc.project.source.as_ref().unwrap().sync.fields, vec![keep]);
}

#[test]
fn a_field_that_is_not_pulled_is_neither_read_nor_recorded() {
    for direction in [Direction::Off, Direction::Push] {
        let mut doc = doc(with(vec![policy("due", direction, OnConflict::Ask), policy("work", direction, OnConflict::Ask)]));
        pull(&mut doc, vec![issue()], vec![]).unwrap();
        let task = task(&doc);
        assert_eq!(task.deadline, None, "{direction:?}");
        assert_eq!(task.name, "Làm đăng nhập");
        assert_eq!(task.constraint_type, ConstraintType::Snet, "start still comes");
        let base = task.external.as_ref().unwrap().base.as_ref().unwrap();
        assert!(!base.source.contains_key("due") && !base.source.contains_key("work"));

        // Later changes at the source to a field that is not pulled are not applied either.
        let mut row = issue();
        row.due = Some(at(9, 17));
        row.work = Some(2 * HOUR);
        let err = doc.apply(Command::ApplyPull { rows: vec![row], pulled_at: 2, resolutions: Some(vec![]) }).unwrap_err();
        assert_eq!(err, EngineError::Invalid(Invalid::NothingToPull));
    }
}

#[test]
fn a_pull_only_field_still_comes_in() {
    let mut doc = doc(with(vec![policy("due", Direction::Pull, OnConflict::Ask)]));
    pull(&mut doc, vec![issue()], vec![]).unwrap();
    assert_eq!(task(&doc).deadline, Some(at(8, 17)));
}

fn conflicting(sync: SyncSettings) -> (Document, Vec<PullTask>) {
    let mut doc = doc(sync);
    pull(&mut doc, vec![issue()], vec![]).unwrap();
    let uid = task(&doc).uid;
    doc.apply(Command::RenameTask { uid, name: "Tên của tôi".into() }).unwrap();
    let mut row = issue();
    row.name = "Tên ở nguồn".into();
    (doc, vec![row])
}

#[test]
fn a_conflict_policy_settles_the_conflict_without_asking() {
    let (mut doc, rows) = conflicting(with(vec![policy("name", Direction::Both, OnConflict::Source)]));
    assert!(diff_with(&doc.project, &rows, 5, Some(&[])).conflicts.is_empty());
    pull(&mut doc, rows, vec![]).unwrap();
    assert_eq!(task(&doc).name, "Tên ở nguồn");

    let (mut doc, rows) = conflicting(with(vec![policy("name", Direction::Both, OnConflict::Sheafly)]));
    let preview = diff_with(&doc.project, &rows, 5, Some(&[]));
    assert!(preview.conflicts.is_empty());
    assert!(preview.bases_changed, "keeping Sheafly is recorded");
    pull(&mut doc, rows.clone(), vec![]).unwrap();
    assert_eq!(task(&doc).name, "Tên của tôi");
    // The kept value stays protected from a later change at the source.
    let mut again = rows;
    again[0].name = "Lại đổi".into();
    assert!(diff_with(&doc.project, &again, 6, Some(&[])).conflicts.is_empty(), "policy says Sheafly wins again");
}

#[test]
fn the_users_own_choice_beats_the_policy() {
    let (mut doc, rows) = conflicting(with(vec![policy("name", Direction::Both, OnConflict::Sheafly)]));
    let choice = vec![Resolution { key: "P-1".into(), field: "name".into(), keep: Side::Source }];
    pull(&mut doc, rows, choice).unwrap();
    assert_eq!(task(&doc).name, "Tên ở nguồn");
}

#[test]
fn a_policy_for_one_field_leaves_the_others_asking() {
    let (mut doc, mut rows) = conflicting(with(vec![policy("due", Direction::Both, OnConflict::Source)]));
    let uid = task(&doc).uid;
    doc.apply(Command::SetDeadline { uid, date: Some(at(20, 17)) }).unwrap();
    rows[0].due = Some(at(9, 17));
    let preview = diff_with(&doc.project, &rows, 5, Some(&[]));
    let fields: Vec<&str> = preview.conflicts.iter().map(|c| c.field.as_str()).collect();
    assert_eq!(fields, ["name"], "the due conflict settled itself; the name still asks");
}

#[test]
fn the_in_progress_percent_is_the_sources_setting_in_a_three_way_pull() {
    let mut doc = doc(SyncSettings { in_progress_percent: 30, ..SyncSettings::default() });
    pull(&mut doc, vec![issue()], vec![]).unwrap();
    assert_eq!(task(&doc).percent_complete, 30.0);

    // The original pull keeps its fixed 50 so older logs replay as they were.
    let mut old = self::doc(SyncSettings { in_progress_percent: 30, ..SyncSettings::default() });
    old.apply(Command::ApplyPull { rows: vec![issue()], pulled_at: 1, resolutions: None }).unwrap();
    assert_eq!(task(&old).percent_complete, 50.0);
}

#[test]
fn push_leaves_out_what_the_settings_do_not_push() {
    let mut doc = doc(SyncSettings::default());
    pull(&mut doc, vec![PullTask { key: "P-1".into(), name: "Việc".into(), work: Some(16 * HOUR), ..PullTask::default() }], vec![]).unwrap();
    let rows = vec![PullTask { key: "P-1".into(), name: "Việc".into(), ..PullTask::default() }];
    let all = PushOptions { start: true, due: true, assignee: true, overwrite: false };
    assert!(plan(&doc.project, &rows, all).changes[0].start.is_some());

    let settings = with(vec![
        policy("start", Direction::Pull, OnConflict::Ask),
        policy("due", Direction::Off, OnConflict::Ask),
        policy("assignee", Direction::Push, OnConflict::Ask),
    ]);
    doc.apply(Command::SetSource { source: Some(source(settings)) }).unwrap();
    let result = plan(&doc.project, &rows, all);
    assert!(result.changes.is_empty(), "start is pull-only and due is off; nobody is assigned");

    let both = with(vec![policy("start", Direction::Push, OnConflict::Ask), policy("due", Direction::Both, OnConflict::Ask)]);
    doc.apply(Command::SetSource { source: Some(source(both)) }).unwrap();
    let result = plan(&doc.project, &rows, all);
    assert!(result.changes[0].start.is_some() && result.changes[0].due.is_some());
}

#[test]
fn the_settings_survive_undo_and_a_file() {
    let mut doc = doc(SyncSettings::default());
    let before = doc.project.clone();
    let settings = SyncSettings {
        fields: vec![FieldPolicy {
            push_field: Some("customField_7".into()),
            ..policy("due", Direction::Push, OnConflict::Sheafly)
        }],
        in_progress_percent: 40,
        start_hour: 9,
        due_hour: 18,
    };
    doc.apply(Command::SetSource { source: Some(source(settings.clone())) }).unwrap();
    let after = doc.project.clone();
    doc.undo().unwrap();
    assert_eq!(doc.project, before);
    doc.redo().unwrap();
    assert_eq!(doc.project, after);
    let dir = std::env::temp_dir().join(format!("sheafly-syncconf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("c.sheafly").to_string_lossy().to_string();
    sheafly_engine::store::save(&doc, &path).unwrap();
    let opened = sheafly_engine::store::load(&path).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(opened.project.source.as_ref().unwrap().sync, settings);
}

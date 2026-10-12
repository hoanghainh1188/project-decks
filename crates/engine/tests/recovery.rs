//! Opening damaged files and files saved by another engine.

use std::fs;

use rusqlite::Connection;

use projectdesk_engine::command::Command;
use projectdesk_engine::document::replay;
use projectdesk_engine::model::DepKind;
use projectdesk_engine::recovery::Damage;
use projectdesk_engine::store::{open, save};
use projectdesk_engine::timeutil::timestamp;
use projectdesk_engine::{Document, Project};

const DAY: i64 = 480;
/// Commands in `built()`.
const TOTAL: usize = 30;

fn temp_path(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "projectdesk-recovery-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.join("du-an.pdproj").to_string_lossy().to_string()
}

/// Ten chained tasks: 9 adds, 10 durations, 9 links, 2 renames.
fn built() -> Document {
    let mut doc = Document::from_project(Project::new("Khôi phục", timestamp(2026, 1, 5, 8, 0)));
    for uid in 2..=10u32 {
        doc.apply(Command::AddTask {
            name: format!("Việc {uid}"),
            after_uid: Some(uid - 1),
        })
        .unwrap();
    }
    for uid in 1..=10u32 {
        doc.apply(Command::SetDuration {
            uid,
            minutes: DAY * i64::from(1 + uid % 3),
        })
        .unwrap();
    }
    for uid in 2..=10u32 {
        doc.apply(Command::AddDependency {
            predecessor_uid: uid - 1,
            successor_uid: uid,
            kind: DepKind::Fs,
            lag: 0,
        })
        .unwrap();
    }
    doc.apply(Command::RenameTask {
        uid: 1,
        name: "Bắt đầu".into(),
    })
    .unwrap();
    doc.apply(Command::RenameTask {
        uid: 10,
        name: "Kết thúc".into(),
    })
    .unwrap();
    assert_eq!(doc.commands.len(), TOTAL);
    doc
}

fn saved(name: &str, doc: &Document) -> String {
    let path = temp_path(name);
    save(doc, &path).unwrap();
    path
}

fn sql(path: &str, statement: &str) {
    Connection::open(path)
        .unwrap()
        .execute_batch(statement)
        .unwrap();
}

fn json(project: &Project) -> serde_json::Value {
    serde_json::to_value(project).unwrap()
}

#[test]
fn a_damaged_command_opens_a_copy_with_the_history_before_it() {
    let doc = built();
    let path = saved("middle", &doc);
    sql(
        &path,
        "DELETE FROM checkpoint; UPDATE command_log SET payload = '{hỏng' WHERE seq = 12;",
    );
    let before = fs::read(&path).unwrap();

    let opened = open(&path).unwrap();

    let recovery = opened.report.recovery.clone().unwrap();
    assert_eq!(recovery.damage, Damage::Unreadable);
    assert_eq!(
        (recovery.at, recovery.total, recovery.saved_cursor),
        (12, TOTAL, TOTAL)
    );
    assert_eq!(recovery.shown_after, 12);
    assert!(recovery.history_kept);
    let copy = opened.document;
    assert_eq!(copy.path, None);
    assert!(copy.dirty);
    assert_eq!((copy.cursor, copy.commands.len()), (12, 12));
    assert_eq!(
        json(&copy.project),
        json(&replay(&doc.base, &doc.commands[..12]).unwrap())
    );
    assert_eq!(
        fs::read(&path).unwrap(),
        before,
        "opening must not touch the file"
    );
    let message = opened.report.message().unwrap();
    assert!(message.contains("thao tác 13/30"), "{message}");
    assert!(
        message.contains("sau thao tác 12 (lúc lưu là 30)"),
        "{message}"
    );
}

#[test]
fn a_saved_checkpoint_past_the_damage_shows_what_the_user_saw() {
    let doc = built();
    let path = saved("checkpoint", &doc);
    sql(
        &path,
        "UPDATE command_log SET payload = '{hỏng' WHERE seq = 3;",
    );

    let opened = open(&path).unwrap();

    let recovery = opened.report.recovery.clone().unwrap();
    assert_eq!(recovery.shown_after, TOTAL);
    assert!(!recovery.history_kept);
    let mut copy = opened.document;
    assert_eq!(json(&copy.project), json(&doc.project));
    assert!(copy.commands.is_empty());
    assert!(opened
        .report
        .message()
        .unwrap()
        .contains("không có lịch sử hoàn tác"));

    // The copy edits and saves like any new project.
    copy.apply(Command::RenameTask {
        uid: 2,
        name: "Sửa sau khôi phục".into(),
    })
    .unwrap();
    let new_path = temp_path("checkpoint-copy");
    save(&copy, &new_path).unwrap();
    let reopened = open(&new_path).unwrap();
    assert_eq!(reopened.report.message(), None);
    assert_eq!(json(&reopened.document.project), json(&copy.project));
}

#[test]
fn a_command_that_no_longer_applies_stops_the_replay_before_it() {
    let doc = built();
    let path = saved("failed", &doc);
    sql(
        &path,
        r#"DELETE FROM checkpoint;
           UPDATE command_log SET payload = '{"type":"rename_task","uid":999,"name":"Ma"}' WHERE seq = 20;"#,
    );

    let opened = open(&path).unwrap();

    let recovery = opened.report.recovery.unwrap();
    assert!(
        matches!(recovery.damage, Damage::Failed(_)),
        "{:?}",
        recovery.damage
    );
    assert_eq!((recovery.at, recovery.shown_after), (20, 20));
    assert_eq!(
        json(&opened.document.project),
        json(&replay(&doc.base, &doc.commands[..20]).unwrap())
    );
}

#[test]
fn damage_only_after_the_cursor_opens_the_file_itself() {
    let mut doc = built();
    for _ in 0..5 {
        doc.undo().unwrap();
    }
    let path = saved("redo", &doc);
    sql(
        &path,
        "DELETE FROM checkpoint; UPDATE command_log SET payload = '{hỏng' WHERE seq = 27;",
    );

    let opened = open(&path).unwrap();

    let recovery = opened.report.recovery.clone().unwrap();
    assert_eq!(
        (recovery.at, recovery.saved_cursor, recovery.shown_after),
        (27, 25, 25)
    );
    assert!(!recovery.copy);
    let mut document = opened.document;
    assert_eq!(document.path.as_deref(), Some(path.as_str()));
    assert!(!document.dirty);
    assert_eq!(json(&document.project), json(&doc.project));
    // The two readable redo commands still redo.
    assert_eq!(document.commands.len(), 27);
    document.redo().unwrap();
    let message = opened.report.message().unwrap();
    assert!(message.contains("từ thao tác 28 trở đi"), "{message}");
    assert!(!message.contains("bản sao"), "{message}");
}

#[test]
fn a_damaged_cursor_or_burndown_table_still_opens() {
    let doc = built();
    let path = saved("meta", &doc);
    sql(
        &path,
        "UPDATE meta SET value = 'hỏng' WHERE key = 'cursor';
         DROP TABLE burndown; CREATE TABLE burndown (khac TEXT);",
    );

    let opened = open(&path).unwrap();

    assert_eq!(opened.report.message(), None);
    assert_eq!(opened.document.cursor, TOTAL);
    assert!(opened.document.burndown.is_empty());
    assert_eq!(json(&opened.document.project), json(&doc.project));
}

#[test]
fn tasks_another_engine_scheduled_differently_are_reported() {
    let doc = built();
    let path = saved("engine", &doc);
    let mut theirs = doc.project.clone();
    let task = theirs.tasks.iter_mut().find(|task| task.uid == 4).unwrap();
    task.start -= 24 * 60 * 60 * 1000;
    let payload = serde_json::to_string(&theirs).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE checkpoint SET engine = '0.0.0+0', project = ?1 WHERE applied = ?2",
            rusqlite::params![payload, TOTAL as i64],
        )
        .unwrap();

    let opened = open(&path).unwrap();

    assert_eq!(opened.report.recovery, None);
    assert_eq!(
        opened
            .report
            .moved
            .iter()
            .map(|task| task.uid)
            .collect::<Vec<_>>(),
        vec![4]
    );
    assert_eq!(opened.document.path.as_deref(), Some(path.as_str()));
    assert!(!opened.document.dirty);
    let message = opened.report.message().unwrap();
    assert!(message.contains("1 công việc đổi ngày"), "{message}");
    assert!(message.contains("• Việc 4:"), "{message}");
}

#[test]
fn another_engine_with_the_same_dates_says_nothing() {
    let doc = built();
    let path = saved("same", &doc);
    sql(&path, "UPDATE checkpoint SET engine = '0.0.0+0';");

    let opened = open(&path).unwrap();

    assert_eq!(opened.report.message(), None);
    assert_eq!(json(&opened.document.project), json(&doc.project));
}

//! Checkpoints only cache replays: undo, redo, and open must give the same project as
//! replaying the whole log from `base`.

use std::fs;

use rusqlite::Connection;

use projectdesk_engine::checkpoint::INTERVAL;
use projectdesk_engine::command::Command;
use projectdesk_engine::document::replay;
use projectdesk_engine::model::{ConstraintType, DepKind, ResourceKind};
use projectdesk_engine::store::{load, save};
use projectdesk_engine::timeutil::timestamp;
use projectdesk_engine::{Document, Project};

const DAY: i64 = 480;

fn temp_path(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "projectdesk-checkpoint-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.join("du-an.pdproj").to_string_lossy().to_string()
}

/// A mixed log: groups, links into and out of summaries, a resource, progress, and a
/// constraint, long enough to cross several checkpoint intervals.
fn commands() -> Vec<Command> {
    let mut commands = vec![Command::AddResource {
        name: "An".into(),
        kind: ResourceKind::Person,
    }];
    for uid in 2..=24u32 {
        commands.push(Command::AddTask {
            name: format!("Việc {uid}"),
            after_uid: Some(uid - 1),
        });
    }
    for uid in 1..=24u32 {
        if uid % 6 != 1 {
            commands.push(Command::Indent { uid });
            commands.push(Command::SetDuration {
                uid,
                minutes: DAY * i64::from(1 + uid % 3),
            });
        }
    }
    for uid in 3..=24u32 {
        if uid % 6 != 1 && uid % 6 != 2 {
            commands.push(dependency(uid - 1, uid));
        }
    }
    commands.push(dependency(1, 7));
    commands.push(dependency(7, 14));
    for uid in [3, 9, 15] {
        commands.push(Command::AssignResource {
            task_uid: uid,
            resource_uid: 1,
            units: 1.0,
        });
    }
    commands.push(Command::SetPercentComplete {
        uid: 3,
        percent: 50.0,
    });
    commands.push(Command::SetConstraint {
        uid: 20,
        constraint_type: ConstraintType::Snet,
        date: Some(timestamp(2026, 3, 2, 8, 0)),
    });
    for round in 0..20u32 {
        let uid = 2 + round % 22;
        if uid % 6 != 1 {
            commands.push(Command::SetDuration {
                uid,
                minutes: DAY * i64::from(1 + round % 4),
            });
        }
        commands.push(Command::RenameTask {
            uid,
            name: format!("Đổi {round}"),
        });
    }
    commands
}

fn dependency(predecessor_uid: u32, successor_uid: u32) -> Command {
    Command::AddDependency {
        predecessor_uid,
        successor_uid,
        kind: DepKind::Fs,
        lag: 0,
    }
}

fn built() -> Document {
    let mut doc = Document::from_project(Project::new("Mốc", timestamp(2026, 1, 5, 8, 0)));
    for command in commands() {
        doc.apply(command).unwrap();
    }
    doc
}

fn json(project: &Project) -> serde_json::Value {
    serde_json::to_value(project).unwrap()
}

fn assert_matches_replay(doc: &Document) {
    let expected = replay(&doc.base, &doc.commands[..doc.cursor]).unwrap();
    assert_eq!(json(&doc.project), json(&expected), "cursor {}", doc.cursor);
}

#[test]
fn the_log_crosses_several_intervals() {
    let doc = built();
    assert!(
        doc.commands.len() > 4 * INTERVAL,
        "{} commands",
        doc.commands.len()
    );
    assert!(doc.checkpoints.len() >= 4);
}

#[test]
fn every_undo_and_redo_matches_a_full_replay() {
    let mut doc = built();
    while doc.cursor > 0 {
        doc.undo().unwrap();
        assert_matches_replay(&doc);
    }
    while doc.cursor < doc.commands.len() {
        doc.redo().unwrap();
        assert_matches_replay(&doc);
    }
}

#[test]
fn a_new_command_after_undo_drops_the_replaced_checkpoints() {
    let mut doc = built();
    let target = doc.commands.len() - INTERVAL - 3;
    while doc.cursor > target {
        doc.undo().unwrap();
    }
    doc.apply(Command::RenameTask {
        uid: 2,
        name: "Nhánh mới".into(),
    })
    .unwrap();
    assert!(doc
        .checkpoints
        .iter()
        .all(|(applied, _)| applied <= doc.cursor));
    for _ in 0..INTERVAL + 2 {
        doc.apply(Command::SetDuration {
            uid: 2,
            minutes: DAY * 2,
        })
        .unwrap();
    }
    while doc.cursor > target - 2 {
        doc.undo().unwrap();
        assert_matches_replay(&doc);
    }
}

#[test]
fn opening_uses_the_saved_checkpoint_and_undo_still_matches() {
    let path = temp_path("open");
    let mut doc = built();
    doc.undo().unwrap();
    save(&doc, &path).unwrap();

    let connection = Connection::open(&path).unwrap();
    let saved: Vec<i64> = connection
        .prepare("SELECT applied FROM checkpoint ORDER BY applied")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(saved.last().copied(), Some(doc.cursor as i64));
    assert!(saved.len() >= 2, "{saved:?}");

    let mut loaded = load(&path).unwrap();
    assert_eq!(loaded.cursor, doc.cursor);
    assert_matches_replay(&loaded);
    for _ in 0..INTERVAL * 2 {
        loaded.undo().unwrap();
        assert_matches_replay(&loaded);
    }
    loaded.redo().unwrap();
    assert_matches_replay(&loaded);
}

#[test]
fn checkpoints_from_another_engine_are_ignored() {
    let path = temp_path("engine");
    let doc = built();
    save(&doc, &path).unwrap();
    let mut wrong = doc.project.clone();
    wrong.name = "Lịch của engine khác".into();
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE checkpoint SET engine = 'cũ', project = ?1",
            [serde_json::to_string(&wrong).unwrap()],
        )
        .unwrap();

    let loaded = load(&path).unwrap();
    assert_eq!(loaded.project.name, "Mốc");
    assert!(loaded
        .checkpoints
        .iter()
        .all(|(_, project)| project.name != wrong.name));
    assert_matches_replay(&loaded);
}

#[test]
fn a_damaged_checkpoint_falls_back_to_replay() {
    let path = temp_path("damaged");
    let doc = built();
    save(&doc, &path).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection
        .execute("UPDATE checkpoint SET project = '{hỏng'", [])
        .unwrap();

    let loaded = load(&path).unwrap();
    assert_matches_replay(&loaded);
}

#[test]
fn a_damaged_checkpoint_table_falls_back_to_replay() {
    let path = temp_path("table");
    let doc = built();
    save(&doc, &path).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch("DROP TABLE checkpoint; CREATE TABLE checkpoint (khac TEXT);")
        .unwrap();

    let loaded = load(&path).unwrap();
    assert!(loaded.checkpoints.iter().count() > 0);
    assert_matches_replay(&loaded);
}

#[test]
fn files_without_a_checkpoint_table_still_open() {
    let path = temp_path("v1");
    let doc = built();
    save(&doc, &path).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch("DROP TABLE checkpoint;")
        .unwrap();

    let loaded = load(&path).unwrap();
    assert_eq!(loaded.cursor, doc.commands.len());
    assert_matches_replay(&loaded);
}

//! The autosave slot and history compaction.

use std::fs;

use sheafly_engine::autosave::AutosaveSlot;
use sheafly_engine::command::Command;
use sheafly_engine::document::replay;
use sheafly_engine::store::{open, save};
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{Document, Project};

const DAY: i64 = 480;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "sheafly-autosave-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// `count` edits after a new project: tasks, then durations on them.
fn edited(count: usize) -> Document {
    let mut doc = Document::from_project(Project::new("Tự lưu", timestamp(2026, 1, 5, 8, 0)));
    for step in 0..count {
        let command = if step % 2 == 0 {
            Command::AddTask {
                name: format!("Việc {step}"),
                after_uid: None,
            }
        } else {
            Command::SetDuration {
                uid: 2 + (step as u32) / 2,
                minutes: DAY * (1 + step as i64 % 4),
            }
        };
        doc.apply(command).unwrap();
    }
    doc
}

fn json(project: &Project) -> serde_json::Value {
    serde_json::to_value(project).unwrap()
}

#[test]
fn an_autosave_restores_as_unsaved_changes_to_its_file() {
    let dir = temp_dir("restore");
    let slot = AutosaveSlot::new(dir.join("slot"));
    let mut doc = edited(12);
    let file = dir.join("du-an.pdproj").to_string_lossy().to_string();
    save(&doc, &file).unwrap();
    doc.path = Some(file.clone());
    doc.apply(Command::RenameTask {
        uid: 2,
        name: "Chưa lưu".into(),
    })
    .unwrap();

    assert_eq!(slot.pending(), None);
    slot.write(&doc, 1_000).unwrap();

    let pending = slot.pending().unwrap();
    assert_eq!(pending.original_path.as_deref(), Some(file.as_str()));
    assert_eq!(
        (pending.project_name.as_str(), pending.saved_at),
        ("Tự lưu", 1_000)
    );
    let mut restored = slot.restore().unwrap().document;
    assert_eq!(restored.path.as_deref(), Some(file.as_str()));
    assert!(restored.dirty);
    assert_eq!(json(&restored.project), json(&doc.project));
    // Undo history comes back with it.
    restored.undo().unwrap();
    assert_eq!(restored.project.task(2).unwrap().name, "Việc 0");
    // The file itself still holds the last real save.
    assert_eq!(
        open(&file).unwrap().document.project.task(2).unwrap().name,
        "Việc 0"
    );
}

#[test]
fn an_autosave_older_than_its_file_is_stale() {
    let dir = temp_dir("stale");
    let slot = AutosaveSlot::new(dir.join("slot"));
    let mut doc = edited(3);
    let file = dir.join("du-an.pdproj").to_string_lossy().to_string();
    save(&doc, &file).unwrap();
    doc.path = Some(file);

    slot.write(&doc, 0).unwrap();
    assert!(slot.pending().unwrap().is_older_than_file());
    slot.write(&doc, i64::MAX).unwrap();
    assert!(!slot.pending().unwrap().is_older_than_file());

    doc.path = None;
    slot.write(&doc, 0).unwrap();
    assert!(
        !slot.pending().unwrap().is_older_than_file(),
        "a new project has no file"
    );
}

#[test]
fn a_new_project_restores_without_a_path() {
    let slot = AutosaveSlot::new(temp_dir("untitled").join("slot"));
    slot.write(&edited(3), 5).unwrap();

    assert_eq!(slot.pending().unwrap().original_path, None);
    assert_eq!(slot.restore().unwrap().document.path, None);
}

#[test]
fn clearing_empties_the_slot_and_is_safe_to_repeat() {
    let slot = AutosaveSlot::new(temp_dir("clear").join("slot"));
    slot.clear().unwrap();
    slot.write(&edited(3), 5).unwrap();
    slot.clear().unwrap();
    slot.clear().unwrap();
    assert_eq!(slot.pending(), None);
    assert!(slot.restore().is_err());
}

#[test]
fn a_note_without_a_project_or_a_broken_note_is_ignored() {
    let dir = temp_dir("broken").join("slot");
    let slot = AutosaveSlot::new(&dir);
    slot.write(&edited(3), 5).unwrap();
    fs::write(dir.join("autosave.json"), "{hỏng").unwrap();
    assert_eq!(slot.pending(), None);

    slot.write(&edited(3), 5).unwrap();
    fs::remove_file(dir.join("autosave.pdproj")).unwrap();
    assert_eq!(slot.pending(), None);
}

#[test]
fn compacting_keeps_the_recent_undo_steps_and_the_project() {
    let original = edited(90);
    let mut doc = original.clone();
    for _ in 0..4 {
        doc.undo().unwrap();
    }
    let before = doc.project.clone();

    let folded = doc.compact_history(30).unwrap();

    assert_eq!(folded, 86 - 30);
    assert_eq!((doc.cursor, doc.commands.len()), (30, 34));
    assert!(doc.dirty);
    assert_eq!(json(&doc.project), json(&before));
    for step in 1..=30 {
        doc.undo().unwrap();
        let expected = replay(&original.base, &original.commands[..86 - step]).unwrap();
        assert_eq!(json(&doc.project), json(&expected), "{step} undos");
    }
    assert!(!doc.snapshot().can_undo);
    // Redo still reaches the commands that were undone before compacting.
    while doc.snapshot().can_redo {
        doc.redo().unwrap();
    }
    assert_eq!(json(&doc.project), json(&original.project));
}

#[test]
fn a_compacted_project_saves_and_opens_the_same() {
    let dir = temp_dir("compact-file");
    let path = dir.join("du-an.pdproj").to_string_lossy().to_string();
    let mut doc = edited(80);
    assert_eq!(doc.compact_history(10).unwrap(), 70);
    save(&doc, &path).unwrap();

    let mut opened = open(&path).unwrap().document;
    assert_eq!(opened.commands.len(), 10);
    assert_eq!(json(&opened.project), json(&doc.project));
    opened.undo().unwrap();
    doc.undo().unwrap();
    assert_eq!(json(&opened.project), json(&doc.project));
}

#[test]
fn a_short_history_is_left_alone() {
    let mut doc = edited(5);
    assert_eq!(doc.compact_history(10).unwrap(), 0);
    assert!(doc.dirty, "edits already made it dirty");
    assert_eq!(doc.commands.len(), 5);
}

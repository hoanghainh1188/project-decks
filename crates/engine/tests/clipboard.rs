//! Copy/paste of tasks, fill-down, and dragging one segment of a split task (phase 17 PR D).

use chrono::Datelike;

use sheafly_engine::clipboard::{copy_tasks, FillField};
use sheafly_engine::command::Command;
use sheafly_engine::model::{DepKind, Dependency, ResourceKind, Task};
use sheafly_engine::schedule::schedule;
use sheafly_engine::split::{Segment, Split};
use sheafly_engine::store::{load, save};
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{Document, EngineError, Invalid, Project};

/// 2026-01-05 is a Monday; `day` 0 is that Monday.
fn at(day: u32, hour: u32, minute: u32) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(i64::from(day)))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, minute)
}

/// Tasks A (1 day), B (2 days), C (1 day); A → B linked FS.
fn doc() -> Document {
    let mut project = Project::new("Dán", at(0, 8, 0));
    for (uid, name, duration) in [(1, "A", 480), (2, "B", 960), (3, "C", 480)] {
        let mut task = Task::new(uid, name);
        task.duration = duration;
        task.work = duration;
        task.remaining_duration = duration;
        task.remaining_work = duration;
        project.tasks.push(task);
    }
    project.next_task_uid = 4;
    project.dependencies.push(Dependency {
        uid: 1,
        predecessor_uid: 1,
        successor_uid: 2,
        kind: DepKind::Fs,
        lag: 0,
    });
    project.next_dependency_uid = 2;
    schedule(&mut project).unwrap();
    Document::from_project(project)
}

fn names(doc: &Document) -> Vec<&str> {
    doc.project
        .tasks
        .iter()
        .map(|task| task.name.as_str())
        .collect()
}

#[test]
fn paste_inserts_copies_after_the_task_and_keeps_links_between_them() {
    let mut doc = doc();
    doc.apply(Command::SetNotes {
        uid: 1,
        notes: "ghi chú".into(),
    })
    .unwrap();
    let clipboard = copy_tasks(&doc.project, &[1, 2]);
    assert_eq!(clipboard.tasks.len(), 2);
    assert_eq!(clipboard.links.len(), 1);
    doc.apply(Command::PasteTasks {
        after_uid: Some(3),
        clipboard,
    })
    .unwrap();
    assert_eq!(names(&doc), ["A", "B", "C", "A", "B"]);
    let copy_a = &doc.project.tasks[3];
    let copy_b = &doc.project.tasks[4];
    assert_eq!(copy_a.notes, "ghi chú");
    assert_eq!(copy_b.duration, 960);
    assert!(doc
        .project
        .dependencies
        .iter()
        .any(|dep| dep.predecessor_uid == copy_a.uid && dep.successor_uid == copy_b.uid));
    // The copy of B follows the copy of A, not the original A.
    assert!(copy_b.start >= copy_a.finish);
}

#[test]
fn copy_takes_the_subtree_and_assignments() {
    let mut doc = doc();
    doc.apply(Command::RemoveDependency { uid: 1 }).unwrap();
    doc.apply(Command::Indent { uid: 2 }).unwrap();
    doc.apply(Command::AddResource {
        name: "An".into(),
        kind: ResourceKind::Person,
    })
    .unwrap();
    let resource = doc.project.resources[0].uid;
    doc.apply(Command::AssignResource {
        task_uid: 2,
        resource_uid: resource,
        units: 1.0,
    })
    .unwrap();
    let clipboard = copy_tasks(&doc.project, &[1]);
    assert_eq!(
        clipboard.tasks.len(),
        2,
        "A is a summary now and takes B along"
    );
    doc.apply(Command::PasteTasks {
        after_uid: Some(1),
        clipboard,
    })
    .unwrap();
    assert_eq!(names(&doc), ["A", "B", "A", "B", "C"]);
    let copy_a = &doc.project.tasks[2];
    let copy_b = &doc.project.tasks[3];
    assert!(copy_a.summary);
    assert_eq!(copy_b.outline_level, 2);
    let assigned = doc
        .project
        .assignments
        .iter()
        .filter(|a| a.task_uid == copy_b.uid)
        .count();
    assert_eq!(assigned, 1);
}

#[test]
fn paste_replays_after_undo_and_from_a_file() {
    let mut doc = doc();
    let clipboard = copy_tasks(&doc.project, &[2]);
    doc.apply(Command::PasteTasks {
        after_uid: Some(1),
        clipboard,
    })
    .unwrap();
    doc.undo().unwrap();
    assert_eq!(names(&doc), ["A", "B", "C"]);
    doc.redo().unwrap();
    assert_eq!(names(&doc), ["A", "B", "B", "C"]);
    let dir = std::env::temp_dir().join(format!("sheafly-paste-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("paste.sheafly").to_string_lossy().to_string();
    save(&doc, &path).unwrap();
    let opened = load(&path).unwrap();
    assert_eq!(opened.project.tasks, doc.project.tasks);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn paste_of_nothing_is_refused() {
    let mut doc = doc();
    let empty = copy_tasks(&doc.project, &[]);
    let error = doc.apply(Command::PasteTasks {
        after_uid: None,
        clipboard: empty,
    });
    assert!(matches!(
        error,
        Err(EngineError::Invalid(Invalid::NothingToPaste))
    ));
}

#[test]
fn fill_down_copies_the_field_in_one_undo_step() {
    let mut doc = doc();
    doc.apply(Command::FillDown {
        from_uid: 1,
        to_uids: vec![2, 3],
        field: FillField::Duration,
    })
    .unwrap();
    assert!(doc.project.tasks.iter().all(|task| task.duration == 480));
    doc.apply(Command::FillDown {
        from_uid: 1,
        to_uids: vec![2, 3],
        field: FillField::Name,
    })
    .unwrap();
    assert_eq!(names(&doc), ["A", "A", "A"]);
    doc.undo().unwrap();
    assert_eq!(names(&doc), ["A", "B", "C"]);
    assert_eq!(doc.project.task(2).unwrap().duration, 480);
}

#[test]
fn fill_down_of_predecessors_links_the_rows_below() {
    let mut doc = doc();
    doc.apply(Command::FillDown {
        from_uid: 2,
        to_uids: vec![3],
        field: FillField::Predecessors,
    })
    .unwrap();
    assert_eq!(
        doc.project.task(3).unwrap().predecessors,
        doc.project.task(2).unwrap().predecessors
    );
    assert!(doc.project.task(3).unwrap().start >= doc.project.task(1).unwrap().finish);
}

/// Two-day task A split at Monday 13:00 for one working day.
fn split_doc() -> Document {
    let mut doc = doc();
    doc.apply(Command::SplitTask {
        uid: 2,
        at: at(1, 13, 0),
        gap: 480,
    })
    .unwrap();
    doc
}

#[test]
fn moving_a_segment_changes_the_gap_before_it() {
    let mut doc = split_doc();
    let b = doc.project.task(2).unwrap();
    assert_eq!(
        b.segments[1],
        Segment {
            start: at(2, 13, 0),
            finish: at(3, 17, 0)
        }
    );
    // Drag the second piece one working day later.
    doc.apply(Command::MoveSegment {
        uid: 2,
        index: 1,
        start: at(3, 13, 0),
    })
    .unwrap();
    let b = doc.project.task(2).unwrap();
    assert_eq!(
        b.splits,
        vec![Split {
            offset: 240,
            gap: 960
        }]
    );
    assert_eq!(
        b.segments[0],
        Segment {
            start: at(1, 8, 0),
            finish: at(1, 12, 0)
        }
    );
    assert_eq!(
        b.segments[1],
        Segment {
            start: at(3, 13, 0),
            finish: at(4, 17, 0)
        }
    );
    assert_eq!(b.duration, 960);
}

#[test]
fn moving_a_segment_onto_the_one_before_joins_them() {
    let mut doc = split_doc();
    doc.apply(Command::MoveSegment {
        uid: 2,
        index: 1,
        start: at(1, 9, 0),
    })
    .unwrap();
    let b = doc.project.task(2).unwrap();
    assert!(b.splits.is_empty());
    assert_eq!(b.finish, at(2, 17, 0));
}

#[test]
fn the_first_segment_does_not_move_on_its_own() {
    let mut doc = split_doc();
    let error = doc.apply(Command::MoveSegment {
        uid: 2,
        index: 0,
        start: at(3, 8, 0),
    });
    assert!(matches!(
        error,
        Err(EngineError::Invalid(Invalid::FirstSegment))
    ));
    let error = doc.apply(Command::MoveSegment {
        uid: 2,
        index: 2,
        start: at(5, 8, 0),
    });
    assert!(matches!(
        error,
        Err(EngineError::Invalid(Invalid::NoSuchSplit))
    ));
}

#[test]
fn a_moved_segment_replays_the_same_after_undo() {
    let mut doc = split_doc();
    doc.apply(Command::MoveSegment {
        uid: 2,
        index: 1,
        start: at(4, 8, 0),
    })
    .unwrap();
    let moved = doc.project.task(2).unwrap().clone();
    doc.undo().unwrap();
    doc.redo().unwrap();
    assert_eq!(doc.project.task(2).unwrap(), &moved);
}

#[test]
fn paste_into_another_project_matches_resources_by_name() {
    let mut source = doc();
    source
        .apply(Command::AddResource {
            name: "An".into(),
            kind: ResourceKind::Person,
        })
        .unwrap();
    source
        .apply(Command::AddResource {
            name: "Bình".into(),
            kind: ResourceKind::Person,
        })
        .unwrap();
    let binh = source.project.resources[1].uid;
    source
        .apply(Command::AssignResource {
            task_uid: 1,
            resource_uid: binh,
            units: 1.0,
        })
        .unwrap();
    let clipboard = copy_tasks(&source.project, &[1]);

    // Here "Bình" has the UID "An" had there, and there is no "An".
    let mut target = doc();
    target
        .apply(Command::AddResource {
            name: "Bình".into(),
            kind: ResourceKind::Person,
        })
        .unwrap();
    let here = target.project.resources[0].uid;
    target
        .apply(Command::PasteTasks {
            after_uid: None,
            clipboard: clipboard.clone(),
        })
        .unwrap();
    let pasted = target.project.tasks.last().unwrap().uid;
    let assigned: Vec<u32> = target
        .project
        .assignments
        .iter()
        .filter(|a| a.task_uid == pasted)
        .map(|a| a.resource_uid)
        .collect();
    assert_eq!(assigned, vec![here]);

    // With no matching resource the assignment and its work are left out.
    let mut empty = doc();
    empty
        .apply(Command::PasteTasks {
            after_uid: None,
            clipboard,
        })
        .unwrap();
    let pasted = empty.project.tasks.last().unwrap();
    assert!(empty
        .project
        .assignments
        .iter()
        .all(|a| a.task_uid != pasted.uid));
    assert_eq!(pasted.duration, 480);
}

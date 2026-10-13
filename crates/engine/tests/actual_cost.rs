//! Hand-entered actual costs: replayable, undoable, and kept across save and open.

use sheafly_engine::command::Command;
use sheafly_engine::store::{load, save};
use sheafly_engine::Document;

fn entered() -> Document {
    let mut doc = Document::new_default();
    doc.apply(Command::SetFixedCost {
        uid: 1,
        cost: 1000.0,
        accrual: sheafly_engine::model::Accrual::Start,
    })
    .unwrap();
    doc.apply(Command::SetEditableActualCosts { editable: true })
        .unwrap();
    doc.apply(Command::SetTaskActualCost {
        uid: 1,
        cost: Some(300.0),
    })
    .unwrap();
    doc
}

#[test]
fn an_entered_cost_replaces_the_computed_one_and_undo_restores_it() {
    let mut doc = entered();
    let task = doc.project.task(1).unwrap();
    assert_eq!((task.actual_cost, task.remaining_cost), (300.0, 700.0));
    doc.undo().unwrap();
    assert_eq!(
        doc.project.task(1).unwrap().actual_cost,
        0.0,
        "entry undone, flag still on"
    );
    doc.undo().unwrap();
    assert!(!doc.project.editable_actual_costs);
    doc.redo().unwrap();
    doc.redo().unwrap();
    assert_eq!(doc.project.task(1).unwrap().actual_cost, 300.0);
}

#[test]
fn a_cost_above_the_total_leaves_nothing_remaining_and_none_goes_back_to_computed() {
    let mut doc = entered();
    doc.apply(Command::SetTaskActualCost {
        uid: 1,
        cost: Some(5000.0),
    })
    .unwrap();
    assert_eq!(doc.project.task(1).unwrap().remaining_cost, 0.0);
    doc.apply(Command::SetTaskActualCost { uid: 1, cost: None })
        .unwrap();
    assert_eq!(
        doc.project.task(1).unwrap().actual_cost,
        0.0,
        "no progress: nothing computed yet"
    );
}

#[test]
fn entries_survive_save_and_open() {
    let dir = std::env::temp_dir().join(format!("sheafly-ac-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("cost.sheafly").to_string_lossy().into_owned();
    let doc = entered();
    save(&doc, &path).unwrap();
    let back = load(&path).unwrap();
    assert!(back.project.editable_actual_costs);
    assert_eq!(
        back.project.task(1).unwrap().manual_actual_cost,
        Some(300.0)
    );
    assert_eq!(back.project.task(1).unwrap().actual_cost, 300.0);
    std::fs::remove_dir_all(dir).ok();
}

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

fn with_assignment() -> Document {
    use sheafly_engine::model::ResourceKind;
    let mut doc = Document::new_default();
    doc.apply(Command::AddResource {
        name: "An".into(),
        kind: ResourceKind::Person,
    })
    .unwrap();
    doc.apply(Command::UpdateResource {
        uid: 1,
        name: "An".into(),
        kind: ResourceKind::Person,
        max_units: 1.0,
        calendar_id: None,
        standard_rate: 100.0,
        overtime_rate: 0.0,
        cost_per_use: 0.0,
    })
    .unwrap();
    doc.apply(Command::AssignResource {
        task_uid: 1,
        resource_uid: 1,
        units: 1.0,
    })
    .unwrap();
    doc.apply(Command::SetFixedCost {
        uid: 1,
        cost: 200.0,
        accrual: sheafly_engine::model::Accrual::Prorated,
    })
    .unwrap();
    doc.apply(Command::SetPercentComplete {
        uid: 1,
        percent: 50.0,
    })
    .unwrap();
    doc
}

#[test]
fn an_assignment_entry_replaces_its_share_and_the_task_total_follows() {
    let mut doc = with_assignment();
    // 8 h at 100 = 800 labour + 200 fixed; half done: 400 + 100.
    assert_eq!(doc.project.task(1).unwrap().actual_cost, 500.0);
    doc.apply(Command::SetAssignmentActualCost {
        uid: 1,
        cost: Some(650.0),
    })
    .unwrap();
    assert_eq!(
        doc.project.assignments[0].actual_cost, 400.0,
        "ignored while costs are computed"
    );
    doc.apply(Command::SetEditableActualCosts { editable: true })
        .unwrap();
    assert_eq!(doc.project.assignments[0].actual_cost, 650.0);
    assert_eq!(
        doc.project.task(1).unwrap().actual_cost,
        750.0,
        "650 entered + 100 fixed"
    );
    assert_eq!(doc.project.task(1).unwrap().remaining_cost, 250.0);
    // The task's own entry wins over the assignments.
    doc.apply(Command::SetTaskActualCost {
        uid: 1,
        cost: Some(900.0),
    })
    .unwrap();
    assert_eq!(doc.project.task(1).unwrap().actual_cost, 900.0);
    doc.apply(Command::SetTaskActualCost { uid: 1, cost: None })
        .unwrap();
    doc.apply(Command::SetAssignmentActualCost { uid: 1, cost: None })
        .unwrap();
    assert_eq!(doc.project.task(1).unwrap().actual_cost, 500.0);
    doc.undo().unwrap();
    assert_eq!(doc.project.assignments[0].actual_cost, 650.0);
    let missing = doc.apply(Command::SetAssignmentActualCost {
        uid: 99,
        cost: Some(1.0),
    });
    assert!(missing.is_err());
}

#[test]
fn assignment_entries_round_trip_through_mspdi_only_when_editable() {
    use sheafly_engine::mspdi::{read_mspdi, write_mspdi};
    let mut doc = with_assignment();
    doc.apply(Command::SetEditableActualCosts { editable: true })
        .unwrap();
    doc.apply(Command::SetAssignmentActualCost {
        uid: 1,
        cost: Some(650.0),
    })
    .unwrap();
    let back = read_mspdi(&write_mspdi(&doc.project).unwrap()).unwrap();
    assert_eq!(back.assignments[0].manual_actual_cost, Some(650.0));
    assert_eq!(back.assignments[0].actual_cost, 650.0);
    assert_eq!(back.task(1).unwrap().actual_cost, 750.0);
    doc.apply(Command::SetEditableActualCosts { editable: false })
        .unwrap();
    let back = read_mspdi(&write_mspdi(&doc.project).unwrap()).unwrap();
    assert_eq!(back.assignments[0].manual_actual_cost, None);
}

#[test]
fn a_file_whose_actual_costs_are_the_computed_ones_gives_no_entries() {
    use sheafly_engine::mspdi::{read_mspdi, write_mspdi};
    let mut doc = with_assignment();
    doc.apply(Command::SetEditableActualCosts { editable: true })
        .unwrap();
    let back = read_mspdi(&write_mspdi(&doc.project).unwrap()).unwrap();
    assert!(back.editable_actual_costs);
    assert_eq!(back.task(1).unwrap().manual_actual_cost, None);
    assert_eq!(back.assignments[0].manual_actual_cost, None);
    assert_eq!(back.task(1).unwrap().actual_cost, 500.0);
}

#[test]
fn a_task_entry_makes_the_assignments_share_it() {
    let mut doc = with_assignment();
    doc.apply(Command::SetEditableActualCosts { editable: true })
        .unwrap();
    doc.apply(Command::SetAssignmentActualCost {
        uid: 1,
        cost: Some(300.0),
    })
    .unwrap();
    doc.apply(Command::SetTaskActualCost {
        uid: 1,
        cost: Some(100.0),
    })
    .unwrap();
    let task = doc.project.task(1).unwrap();
    assert_eq!(task.actual_cost, 100.0);
    assert!(doc.project.assignments[0].actual_cost <= task.actual_cost);
}

#[test]
fn an_occurrence_with_an_assignment_entry_survives_regenerating_its_series() {
    use sheafly_engine::model::ResourceKind;
    use sheafly_engine::recurrence::{RecurrenceEnd, RecurrencePattern, RecurrenceSpec};
    let mut doc = Document::new_default();
    let start = doc.project.start;
    let spec = RecurrenceSpec {
        name: "Họp".into(),
        duration: 60,
        start,
        pattern: RecurrencePattern::Weekly {
            every: 1,
            weekdays: vec![0],
        },
        end: RecurrenceEnd::Count { count: 3 },
        calendar_id: None,
        resource_uids: Vec::new(),
        resources: Vec::new(),
        background: false,
    };
    doc.apply(Command::CreateRecurringTask {
        after_uid: None,
        spec: spec.clone(),
    })
    .unwrap();
    doc.apply(Command::AddResource {
        name: "An".into(),
        kind: ResourceKind::Person,
    })
    .unwrap();
    let occurrence = doc
        .project
        .tasks
        .iter()
        .find(|task| task.series_uid.is_some())
        .unwrap()
        .uid;
    doc.apply(Command::AssignResource {
        task_uid: occurrence,
        resource_uid: 1,
        units: 1.0,
    })
    .unwrap();
    doc.apply(Command::SetEditableActualCosts { editable: true })
        .unwrap();
    let assignment = doc.project.assignments[0].uid;
    doc.apply(Command::SetAssignmentActualCost {
        uid: assignment,
        cost: Some(80.0),
    })
    .unwrap();
    let summary = doc
        .project
        .tasks
        .iter()
        .find(|task| task.recurrence.is_some())
        .unwrap()
        .uid;
    doc.apply(Command::UpdateRecurringTask {
        uid: summary,
        spec: RecurrenceSpec {
            name: "Họp tuần".into(),
            ..spec
        },
    })
    .unwrap();
    assert!(doc.project.task(occurrence).is_some());
    assert_eq!(doc.project.assignments[0].manual_actual_cost, Some(80.0));
}

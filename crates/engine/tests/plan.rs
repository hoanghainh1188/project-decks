//! Planning preview (phase 18 P3, step 3c): `preview_plan` assigns the chosen people and
//! levels on a copy, and `ApplyPlan` commits exactly that as one undo step.

use chrono::Datelike;

use sheafly_engine::command::Command;
use sheafly_engine::model::{ResourceKind, Task, TaskType};
use sheafly_engine::store::{load, save};
use sheafly_engine::plan::{preview_plan, PlanMove};
use sheafly_engine::skills::TaskAssignment;
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{Document, EngineError, Invalid, Project};

const DAY: i64 = 480;

fn at(day: u32, hour: u32, minute: u32) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(i64::from(day)))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, minute)
}

/// Tasks 1..=`tasks` of one day of work each (Fixed Work) and people 1..=`people`.
fn doc(tasks: u32, people: u32) -> Document {
    let mut project = Project::new("Kế hoạch", at(0, 8, 0));
    for uid in 1..=tasks {
        let mut task = Task::new(uid, format!("Việc {uid}"));
        task.task_type = TaskType::FixedWork;
        task.effort_driven = true;
        task.duration = DAY;
        task.work = DAY;
        task.remaining_duration = DAY;
        task.remaining_work = DAY;
        project.tasks.push(task);
    }
    project.next_task_uid = tasks + 1;
    let mut doc = Document::from_project(project);
    for person in 1..=people {
        doc.apply(Command::AddResource {
            name: format!("Người {person}"),
            kind: ResourceKind::Person,
        })
        .unwrap();
    }
    doc
}

fn to(task_uid: u32, resource_uid: u32) -> TaskAssignment {
    TaskAssignment {
        task_uid,
        resource_uid,
        units: 1.0,
    }
}

#[test]
fn assigns_then_levels_on_a_copy_and_lists_what_moves() {
    let doc = doc(3, 1);
    let before = doc.project.clone();
    let preview = preview_plan(&doc.project, &[to(1, 1), to(2, 1), to(3, 1)]).unwrap();
    assert_eq!(doc.project, before);
    assert_eq!(preview.assignments.len(), 3);
    // One person, three one-day tasks: two of them wait.
    assert_eq!(preview.delays.len(), 2);
    let moved: Vec<u32> = preview.moves.iter().map(|change| change.uid).collect();
    assert_eq!(moved.len(), 2);
    let last = preview.moves.iter().max_by_key(|change| change.finish).unwrap();
    assert_eq!(
        *last,
        PlanMove {
            uid: last.uid,
            start_before: at(0, 8, 0),
            finish_before: at(0, 17, 0),
            start: at(2, 8, 0),
            finish: at(2, 17, 0),
        }
    );
    assert!(preview.overallocated.is_empty());
}

#[test]
fn lists_tasks_that_end_after_their_deadline_and_people_left_overallocated() {
    let mut started = doc(2, 1);
    // Started work is never moved, so the clash stays.
    started.apply(Command::AssignResource { task_uid: 1, resource_uid: 1, units: 1.0 }).unwrap();
    started.apply(Command::SetActualStart { uid: 1, start: Some(at(0, 8, 0)) }).unwrap();
    started.apply(Command::AssignResource { task_uid: 2, resource_uid: 1, units: 1.0 }).unwrap();
    started.apply(Command::SetActualStart { uid: 2, start: Some(at(0, 8, 0)) }).unwrap();
    let preview = preview_plan(&started.project, &[]).unwrap();
    assert_eq!(preview.overallocated, vec![1]);
    let mut late = doc(2, 2);
    late.apply(Command::SetDeadline { uid: 2, date: Some(at(0, 12, 0)) }).unwrap();
    let preview = preview_plan(&late.project, &[to(1, 1), to(2, 1)]).unwrap();
    let late_uid = preview.moves.iter().map(|change| change.uid).next().unwrap();
    assert_eq!(preview.late, vec![late_uid]);
}

#[test]
fn apply_plan_matches_the_preview_in_one_undo_step() {
    let mut doc = doc(4, 2);
    let before = doc.project.clone();
    let preview = preview_plan(&doc.project, &[to(1, 1), to(2, 1), to(3, 2), to(4, 2)]).unwrap();
    doc.apply(Command::ApplyPlan {
        assignments: preview.assignments.clone(),
        delays: preview.delays.clone(),
        splits: preview.splits.clone(),
    })
    .unwrap();
    for change in &preview.moves {
        let task = doc.project.task(change.uid).unwrap();
        assert_eq!((task.start, task.finish), (change.start, change.finish));
    }
    assert_eq!(doc.project.assignments.len(), 4);
    assert!(doc.project.overallocations.is_empty());
    doc.undo().unwrap();
    assert_eq!(doc.project, before);
    let after = {
        doc.redo().unwrap();
        doc.project.clone()
    };
    assert_eq!(doc.project, after);
}

#[test]
fn levels_without_new_assignments_and_refuses_an_empty_plan() {
    let mut doc = doc(2, 1);
    doc.apply(Command::AssignResource { task_uid: 1, resource_uid: 1, units: 1.0 }).unwrap();
    doc.apply(Command::AssignResource { task_uid: 2, resource_uid: 1, units: 1.0 }).unwrap();
    let preview = preview_plan(&doc.project, &[]).unwrap();
    assert!(preview.assignments.is_empty());
    assert_eq!(preview.delays.len(), 1);
    let refused = doc.apply(Command::ApplyPlan { assignments: vec![], delays: vec![], splits: vec![] });
    assert_eq!(refused.map(|_| ()), Err(EngineError::Invalid(Invalid::EmptyPlan)));
}

#[test]
fn a_bad_assignment_fails_the_preview() {
    let doc = doc(1, 1);
    assert_eq!(preview_plan(&doc.project, &[to(1, 9)]).map(|_| ()), Err(EngineError::ResourceNotFound));
}

/// The same plan done in two steps the way the app did before: assign, then level.
fn two_steps(doc: &Document, assignments: &[TaskAssignment]) -> Project {
    let mut other = Document::from_project(doc.project.clone());
    other.apply(Command::AssignTasks { assignments: assignments.to_vec() }).unwrap();
    other.apply(Command::LevelResources).unwrap();
    other.project
}

#[test]
fn apply_plan_gives_the_same_project_as_assigning_then_leveling_with_urgent_work() {
    let mut doc = doc(4, 1);
    // Task 1 is under way; task 4, urgent and ranked higher, splits it when leveled.
    doc.apply(Command::AssignResource { task_uid: 1, resource_uid: 1, units: 1.0 }).unwrap();
    doc.apply(Command::SetDuration { uid: 1, minutes: 3 * DAY }).unwrap();
    doc.apply(Command::SetActualStart { uid: 1, start: Some(at(0, 8, 0)) }).unwrap();
    doc.apply(Command::SetInterrupts { uid: 4, interrupts: true }).unwrap();
    doc.apply(Command::SetPriority { uid: 4, priority: 900 }).unwrap();
    doc.apply(Command::SetConstraint {
        uid: 4,
        constraint_type: sheafly_engine::model::ConstraintType::Snet,
        date: Some(at(1, 8, 0)),
    })
    .unwrap();
    let chosen = [to(2, 1), to(3, 1), to(4, 1)];
    let expected = two_steps(&doc, &chosen);
    let preview = preview_plan(&doc.project, &chosen).unwrap();
    assert!(!preview.splits.is_empty(), "the urgent task should split task 1");
    doc.apply(Command::ApplyPlan {
        assignments: preview.assignments,
        delays: preview.delays,
        splits: preview.splits,
    })
    .unwrap();
    assert_eq!(doc.project.tasks, expected.tasks);
    assert_eq!(doc.project.assignments, expected.assignments);

    let dir = std::env::temp_dir().join(format!("sheafly-plan-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("plan.sheafly").to_string_lossy().to_string();
    save(&doc, &path).unwrap();
    let opened = load(&path).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(opened.project, doc.project);
}

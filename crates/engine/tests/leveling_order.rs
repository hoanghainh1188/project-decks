//! The leveling order (phase 18 P2, step 2): which task leveling places first, by a list
//! of criteria the project keeps. Every test puts one-day tasks on one person, so the
//! order leveling placed them in is the order of their starts.

use chrono::Datelike;

use sheafly_engine::command::Command;
use sheafly_engine::fields::{FieldDef, FieldKind, FieldValue};
use sheafly_engine::level_order::{Criterion, SortKey, MAX_SORT_KEYS};
use sheafly_engine::model::{ConstraintType, DepKind, ResourceKind, Task};
use sheafly_engine::store::{load, save};
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{Document, EngineError, Invalid, Project};

const DAY: i64 = 480;

/// 2026-01-05 is a Monday; `day` 0 is that Monday.
fn at(day: u32, hour: u32, minute: u32) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(i64::from(day)))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, minute)
}

/// Tasks with these uids, in this outline order, of one day each, all on one person.
fn project_of(uids: &[u32]) -> Project {
    let mut project = Project::new("Thứ tự", at(0, 8, 0));
    for &uid in uids {
        let mut task = Task::new(uid, format!("Việc {uid}"));
        task.duration = DAY;
        task.work = DAY;
        task.remaining_duration = DAY;
        task.remaining_work = DAY;
        project.tasks.push(task);
    }
    project.next_task_uid = uids.iter().max().copied().unwrap_or(0) + 1;
    project
}

/// `project` with one person on every task.
fn staffed(project: Project) -> Document {
    let mut doc = Document::from_project(project);
    let name = "An".to_string();
    doc.apply(Command::AddResource { name, kind: ResourceKind::Person }).unwrap();
    let uids: Vec<u32> = doc.project.tasks.iter().map(|task| task.uid).collect();
    for task_uid in uids {
        doc.apply(Command::AssignResource { task_uid, resource_uid: 1, units: 1.0 }).unwrap();
    }
    doc
}

fn doc_of(uids: &[u32]) -> Document {
    staffed(project_of(uids))
}

fn key(criterion: Criterion, descending: bool) -> SortKey {
    SortKey { criterion, descending }
}

fn asc(criterion: Criterion) -> SortKey {
    key(criterion, false)
}

fn desc(criterion: Criterion) -> SortKey {
    key(criterion, true)
}

fn set_order(doc: &mut Document, order: Vec<SortKey>) -> Result<(), EngineError> {
    doc.apply(Command::SetLevelingOrder { order }).map(|_| ())
}

fn priority(doc: &mut Document, uid: u32, priority: i32) {
    doc.apply(Command::SetPriority { uid, priority }).unwrap();
}

/// Level, then the uids by start: the order leveling placed them in.
fn placed(doc: &mut Document) -> Vec<u32> {
    doc.apply(Command::LevelResources).unwrap();
    let mut tasks: Vec<&Task> = doc.project.tasks.iter().filter(|task| !task.summary).collect();
    tasks.sort_by_key(|task| (task.start, task.uid));
    tasks.iter().map(|task| task.uid).collect()
}

fn delays(doc: &Document) -> Vec<(u32, i64, i64)> {
    doc.project
        .tasks
        .iter()
        .map(|task| (task.uid, task.leveling_delay, task.start))
        .collect()
}

fn invalid(result: Result<(), EngineError>) -> Invalid {
    match result {
        Err(EngineError::Invalid(reason)) => reason,
        other => panic!("expected a validation error, got {other:?}"),
    }
}

fn add_field(doc: &mut Document, name: &str, kind: FieldKind) -> u32 {
    doc.apply(Command::AddField { name: name.into(), kind }).unwrap();
    doc.project.fields.last().unwrap().id
}

fn set_value(doc: &mut Document, uid: u32, field_id: u32, value: FieldValue) {
    doc.apply(Command::SetFieldValue { uid, field_id, value: Some(value) }).unwrap();
}

fn choice(options: &[&str]) -> FieldKind {
    FieldKind::Choice {
        options: options.iter().map(|option| option.to_string()).collect(),
    }
}

fn text(value: &str) -> FieldValue {
    FieldValue::Text(value.into())
}

/// A mix where every default criterion matters: starts held back by dates and links,
/// priorities, and slack from a chain with a late deadline task after it.
fn mixed() -> Document {
    let mut doc = doc_of(&[1, 2, 3, 4, 5, 6, 7]);
    for (uid, value) in [(1, 500), (2, 700), (3, 500), (4, 300), (5, 700), (6, 500), (7, 500)] {
        priority(&mut doc, uid, value);
    }
    let snet = |uid, date| Command::SetConstraint {
        uid,
        constraint_type: ConstraintType::Snet,
        date: Some(date),
    };
    doc.apply(snet(4, at(1, 8, 0))).unwrap();
    doc.apply(snet(5, at(1, 8, 0))).unwrap();
    for (predecessor_uid, successor_uid) in [(1, 2), (6, 7)] {
        let link = Command::AddDependency { predecessor_uid, successor_uid, kind: DepKind::Fs, lag: 0 };
        doc.apply(link).unwrap();
    }
    doc.apply(Command::SetDuration { uid: 3, minutes: 3 * DAY }).unwrap();
    doc
}

#[test]
fn the_default_order_spelled_out_levels_like_the_empty_list() {
    let mut empty = mixed();
    assert!(empty.project.leveling_order.is_empty());
    let mut spelled = mixed();
    let default = vec![asc(Criterion::Start), desc(Criterion::Priority), asc(Criterion::Slack)];
    set_order(&mut spelled, default.clone()).unwrap();
    assert_eq!(spelled.project.leveling_order, default);
    let order = placed(&mut empty);
    assert_eq!(order, placed(&mut spelled));
    assert_eq!(delays(&empty), delays(&spelled));
    // The mix is worth comparing only if leveling moved several tasks.
    let moved = empty.project.tasks.iter().filter(|task| task.leveling_delay > 0).count();
    assert!(moved >= 4, "only {moved} moved: {order:?}");
}

#[test]
fn setting_the_order_does_not_level_again() {
    let mut doc = mixed();
    doc.apply(Command::LevelResources).unwrap();
    let before = delays(&doc);
    set_order(&mut doc, vec![asc(Criterion::Id)]).unwrap();
    assert_eq!(delays(&doc), before);
    doc.apply(Command::LevelResources).unwrap();
    assert_ne!(delays(&doc), before, "leveling again follows the new order");
}

#[test]
fn priority_first_places_an_important_task_that_starts_later() {
    let mut doc = doc_of(&[1, 2]);
    doc.apply(Command::SetDuration { uid: 1, minutes: 2 * DAY }).unwrap();
    priority(&mut doc, 2, 900);
    let snet = Command::SetConstraint {
        uid: 2,
        constraint_type: ConstraintType::Snet,
        date: Some(at(1, 8, 0)),
    };
    doc.apply(snet).unwrap();

    // By default the earlier start goes first and pushes the important task back.
    assert_eq!(placed(&mut doc), vec![1, 2]);
    assert_eq!(doc.project.task(2).unwrap().start, at(2, 8, 0));

    set_order(&mut doc, vec![desc(Criterion::Priority), asc(Criterion::Start)]).unwrap();
    assert_eq!(placed(&mut doc), vec![2, 1]);
    assert_eq!(doc.project.task(2).unwrap().start, at(1, 8, 0));
    assert_eq!(doc.project.task(1).unwrap().start, at(2, 8, 0));
}

#[test]
fn a_later_start_can_go_first() {
    let mut doc = doc_of(&[1, 2]);
    doc.apply(Command::SetDuration { uid: 1, minutes: 2 * DAY }).unwrap();
    let snet = Command::SetConstraint {
        uid: 2,
        constraint_type: ConstraintType::Snet,
        date: Some(at(1, 8, 0)),
    };
    doc.apply(snet).unwrap();
    set_order(&mut doc, vec![desc(Criterion::Start)]).unwrap();
    assert_eq!(placed(&mut doc), vec![2, 1]);
    assert_eq!(doc.project.task(1).unwrap().start, at(2, 8, 0));
}

#[test]
fn choices_go_by_their_list_order_and_missing_values_last_both_ways() {
    let mut project = project_of(&[1, 2, 3, 4, 5]);
    project.fields.push(FieldDef { id: 1, name: "Mức".into(), kind: choice(&["Cao", "Vừa", "Thấp"]) });
    project.next_field_id = 2;
    // Task 3 has none, and task 5 a value the list does not have (a damaged file).
    for (uid, value) in [(1, "Thấp"), (2, "Cao"), (4, "Vừa"), (5, "Khác")] {
        project.tasks[uid - 1].values.insert(1, text(value));
    }
    let mut doc = staffed(project);
    let field = Criterion::Field { id: 1 };
    set_order(&mut doc, vec![asc(field)]).unwrap();
    assert_eq!(placed(&mut doc), vec![2, 4, 1, 3, 5]);
    set_order(&mut doc, vec![desc(field)]).unwrap();
    assert_eq!(placed(&mut doc), vec![1, 4, 2, 3, 5]);
}

#[test]
fn numbers_go_by_value_either_way() {
    let mut doc = doc_of(&[1, 2, 3, 4]);
    let field = add_field(&mut doc, "Điểm", FieldKind::Number);
    for (uid, value) in [(1, 1.0), (2, 5.0), (4, -3.5)] {
        set_value(&mut doc, uid, field, FieldValue::Number(value));
    }
    let field = Criterion::Field { id: field };
    set_order(&mut doc, vec![desc(field)]).unwrap();
    assert_eq!(placed(&mut doc), vec![2, 1, 4, 3]);
    set_order(&mut doc, vec![asc(field)]).unwrap();
    assert_eq!(placed(&mut doc), vec![4, 1, 2, 3]);
}

#[test]
fn text_goes_by_code_point_not_by_language() {
    let mut doc = doc_of(&[1, 2, 3, 4]);
    let field = add_field(&mut doc, "Mã", FieldKind::Text);
    // Code points: "B" < "a" < "b" < "Đ" (U+0110), whatever a locale would say.
    for (uid, value) in [(1, "b"), (2, "Đ"), (3, "a"), (4, "B")] {
        set_value(&mut doc, uid, field, text(value));
    }
    set_order(&mut doc, vec![asc(Criterion::Field { id: field })]).unwrap();
    assert_eq!(placed(&mut doc), vec![4, 3, 1, 2]);
}

#[test]
fn a_later_criterion_breaks_ties_of_an_earlier_one() {
    let mut doc = doc_of(&[1, 2, 3, 4]);
    let field = add_field(&mut doc, "Nhóm", choice(&["A", "B"]));
    for (uid, value) in [(1, "B"), (2, "A"), (3, "B"), (4, "A")] {
        set_value(&mut doc, uid, field, text(value));
    }
    for (uid, value) in [(1, 300), (2, 300), (3, 900), (4, 900)] {
        priority(&mut doc, uid, value);
    }
    let field = Criterion::Field { id: field };
    set_order(&mut doc, vec![asc(field), desc(Criterion::Priority)]).unwrap();
    assert_eq!(placed(&mut doc), vec![4, 2, 3, 1]);
}

#[test]
fn id_order_follows_the_outline() {
    // Uids out of outline order, priorities against it.
    let mut doc = doc_of(&[3, 1, 2]);
    for (uid, value) in [(3, 100), (1, 500), (2, 900)] {
        priority(&mut doc, uid, value);
    }
    let ids: Vec<(u32, u32)> = doc.project.tasks.iter().map(|task| (task.uid, task.id)).collect();
    assert_eq!(ids, vec![(3, 1), (1, 2), (2, 3)]);
    assert_eq!(placed(&mut doc), vec![2, 1, 3], "by priority");
    set_order(&mut doc, vec![asc(Criterion::Id)]).unwrap();
    assert_eq!(placed(&mut doc), vec![3, 1, 2]);
    set_order(&mut doc, vec![desc(Criterion::Id)]).unwrap();
    assert_eq!(placed(&mut doc), vec![2, 1, 3]);
}

#[test]
fn deadlines_go_earliest_first_and_tasks_without_one_after() {
    let mut doc = doc_of(&[1, 2, 3, 4]);
    for (uid, day) in [(2, 9), (3, 4)] {
        doc.apply(Command::SetDeadline { uid, date: Some(at(day, 17, 0)) }).unwrap();
    }
    priority(&mut doc, 4, 900);
    set_order(&mut doc, vec![asc(Criterion::Deadline), desc(Criterion::Priority)]).unwrap();
    assert_eq!(placed(&mut doc), vec![3, 2, 4, 1]);
    set_order(&mut doc, vec![desc(Criterion::Deadline)]).unwrap();
    assert_eq!(placed(&mut doc), vec![2, 3, 1, 4]);
}

#[test]
fn durations_go_shortest_first() {
    let mut doc = doc_of(&[1, 2, 3]);
    for (uid, minutes) in [(1, 2 * DAY), (2, DAY), (3, DAY / 2)] {
        doc.apply(Command::SetDuration { uid, minutes }).unwrap();
    }
    set_order(&mut doc, vec![asc(Criterion::Duration)]).unwrap();
    assert_eq!(placed(&mut doc), vec![3, 2, 1]);
    set_order(&mut doc, vec![desc(Criterion::Duration)]).unwrap();
    assert_eq!(placed(&mut doc), vec![1, 2, 3]);
}

#[test]
fn tasks_waiting_on_a_predecessor_still_come_after_it() {
    let mut doc = doc_of(&[1, 2]);
    let link = Command::AddDependency { predecessor_uid: 1, successor_uid: 2, kind: DepKind::Ss, lag: 0 };
    doc.apply(link).unwrap();
    // Task 2 would go first by id descending, but it waits on task 1.
    set_order(&mut doc, vec![desc(Criterion::Id)]).unwrap();
    assert_eq!(placed(&mut doc), vec![1, 2]);
}

#[test]
fn deleting_a_field_drops_its_criterion_and_undo_brings_it_back() {
    let mut doc = doc_of(&[1, 2]);
    let field = add_field(&mut doc, "Điểm", FieldKind::Number);
    let order = vec![desc(Criterion::Field { id: field }), desc(Criterion::Priority)];
    set_order(&mut doc, order.clone()).unwrap();
    doc.apply(Command::DeleteField { id: field }).unwrap();
    assert_eq!(doc.project.leveling_order, vec![desc(Criterion::Priority)]);
    doc.undo().unwrap();
    assert_eq!(doc.project.leveling_order, order);
    doc.redo().unwrap();
    assert_eq!(doc.project.leveling_order, vec![desc(Criterion::Priority)]);
}

#[test]
fn a_field_that_changes_kind_keeps_its_criterion_and_sorts_by_the_new_kind() {
    let mut doc = doc_of(&[1, 2]);
    let field = add_field(&mut doc, "Điểm", FieldKind::Number);
    set_value(&mut doc, 1, field, FieldValue::Number(10.0));
    set_value(&mut doc, 2, field, FieldValue::Number(9.0));
    set_order(&mut doc, vec![asc(Criterion::Field { id: field })]).unwrap();
    assert_eq!(placed(&mut doc), vec![2, 1]);
    let update = Command::UpdateField {
        id: field,
        name: "Điểm".into(),
        kind: FieldKind::Text,
        renamed_options: Vec::new(),
    };
    doc.apply(update).unwrap();
    assert_eq!(doc.project.leveling_order, vec![asc(Criterion::Field { id: field })]);
    // As text, "10" comes before "9".
    assert_eq!(placed(&mut doc), vec![1, 2]);
}

#[test]
fn orders_that_cannot_be_kept_are_refused() {
    let mut doc = doc_of(&[1]);
    let unknown = vec![asc(Criterion::Field { id: 7 })];
    assert_eq!(invalid(set_order(&mut doc, unknown)), Invalid::NoSuchField);
    let twice = vec![asc(Criterion::Priority), desc(Criterion::Priority)];
    assert_eq!(invalid(set_order(&mut doc, twice)), Invalid::DuplicateSortKey);
    let field = add_field(&mut doc, "Điểm", FieldKind::Number);
    let twice = vec![asc(Criterion::Field { id: field }), desc(Criterion::Field { id: field })];
    assert_eq!(invalid(set_order(&mut doc, twice)), Invalid::DuplicateSortKey);

    let mut many = vec![
        asc(Criterion::Start),
        asc(Criterion::Priority),
        asc(Criterion::Slack),
        asc(Criterion::Deadline),
        asc(Criterion::Duration),
        asc(Criterion::Id),
        asc(Criterion::Field { id: field }),
    ];
    for name in ["Hai", "Ba"] {
        let id = add_field(&mut doc, name, FieldKind::Text);
        many.push(asc(Criterion::Field { id }));
    }
    assert_eq!(many.len(), MAX_SORT_KEYS + 1);
    assert_eq!(invalid(set_order(&mut doc, many.clone())), Invalid::TooManySortKeys);
    many.pop();
    set_order(&mut doc, many.clone()).unwrap();
    assert_eq!(doc.project.leveling_order, many);
}

#[test]
fn the_order_survives_a_file_and_old_projects_have_the_default() {
    let mut doc = doc_of(&[1, 2]);
    let field = add_field(&mut doc, "Mức", choice(&["Cao", "Thấp"]));
    let order = vec![asc(Criterion::Field { id: field }), desc(Criterion::Deadline)];
    set_order(&mut doc, order.clone()).unwrap();

    let dir = std::env::temp_dir().join(format!("sheafly-order-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("order.sheafly").to_string_lossy().to_string();
    save(&doc, &path).unwrap();
    let opened = load(&path).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(opened.project.leveling_order, order);

    let mut json = serde_json::to_value(&doc.project).unwrap();
    json.as_object_mut().unwrap().remove("leveling_order");
    let old: Project = serde_json::from_value(json).unwrap();
    assert!(old.leveling_order.is_empty());
}

#[test]
fn the_command_reads_the_shape_the_app_sends() {
    let sent = r#"{"type":"set_leveling_order","order":[
        {"criterion":{"type":"field","id":3},"descending":true},
        {"criterion":{"type":"start"},"descending":false}]}"#;
    let command: Command = serde_json::from_str(sent).unwrap();
    let Command::SetLevelingOrder { order } = command else {
        panic!("read as {command:?}");
    };
    assert_eq!(order, vec![desc(Criterion::Field { id: 3 }), asc(Criterion::Start)]);
}

//! Importing a table (phase 18 P3, step 3a): `ImportRows` adds people and tasks read from
//! a spreadsheet in one undo step, and `check_import` lists what is wrong with each row
//! before anything changes.

use chrono::Datelike;

use sheafly_engine::command::Command;
use sheafly_engine::fields::{FieldKind, FieldValue};
use sheafly_engine::import::{
    check_import, ImportAssignee, ImportColumn, ImportResource, ImportTable, ImportTask,
    RowError, MAX_IMPORT_ROWS,
};
use sheafly_engine::lang::Lang;
use sheafly_engine::model::{ConstraintType, ResourceKind, TaskType};
use sheafly_engine::store::{load, save};
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{Document, EngineError, Invalid, Project};

const DAY: i64 = 480;
const HOUR: i64 = 60;

fn at(day: u32, hour: u32, minute: u32) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(i64::from(day)))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, minute)
}

/// A project with one task and the people `names`, plus a choice field "Mảng".
fn doc(names: &[&str]) -> Document {
    let mut doc = Document::from_project(Project::new("Nhập", at(0, 8, 0)));
    for name in names {
        doc.apply(Command::AddResource {
            name: (*name).into(),
            kind: ResourceKind::Person,
        })
        .unwrap();
    }
    doc.apply(Command::AddField {
        name: "Mảng".into(),
        kind: FieldKind::Choice {
            options: vec!["BE".into(), "FE".into()],
        },
    })
    .unwrap();
    doc
}

fn task(name: &str) -> ImportTask {
    ImportTask {
        name: name.into(),
        ..ImportTask::default()
    }
}

fn person(name: &str) -> ImportResource {
    ImportResource {
        name: name.into(),
        kind: ResourceKind::Person,
        ..ImportResource::default()
    }
}

fn import(doc: &mut Document, resources: Vec<ImportResource>, tasks: Vec<ImportTask>) -> Result<(), EngineError> {
    doc.apply(Command::ImportRows {
        after_uid: Some(1),
        resources,
        tasks,
    })
    .map(|_| ())
}

fn errors(doc: &Document, resources: &[ImportResource], tasks: &[ImportTask]) -> Vec<(ImportTable, Option<u32>, ImportColumn)> {
    check_import(&doc.project, Some(1), resources, tasks)
        .into_iter()
        .map(|error: RowError| (error.table, error.row, error.column))
        .collect()
}

fn named<'a>(project: &'a Project, name: &str) -> &'a sheafly_engine::model::Task {
    project.tasks.iter().find(|task| task.name == name).unwrap()
}

#[test]
fn builds_groups_from_levels_after_the_anchor() {
    let mut doc = doc(&[]);
    let rows = vec![
        task("Nhóm A"),
        ImportTask { level: 1, ..task("A1") },
        ImportTask { level: 1, ..task("A2") },
        task("Nhóm B"),
        ImportTask { level: 1, ..task("B1") },
    ];
    import(&mut doc, vec![], rows).unwrap();
    let names: Vec<&str> = doc.project.tasks.iter().map(|task| task.name.as_str()).collect();
    assert_eq!(names[1..], ["Nhóm A", "A1", "A2", "Nhóm B", "B1"]);
    let group = named(&doc.project, "Nhóm A");
    assert!(group.summary);
    assert_eq!(named(&doc.project, "A2").parent_uid, Some(group.uid));
    assert!(!named(&doc.project, "B1").summary);
}

#[test]
fn work_makes_a_fixed_work_task_whose_duration_follows_the_units() {
    let mut doc = doc(&["An", "Bình"]);
    let rows = vec![
        ImportTask {
            work: Some(16 * HOUR),
            assignees: vec![ImportAssignee { name: "an".into(), units: 0.5 }],
            ..task("Thiết kế")
        },
        ImportTask { work: Some(3 * HOUR), ..task("Họp") },
        ImportTask { duration: Some(2 * DAY), ..task("Chờ duyệt") },
    ];
    import(&mut doc, vec![], rows).unwrap();
    let design = named(&doc.project, "Thiết kế");
    assert_eq!(design.task_type, TaskType::FixedWork);
    assert_eq!(design.work, 16 * HOUR);
    // 16 hours at half time is four working days.
    assert_eq!(design.duration, 4 * DAY);
    let assignment = doc.project.assignments.iter().find(|a| a.task_uid == design.uid).unwrap();
    assert_eq!(assignment.resource_uid, doc.project.resources[0].uid);
    assert_eq!(assignment.units, 0.5);
    let meeting = named(&doc.project, "Họp");
    assert_eq!((meeting.task_type, meeting.duration, meeting.work), (TaskType::FixedWork, 3 * HOUR, 3 * HOUR));
    let wait = named(&doc.project, "Chờ duyệt");
    assert_eq!((wait.task_type, wait.duration), (TaskType::FixedUnits, 2 * DAY));
}

#[test]
fn sets_priority_deadline_start_and_field_values() {
    let mut doc = doc(&[]);
    let rows = vec![ImportTask {
        priority: Some(900),
        deadline: Some(at(4, 17, 0)),
        start_no_earlier: Some(at(2, 8, 0)),
        values: vec![("mảng".into(), FieldValue::Text("FE".into()))],
        ..task("Màn hình")
    }];
    import(&mut doc, vec![], rows).unwrap();
    let screen = named(&doc.project, "Màn hình");
    assert_eq!(screen.priority, 900);
    assert_eq!(screen.deadline, Some(at(4, 17, 0)));
    assert_eq!(screen.constraint_type, ConstraintType::Snet);
    assert_eq!(screen.constraint_date, Some(at(2, 8, 0)));
    assert_eq!(screen.start, at(2, 8, 0));
    let field = doc.project.fields[0].id;
    assert_eq!(screen.values.get(&field), Some(&FieldValue::Text("FE".into())));
}

#[test]
fn links_rows_by_their_number_in_the_import() {
    let mut doc = doc(&[]);
    let rows = vec![
        ImportTask { duration: Some(DAY), ..task("Đầu") },
        ImportTask { duration: Some(DAY), after: vec![0], ..task("Giữa") },
        ImportTask { duration: Some(DAY), after: vec![0, 1], ..task("Cuối") },
    ];
    import(&mut doc, vec![], rows).unwrap();
    assert_eq!(named(&doc.project, "Giữa").start, at(1, 8, 0));
    assert_eq!(named(&doc.project, "Cuối").start, at(2, 8, 0));
    assert_eq!(doc.project.dependencies.len(), 3);
}

#[test]
fn adds_new_people_and_their_skills_and_keeps_existing_ones() {
    let mut doc = doc(&["An"]);
    let people = vec![
        ImportResource {
            max_units: Some(0.5),
            standard_rate: Some(200_000.0),
            skills: vec!["Backend".into(), "SQL".into()],
            ..person("Chi")
        },
        // Already in the project: left as it is.
        ImportResource { max_units: Some(3.0), ..person("an") },
        ImportResource { kind: ResourceKind::Machine, ..person("Máy in") },
    ];
    let rows = vec![ImportTask {
        work: Some(DAY),
        assignees: vec![ImportAssignee { name: "Chi".into(), units: 1.0 }],
        ..task("API")
    }];
    import(&mut doc, people, rows).unwrap();
    let names: Vec<&str> = doc.project.resources.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["An", "Chi", "Máy in"]);
    assert_eq!(doc.project.resources[0].max_units, 1.0);
    let chi = &doc.project.resources[1];
    assert_eq!((chi.max_units, chi.standard_rate), (0.5, 200_000.0));
    let skills: Vec<&str> = doc.project.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(skills, ["Backend", "SQL"]);
    assert_eq!(chi.skills.len(), 2);
    assert_eq!(doc.project.resources[2].kind, ResourceKind::Machine);
    let api = named(&doc.project, "API");
    assert!(doc.project.assignments.iter().any(|a| a.task_uid == api.uid && a.resource_uid == chi.uid));
}

#[test]
fn reports_each_bad_row_and_cell() {
    let doc = doc(&["An"]);
    let people = vec![
        ImportResource { max_units: Some(0.0), ..person("Dũng") },
        person("Dũng"),
        ImportResource { skills: vec!["a,b".into()], ..person("Em") },
        person(" "),
    ];
    let rows = vec![
        task(""),
        ImportTask { level: 2, ..task("Nhảy cấp") },
        ImportTask { work: Some(HOUR), duration: Some(HOUR), ..task("Cả hai") },
        ImportTask { work: Some(-1), ..task("Âm") },
        ImportTask { priority: Some(1001), ..task("Ưu tiên") },
        ImportTask { assignees: vec![ImportAssignee { name: "Không có".into(), units: 1.0 }], ..task("Lạ") },
        ImportTask { assignees: vec![ImportAssignee { name: "An".into(), units: 0.0 }], ..task("Không đơn vị") },
        ImportTask { values: vec![("Không có".into(), FieldValue::Text("x".into()))], ..task("Trường lạ") },
        ImportTask { values: vec![("Mảng".into(), FieldValue::Text("QA".into()))], ..task("Giá trị lạ") },
        ImportTask { after: vec![9], ..task("Tự chờ") },
        ImportTask { after: vec![99], ..task("Chờ dòng không có") },
        ImportTask { work: Some(HOUR), ..task("Nhóm có effort") },
        ImportTask { level: 1, ..task("Con") },
    ];
    use ImportColumn as C;
    use ImportTable::{Resources as R, Tasks as T};
    assert_eq!(
        errors(&doc, &people, &rows),
        vec![
            (R, Some(0), C::MaxUnits),
            (R, Some(1), C::Name),
            (R, Some(2), C::Skills),
            (R, Some(3), C::Name),
            (T, Some(0), C::Name),
            (T, Some(1), C::Level),
            (T, Some(2), C::Duration),
            (T, Some(3), C::Work),
            (T, Some(4), C::Priority),
            (T, Some(5), C::Assignees),
            (T, Some(6), C::Assignees),
            (T, Some(7), C::Field("Không có".into())),
            (T, Some(8), C::Field("Mảng".into())),
            (T, Some(9), C::After),
            (T, Some(10), C::After),
            (T, Some(11), C::Work),
        ]
    );
}

#[test]
fn reports_rows_that_wait_on_each_other() {
    let doc = doc(&[]);
    let rows = vec![
        ImportTask { after: vec![2], ..task("A") },
        task("B"),
        ImportTask { after: vec![0], ..task("C") },
    ];
    let found = errors(&doc, &[], &rows);
    assert_eq!(
        found,
        vec![
            (ImportTable::Tasks, Some(0), ImportColumn::After),
            (ImportTable::Tasks, Some(2), ImportColumn::After),
        ]
    );
    let messages = check_import(&doc.project, Some(1), &[], &rows);
    assert!(messages.iter().all(|error| !error.message(Lang::En).is_empty() && error.message(Lang::En).is_ascii()));
}

#[test]
fn refuses_an_import_with_errors_or_nothing_in_it() {
    let mut doc = doc(&[]);
    let before = doc.project.clone();
    assert_eq!(
        import(&mut doc, vec![], vec![task("")]),
        Err(EngineError::Invalid(Invalid::ImportHasErrors))
    );
    assert_eq!(import(&mut doc, vec![], vec![]), Err(EngineError::Invalid(Invalid::NothingToImport)));
    assert_eq!(doc.project, before);
    let too_many = vec![task("x"); MAX_IMPORT_ROWS + 1];
    assert_eq!(
        errors(&doc, &[], &too_many),
        vec![(ImportTable::Tasks, None, ImportColumn::Row)]
    );
}

#[test]
fn one_undo_removes_the_whole_import_and_replay_rebuilds_it() {
    let mut doc = doc(&["An"]);
    let before = doc.project.clone();
    let rows: Vec<ImportTask> = (0..50)
        .map(|i| ImportTask {
            work: Some(HOUR * (1 + i % 8)),
            assignees: vec![ImportAssignee { name: "An".into(), units: 1.0 }],
            after: if i > 0 { vec![i as u32 - 1] } else { vec![] },
            ..task(&format!("Việc {i}"))
        })
        .collect();
    import(&mut doc, vec![person("Bình")], rows).unwrap();
    let after = doc.project.clone();
    doc.undo().unwrap();
    assert_eq!(doc.project, before);
    doc.redo().unwrap();
    assert_eq!(doc.project, after);

    let dir = std::env::temp_dir().join(format!("sheafly-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("import.sheafly").to_string_lossy().to_string();
    save(&doc, &path).unwrap();
    let opened = load(&path).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(opened.project, after);
}

#[test]
fn imports_to_the_end_without_an_anchor() {
    let mut doc = doc(&[]);
    doc.apply(Command::ImportRows {
        after_uid: None,
        resources: vec![],
        tasks: vec![task("Cuối cùng")],
    })
    .unwrap();
    assert_eq!(doc.project.tasks.last().unwrap().name, "Cuối cùng");
}

#[test]
fn a_huge_level_after_a_bad_row_is_an_error_not_an_overflow() {
    let doc = doc(&[]);
    let rows = vec![ImportTask { level: u32::MAX, ..task("A") }, ImportTask { level: 1, ..task("B") }];
    assert_eq!(
        errors(&doc, &[], &rows),
        vec![(ImportTable::Tasks, Some(0), ImportColumn::Level)]
    );
}

#[test]
fn without_an_anchor_rows_go_to_the_top_level_after_a_group() {
    let mut doc = doc(&[]);
    doc.apply(Command::AddTask { name: "Con".into(), after_uid: Some(1) }).unwrap();
    let child = doc.project.tasks[1].uid;
    doc.apply(Command::Indent { uid: child }).unwrap();
    doc.apply(Command::ImportRows { after_uid: None, resources: vec![], tasks: vec![task("Cuối")] })
        .unwrap();
    let last = named(&doc.project, "Cuối");
    assert_eq!((last.outline_level, last.parent_uid), (1, None));
}

#[test]
fn the_same_person_twice_on_a_row_is_one_assignment() {
    let mut doc = doc(&["An"]);
    let twice = vec![
        ImportAssignee { name: "An".into(), units: 1.0 },
        ImportAssignee { name: "an".into(), units: 0.5 },
    ];
    import(&mut doc, vec![], vec![ImportTask { work: Some(DAY), assignees: twice, ..task("Hai lần") }]).unwrap();
    let uid = named(&doc.project, "Hai lần").uid;
    let on_task: Vec<_> = doc.project.assignments.iter().filter(|a| a.task_uid == uid).collect();
    assert_eq!(on_task.len(), 1);
    assert_eq!(on_task[0].units, 0.5);
}

#[test]
fn a_child_waiting_for_its_own_group_is_reported_for_the_whole_import() {
    let doc = doc(&[]);
    let rows = vec![task("Nhóm"), ImportTask { level: 1, after: vec![0], ..task("Con") }];
    let found = check_import(&doc.project, Some(1), &[], &rows);
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].row, &found[0].column), (None, &ImportColumn::Row));
    assert_eq!(found[0].error, EngineError::DependencyCycle);
}

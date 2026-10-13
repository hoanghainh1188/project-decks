//! Custom task fields (phase 18 P2, step 1): declaring, editing, values, limits, fill-down,
//! paste, files, and MSPDI `ExtendedAttribute`s.

use chrono::Datelike;

use sheafly_engine::clipboard::{copy_tasks, FillField};
use sheafly_engine::command::Command;
use sheafly_engine::fields::{FieldKind, FieldValue, OptionRename};
use sheafly_engine::lang::Lang;
use sheafly_engine::model::Task;
use sheafly_engine::mspdi::{read_mspdi_with_report, write_mspdi};
use sheafly_engine::schedule::schedule;
use sheafly_engine::store::{load, save};
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{Document, EngineError, Invalid, Project};

const FIXTURE: &str = include_str!("fixtures/custom_fields.xml");

/// 2026-01-05 is a Monday; `day` 0 is that Monday.
fn at(day: u32, hour: u32, minute: u32) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(i64::from(day)))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, minute)
}

/// Tasks A, B, C of one day each.
fn doc() -> Document {
    let mut project = Project::new("Trường", at(0, 8, 0));
    for (uid, name) in [(1, "A"), (2, "B"), (3, "C")] {
        let mut task = Task::new(uid, name);
        task.duration = 480;
        task.work = 480;
        task.remaining_duration = 480;
        task.remaining_work = 480;
        project.tasks.push(task);
    }
    project.next_task_uid = 4;
    schedule(&mut project).unwrap();
    Document::from_project(project)
}

fn choice(options: &[&str]) -> FieldKind {
    FieldKind::Choice {
        options: options.iter().map(|option| option.to_string()).collect(),
    }
}

fn add(doc: &mut Document, name: &str, kind: FieldKind) -> u32 {
    doc.apply(Command::AddField { name: name.into(), kind }).unwrap();
    doc.project.fields.last().unwrap().id
}

fn set(doc: &mut Document, uid: u32, field_id: u32, value: FieldValue) -> Result<(), EngineError> {
    doc.apply(Command::SetFieldValue { uid, field_id, value: Some(value) }).map(|_| ())
}

fn text(value: &str) -> FieldValue {
    FieldValue::Text(value.into())
}

fn value(doc: &Document, uid: u32, field_id: u32) -> Option<FieldValue> {
    doc.project.task(uid).unwrap().values.get(&field_id).cloned()
}

fn invalid(result: Result<(), EngineError>) -> Invalid {
    match result {
        Err(EngineError::Invalid(reason)) => reason,
        other => panic!("expected a validation error, got {other:?}"),
    }
}

fn add_result(doc: &mut Document, name: &str, kind: FieldKind) -> Result<(), EngineError> {
    doc.apply(Command::AddField { name: name.into(), kind }).map(|_| ())
}

#[test]
fn fields_take_values_of_their_kind() {
    let mut doc = doc();
    let client = add(&mut doc, "Khách hàng", choice(&["ACME", "Globex"]));
    let score = add(&mut doc, "Điểm", FieldKind::Number);
    let note = add(&mut doc, "Ghi chú", FieldKind::Text);
    assert_eq!((client, score, note), (1, 2, 3));

    set(&mut doc, 1, client, text("ACME")).unwrap();
    set(&mut doc, 1, score, FieldValue::Number(7.5)).unwrap();
    set(&mut doc, 2, note, text("  cần duyệt  ")).unwrap();
    assert_eq!(value(&doc, 1, client), Some(text("ACME")));
    assert_eq!(value(&doc, 1, score), Some(FieldValue::Number(7.5)));
    assert_eq!(value(&doc, 2, note), Some(text("cần duyệt")), "text is trimmed");

    assert_eq!(invalid(set(&mut doc, 1, client, text("Initech"))), Invalid::FieldValue);
    assert_eq!(invalid(set(&mut doc, 1, score, text("7"))), Invalid::FieldValue);
    assert_eq!(invalid(set(&mut doc, 1, note, FieldValue::Number(1.0))), Invalid::FieldValue);
    assert_eq!(invalid(set(&mut doc, 1, note, text(&"x".repeat(256)))), Invalid::FieldValue);
    assert_eq!(invalid(set(&mut doc, 1, 99, text("x"))), Invalid::NoSuchField);
    assert!(matches!(set(&mut doc, 99, note, text("x")), Err(EngineError::TaskNotFound)));

    // Empty text and `None` both clear the value.
    set(&mut doc, 2, note, text("   ")).unwrap();
    assert_eq!(value(&doc, 2, note), None);
    set(&mut doc, 1, score, FieldValue::Number(3.0)).unwrap();
    doc.apply(Command::SetFieldValue { uid: 1, field_id: score, value: None }).unwrap();
    assert_eq!(value(&doc, 1, score), None);
}

#[test]
fn names_and_choices_are_checked() {
    let mut doc = doc();
    add(&mut doc, "Khách hàng", FieldKind::Text);
    assert_eq!(invalid(add_result(&mut doc, "  ", FieldKind::Text)), Invalid::FieldName);
    assert_eq!(invalid(add_result(&mut doc, &"n".repeat(51), FieldKind::Text)), Invalid::FieldName);
    assert_eq!(
        invalid(add_result(&mut doc, " KHÁCH HÀNG ", FieldKind::Number)),
        Invalid::FieldNameTaken,
        "names are unique whatever the letter case"
    );
    for options in [vec![], vec!["A", " "], vec!["A", "A "]] {
        assert_eq!(invalid(add_result(&mut doc, "Mảng", choice(&options))), Invalid::ChoiceOptions);
    }
    let area = add(&mut doc, " Mảng ", choice(&[" BE ", "FE"]));
    let field = doc.project.fields.iter().find(|field| field.id == area).unwrap();
    assert_eq!(field.name, "Mảng");
    assert_eq!(field.kind, choice(&["BE", "FE"]));
}

#[test]
fn a_project_holds_as_many_fields_as_microsoft_project() {
    let mut doc = doc();
    for index in 0..30 {
        let kind = if index % 2 == 0 { FieldKind::Text } else { choice(&["x"]) };
        add(&mut doc, &format!("Chữ {index}"), kind);
    }
    assert_eq!(invalid(add_result(&mut doc, "Chữ 30", FieldKind::Text)), Invalid::TooManyTextFields);
    for index in 0..20 {
        add(&mut doc, &format!("Số {index}"), FieldKind::Number);
    }
    assert_eq!(
        invalid(add_result(&mut doc, "Số 20", FieldKind::Number)),
        Invalid::TooManyNumberFields
    );
    // Turning a number field into text needs a free text slot.
    let number = doc.project.fields.last().unwrap().id;
    let update = Command::UpdateField {
        id: number,
        name: "Số 19".into(),
        kind: FieldKind::Text,
        renamed_options: Vec::new(),
    };
    assert_eq!(invalid(doc.apply(update).map(|_| ())), Invalid::TooManyTextFields);
}

#[test]
fn changing_a_field_keeps_the_values_that_still_fit() {
    let mut doc = doc();
    let field = add(&mut doc, "Mã", FieldKind::Text);
    set(&mut doc, 1, field, text("12,5")).unwrap();
    set(&mut doc, 2, field, text("abc")).unwrap();
    set(&mut doc, 3, field, text("FE")).unwrap();

    let update = |name: &str, kind: FieldKind, renamed: Vec<OptionRename>| Command::UpdateField {
        id: field,
        name: name.into(),
        kind,
        renamed_options: renamed,
    };
    doc.apply(update("Mã số", FieldKind::Number, Vec::new())).unwrap();
    assert_eq!(doc.project.fields[0].name, "Mã số");
    assert_eq!(value(&doc, 1, field), Some(FieldValue::Number(12.5)), "a comma decimal reads");
    assert_eq!(value(&doc, 2, field), None, "text that is no number is cleared");
    assert_eq!(value(&doc, 3, field), None);

    doc.apply(update("Mã số", FieldKind::Text, Vec::new())).unwrap();
    assert_eq!(value(&doc, 1, field), Some(text("12.5")));

    // Renaming a choice keeps the tasks that had it; dropping one clears them.
    set(&mut doc, 2, field, text("BE")).unwrap();
    set(&mut doc, 3, field, text("FE")).unwrap();
    doc.apply(update("Mảng", choice(&["BE", "FE"]), Vec::new())).unwrap();
    assert_eq!(value(&doc, 1, field), None, "12.5 is not one of the choices");
    let renamed = vec![OptionRename { from: "BE".into(), to: "Backend".into() }];
    doc.apply(update("Mảng", choice(&["Backend"]), renamed)).unwrap();
    assert_eq!(value(&doc, 2, field), Some(text("Backend")));
    assert_eq!(value(&doc, 3, field), None);

    // Deleting the field clears every value; undo brings them back.
    doc.apply(Command::DeleteField { id: field }).unwrap();
    assert!(doc.project.fields.is_empty());
    assert!(doc.project.tasks.iter().all(|task| task.values.is_empty()));
    doc.undo().unwrap();
    assert_eq!(value(&doc, 2, field), Some(text("Backend")));
}

#[test]
fn fill_down_copies_a_field_to_tasks_and_groups() {
    let mut doc = doc();
    doc.apply(Command::Indent { uid: 3 }).unwrap();
    assert!(doc.project.task(2).unwrap().summary);
    let field = add(&mut doc, "Khách hàng", choice(&["ACME"]));
    set(&mut doc, 1, field, text("ACME")).unwrap();
    doc.apply(Command::FillDown { from_uid: 1, to_uids: vec![2, 3], field: FillField::Field(field) })
        .unwrap();
    assert_eq!(value(&doc, 2, field), Some(text("ACME")), "a group takes a field value");
    assert_eq!(value(&doc, 3, field), Some(text("ACME")));

    // An empty source clears the targets.
    doc.apply(Command::SetFieldValue { uid: 1, field_id: field, value: None }).unwrap();
    doc.apply(Command::FillDown { from_uid: 1, to_uids: vec![3], field: FillField::Field(field) })
        .unwrap();
    assert_eq!(value(&doc, 3, field), None);
}

#[test]
fn pasted_tasks_keep_values_whose_field_has_the_same_name() {
    let mut source = doc();
    let client = add(&mut source, "Khách hàng", choice(&["ACME", "Globex"]));
    let score = add(&mut source, "Điểm", FieldKind::Number);
    let only_here = add(&mut source, "Chỉ ở đây", FieldKind::Text);
    set(&mut source, 1, client, text("Globex")).unwrap();
    set(&mut source, 1, score, FieldValue::Number(4.0)).unwrap();
    set(&mut source, 1, only_here, text("x")).unwrap();
    let clipboard = copy_tasks(&source.project, &[1]);

    let mut target = doc();
    let target_score = add(&mut target, "Điểm", FieldKind::Text);
    let target_client = add(&mut target, "Khách hàng", choice(&["ACME"]));
    target.apply(Command::PasteTasks { after_uid: Some(3), clipboard }).unwrap();
    let pasted = target.project.tasks.last().unwrap();
    assert_eq!(pasted.values.get(&target_score), Some(&text("4")), "a number becomes text");
    assert_eq!(pasted.values.get(&target_client), None, "Globex is not a choice here");
    assert_eq!(pasted.values.len(), 1);
}

#[test]
fn fields_survive_a_save_and_files_without_them_still_open() {
    let mut doc = doc();
    let client = add(&mut doc, "Khách hàng", choice(&["ACME"]));
    let score = add(&mut doc, "Điểm", FieldKind::Number);
    set(&mut doc, 1, client, text("ACME")).unwrap();
    set(&mut doc, 2, score, FieldValue::Number(-2.25)).unwrap();

    let dir = std::env::temp_dir().join(format!("sheafly-fields-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fields.sheafly").to_string_lossy().to_string();
    save(&doc, &path).unwrap();
    let opened = load(&path).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(opened.project.fields, doc.project.fields);
    assert_eq!(opened.project.tasks, doc.project.tasks);

    // A project written before custom fields has neither `fields` nor `values`.
    let mut json = serde_json::to_value(&doc.project).unwrap();
    json.as_object_mut().unwrap().remove("fields");
    for task in json["tasks"].as_array_mut().unwrap() {
        task.as_object_mut().unwrap().remove("values");
    }
    let old: Project = serde_json::from_value(json).unwrap();
    assert!(old.fields.is_empty());
    assert!(old.tasks.iter().all(|task| task.values.is_empty()));

    // Values are plain JSON numbers and strings.
    let task = serde_json::to_value(&doc.project.tasks[0]).unwrap();
    assert_eq!(task["values"][client.to_string()], "ACME");
}

#[test]
fn mspdi_round_trip_keeps_fields_and_values() {
    let mut doc = doc();
    let note = add(&mut doc, "Ghi chú <nội bộ>", FieldKind::Text);
    let score = add(&mut doc, "Điểm", FieldKind::Number);
    let client = add(&mut doc, "Khách hàng", choice(&["A & B", "Globex"]));
    set(&mut doc, 1, note, text("x < y")).unwrap();
    set(&mut doc, 1, score, FieldValue::Number(1234.5)).unwrap();
    set(&mut doc, 2, client, text("A & B")).unwrap();

    let xml = write_mspdi(&doc.project).unwrap();
    // Text fields take Text1, Text2…, number fields Number1…, whatever order they came in.
    assert!(xml.contains("<FieldID>188743731</FieldID>\n<FieldName>Text1</FieldName>"));
    assert!(xml.contains("<FieldID>188743767</FieldID>\n<FieldName>Number1</FieldName>"));
    assert!(xml.contains("<FieldID>188743734</FieldID>\n<FieldName>Text2</FieldName>"));
    assert!(xml.contains("<Alias>Ghi chú &lt;nội bộ&gt;</Alias>"));
    assert!(xml.find("<ExtendedAttributes>").unwrap() < xml.find("<Calendars>").unwrap());

    let (imported, report) = read_mspdi_with_report(&xml).unwrap();
    assert!(report.notes.is_empty(), "{:?}", report.notes);
    assert_eq!(imported.fields, doc.project.fields);
    for (back, original) in imported.tasks.iter().zip(&doc.project.tasks) {
        assert_eq!(back.values, original.values, "task {}", original.uid);
    }
}

#[test]
fn mspdi_import_reads_microsoft_project_fields() {
    let (project, report) = read_mspdi_with_report(FIXTURE).unwrap();
    let names: Vec<&str> = project.fields.iter().map(|field| field.name.as_str()).collect();
    assert_eq!(names, ["Khách hàng", "Điểm", "khách hàng (Text2)", "Text5"]);
    // A value the list lacks is added to it rather than lost.
    assert_eq!(project.fields[0].kind, choice(&["ACME", "Globex", "Initech"]));
    assert_eq!(project.fields[1].kind, FieldKind::Number);
    assert_eq!(project.fields[2].kind, FieldKind::Text);
    assert_eq!(project.fields[3].kind, FieldKind::Text);

    let values = |uid: u32| project.task(uid).unwrap().values.clone();
    assert_eq!(values(1).get(&1), Some(&text("ACME")));
    assert_eq!(values(1).get(&2), Some(&FieldValue::Number(12.5)));
    assert_eq!(values(1).get(&4), Some(&text("ghi chú & thêm")));
    assert_eq!(values(2).get(&1), Some(&text("Initech")));
    assert_eq!(values(2).get(&3), Some(&text("nội bộ")));
    assert_eq!(values(2).get(&2), None, "text in a number field is left out");

    // The cost field is noted, with its one value; the resource field is not a task field.
    let message = report.message(Lang::En).unwrap();
    assert!(message.contains("“Ngân sách”: 1 value"), "{message}");
    assert!(!message.contains("205521019"), "{message}");
    let vietnamese = report.message(Lang::Vi).unwrap();
    assert!(vietnamese.contains("“Ngân sách”: 1 giá trị"), "{vietnamese}");
}

#[test]
fn a_value_on_one_meeting_detaches_it_so_regenerating_the_series_keeps_it() {
    use sheafly_engine::recurrence::{RecurrenceEnd, RecurrencePattern, RecurrenceSpec};
    let mut doc = Document::new_default();
    let spec = RecurrenceSpec {
        name: "Họp".into(),
        duration: 60,
        start: doc.project.start,
        pattern: RecurrencePattern::Weekly { every: 1, weekdays: vec![0] },
        end: RecurrenceEnd::Count { count: 3 },
        calendar_id: None,
        resource_uids: Vec::new(),
        resources: Vec::new(),
        background: false,
    };
    doc.apply(Command::CreateRecurringTask { after_uid: None, spec: spec.clone() }).unwrap();
    let field = add(&mut doc, "Người chủ trì", FieldKind::Text);
    let summary = doc.project.tasks.iter().find(|task| task.recurrence.is_some()).unwrap().uid;
    let first = doc.project.tasks.iter().find(|task| task.series_uid.is_some()).unwrap().uid;
    set(&mut doc, summary, field, text("Nhóm A")).unwrap();
    set(&mut doc, first, field, text("An")).unwrap();
    assert!(doc.project.task(first).unwrap().detached);

    let mut longer = spec;
    longer.end = RecurrenceEnd::Count { count: 4 };
    doc.apply(Command::UpdateRecurringTask { uid: summary, spec: longer }).unwrap();
    assert_eq!(value(&doc, first, field), Some(text("An")));
    assert_eq!(value(&doc, summary, field), Some(text("Nhóm A")), "the series keeps its own value");
}

#[test]
fn renamed_choices_are_trimmed_and_must_be_new_choices() {
    let mut doc = doc();
    let field = add(&mut doc, "Mảng", choice(&["BE", "FE"]));
    set(&mut doc, 1, field, text("BE")).unwrap();
    let update = |kind: FieldKind, from: &str, to: &str| Command::UpdateField {
        id: field,
        name: "Mảng".into(),
        kind,
        renamed_options: vec![OptionRename { from: from.into(), to: to.into() }],
    };
    let refused = doc.apply(update(choice(&["Backend", "FE"]), "BE", "Back end")).map(|_| ());
    assert_eq!(invalid(refused), Invalid::ChoiceOptions, "the new name is not a choice");
    doc.apply(update(choice(&["Backend", "FE"]), " BE ", " Backend ")).unwrap();
    assert_eq!(value(&doc, 1, field), Some(text("Backend")));
    // A rename means nothing once the field is no list: the value is kept as text.
    doc.apply(update(FieldKind::Text, "Backend", "x")).unwrap();
    assert_eq!(value(&doc, 1, field), Some(text("Backend")));
}

#[test]
fn a_deleted_field_id_is_never_given_again() {
    let mut doc = doc();
    let first = add(&mut doc, "A", FieldKind::Text);
    doc.apply(Command::DeleteField { id: first }).unwrap();
    let second = add(&mut doc, "B", FieldKind::Text);
    assert_ne!(first, second);
    doc.undo().unwrap();
    doc.undo().unwrap();
    doc.redo().unwrap();
    doc.redo().unwrap();
    assert_eq!(doc.project.fields[0].id, second, "replay gives the same id");

    // A file written before the counter existed has 0: the next id is past the largest.
    let mut json = serde_json::to_value(&doc.project).unwrap();
    json["next_field_id"] = serde_json::json!(0);
    let mut old: Project = serde_json::from_value(json).unwrap();
    let mut old_doc = Document::from_project(old.clone());
    old_doc.apply(Command::AddField { name: "C".into(), kind: FieldKind::Number }).unwrap();
    assert_eq!(old_doc.project.fields.last().unwrap().id, second + 1);
    old.fields.clear();
    assert_eq!(serde_json::to_value(&old).unwrap()["next_field_id"], 0);
}

#[test]
fn mspdi_keeps_numbers_exact() {
    let mut doc = doc();
    let field = add(&mut doc, "Hệ số", FieldKind::Number);
    for (uid, number) in [(1, 0.000_000_4), (2, -0.0), (3, 1e21)] {
        set(&mut doc, uid, field, FieldValue::Number(number)).unwrap();
    }
    let xml = write_mspdi(&doc.project).unwrap();
    assert!(xml.contains("<Value>0.0000004</Value>"), "no exponent, no rounding");
    assert!(xml.contains("<Value>0</Value>"), "-0 is written 0");
    let (imported, _) = read_mspdi_with_report(&xml).unwrap();
    let back = |uid: u32| imported.task(uid).unwrap().values.get(&1).cloned();
    assert_eq!(back(1), Some(FieldValue::Number(0.000_000_4)));
    assert_eq!(back(2), Some(FieldValue::Number(0.0)));
    assert_eq!(back(3), Some(FieldValue::Number(1e21)));
}

#[test]
fn a_paste_matches_field_names_whatever_the_letter_case() {
    let mut source = doc();
    let owner = add(&mut source, "Owner", FieldKind::Text);
    set(&mut source, 1, owner, text("An")).unwrap();
    let clipboard = copy_tasks(&source.project, &[1]);
    let mut target = doc();
    let target_owner = add(&mut target, "owner", FieldKind::Text);
    target.apply(Command::PasteTasks { after_uid: Some(3), clipboard }).unwrap();
    assert_eq!(target.project.tasks.last().unwrap().values.get(&target_owner), Some(&text("An")));
}

#[test]
fn mspdi_import_notes_number_values_it_cannot_read() {
    let (_, report) = read_mspdi_with_report(FIXTURE).unwrap();
    let message = report.message(Lang::En).unwrap();
    assert!(message.contains("“Điểm”: 1 value that is not a number"), "{message}");
    let vietnamese = report.message(Lang::Vi).unwrap();
    assert!(vietnamese.contains("“Điểm”: 1 giá trị không phải số"), "{vietnamese}");
}

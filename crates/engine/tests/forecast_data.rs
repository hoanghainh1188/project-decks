//! Data for the finish forecast (phase 18 P4, step 4a): an effort range on leaf tasks and
//! the project's correlation settings. Nothing schedules from them; the forecast reads
//! them (step 4b). They live in `.sheafly` files only.

use chrono::Datelike;

use sheafly_engine::clipboard::{copy_tasks, FillField};
use sheafly_engine::command::Command;
use sheafly_engine::fields::FieldKind;
use sheafly_engine::import::{check_import, ImportColumn, ImportTask};
use sheafly_engine::mspdi::{read_mspdi_with_report, write_mspdi};
use sheafly_engine::model::{ForecastSettings, ResourceKind, Strength, Task};
use sheafly_engine::store::{load, save, FORMAT_VERSION};
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{Document, EngineError, Invalid, Project};

const DAY: i64 = 480;

fn at(day: u32, hour: u32) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(i64::from(day)))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, 0)
}

/// Three one-day tasks, the first two under a group made by indenting.
fn doc() -> Document {
    let mut project = Project::new("Dự báo", at(0, 8));
    for uid in 1..=3 {
        let mut task = Task::new(uid, format!("Việc {uid}"));
        task.duration = DAY;
        task.remaining_duration = DAY;
        task.work = DAY;
        task.remaining_work = DAY;
        project.tasks.push(task);
    }
    project.next_task_uid = 4;
    let mut doc = Document::from_project(project);
    doc.apply(Command::AddResource { name: "An".into(), kind: ResourceKind::Person }).unwrap();
    doc
}

fn range(doc: &mut Document, uid: u32, low: Option<i64>, high: Option<i64>) -> Result<(), EngineError> {
    doc.apply(Command::SetWorkRange { uid, low, high }).map(|_| ())
}

fn invalid(result: Result<(), EngineError>) -> Invalid {
    match result {
        Err(EngineError::Invalid(reason)) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn task(doc: &Document, uid: u32) -> &Task {
    doc.project.task(uid).unwrap()
}

#[test]
fn a_leaf_task_takes_a_range_that_undo_removes() {
    let mut doc = doc();
    range(&mut doc, 1, Some(DAY / 2), Some(3 * DAY)).unwrap();
    assert_eq!((task(&doc, 1).work_low, task(&doc, 1).work_high), (Some(DAY / 2), Some(3 * DAY)));
    // The effort and the dates stay: only the forecast reads the range.
    assert_eq!(task(&doc, 1).work, DAY);
    doc.undo().unwrap();
    assert_eq!((task(&doc, 1).work_low, task(&doc, 1).work_high), (None, None));
    range(&mut doc, 1, Some(DAY), Some(2 * DAY)).unwrap();
    range(&mut doc, 1, None, None).unwrap();
    assert_eq!(task(&doc, 1).work_high, None);
}

#[test]
fn a_range_is_two_ordered_amounts_on_a_leaf() {
    let mut doc = doc();
    assert_eq!(invalid(range(&mut doc, 1, Some(2 * DAY), Some(DAY))), Invalid::BadWorkRange);
    assert_eq!(invalid(range(&mut doc, 1, Some(-1), Some(DAY))), Invalid::BadWorkRange);
    assert_eq!(invalid(range(&mut doc, 1, Some(DAY), None)), Invalid::BadWorkRange);
    // The current effort may sit outside the range; the forecast clamps it.
    range(&mut doc, 1, Some(2 * DAY), Some(3 * DAY)).unwrap();
    doc.apply(Command::Indent { uid: 2 }).unwrap();
    assert_eq!(invalid(range(&mut doc, 1, Some(DAY), Some(DAY))), Invalid::WorkRangeLeaf);
    doc.apply(Command::AddMilestone { name: "Mốc".into(), after_uid: None }).unwrap();
    let milestone = doc.project.tasks.last().unwrap().uid;
    assert_eq!(invalid(range(&mut doc, milestone, Some(0), Some(DAY))), Invalid::WorkRangeLeaf);
    assert!(matches!(range(&mut doc, 99, Some(0), Some(DAY)), Err(EngineError::TaskNotFound)));
}

#[test]
fn settings_default_to_a_medium_group_and_a_low_common_part() {
    let doc = doc();
    assert_eq!(
        doc.project.forecast,
        ForecastSettings { field: None, group: Strength::Medium, common: Strength::Low }
    );
    let json = r#"{"type":"set_forecast_settings","settings":{"field":null,"group":"high","common":"none"}}"#;
    assert!(serde_json::from_str::<Command>(json).is_ok());
    let json = r#"{"type":"set_work_range","uid":1,"low":240,"high":960}"#;
    assert!(serde_json::from_str::<Command>(json).is_ok());
}

#[test]
fn correlation_goes_by_a_text_or_choice_field_and_follows_its_edits() {
    let mut doc = doc();
    doc.apply(Command::AddField { name: "Mảng".into(), kind: FieldKind::Choice { options: vec!["BE".into(), "FE".into()] } })
        .unwrap();
    doc.apply(Command::AddField { name: "Điểm".into(), kind: FieldKind::Number }).unwrap();
    let settings = |field| ForecastSettings { field, group: Strength::High, common: Strength::Low };
    let set = |doc: &mut Document, field| doc.apply(Command::SetForecastSettings { settings: settings(field) }).map(|_| ());
    assert_eq!(invalid(set(&mut doc, Some(2))), Invalid::ForecastFieldKind);
    assert_eq!(invalid(set(&mut doc, Some(9))), Invalid::NoSuchField);
    set(&mut doc, Some(1)).unwrap();
    assert_eq!(doc.project.forecast.field, Some(1));

    // A field turned into numbers can no longer group tasks.
    doc.apply(Command::UpdateField { id: 1, name: "Mảng".into(), kind: FieldKind::Number, renamed_options: vec![] }).unwrap();
    assert_eq!(doc.project.forecast, settings(None));
    doc.undo().unwrap();
    assert_eq!(doc.project.forecast.field, Some(1));
    doc.apply(Command::DeleteField { id: 1 }).unwrap();
    assert_eq!(doc.project.forecast, settings(None));
}

#[test]
fn ranges_and_settings_survive_a_file_of_the_new_version() {
    assert_eq!(FORMAT_VERSION, 16);
    let mut doc = doc();
    range(&mut doc, 2, Some(DAY / 2), Some(2 * DAY)).unwrap();
    let settings = ForecastSettings { field: None, group: Strength::Low, common: Strength::None };
    doc.apply(Command::SetForecastSettings { settings: settings.clone() }).unwrap();
    let dir = tempfile_dir();
    let path = dir.join("forecast.sheafly");
    let path = path.to_str().unwrap();
    save(&doc, path).unwrap();
    let loaded = load(path).unwrap();
    assert_eq!((task(&loaded, 2).work_low, task(&loaded, 2).work_high), (Some(DAY / 2), Some(2 * DAY)));
    assert_eq!(loaded.project.forecast, settings);
    // A task without a range writes nothing for it.
    let json = serde_json::to_string(task(&loaded, 1)).unwrap();
    assert!(!json.contains("work_low"), "{json}");
}

#[test]
fn projects_and_tasks_written_before_the_forecast_read_with_defaults() {
    let mut value = serde_json::to_value(&doc().project).unwrap();
    let object = value.as_object_mut().unwrap();
    object.remove("forecast");
    let task = object["tasks"][0].as_object().unwrap();
    assert!(!task.contains_key("work_low"));
    let older: Project = serde_json::from_value(value).unwrap();
    assert_eq!(older.forecast, ForecastSettings::default());
    assert_eq!(older.tasks[0].work_low, None);
    // A settings object with no strengths reads as the defaults too.
    let partial: ForecastSettings = serde_json::from_str(r#"{"field":null}"#).unwrap();
    assert_eq!(partial, ForecastSettings::default());
}

#[test]
fn fill_down_and_paste_carry_the_range() {
    let mut doc = doc();
    range(&mut doc, 1, Some(DAY / 2), Some(2 * DAY)).unwrap();
    doc.apply(Command::FillDown { from_uid: 1, to_uids: vec![2, 3], field: FillField::WorkRange }).unwrap();
    assert_eq!(task(&doc, 3).work_high, Some(2 * DAY));
    let copied = copy_tasks(&doc.project, &[1]);
    doc.apply(Command::PasteTasks { after_uid: Some(3), clipboard: copied }).unwrap();
    let pasted = doc.project.tasks.last().unwrap();
    assert_eq!((pasted.work_low, pasted.work_high), (Some(DAY / 2), Some(2 * DAY)));
}

#[test]
fn fill_down_skips_groups_and_milestones() {
    let mut doc = doc();
    range(&mut doc, 3, Some(DAY / 2), Some(2 * DAY)).unwrap();
    doc.apply(Command::Indent { uid: 2 }).unwrap();
    doc.apply(Command::FillDown { from_uid: 3, to_uids: vec![1, 2], field: FillField::WorkRange }).unwrap();
    assert_eq!(task(&doc, 1).work_low, None);
    assert_eq!(task(&doc, 2).work_low, Some(DAY / 2));
}

#[test]
fn a_table_import_reads_ranges_and_checks_them() {
    let mut doc = doc();
    let row = |name: &str, low, high| ImportTask {
        name: name.into(),
        work: Some(DAY),
        work_low: low,
        work_high: high,
        ..ImportTask::default()
    };
    let errors = check_import(&doc.project, None, &[], &[row("Sai", Some(2 * DAY), Some(DAY)), row("Thiếu", Some(DAY), None)]);
    let columns: Vec<(Option<u32>, ImportColumn)> = errors.iter().map(|error| (error.row, error.column.clone())).collect();
    assert_eq!(columns, [(Some(0), ImportColumn::WorkHigh), (Some(1), ImportColumn::WorkHigh)]);
    doc.apply(Command::ImportRows { after_uid: None, resources: vec![], tasks: vec![row("Đúng", Some(DAY / 2), Some(3 * DAY))] })
        .unwrap();
    let imported = doc.project.tasks.last().unwrap();
    assert_eq!((imported.work_low, imported.work_high), (Some(DAY / 2), Some(3 * DAY)));
}

/// A fresh directory under the system temp dir, so tests do not share files.
fn tempfile_dir() -> std::path::PathBuf {
    let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("sheafly-forecast-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn mspdi_carries_ranges_as_two_duration_fields() {
    let mut doc = doc();
    range(&mut doc, 2, Some(DAY / 2), Some(3 * DAY)).unwrap();
    let xml = write_mspdi(&doc.project).unwrap();
    assert!(xml.contains("<FieldID>188743783</FieldID>\n<FieldName>Duration1</FieldName>"));
    assert!(xml.contains("<Alias>Effort thấp</Alias>"));
    assert!(xml.contains("<Alias>Effort cao</Alias>"));
    assert!(xml.contains("<FieldID>188743784</FieldID>\n<Value>PT24H0M0S</Value>"));

    let (imported, report) = read_mspdi_with_report(&xml).unwrap();
    assert!(report.notes.is_empty(), "{:?}", report.notes);
    assert!(imported.fields.is_empty());
    let back = imported.task(2).unwrap();
    assert_eq!((back.work_low, back.work_high), (Some(DAY / 2), Some(3 * DAY)));
    assert_eq!(imported.task(1).unwrap().work_low, None);

    // Without a range nothing is declared.
    range(&mut doc, 2, None, None).unwrap();
    assert!(!write_mspdi(&doc.project).unwrap().contains("<ExtendedAttributes>"));
}

#[test]
fn mspdi_import_reads_other_duration_slots_by_alias_and_drops_bad_ranges() {
    let mut doc = doc();
    range(&mut doc, 2, Some(DAY), Some(2 * DAY)).unwrap();
    range(&mut doc, 3, Some(DAY), Some(2 * DAY)).unwrap();
    let xml = write_mspdi(&doc.project)
        .unwrap()
        .replace("188743783", "188743785")
        .replace("188743784", "188743955")
        .replace("Effort thấp", "LOW EFFORT")
        .replace("Effort cao", "High effort");
    // Task 3's low comes after its high: Sheafly refuses that range, so it is dropped.
    let task3 = xml.find("<UID>3</UID>").unwrap();
    let low3 = task3 + xml[task3..].find("<Value>PT8H0M0S</Value>").unwrap();
    let xml = format!("{}<Value>PT40H0M0S</Value>{}", &xml[..low3], &xml[low3 + 23..]);

    let (imported, report) = read_mspdi_with_report(&xml).unwrap();
    assert!(report.notes.is_empty(), "{:?}", report.notes);
    let two = imported.task(2).unwrap();
    assert_eq!((two.work_low, two.work_high), (Some(DAY), Some(2 * DAY)));
    assert_eq!(imported.task(3).unwrap().work_low, None);
}

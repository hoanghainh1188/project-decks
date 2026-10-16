//! Which Backlog field each Sheafly field is read from and written to (phase 19, phase 4B).
//!
//! The source's sync settings name, per field, a field at the source to read
//! (`source_field`) and one to write (`push_field`) when it is not the usual one. A
//! reference is a Backlog attribute (`summary`, `estimatedHours`, `startDate`, `dueDate`,
//! `status`, `assignee`) or `custom:<name>`, a custom field of the project found by name
//! (letter case aside) in `GET /api/v2/projects/:key/customFields`. A start, due or
//! planned day may be read from and written to a **date** custom field; an effort may be
//! read from a **number** custom field, taken as story points and turned into hours by the
//! source's `hours_per_point`.
//!
//! `read_plan` checks the references once, against the project's definitions, so a wrong
//! name or a field of the wrong kind is an error before anything is pulled or pushed. The
//! codes for the kinds of custom field are not in the public API docs: `TYPE_DATE` and
//! `TYPE_NUMBER` are what Backlog is known to use and must be confirmed on a real space;
//! a field of any other kind is refused, never guessed.

use std::collections::HashMap;

use serde_json::Value;
use sheafly_engine::fields::FieldDef;
use sheafly_engine::source::{Source, SyncSettings};

use crate::tracker::{Api, TrackerError};

/// `typeId` of a number custom field (unverified; see the module note).
pub const TYPE_NUMBER: i64 = 3;
/// `typeId` of a date custom field (unverified; see the module note).
pub const TYPE_DATE: i64 = 4;
/// Longest custom field name accepted in a reference.
const MAX_NAME_CHARS: usize = 100;

/// A custom field of the project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomFieldDef {
    pub id: i64,
    pub name: String,
    pub type_id: i64,
}

/// A reference to a field at the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldRef {
    /// A Backlog attribute, by its name in the API.
    Native(String),
    /// A custom field, by name.
    Custom(String),
}

pub fn parse_ref(text: &str) -> Result<FieldRef, TrackerError> {
    let text = text.trim();
    match text.strip_prefix("custom:") {
        Some(name) => {
            let name = name.trim();
            if name.is_empty() || name.chars().count() > MAX_NAME_CHARS || name.chars().any(char::is_control) {
                Err(TrackerError::BadField)
            } else {
                Ok(FieldRef::Custom(name.to_string()))
            }
        }
        None if text.is_empty() || text.chars().any(|c| !c.is_ascii_alphanumeric()) => Err(TrackerError::BadField),
        None => Ok(FieldRef::Native(text.to_string())),
    }
}

/// The project's custom fields.
pub fn custom_fields(api: &dyn Api, project_key: &str) -> Result<Vec<CustomFieldDef>, TrackerError> {
    let body = api.get(&format!("/api/v2/projects/{project_key}/customFields"), &[])?;
    let list = body.as_array().ok_or(TrackerError::BadResponse)?;
    Ok(list
        .iter()
        .filter_map(|field| {
            Some(CustomFieldDef {
                id: field.get("id")?.as_i64()?,
                name: field.get("name")?.as_str()?.trim().to_string(),
                type_id: field.get("typeId").and_then(Value::as_i64).unwrap_or(0),
            })
        })
        .collect())
}

fn find<'a>(defs: &'a [CustomFieldDef], name: &str) -> Option<&'a CustomFieldDef> {
    let lower = name.trim().to_lowercase();
    defs.iter().find(|def| def.name.to_lowercase() == lower)
}

/// A custom field of the kind `type_id`, by name.
fn custom_of_kind<'a>(defs: &'a [CustomFieldDef], name: &str, type_id: i64) -> Result<&'a CustomFieldDef, TrackerError> {
    find(defs, name).filter(|def| def.type_id == type_id).ok_or(TrackerError::BadField)
}

/// Where a day is read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DateSource {
    /// The attribute `startDate` or `dueDate`.
    Native(&'static str),
    /// A date custom field, by name.
    Custom(String),
}

/// Where effort is read from.
#[derive(Clone, Debug, PartialEq)]
pub enum WorkSource {
    /// `estimatedHours`.
    Hours,
    /// A number custom field, by name, in story points of `hours_per_point` hours each.
    Points { field: String, hours_per_point: f64 },
}

/// What the sync reads, resolved. `standard` is the Backlog default.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadPlan {
    pub work: WorkSource,
    pub start: DateSource,
    pub due: DateSource,
    /// The date custom fields the planned days are pushed to, read to compare with.
    pub planned_start: Option<String>,
    pub planned_finish: Option<String>,
    /// The hours a start date and a due date become.
    pub start_hour: u32,
    pub due_hour: u32,
    /// The name at the source for a project custom field whose policy names another.
    renamed: HashMap<u32, String>,
}

impl ReadPlan {
    /// What is read with no settings: the usual attributes at the default hours.
    #[cfg(test)]
    pub fn standard(source: &Source) -> Self {
        Self::from_defaults(&source.sync)
    }

    fn from_defaults(sync: &SyncSettings) -> Self {
        Self {
            work: WorkSource::Hours,
            start: DateSource::Native("startDate"),
            due: DateSource::Native("dueDate"),
            planned_start: None,
            planned_finish: None,
            start_hour: u32::from(sync.start_hour),
            due_hour: u32::from(sync.due_hour),
            renamed: HashMap::new(),
        }
    }

    /// The name at the source of a project custom field: the one its policy names, else its own.
    pub fn name_for<'a>(&'a self, field: &'a FieldDef) -> &'a str {
        self.renamed.get(&field.id).map_or(field.name.as_str(), String::as_str)
    }
}

/// Whether the settings refer to custom fields at the source, so their definitions are needed.
pub fn needs_definitions(sync: &SyncSettings) -> bool {
    sync.fields.iter().any(|policy| {
        [&policy.source_field, &policy.push_field]
            .iter()
            .filter_map(|reference| reference.as_deref())
            .any(|reference| reference.trim_start().starts_with("custom:"))
    })
}

/// Resolve and check the source's references against the project's custom fields.
pub fn read_plan(source: &Source, project_fields: &[FieldDef], defs: &[CustomFieldDef]) -> Result<ReadPlan, TrackerError> {
    let sync = &source.sync;
    let mut plan = ReadPlan::from_defaults(sync);
    let policy = |key: &str| sync.fields.iter().find(|policy| policy.field == key);

    // The built-in attributes that have only one source.
    for (key, native) in [("name", "summary"), ("status", "status"), ("assignee", "assignee")] {
        if let Some(reference) = policy(key).and_then(|policy| policy.source_field.as_deref())
            && parse_ref(reference)? != FieldRef::Native(native.to_string())
        {
            return Err(TrackerError::BadField);
        }
    }
    if let Some(reference) = policy("work").and_then(|policy| policy.source_field.as_deref()) {
        plan.work = match parse_ref(reference)? {
            FieldRef::Native(name) if name == "estimatedHours" => WorkSource::Hours,
            FieldRef::Custom(name) => {
                let def = custom_of_kind(defs, &name, TYPE_NUMBER)?;
                WorkSource::Points { field: def.name.clone(), hours_per_point: source.hours_per_point }
            }
            FieldRef::Native(_) => return Err(TrackerError::BadField),
        };
    }
    for (key, native, slot) in [("start", "startDate", &mut plan.start), ("due", "dueDate", &mut plan.due)] {
        if let Some(reference) = policy(key).and_then(|policy| policy.source_field.as_deref()) {
            *slot = match parse_ref(reference)? {
                FieldRef::Native(name) if name == native => DateSource::Native(native),
                FieldRef::Custom(name) => DateSource::Custom(custom_of_kind(defs, &name, TYPE_DATE)?.name.clone()),
                FieldRef::Native(_) => return Err(TrackerError::BadField),
            };
        }
    }
    for (key, slot) in [("planned_start", &mut plan.planned_start), ("planned_finish", &mut plan.planned_finish)] {
        if let Some(reference) = policy(key).and_then(|policy| policy.push_field.as_deref()) {
            match parse_ref(reference)? {
                FieldRef::Custom(name) => *slot = Some(custom_of_kind(defs, &name, TYPE_DATE)?.name.clone()),
                FieldRef::Native(_) => return Err(TrackerError::BadField),
            }
        }
    }
    for field in project_fields {
        if let Some(reference) = policy(&format!("field:{}", field.id)).and_then(|policy| policy.source_field.as_deref()) {
            match parse_ref(reference)? {
                FieldRef::Custom(name) => {
                    let def = find(defs, &name).ok_or(TrackerError::BadField)?;
                    plan.renamed.insert(field.id, def.name.clone());
                }
                FieldRef::Native(_) => return Err(TrackerError::BadField),
            }
        }
    }
    Ok(plan)
}

/// The plan for reading the source's issues, and the project's custom field definitions it
/// was checked against (fetched only when the settings refer to custom fields).
pub fn plan_for(
    api: &dyn Api,
    source: &Source,
    project_fields: &[FieldDef],
) -> Result<(ReadPlan, Vec<CustomFieldDef>), TrackerError> {
    let defs = if needs_definitions(&source.sync) {
        custom_fields(api, &source.project_key)?
    } else {
        Vec::new()
    };
    Ok((read_plan(source, project_fields, &defs)?, defs))
}

/// The ids of the date custom fields a push writes to, by lower-cased name; an error if a
/// change names a field that is not a date field of the project.
pub fn push_targets(defs: &[CustomFieldDef], references: &[&str]) -> Result<HashMap<String, i64>, TrackerError> {
    let mut ids = HashMap::new();
    for reference in references {
        match parse_ref(reference)? {
            FieldRef::Custom(name) => {
                let def = custom_of_kind(defs, &name, TYPE_DATE)?;
                ids.insert(def.name.to_lowercase(), def.id);
            }
            FieldRef::Native(_) => return Err(TrackerError::BadField),
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use sheafly_engine::fields::FieldKind;
    use sheafly_engine::source::{Direction, FieldPolicy, OnConflict, TrackerKind};

    use super::*;

    fn defs() -> Vec<CustomFieldDef> {
        vec![
            CustomFieldDef { id: 11, name: "Bắt đầu dự kiến".into(), type_id: TYPE_DATE },
            CustomFieldDef { id: 12, name: "Điểm".into(), type_id: TYPE_NUMBER },
            CustomFieldDef { id: 13, name: "Ghi chú".into(), type_id: 1 },
        ]
    }

    fn policy(field: &str, source_field: Option<&str>, push_field: Option<&str>) -> FieldPolicy {
        FieldPolicy {
            field: field.into(),
            direction: Direction::Both,
            on_conflict: OnConflict::Ask,
            source_field: source_field.map(str::to_string),
            push_field: push_field.map(str::to_string),
        }
    }

    fn source(fields: Vec<FieldPolicy>) -> Source {
        Source {
            kind: TrackerKind::Backlog,
            site: "team.backlog.com".into(),
            project_key: "PROJ".into(),
            filter: String::new(),
            hours_per_point: 2.0,
            status_map: vec![],
            sync: SyncSettings { fields, start_hour: 9, due_hour: 18, ..SyncSettings::default() },
        }
    }

    #[test]
    fn references_are_attributes_or_custom_fields_by_name() {
        assert_eq!(parse_ref("startDate").unwrap(), FieldRef::Native("startDate".into()));
        assert_eq!(parse_ref(" custom: Bắt đầu dự kiến ").unwrap(), FieldRef::Custom("Bắt đầu dự kiến".into()));
        for bad in ["", "custom:", "custom:   ", "start date", "a/b", "../x", "custom:\u{7}"] {
            assert_eq!(parse_ref(bad), Err(TrackerError::BadField), "{bad:?}");
        }
        assert_eq!(parse_ref(&format!("custom:{}", "x".repeat(101))), Err(TrackerError::BadField));
    }

    #[test]
    fn without_settings_the_usual_attributes_are_read_at_the_set_hours() {
        let plan = read_plan(&source(vec![]), &[], &[]).unwrap();
        assert_eq!(plan, ReadPlan { start_hour: 9, due_hour: 18, ..ReadPlan::standard(&source(vec![])) });
        assert_eq!((plan.work.clone(), plan.start.clone(), plan.due.clone()), (WorkSource::Hours, DateSource::Native("startDate"), DateSource::Native("dueDate")));
        assert!(!needs_definitions(&source(vec![]).sync));
    }

    #[test]
    fn dates_and_points_can_come_from_custom_fields_of_the_right_kind() {
        let settings = source(vec![
            policy("start", Some("custom:bắt đầu dự kiến"), None),
            policy("work", Some("custom:Điểm"), None),
        ]);
        assert!(needs_definitions(&settings.sync));
        let plan = read_plan(&settings, &[], &defs()).unwrap();
        assert_eq!(plan.start, DateSource::Custom("Bắt đầu dự kiến".into()));
        assert_eq!(plan.work, WorkSource::Points { field: "Điểm".into(), hours_per_point: 2.0 });
        assert_eq!(plan.due, DateSource::Native("dueDate"));
    }

    #[test]
    fn a_missing_or_wrongly_typed_field_is_refused() {
        for fields in [
            vec![policy("start", Some("custom:Không có"), None)],
            vec![policy("start", Some("custom:Điểm"), None)],
            vec![policy("work", Some("custom:Bắt đầu dự kiến"), None)],
            vec![policy("due", Some("startDate"), None)],
            vec![policy("name", Some("description"), None)],
            vec![policy("planned_start", None, Some("startDate"))],
            vec![policy("planned_finish", None, Some("custom:Ghi chú"))],
        ] {
            assert_eq!(read_plan(&source(fields), &[], &defs()), Err(TrackerError::BadField));
        }
    }

    #[test]
    fn planned_days_are_read_from_the_field_they_are_pushed_to() {
        let settings = source(vec![policy("planned_start", None, Some("custom:Bắt đầu dự kiến"))]);
        let plan = read_plan(&settings, &[], &defs()).unwrap();
        assert_eq!(plan.planned_start.as_deref(), Some("Bắt đầu dự kiến"));
        assert_eq!(plan.planned_finish, None);
    }

    #[test]
    fn a_project_custom_field_can_be_read_from_a_field_with_another_name() {
        let fields = vec![FieldDef { id: 7, name: "Loại".into(), kind: FieldKind::Text }];
        let settings = source(vec![policy("field:7", Some("custom:Ghi chú"), None)]);
        let plan = read_plan(&settings, &fields, &defs()).unwrap();
        assert_eq!(plan.name_for(&fields[0]), "Ghi chú");
        let plain = read_plan(&source(vec![]), &fields, &defs()).unwrap();
        assert_eq!(plain.name_for(&fields[0]), "Loại");
    }

    #[test]
    fn push_targets_are_date_fields_only() {
        let ids = push_targets(&defs(), &["custom:Bắt đầu dự kiến"]).unwrap();
        assert_eq!(ids.get("bắt đầu dự kiến"), Some(&11));
        assert_eq!(push_targets(&defs(), &["custom:Điểm"]), Err(TrackerError::BadField));
        assert_eq!(push_targets(&defs(), &["startDate"]), Err(TrackerError::BadField));
    }
}

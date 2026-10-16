//! Which Jira field each Sheafly field is read from (phase 19, phase 4D-2).
//!
//! Jira has no fixed start date or story points: they are custom fields whose ids differ per
//! site. The sync settings name them, as a reference `custom:<name>` found by name (letter
//! case aside) in `GET /rest/api/3/field`, with a kind (`schema.type`) the sync checks: a
//! **date** (or datetime) field for a day, a **number** field for effort taken as story points
//! and turned into hours by `hours_per_point`. Without settings Jira's own `duedate` is the
//! deadline and the original estimate (`timetracking.originalEstimateSeconds`) the effort.
//! `read_plan` checks the references once against the site's field list, so a wrong name or
//! kind is an error before anything is pulled.

use std::collections::HashMap;

use serde_json::Value;
use sheafly_engine::fields::FieldDef;
use sheafly_engine::source::Source;

use crate::backlog_fields::{parse_ref, FieldRef};
use crate::tracker::{Api, TrackerError};

/// A field of the Jira site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JiraField {
    /// The id used in an issue's `fields`, such as `duedate` or `customfield_10016`.
    pub id: String,
    pub name: String,
    /// `schema.type`: `date`, `datetime`, `number`, `string`…
    pub kind: String,
}

/// The fields of the site.
pub fn fields(api: &dyn Api) -> Result<Vec<JiraField>, TrackerError> {
    let body = api.get("/rest/api/3/field", &[])?;
    let list = body.as_array().ok_or(TrackerError::BadResponse)?;
    Ok(list
        .iter()
        .filter_map(|field| {
            Some(JiraField {
                id: field.get("id")?.as_str()?.to_string(),
                name: field.get("name")?.as_str()?.trim().to_string(),
                kind: field.pointer("/schema/type").and_then(Value::as_str).unwrap_or("").to_string(),
            })
        })
        .collect())
}

fn find<'a>(defs: &'a [JiraField], name: &str) -> Option<&'a JiraField> {
    let lower = name.trim().to_lowercase();
    defs.iter().find(|def| def.name.to_lowercase() == lower)
}

fn of_kind<'a>(defs: &'a [JiraField], name: &str, kinds: &[&str]) -> Result<&'a JiraField, TrackerError> {
    find(defs, name).filter(|def| kinds.contains(&def.kind.as_str())).ok_or(TrackerError::BadField)
}

const DATE_KINDS: &[&str] = &["date", "datetime"];

/// Where effort is read from.
#[derive(Clone, Debug, PartialEq)]
pub enum WorkSource {
    /// The original estimate, in seconds.
    Estimate,
    /// A number field, in story points of `hours_per_point` hours each.
    Points { field: String, hours_per_point: f64 },
}

/// What the sync reads, resolved. Field values are ids in an issue's `fields`.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadPlan {
    pub work: WorkSource,
    /// The date field of a start; Jira has no standard one.
    pub start: Option<String>,
    /// The field of a due date: `duedate` or a date field.
    pub due: String,
    pub planned_start: Option<String>,
    pub planned_finish: Option<String>,
    pub start_hour: u32,
    pub due_hour: u32,
    /// The Jira field of each project custom field that exists on the site (by project field id).
    pub custom: HashMap<u32, String>,
    /// Every field the search asks for.
    pub request: Vec<String>,
}

/// Resolve and check the source's references against the site's fields.
pub fn read_plan(source: &Source, project_fields: &[FieldDef], defs: &[JiraField]) -> Result<ReadPlan, TrackerError> {
    let sync = &source.sync;
    let policy = |key: &str| sync.fields.iter().find(|policy| policy.field == key);
    let mut plan = ReadPlan {
        work: WorkSource::Estimate,
        start: None,
        due: "duedate".to_string(),
        planned_start: None,
        planned_finish: None,
        start_hour: u32::from(sync.start_hour),
        due_hour: u32::from(sync.due_hour),
        custom: HashMap::new(),
        request: ["summary", "status", "assignee", "duedate", "timetracking", "parent", "issuelinks"]
            .iter()
            .map(|field| field.to_string())
            .collect(),
    };

    for (key, native) in [("name", "summary"), ("status", "status"), ("assignee", "assignee")] {
        if let Some(reference) = policy(key).and_then(|policy| policy.source_field.as_deref())
            && parse_ref(reference)? != FieldRef::Native(native.to_string())
        {
            return Err(TrackerError::BadField);
        }
    }
    if let Some(reference) = policy("work").and_then(|policy| policy.source_field.as_deref()) {
        plan.work = match parse_ref(reference)? {
            FieldRef::Native(name) if name == "originalEstimate" => WorkSource::Estimate,
            FieldRef::Custom(name) => {
                let def = of_kind(defs, &name, &["number"])?;
                plan.request.push(def.id.clone());
                WorkSource::Points { field: def.id.clone(), hours_per_point: source.hours_per_point }
            }
            FieldRef::Native(_) => return Err(TrackerError::BadField),
        };
    }
    if let Some(reference) = policy("start").and_then(|policy| policy.source_field.as_deref()) {
        match parse_ref(reference)? {
            FieldRef::Custom(name) => {
                let def = of_kind(defs, &name, DATE_KINDS)?;
                plan.request.push(def.id.clone());
                plan.start = Some(def.id.clone());
            }
            FieldRef::Native(_) => return Err(TrackerError::BadField),
        }
    }
    if let Some(reference) = policy("due").and_then(|policy| policy.source_field.as_deref()) {
        plan.due = match parse_ref(reference)? {
            FieldRef::Native(name) if name == "duedate" => "duedate".to_string(),
            FieldRef::Custom(name) => {
                let def = of_kind(defs, &name, DATE_KINDS)?;
                plan.request.push(def.id.clone());
                def.id.clone()
            }
            FieldRef::Native(_) => return Err(TrackerError::BadField),
        };
    }
    for (key, slot) in [("planned_start", &mut plan.planned_start), ("planned_finish", &mut plan.planned_finish)] {
        if let Some(reference) = policy(key).and_then(|policy| policy.push_field.as_deref()) {
            match parse_ref(reference)? {
                FieldRef::Custom(name) => {
                    let def = of_kind(defs, &name, DATE_KINDS)?;
                    plan.request.push(def.id.clone());
                    *slot = Some(def.id.clone());
                }
                FieldRef::Native(_) => return Err(TrackerError::BadField),
            }
        }
    }
    for field in project_fields {
        // The field the project's custom field reads from: the one its policy names, else the
        // site field of the same name, if there is one.
        let name = match policy(&format!("field:{}", field.id)).and_then(|policy| policy.source_field.as_deref()) {
            Some(reference) => match parse_ref(reference)? {
                FieldRef::Custom(name) => {
                    find(defs, &name).ok_or(TrackerError::BadField)?;
                    name
                }
                FieldRef::Native(_) => return Err(TrackerError::BadField),
            },
            None => field.name.clone(),
        };
        if let Some(def) = find(defs, &name) {
            plan.request.push(def.id.clone());
            plan.custom.insert(field.id, def.id.clone());
        }
    }
    plan.request.sort();
    plan.request.dedup();
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use sheafly_engine::fields::FieldKind;
    use sheafly_engine::source::{Direction, FieldPolicy, OnConflict, SyncSettings, TrackerKind};

    use super::*;

    fn defs() -> Vec<JiraField> {
        let field = |id: &str, name: &str, kind: &str| JiraField { id: id.into(), name: name.into(), kind: kind.into() };
        vec![
            field("duedate", "Due date", "date"),
            field("customfield_10015", "Start date", "date"),
            field("customfield_10016", "Story point estimate", "number"),
            field("customfield_10020", "Kế hoạch", "date"),
            field("customfield_10030", "Mảng", "string"),
            field("customfield_10031", "Nhóm", "string"),
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
            kind: TrackerKind::Jira,
            site: "team.atlassian.net".into(),
            project_key: "PROJ".into(),
            filter: String::new(),
            hours_per_point: 3.0,
            status_map: vec![],
            sync: SyncSettings { fields, start_hour: 9, due_hour: 18, ..SyncSettings::default() },
        }
    }

    #[test]
    fn without_settings_jiras_own_fields_are_read() {
        let plan = read_plan(&source(vec![]), &[], &defs()).unwrap();
        assert_eq!((plan.work.clone(), plan.start.clone(), plan.due.as_str()), (WorkSource::Estimate, None, "duedate"));
        assert_eq!((plan.start_hour, plan.due_hour), (9, 18));
        assert!(plan.request.contains(&"timetracking".to_string()) && plan.request.contains(&"issuelinks".to_string()));
    }

    #[test]
    fn a_start_date_and_story_points_come_from_custom_fields_of_the_right_kind() {
        let settings = source(vec![policy("start", Some("custom:start DATE"), None), policy("work", Some("custom:Story point estimate"), None)]);
        let plan = read_plan(&settings, &[], &defs()).unwrap();
        assert_eq!(plan.start.as_deref(), Some("customfield_10015"));
        assert_eq!(plan.work, WorkSource::Points { field: "customfield_10016".into(), hours_per_point: 3.0 });
        assert!(plan.request.contains(&"customfield_10015".to_string()) && plan.request.contains(&"customfield_10016".to_string()));
    }

    #[test]
    fn a_missing_or_wrongly_typed_field_is_refused() {
        for fields in [
            vec![policy("start", Some("custom:Không có"), None)],
            vec![policy("start", Some("startDate"), None)],
            vec![policy("start", Some("custom:Story point estimate"), None)],
            vec![policy("work", Some("custom:Start date"), None)],
            vec![policy("due", Some("dueDate"), None)],
            vec![policy("name", Some("description"), None)],
            vec![policy("planned_finish", None, Some("custom:Mảng"))],
            vec![policy("planned_finish", None, Some("duedate"))],
        ] {
            assert_eq!(read_plan(&source(fields), &[], &defs()), Err(TrackerError::BadField));
        }
    }

    #[test]
    fn the_due_date_and_the_planned_days_can_be_other_date_fields() {
        let settings = source(vec![policy("due", Some("custom:Start date"), None), policy("planned_start", None, Some("custom:Kế hoạch"))]);
        let plan = read_plan(&settings, &[], &defs()).unwrap();
        assert_eq!(plan.due, "customfield_10015");
        assert_eq!(plan.planned_start.as_deref(), Some("customfield_10020"));
    }

    #[test]
    fn a_project_custom_field_is_read_from_the_site_field_of_its_name_or_the_one_its_policy_names() {
        let fields = vec![
            FieldDef { id: 1, name: "mảng".into(), kind: FieldKind::Text },
            FieldDef { id: 2, name: "Không có ở Jira".into(), kind: FieldKind::Text },
            FieldDef { id: 3, name: "Khác".into(), kind: FieldKind::Text },
        ];
        let settings = source(vec![policy("field:3", Some("custom:Nhóm"), None)]);
        let plan = read_plan(&settings, &fields, &defs()).unwrap();
        assert_eq!(plan.custom.get(&1).map(String::as_str), Some("customfield_10030"));
        assert_eq!(plan.custom.get(&2), None, "a field Jira does not have is left alone");
        assert_eq!(plan.custom.get(&3).map(String::as_str), Some("customfield_10031"));
        assert_eq!(read_plan(&source(vec![policy("field:3", Some("custom:Không có"), None)]), &fields, &defs()), Err(TrackerError::BadField));
    }
}

//! The Backlog (Nulab) adapter: API v2, read only.
//!
//! `fetch` lists the issues of one project in pages of 100 and `convert` turns them into
//! the engine's `PullTask` rows. The mapping:
//!
//! - `issueKey` is the key, `summary` the name;
//! - `estimatedHours` is the effort (zero or missing: no estimate);
//! - `startDate` and `dueDate` are dates; the start becomes `start_hour` (08:00) and the due
//!   date `due_hour` (17:00) of that day (the engine moves a start past non-working time);
//!   the source's sync settings may read them, or the effort (as story points), from a custom
//!   field instead (`backlog_fields.rs`);
//! - `parentIssueId` is the parent when that issue is in the pull (a parent outside the
//!   filter is ignored), and parents come before their children;
//! - `status` becomes a status class through the source's status map by name, else the
//!   four statuses every Backlog space has (Open, In Progress, Resolved, Closed); a
//!   custom status the map does not list says nothing;
//! - `assignee.mailAddress` is the assignee's email; Backlog sends it only to those allowed
//!   to see it, and an assignee without one is reported as a warning.
//!
//! - `customFields` fill the project's custom fields of the same name (letter case aside):
//!   a number field takes a number, a text or choice field the text, the name of a list item
//!   or the names of several joined by commas. Only a field that some issue of the pull has
//!   at the source is touched, so a field that exists only in Sheafly keeps its values; an
//!   issue with no value for such a field clears it. The public API docs show only an empty
//!   `customFields` list, so the items are read tolerantly (`name`, and a `value` that is
//!   text, a number, an object with a `name`, or a list of those).
//!
//! Backlog has no "blocks" links, so `after` is not used here.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{Datelike, NaiveDate};
use serde_json::Value;
use sheafly_engine::fields::{FieldDef, FieldKind, FieldValue};
use sheafly_engine::pull::{PullTask, MAX_PULL_ROWS};
use sheafly_engine::source::{Source, StatusClass};
use sheafly_engine::timeutil::timestamp;

use crate::backlog_fields::{DateSource, ReadPlan, WorkSource};
use crate::connector::{link_parents, Converted, Progress, PullWarning, WarningKind};
use crate::tracker::{Api, TrackerError};

/// Issues one request returns at most.
const PAGE: usize = 100;

/// Filter parameters a user may give; anything else (the key, the paging, the project)
/// stays under the app's control.
const FILTER_NAMES: &[&str] = &[
    "statusId[]",
    "assigneeId[]",
    "categoryId[]",
    "versionId[]",
    "milestoneId[]",
    "issueTypeId[]",
    "priorityId[]",
    "resolutionId[]",
    "createdUserId[]",
    "parentChild",
    "keyword",
    "createdSince",
    "createdUntil",
    "updatedSince",
    "updatedUntil",
    "startDateSince",
    "startDateUntil",
    "dueDateSince",
    "dueDateUntil",
];

/// A project key or id is safe in a path.
fn path_safe(project: &str) -> bool {
    !project.is_empty()
        && project.len() <= 100
        && project.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// `a=b&c=d` as pairs; only the names in `FILTER_NAMES`. Empty gives none.
pub fn parse_filter(filter: &str) -> Result<Vec<(String, String)>, TrackerError> {
    let filter = filter.trim().trim_start_matches('?');
    let mut pairs = Vec::new();
    for part in filter.split('&').map(str::trim).filter(|part| !part.is_empty()) {
        let (name, value) = part.split_once('=').ok_or(TrackerError::BadFilter)?;
        let (name, value) = (name.trim(), value.trim());
        if !FILTER_NAMES.contains(&name)
            || value.is_empty()
            || value.chars().count() > 100
            || value.chars().any(char::is_control)
            || pairs.len() >= 30
        {
            return Err(TrackerError::BadFilter);
        }
        pairs.push((name.to_string(), value.to_string()));
    }
    Ok(pairs)
}

/// The project's numeric id and name; also proves the key works.
pub fn project(api: &dyn Api, project_key: &str) -> Result<(i64, String), TrackerError> {
    if !path_safe(project_key) {
        return Err(TrackerError::BadProject);
    }
    let body = api.get(&format!("/api/v2/projects/{project_key}"), &[])?;
    let id = body.get("id").and_then(Value::as_i64).ok_or(TrackerError::BadResponse)?;
    let name = body.get("name").and_then(Value::as_str).unwrap_or(project_key);
    Ok((id, name.to_string()))
}

/// Every issue of the source's project that the filter lets through. `progress` hears
/// after each page; `cancel` is looked at before each request.
pub fn fetch(
    api: &dyn Api,
    source: &Source,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Vec<Value>, TrackerError> {
    let filter = parse_filter(&source.filter)?;
    let stop = || {
        if cancel.load(Ordering::Relaxed) {
            Err(TrackerError::Cancelled)
        } else {
            Ok(())
        }
    };
    stop()?;
    let (project_id, _) = project(api, &source.project_key)?;
    let mut query = vec![("projectId[]".to_string(), project_id.to_string())];
    query.extend(filter);

    stop()?;
    let total = api
        .get("/api/v2/issues/count", &query)?
        .get("count")
        .and_then(Value::as_u64)
        .ok_or(TrackerError::BadResponse)? as usize;
    if total > MAX_PULL_ROWS {
        return Err(TrackerError::TooManyIssues);
    }
    progress(Progress { fetched: 0, total: Some(total) });

    query.push(("sort".into(), "created".into()));
    query.push(("order".into(), "asc".into()));
    query.push(("count".into(), PAGE.to_string()));
    let mut issues: Vec<Value> = Vec::with_capacity(total);
    // One more page than the count says is enough to see the end; more means the tracker
    // is not behaving, and the loop must still end.
    for _ in 0..=(total / PAGE + 2) {
        stop()?;
        let mut page_query = query.clone();
        page_query.push(("offset".into(), issues.len().to_string()));
        let page = api.get("/api/v2/issues", &page_query)?;
        let page = page.as_array().ok_or(TrackerError::BadResponse)?;
        issues.extend(page.iter().cloned());
        if issues.len() > MAX_PULL_ROWS {
            return Err(TrackerError::TooManyIssues);
        }
        progress(Progress { fetched: issues.len(), total: Some(total.max(issues.len())) });
        if page.len() < PAGE {
            break;
        }
    }
    Ok(issues)
}

/// The value of the custom field called `name` (letter case aside) on an issue.
fn custom_value<'a>(issue: &'a Value, name: &str) -> Option<&'a Value> {
    let lower = name.trim().to_lowercase();
    issue
        .get("customFields")
        .and_then(Value::as_array)?
        .iter()
        .find(|entry| entry.get("name").and_then(Value::as_str).is_some_and(|n| n.trim().to_lowercase() == lower))
        .and_then(|entry| entry.get("value"))
}

/// A day an issue holds, from the attribute or the date custom field the plan names, as
/// `hour:00` of that day.
fn date_of(issue: &Value, source: &DateSource, hour: u32) -> Option<i64> {
    match source {
        DateSource::Native(attribute) => instant_of(issue.get(*attribute), hour),
        DateSource::Custom(name) => instant_of(custom_value(issue, name), hour),
    }
}

/// A number in a value: a number, or text that reads as one.
fn number_of(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().replace(',', ".").parse().ok(),
        _ => None,
    }
    .filter(|number| number.is_finite())
}

/// The effort of an issue in minutes: its estimated hours, or story points of a number
/// custom field times the hours one point stands for. A point value that is no number is
/// reported and leaves the effort unset.
fn work_of(issue: &Value, reads: &ReadPlan, key: &str, warnings: &mut Vec<PullWarning>) -> Option<i64> {
    match &reads.work {
        WorkSource::Hours => minutes_of(issue.get("estimatedHours")),
        WorkSource::Points { field, hours_per_point } => {
            let value = custom_value(issue, field).filter(|value| !value_text(value).is_empty())?;
            match number_of(value) {
                Some(points) => minutes_of(Some(&Value::from(points * hours_per_point))),
                None => {
                    warnings.push(PullWarning { key: key.to_string(), kind: WarningKind::FieldValue });
                    None
                }
            }
        }
    }
}

/// The date part of a Backlog timestamp (`2026-01-05T00:00:00Z`) as `hour:00` that day.
fn instant_of(text: Option<&Value>, hour: u32) -> Option<i64> {
    let text = text?.as_str()?;
    let day = text.get(..10)?;
    let date = NaiveDate::parse_from_str(day, "%Y-%m-%d").ok()?;
    Some(timestamp(date.year(), date.month(), date.day(), hour, 0))
}

/// Minutes of an estimate in hours; none for a missing, zero, negative or absurd one.
fn minutes_of(hours: Option<&Value>) -> Option<i64> {
    let hours = hours?.as_f64()?;
    (hours.is_finite() && hours > 0.0 && hours <= 100_000.0).then(|| (hours * 60.0).round() as i64)
}

/// What the issue's status means: the source's map by name (letter case aside), else the
/// four built-in statuses by id.
fn status_class(status: Option<&Value>, source: &Source) -> Option<StatusClass> {
    let status = status?;
    if let Some(name) = status.get("name").and_then(Value::as_str) {
        let lower = name.trim().to_lowercase();
        if let Some(mapping) = source.status_map.iter().find(|m| m.status.to_lowercase() == lower) {
            return Some(mapping.class);
        }
    }
    match status.get("id").and_then(Value::as_i64)? {
        1 => Some(StatusClass::NotStarted),
        2 => Some(StatusClass::InProgress),
        3 | 4 => Some(StatusClass::Done),
        _ => None,
    }
}

/// The issues as rows, parents before their children, otherwise in the order given.
pub fn convert(issues: &[Value], source: &Source, fields: &[FieldDef], reads: &ReadPlan) -> Converted {
    let mut out = Converted::default();
    let at_source = fields_at_source(issues, fields, reads);
    let mut seen_keys = HashSet::new();
    // The id of each issue we keep, to find parents.
    let mut ids: HashMap<i64, usize> = HashMap::new();
    let mut parent_ids: Vec<Option<i64>> = Vec::new();
    for issue in issues {
        let key = issue.get("issueKey").and_then(Value::as_str).unwrap_or("").trim();
        let name = issue.get("summary").and_then(Value::as_str).unwrap_or("").trim();
        if key.is_empty() || name.is_empty() || !seen_keys.insert(key.to_string()) {
            out.warnings.push(PullWarning { key: key.to_string(), kind: WarningKind::Skipped });
            continue;
        }
        let assignee = issue.get("assignee").filter(|assignee| !assignee.is_null());
        let email = assignee
            .and_then(|assignee| assignee.get("mailAddress"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|email| !email.is_empty());
        if assignee.is_some() && email.is_none() {
            out.warnings.push(PullWarning { key: key.to_string(), kind: WarningKind::NoEmail });
        }
        if let Some(id) = issue.get("id").and_then(Value::as_i64) {
            ids.insert(id, out.rows.len());
        }
        parent_ids.push(issue.get("parentIssueId").and_then(Value::as_i64));
        let values = custom_values(issue, &at_source, reads, key, &mut out.warnings);
        let work = work_of(issue, reads, key, &mut out.warnings);
        out.rows.push(PullTask {
            key: key.to_string(),
            parent_key: None,
            name: name.to_string(),
            work,
            start: date_of(issue, &reads.start, reads.start_hour),
            due: date_of(issue, &reads.due, reads.due_hour),
            status: status_class(issue.get("status"), source),
            assignee_email: email.map(str::to_string),
            assignee_name: None,
            values,
            after: Vec::new(),
            planned_start: reads.planned_start.as_deref().and_then(|name| instant_of(custom_value(issue, name), reads.start_hour)),
            planned_finish: reads.planned_finish.as_deref().and_then(|name| instant_of(custom_value(issue, name), reads.due_hour)),
        });
    }
    let parents: Vec<Option<usize>> = parent_ids
        .iter()
        .enumerate()
        .map(|(row, parent)| parent.and_then(|id| ids.get(&id).copied()).filter(|p| *p != row))
        .collect();
    link_parents(&mut out.rows, &parents);
    out
}

/// The custom fields of the project that some issue has at the source, by lower-cased name.
fn fields_at_source<'a>(issues: &[Value], fields: &'a [FieldDef], reads: &ReadPlan) -> Vec<&'a FieldDef> {
    let names: HashSet<String> = issues
        .iter()
        .flat_map(|issue| issue.get("customFields").and_then(Value::as_array).into_iter().flatten())
        .filter_map(|field| field.get("name").and_then(Value::as_str))
        .map(|name| name.trim().to_lowercase())
        .collect();
    fields.iter().filter(|field| names.contains(&reads.name_for(field).trim().to_lowercase())).collect()
}

/// The text of a custom field value: text, a number, the name of a list item, or several
/// of those joined by commas. Empty for nothing.
fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.trim().to_string(),
        Value::Number(number) => number.to_string(),
        Value::Object(item) => item.get("name").and_then(Value::as_str).unwrap_or("").trim().to_string(),
        Value::Array(items) => items.iter().map(value_text).filter(|text| !text.is_empty()).collect::<Vec<_>>().join(", "),
        _ => String::new(),
    }
}

/// What the issue says for each project field the source has. An empty value clears the
/// field (empty text clears any kind); a number field whose value is not a number is left
/// alone and reported.
fn custom_values(
    issue: &Value,
    at_source: &[&FieldDef],
    reads: &ReadPlan,
    key: &str,
    warnings: &mut Vec<PullWarning>,
) -> Vec<(String, FieldValue)> {
    let entries = issue.get("customFields").and_then(Value::as_array);
    let mut values = Vec::new();
    for field in at_source {
        let lower = reads.name_for(field).trim().to_lowercase();
        let text = entries
            .into_iter()
            .flatten()
            .find(|entry| entry.get("name").and_then(Value::as_str).is_some_and(|name| name.trim().to_lowercase() == lower))
            .and_then(|entry| entry.get("value"))
            .map(value_text)
            .unwrap_or_default();
        let value = match (&field.kind, text.is_empty()) {
            (_, true) => FieldValue::Text(String::new()),
            (FieldKind::Number, false) => match text.replace(',', ".").parse::<f64>() {
                Ok(number) if number.is_finite() => FieldValue::Number(number),
                _ => {
                    warnings.push(PullWarning { key: key.to_string(), kind: WarningKind::FieldValue });
                    continue;
                }
            },
            (_, false) => FieldValue::Text(text),
        };
        values.push((field.name.clone(), value));
    }
    values
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use sheafly_engine::source::{StatusMapping, TrackerKind};

    use super::*;

    const ISSUES: &str = include_str!("../fixtures/backlog_issues.json");

    fn source() -> Source {
        Source {
            kind: TrackerKind::Backlog,
            site: "team.backlog.com".into(),
            project_key: "PROJ".into(),
            filter: String::new(),
            hours_per_point: 4.0,
            status_map: vec![StatusMapping {
                status: "Đang review".into(),
                class: StatusClass::InProgress,
            }],
            sync: Default::default(),
        }
    }

    fn issues() -> Vec<Value> {
        serde_json::from_str(ISSUES).unwrap()
    }

    type Seen = Vec<(String, Vec<(String, String)>)>;

    /// A fake Backlog: the project, a count, and the issues in pages. It records the
    /// queries it saw.
    struct Fake {
        issues: Vec<Value>,
        seen: RefCell<Seen>,
    }

    impl Fake {
        fn new(issues: Vec<Value>) -> Self {
            Self { issues, seen: RefCell::new(Vec::new()) }
        }
    }

    impl Api for Fake {
        fn patch(&self, _: &str, _: &[(String, String)]) -> Result<Value, TrackerError> {
            Err(TrackerError::NotFound)
        }

        fn get(&self, path: &str, query: &[(String, String)]) -> Result<Value, TrackerError> {
            self.seen.borrow_mut().push((path.to_string(), query.to_vec()));
            let arg = |name: &str| query.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone());
            match path {
                "/api/v2/projects/PROJ" => Ok(serde_json::json!({ "id": 77, "projectKey": "PROJ", "name": "Dự án mẫu" })),
                "/api/v2/projects/NOPE" => Err(TrackerError::NotFound),
                "/api/v2/issues/count" => Ok(serde_json::json!({ "count": self.issues.len() })),
                "/api/v2/issues" => {
                    let offset: usize = arg("offset").unwrap().parse().unwrap();
                    let count: usize = arg("count").unwrap().parse().unwrap();
                    let end = (offset + count).min(self.issues.len());
                    Ok(Value::Array(self.issues[offset.min(end)..end].to_vec()))
                }
                _ => Err(TrackerError::NotFound),
            }
        }
    }

    fn run(fake: &Fake, source: &Source) -> Result<Vec<Value>, TrackerError> {
        fetch(fake, source, &AtomicBool::new(false), &mut |_| {})
    }

    #[test]
    fn the_sample_becomes_rows() {
        let converted = convert(&issues(), &source(), &[], &ReadPlan::standard(&source()));
        let keys: Vec<&str> = converted.rows.iter().map(|row| row.key.as_str()).collect();
        // The parent comes first even though the sample lists the child before it.
        assert_eq!(keys, ["PROJ-1", "PROJ-3", "PROJ-5", "PROJ-2"]);
        let by = |key: &str| converted.rows.iter().find(|row| row.key == key).unwrap();

        let parent = by("PROJ-1");
        assert_eq!(parent.name, "Màn hình đăng nhập");
        assert_eq!(parent.parent_key, None);
        assert_eq!(parent.work, Some(16 * 60));
        assert_eq!(parent.status, Some(StatusClass::InProgress));
        assert_eq!(parent.assignee_email.as_deref(), Some("an@example.com"));
        assert_eq!(parent.start, Some(timestamp(2026, 1, 5, 8, 0)));
        assert_eq!(parent.due, Some(timestamp(2026, 1, 16, 17, 0)));

        let child = by("PROJ-2");
        assert_eq!(child.parent_key.as_deref(), Some("PROJ-1"));
        assert_eq!(child.work, Some(150));
        assert_eq!(child.status, Some(StatusClass::NotStarted));
        assert_eq!(child.start, None);
        assert_eq!(child.due, None);

        // A custom status is read through the source's map, by name and letter case.
        let reviewed = by("PROJ-3");
        assert_eq!(reviewed.status, Some(StatusClass::InProgress));
        assert_eq!(reviewed.work, None, "zero hours is no estimate");
        // A parent outside the pull is ignored; Closed is done; no assignee, no email.
        let closed = by("PROJ-5");
        assert_eq!(closed.parent_key, None);
        assert_eq!(closed.status, Some(StatusClass::Done));
        assert_eq!(closed.assignee_email, None);
    }

    #[test]
    fn warnings_name_the_issues_that_could_not_be_matched_or_read() {
        let converted = convert(&issues(), &source(), &[], &ReadPlan::standard(&source()));
        assert_eq!(
            converted.warnings,
            vec![
                PullWarning { key: "PROJ-3".into(), kind: WarningKind::NoEmail },
                PullWarning { key: "".into(), kind: WarningKind::Skipped },
            ]
        );
    }

    #[test]
    fn a_loop_of_parents_leaves_the_rows_standing_alone() {
        let a = serde_json::json!({ "id": 1, "issueKey": "L-1", "summary": "Một", "parentIssueId": 2 });
        let b = serde_json::json!({ "id": 2, "issueKey": "L-2", "summary": "Hai", "parentIssueId": 1 });
        let own = serde_json::json!({ "id": 3, "issueKey": "L-3", "summary": "Ba", "parentIssueId": 3 });
        let converted = convert(&[a, b, own], &source(), &[], &ReadPlan::standard(&source()));
        assert!(converted.rows.iter().all(|row| row.parent_key.is_none()));
        assert_eq!(converted.rows.len(), 3);
    }

    #[test]
    fn fetch_pages_through_all_issues_and_reports_progress() {
        let many: Vec<Value> = (0..250)
            .map(|i| serde_json::json!({ "id": i, "issueKey": format!("PROJ-{i}"), "summary": "x" }))
            .collect();
        let fake = Fake::new(many);
        let mut heard = Vec::new();
        let got = fetch(&fake, &source(), &AtomicBool::new(false), &mut |p| heard.push((p.fetched, p.total))).unwrap();
        assert_eq!(got.len(), 250);
        assert_eq!(heard, [(0, Some(250)), (100, Some(250)), (200, Some(250)), (250, Some(250))]);
        // The key never travels in a query; only the project and paging do.
        for (_, query) in fake.seen.borrow().iter() {
            assert!(query.iter().all(|(name, _)| name != "apiKey"));
        }
    }

    #[test]
    fn a_full_last_page_ends_on_the_empty_one() {
        let many: Vec<Value> = (0..200)
            .map(|i| serde_json::json!({ "id": i, "issueKey": format!("PROJ-{i}"), "summary": "x" }))
            .collect();
        assert_eq!(run(&Fake::new(many), &source()).unwrap().len(), 200);
        assert_eq!(run(&Fake::new(Vec::new()), &source()).unwrap().len(), 0);
    }

    #[test]
    fn the_filter_reaches_both_the_count_and_the_pages() {
        let fake = Fake::new(issues());
        let source = Source { filter: "?statusId[]=1&statusId[]=2&keyword=đăng nhập".into(), ..source() };
        run(&fake, &source).unwrap();
        for (path, query) in fake.seen.borrow().iter().filter(|(path, _)| path.starts_with("/api/v2/issues")) {
            let statuses = query.iter().filter(|(n, _)| n == "statusId[]").count();
            assert_eq!(statuses, 2, "{path}");
            assert!(query.contains(&("keyword".to_string(), "đăng nhập".to_string())), "{path}");
            assert!(query.contains(&("projectId[]".to_string(), "77".to_string())), "{path}");
        }
    }

    #[test]
    fn a_filter_cannot_reach_the_key_the_paging_or_the_project() {
        for bad in [
            "apiKey=x", "count=1", "offset=5", "projectId[]=9", "statusId[]", "statusId[]=",
            "sort=created", "unknown=1", "statusId[]=1&oops",
        ] {
            assert_eq!(parse_filter(bad), Err(TrackerError::BadFilter), "{bad}");
        }
        assert_eq!(parse_filter("  ").unwrap(), vec![]);
    }

    #[test]
    fn project_keys_are_path_safe() {
        let fake = Fake::new(Vec::new());
        for bad in ["", "A/B", "../x", "A B", "A?x=1", "PRÓ"] {
            assert_eq!(project(&fake, bad).unwrap_err(), TrackerError::BadProject, "{bad}");
        }
        assert_eq!(project(&fake, "PROJ").unwrap(), (77, "Dự án mẫu".to_string()));
        assert_eq!(project(&fake, "NOPE").unwrap_err(), TrackerError::NotFound);
    }

    #[test]
    fn cancelling_stops_before_the_next_request() {
        let fake = Fake::new(issues());
        let cancel = AtomicBool::new(false);
        let got = fetch(&fake, &source(), &cancel, &mut |p| {
            if p.fetched == 0 {
                cancel.store(true, Ordering::Relaxed);
            }
        });
        assert_eq!(got.unwrap_err(), TrackerError::Cancelled);
        let pages = fake.seen.borrow().iter().filter(|(path, _)| path == "/api/v2/issues").count();
        assert_eq!(pages, 0);
    }

    #[test]
    fn too_many_issues_are_refused_up_front() {
        struct Huge;
        impl Api for Huge {
            fn patch(&self, _: &str, _: &[(String, String)]) -> Result<Value, TrackerError> {
                Err(TrackerError::NotFound)
            }

            fn get(&self, path: &str, _: &[(String, String)]) -> Result<Value, TrackerError> {
                match path {
                    "/api/v2/issues/count" => Ok(serde_json::json!({ "count": 20_001 })),
                    _ => Ok(serde_json::json!({ "id": 1, "name": "x" })),
                }
            }
        }
        let got = fetch(&Huge, &source(), &AtomicBool::new(false), &mut |_| {});
        assert_eq!(got.unwrap_err(), TrackerError::TooManyIssues);
    }

    #[test]
    fn an_answer_that_is_not_a_list_is_a_bad_response() {
        struct Odd;
        impl Api for Odd {
            fn patch(&self, _: &str, _: &[(String, String)]) -> Result<Value, TrackerError> {
                Err(TrackerError::NotFound)
            }

            fn get(&self, path: &str, _: &[(String, String)]) -> Result<Value, TrackerError> {
                match path {
                    "/api/v2/issues/count" => Ok(serde_json::json!({ "count": 3 })),
                    "/api/v2/issues" => Ok(serde_json::json!({ "errors": [] })),
                    _ => Ok(serde_json::json!({ "id": 1, "name": "x" })),
                }
            }
        }
        let got = fetch(&Odd, &source(), &AtomicBool::new(false), &mut |_| {});
        assert_eq!(got.unwrap_err(), TrackerError::BadResponse);
    }

    fn field(id: u32, name: &str, kind: FieldKind) -> FieldDef {
        FieldDef { id, name: name.into(), kind }
    }

    fn project_fields() -> Vec<FieldDef> {
        vec![
            field(1, "Mảng", FieldKind::Choice { options: vec!["BE".into(), "FE".into()] }),
            field(2, "Điểm", FieldKind::Number),
            field(3, "Nhãn", FieldKind::Text),
            // Only in Sheafly: Backlog has no field of this name.
            field(4, "Ghi chú riêng", FieldKind::Text),
        ]
    }

    #[test]
    fn custom_fields_fill_the_project_fields_of_the_same_name() {
        let converted = convert(&issues(), &source(), &project_fields(), &ReadPlan::standard(&source()));
        let by = |key: &str| converted.rows.iter().find(|row| row.key == key).unwrap();
        let first = by("PROJ-1");
        assert_eq!(
            first.values,
            vec![
                ("Mảng".to_string(), FieldValue::Text("BE".into())),
                ("Điểm".to_string(), FieldValue::Number(5.0)),
                ("Nhãn".to_string(), FieldValue::Text("a, b".into())),
            ]
        );
        // A field that only Sheafly has is never sent, so its values are never cleared.
        assert!(converted.rows.iter().all(|row| row.values.iter().all(|(name, _)| name != "Ghi chú riêng")));
    }

    #[test]
    fn an_empty_value_clears_and_a_bad_number_is_left_alone_and_reported() {
        let converted = convert(&issues(), &source(), &project_fields(), &ReadPlan::standard(&source()));
        let second = converted.rows.iter().find(|row| row.key == "PROJ-2").unwrap();
        // Name matching ignores letter case; null clears; "abc" is no number: left out.
        assert!(second.values.contains(&("Mảng".to_string(), FieldValue::Text(String::new()))));
        assert!(second.values.iter().all(|(name, _)| name != "Điểm"));
        assert!(converted.warnings.contains(&PullWarning { key: "PROJ-2".into(), kind: WarningKind::FieldValue }));
        // An issue with no such entry at all clears the field too, since the source has it.
        let closed = converted.rows.iter().find(|row| row.key == "PROJ-5").unwrap();
        assert!(closed.values.contains(&("Mảng".to_string(), FieldValue::Text(String::new()))));
    }

    #[test]
    fn no_project_field_at_the_source_means_no_values() {
        let only_ours = vec![field(4, "Ghi chú riêng", FieldKind::Text)];
        let converted = convert(&issues(), &source(), &only_ours, &ReadPlan::standard(&source()));
        assert!(converted.rows.iter().all(|row| row.values.is_empty()));
    }

    #[test]
    fn values_of_every_shape_read_as_text() {
        assert_eq!(value_text(&serde_json::json!("  x ")), "x");
        assert_eq!(value_text(&serde_json::json!(3)), "3");
        assert_eq!(value_text(&serde_json::json!(2.5)), "2.5");
        assert_eq!(value_text(&serde_json::json!({ "id": 1, "name": " BE " })), "BE");
        assert_eq!(value_text(&serde_json::json!([{ "name": "a" }, "b", null, { "id": 3 }])), "a, b");
        assert_eq!(value_text(&serde_json::json!(true)), "");
        assert_eq!(value_text(&Value::Null), "");
    }

    fn configured(fields: Vec<sheafly_engine::source::FieldPolicy>) -> (Source, ReadPlan) {
        use crate::backlog_fields::{read_plan, CustomFieldDef, TYPE_DATE, TYPE_NUMBER};
        let defs = vec![
            CustomFieldDef { id: 21, name: "Bắt đầu thật".into(), type_id: TYPE_DATE },
            CustomFieldDef { id: 22, name: "Điểm".into(), type_id: TYPE_NUMBER },
            CustomFieldDef { id: 23, name: "Kế hoạch".into(), type_id: TYPE_DATE },
        ];
        let source = Source {
            hours_per_point: 3.0,
            sync: sheafly_engine::source::SyncSettings { fields, start_hour: 9, due_hour: 18, ..Default::default() },
            ..source()
        };
        let plan = read_plan(&source, &[], &defs).unwrap();
        (source, plan)
    }

    fn policy(field: &str, source_field: Option<&str>, push_field: Option<&str>) -> sheafly_engine::source::FieldPolicy {
        sheafly_engine::source::FieldPolicy {
            field: field.into(),
            direction: Default::default(),
            on_conflict: Default::default(),
            source_field: source_field.map(str::to_string),
            push_field: push_field.map(str::to_string),
        }
    }

    #[test]
    fn dates_come_at_the_hours_the_settings_name() {
        let (source, reads) = configured(vec![]);
        let converted = convert(&issues(), &source, &[], &reads);
        let first = converted.rows.iter().find(|row| row.key == "PROJ-1").unwrap();
        assert_eq!(first.start, Some(timestamp(2026, 1, 5, 9, 0)));
        assert_eq!(first.due, Some(timestamp(2026, 1, 16, 18, 0)));
    }

    #[test]
    fn a_day_and_an_effort_can_be_read_from_custom_fields() {
        let (source, reads) = configured(vec![
            policy("start", Some("custom:bắt đầu thật"), None),
            policy("work", Some("custom:Điểm"), None),
        ]);
        let mut issue = issues().remove(1);
        issue["customFields"] = serde_json::json!([
            { "id": 21, "name": "Bắt đầu thật", "value": "2026-02-02T00:00:00Z" },
            { "id": 22, "name": "Điểm", "value": "5" }
        ]);
        let row = &convert(&[issue.clone()], &source, &[], &reads).rows[0];
        // The attribute startDate and estimatedHours of the issue are not what is read now.
        assert_eq!(row.start, Some(timestamp(2026, 2, 2, 9, 0)));
        assert_eq!(row.work, Some(15 * 60), "5 points of 3 hours");
        assert_eq!(row.due, Some(timestamp(2026, 1, 16, 18, 0)), "the due date still comes from dueDate");

        // Points that are no number leave the effort unset and are reported.
        issue["customFields"][1]["value"] = serde_json::json!("nhiều");
        let converted = convert(&[issue], &source, &[], &reads);
        assert_eq!(converted.rows[0].work, None);
        assert_eq!(converted.warnings, vec![PullWarning { key: "PROJ-1".into(), kind: WarningKind::FieldValue }]);
    }

    #[test]
    fn the_days_a_push_writes_to_are_read_to_compare_with() {
        let (source, reads) = configured(vec![policy("planned_start", None, Some("custom:Kế hoạch"))]);
        let mut issue = issues().remove(1);
        issue["customFields"] = serde_json::json!([{ "id": 23, "name": "kế hoạch", "value": "2026-03-03T00:00:00Z" }]);
        let row = &convert(&[issue], &source, &[], &reads).rows[0];
        assert_eq!(row.planned_start, Some(timestamp(2026, 3, 3, 9, 0)));
        assert_eq!(row.planned_finish, None);
    }

    #[test]
    fn a_project_field_can_be_read_from_a_differently_named_custom_field() {
        use crate::backlog_fields::{read_plan, CustomFieldDef};
        let defs = vec![CustomFieldDef { id: 30, name: "Nhóm".into(), type_id: 1 }];
        let fields = vec![field(7, "Mảng", FieldKind::Text)];
        let source = Source { sync: sheafly_engine::source::SyncSettings { fields: vec![policy("field:7", Some("custom:Nhóm"), None)], ..Default::default() }, ..source() };
        let reads = read_plan(&source, &fields, &defs).unwrap();
        let mut issue = issues().remove(1);
        issue["customFields"] = serde_json::json!([{ "id": 30, "name": "Nhóm", "value": "Backend" }]);
        let row = &convert(&[issue], &source, &fields, &reads).rows[0];
        assert_eq!(row.values, vec![("Mảng".to_string(), FieldValue::Text("Backend".into()))]);
    }

    #[test]
    fn dates_and_estimates_are_read_defensively() {
        assert_eq!(instant_of(Some(&Value::from("2026-01-05T00:00:00Z")), 8), Some(timestamp(2026, 1, 5, 8, 0)));
        assert_eq!(instant_of(Some(&Value::from("2026-13-45")), 8), None);
        assert_eq!(instant_of(Some(&Value::from("x")), 8), None);
        assert_eq!(instant_of(Some(&Value::Null), 8), None);
        assert_eq!(minutes_of(Some(&Value::from(1.5))), Some(90));
        assert_eq!(minutes_of(Some(&Value::from(-1.0))), None);
        assert_eq!(minutes_of(Some(&Value::from(1e12))), None);
        assert_eq!(minutes_of(None), None);
    }
}

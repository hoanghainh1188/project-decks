//! The Jira Cloud adapter, reading: REST API v3.
//!
//! `fetch` searches the project with JQL, a page of 100 at a time (`POST
//! /rest/api/3/search/jql`, paged by `nextPageToken`: the old `/search` is gone and the new one
//! gives no total, so progress asks `/search/approximate-count` and shows no total when that
//! fails) and `convert` turns the issues into the engine's `PullTask` rows:
//!
//! - `key` is the key, `fields.summary` the name;
//! - the effort is `timetracking.originalEstimateSeconds`, or story points of a number field
//!   named in the sync settings times `hours_per_point`;
//! - `fields.duedate` is the due date (at `due_hour`); Jira has no standard start date, so a
//!   start comes only from a date field the settings name (at `start_hour`);
//! - `fields.parent.key` is the parent when that issue is in the pull;
//! - an issue link of type "Blocks" on which this issue "is blocked by" another issue of the
//!   pull makes it wait for that one (finish to start);
//! - the status goes through the source's status map by name, else Jira's status category
//!   (`new`, `indeterminate`, `done`);
//! - the assignee is found by `emailAddress` when Jira gives one and by `displayName` when
//!   it hides emails (the engine accepts the name when exactly one person has it);
//! - custom fields fill the project's custom fields of the same name (or the one a policy
//!   names), when the site has such a field.
//!
//! None of this has been run against a real site: the shapes are those of the API
//! documentation and the community posts it was written from, and a response that differs is
//! read as missing, never guessed.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{Datelike, NaiveDate};
use serde_json::{json, Value};
use sheafly_engine::fields::{FieldDef, FieldKind, FieldValue};
use sheafly_engine::pull::{PullTask, MAX_PULL_ROWS};
use sheafly_engine::source::{Source, StatusClass};
use sheafly_engine::timeutil::timestamp;

use crate::connector::{link_parents, Converted, Progress, PullWarning, WarningKind};
use crate::jira_fields::{ReadPlan, WorkSource};
use crate::tracker::{Api, TrackerError};

/// Issues one request returns at most.
const PAGE: usize = 100;
/// Pages one fetch may take; a Jira that keeps answering more would otherwise never end.
const MAX_PAGES: usize = 400;

/// A project key is safe in JQL and a path.
pub fn project_key_ok(key: &str) -> bool {
    !key.is_empty() && key.len() <= 100 && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The JQL that picks the project's issues, narrowed by the source's filter (a JQL condition;
/// not an ordering, which would break the query).
pub fn conditions(source: &Source) -> Result<String, TrackerError> {
    if !project_key_ok(&source.project_key) {
        return Err(TrackerError::BadProject);
    }
    let filter = source.filter.trim();
    if filter.chars().any(char::is_control) || filter.to_lowercase().contains("order by") {
        return Err(TrackerError::BadFilter);
    }
    Ok(if filter.is_empty() {
        format!("project = \"{}\"", source.project_key)
    } else {
        format!("project = \"{}\" AND ({filter})", source.project_key)
    })
}

/// Every issue of the source's project that the filter lets through. `progress` hears after
/// each page; `cancel` is looked at before each request.
pub fn fetch(
    api: &dyn Api,
    source: &Source,
    request: &[String],
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Vec<Value>, TrackerError> {
    let conditions = conditions(source)?;
    let stop = || {
        if cancel.load(Ordering::Relaxed) {
            Err(TrackerError::Cancelled)
        } else {
            Ok(())
        }
    };
    stop()?;
    // The total is for the progress bar only: a Jira that cannot say shows none.
    let total = api
        .post_json("/rest/api/3/search/approximate-count", &json!({ "jql": conditions }))
        .ok()
        .and_then(|body| body.get("count").and_then(Value::as_u64))
        .map(|count| count as usize);
    if total.is_some_and(|total| total > MAX_PULL_ROWS) {
        return Err(TrackerError::TooManyIssues);
    }
    progress(Progress { fetched: 0, total });

    let jql = format!("{conditions} ORDER BY created ASC");
    let mut issues: Vec<Value> = Vec::new();
    let mut token: Option<String> = None;
    for _ in 0..MAX_PAGES {
        stop()?;
        let mut body = json!({ "jql": jql, "fields": request, "maxResults": PAGE });
        if let Some(token) = &token {
            body["nextPageToken"] = Value::from(token.clone());
        }
        let page = api.post_json("/rest/api/3/search/jql", &body)?;
        let list = page.get("issues").and_then(Value::as_array).ok_or(TrackerError::BadResponse)?;
        issues.extend(list.iter().cloned());
        if issues.len() > MAX_PULL_ROWS {
            return Err(TrackerError::TooManyIssues);
        }
        progress(Progress { fetched: issues.len(), total: total.map(|total| total.max(issues.len())) });
        token = page.get("nextPageToken").and_then(Value::as_str).filter(|token| !token.is_empty()).map(str::to_string);
        if page.get("isLast").and_then(Value::as_bool).unwrap_or(false) || token.is_none() || list.is_empty() {
            break;
        }
    }
    Ok(issues)
}

/// The date part of a Jira date or datetime as `hour:00` that day.
fn instant_of(value: Option<&Value>, hour: u32) -> Option<i64> {
    let text = value?.as_str()?;
    let date = NaiveDate::parse_from_str(text.get(..10)?, "%Y-%m-%d").ok()?;
    Some(timestamp(date.year(), date.month(), date.day(), hour, 0))
}

/// Minutes of an estimate in hours; none for a missing, zero, negative or absurd one.
fn minutes_of_hours(hours: f64) -> Option<i64> {
    (hours.is_finite() && hours > 0.0 && hours <= 100_000.0).then(|| (hours * 60.0).round() as i64)
}

/// The text of a custom field value: text, a number, the value or name of an option, a
/// user's display name, or several of those joined by commas. Rich text is left out.
fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.trim().to_string(),
        Value::Number(number) => number.to_string(),
        Value::Object(item) => ["value", "name", "displayName"]
            .iter()
            .find_map(|key| item.get(*key).and_then(Value::as_str))
            .unwrap_or("")
            .trim()
            .to_string(),
        Value::Array(items) => items.iter().map(value_text).filter(|text| !text.is_empty()).collect::<Vec<_>>().join(", "),
        _ => String::new(),
    }
}

fn number_of(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().replace(',', ".").parse().ok(),
        _ => None,
    }
    .filter(|number| number.is_finite())
}

fn status_class(status: Option<&Value>, source: &Source) -> Option<StatusClass> {
    let status = status?;
    if let Some(name) = status.get("name").and_then(Value::as_str) {
        let lower = name.trim().to_lowercase();
        if let Some(mapping) = source.status_map.iter().find(|m| m.status.to_lowercase() == lower) {
            return Some(mapping.class);
        }
    }
    match status.pointer("/statusCategory/key").and_then(Value::as_str)? {
        "new" => Some(StatusClass::NotStarted),
        "indeterminate" => Some(StatusClass::InProgress),
        "done" => Some(StatusClass::Done),
        _ => None,
    }
}

fn work_of(fields: &Value, reads: &ReadPlan, key: &str, warnings: &mut Vec<PullWarning>) -> Option<i64> {
    match &reads.work {
        WorkSource::Estimate => {
            let seconds = fields.pointer("/timetracking/originalEstimateSeconds")?.as_f64()?;
            minutes_of_hours(seconds / 3600.0)
        }
        WorkSource::Points { field, hours_per_point } => {
            let value = fields.get(field).filter(|value| !value_text(value).is_empty())?;
            match number_of(value) {
                Some(points) => minutes_of_hours(points * hours_per_point),
                None => {
                    warnings.push(PullWarning { key: key.to_string(), kind: WarningKind::FieldValue });
                    None
                }
            }
        }
    }
}

/// What the issue says for each project custom field the site has. An empty value clears the
/// field; a number field whose value is no number is left alone and reported.
fn custom_values(
    fields: &Value,
    project_fields: &[FieldDef],
    reads: &ReadPlan,
    key: &str,
    warnings: &mut Vec<PullWarning>,
) -> Vec<(String, FieldValue)> {
    let mut values = Vec::new();
    for field in project_fields {
        let Some(id) = reads.custom.get(&field.id) else { continue };
        let text = fields.get(id).map(value_text).unwrap_or_default();
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

/// The keys of the issues this one is blocked by: a link of type "Blocks" that carries an
/// `inwardIssue` ("is blocked by").
fn blockers(fields: &Value) -> Vec<String> {
    fields
        .get("issuelinks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|link| link.pointer("/type/name").and_then(Value::as_str).is_some_and(|name| name.eq_ignore_ascii_case("blocks")))
        .filter_map(|link| link.pointer("/inwardIssue/key").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

/// The issues as rows, parents before their children, otherwise in the order given.
pub fn convert(issues: &[Value], source: &Source, project_fields: &[FieldDef], reads: &ReadPlan) -> Converted {
    let mut out = Converted::default();
    let mut seen = HashSet::new();
    let mut parent_keys: Vec<Option<String>> = Vec::new();
    for issue in issues {
        let key = issue.get("key").and_then(Value::as_str).unwrap_or("").trim();
        let empty = Value::Null;
        let fields = issue.get("fields").unwrap_or(&empty);
        let name = fields.get("summary").and_then(Value::as_str).unwrap_or("").trim();
        if key.is_empty() || name.is_empty() || !seen.insert(key.to_string()) {
            out.warnings.push(PullWarning { key: key.to_string(), kind: WarningKind::Skipped });
            continue;
        }
        let assignee = fields.get("assignee").filter(|assignee| !assignee.is_null());
        let text = |field: &str| {
            assignee
                .and_then(|assignee| assignee.get(field))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
        };
        let (email, display) = (text("emailAddress"), text("displayName"));
        if assignee.is_some() && email.is_none() && display.is_none() {
            out.warnings.push(PullWarning { key: key.to_string(), kind: WarningKind::NoEmail });
        }
        parent_keys.push(fields.pointer("/parent/key").and_then(Value::as_str).map(str::to_string));
        let values = custom_values(fields, project_fields, reads, key, &mut out.warnings);
        let work = work_of(fields, reads, key, &mut out.warnings);
        out.rows.push(PullTask {
            key: key.to_string(),
            parent_key: None,
            name: name.to_string(),
            work,
            start: reads.start.as_ref().and_then(|id| instant_of(fields.get(id), reads.start_hour)),
            due: instant_of(fields.get(&reads.due), reads.due_hour),
            status: status_class(fields.get("status"), source),
            assignee_email: email,
            assignee_name: display,
            values,
            after: blockers(fields),
            planned_start: reads.planned_start.as_ref().and_then(|id| instant_of(fields.get(id), reads.start_hour)),
            planned_finish: reads.planned_finish.as_ref().and_then(|id| instant_of(fields.get(id), reads.due_hour)),
        });
    }
    // Only an issue of the pull can be a parent or be waited for.
    let index: std::collections::HashMap<&str, usize> =
        out.rows.iter().enumerate().map(|(row, task)| (task.key.as_str(), row)).collect();
    let parents: Vec<Option<usize>> = parent_keys
        .iter()
        .enumerate()
        .map(|(row, parent)| parent.as_deref().and_then(|key| index.get(key).copied()).filter(|p| *p != row))
        .collect();
    let present: HashSet<String> = out.rows.iter().map(|task| task.key.clone()).collect();
    for task in &mut out.rows {
        let own = task.key.clone();
        task.after.retain(|key| *key != own && present.contains(key));
        task.after.sort();
        task.after.dedup();
    }
    link_parents(&mut out.rows, &parents);
    out
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use sheafly_engine::fields::FieldKind;
    use sheafly_engine::source::{Direction, FieldPolicy, OnConflict, StatusMapping, SyncSettings, TrackerKind};

    use super::*;
    use crate::jira_fields::{read_plan, JiraField};

    const ISSUES: &str = include_str!("../fixtures/jira_search.json");

    fn issues() -> Vec<Value> {
        serde_json::from_str::<Value>(ISSUES).unwrap()["issues"].as_array().unwrap().clone()
    }

    fn defs() -> Vec<JiraField> {
        let field = |id: &str, name: &str, kind: &str| JiraField { id: id.into(), name: name.into(), kind: kind.into() };
        vec![
            field("duedate", "Due date", "date"),
            field("customfield_10015", "Start date", "date"),
            field("customfield_10016", "Story point estimate", "number"),
            field("customfield_10020", "Kế hoạch", "date"),
            field("customfield_10030", "Mảng", "string"),
            field("customfield_10040", "Điểm khó", "number"),
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
            status_map: vec![StatusMapping { status: "Đang review".into(), class: StatusClass::InProgress }],
            sync: SyncSettings { fields, ..SyncSettings::default() },
        }
    }

    fn rows(fields: Vec<FieldPolicy>, project_fields: &[FieldDef]) -> Converted {
        let source = source(fields);
        let reads = read_plan(&source, project_fields, &defs()).unwrap();
        convert(&issues(), &source, project_fields, &reads)
    }

    #[test]
    fn the_sample_becomes_rows_with_parents_and_blockers() {
        let converted = rows(vec![], &[]);
        let keys: Vec<&str> = converted.rows.iter().map(|row| row.key.as_str()).collect();
        // The child is listed before its parent in the sample; the parent comes first now.
        assert_eq!(keys, ["PROJ-1", "PROJ-3", "PROJ-2"]);
        let by = |key: &str| converted.rows.iter().find(|row| row.key == key).unwrap();

        let parent = by("PROJ-1");
        assert_eq!(parent.name, "Màn hình đăng nhập");
        assert_eq!(parent.work, Some(16 * 60));
        assert_eq!(parent.due, Some(timestamp(2026, 1, 16, 17, 0)));
        assert_eq!(parent.start, None, "Jira has no standard start date");
        assert_eq!(parent.status, Some(StatusClass::InProgress));
        assert_eq!(parent.assignee_email.as_deref(), Some("an@example.com"));
        assert_eq!(parent.assignee_name.as_deref(), Some("An Nguyễn"));

        let child = by("PROJ-2");
        assert_eq!(child.parent_key.as_deref(), Some("PROJ-1"));
        assert_eq!(child.work, Some(150));
        assert_eq!(child.status, Some(StatusClass::NotStarted));
        // Blocked by PROJ-1 (in the pull) and by PROJ-77 (not in it, so not waited for).
        assert_eq!(child.after, ["PROJ-1"]);
        // Hidden email: only the name travels.
        assert_eq!((child.assignee_email.as_deref(), child.assignee_name.as_deref()), (None, Some("Bình Trần")));

        let done = by("PROJ-3");
        assert_eq!(done.status, Some(StatusClass::Done), "from the status category");
        assert_eq!(done.parent_key, None, "a parent outside the pull is ignored");
        assert!(done.after.is_empty(), "a link that blocks others is not a wait");
    }

    #[test]
    fn a_status_the_map_names_wins_over_its_category() {
        let mut issue = issues().remove(1);
        issue["fields"]["status"] = json!({ "name": "đang REVIEW", "statusCategory": { "key": "new" } });
        let source = source(vec![]);
        let reads = read_plan(&source, &[], &defs()).unwrap();
        let converted = convert(&[issue], &source, &[], &reads);
        assert_eq!(converted.rows[0].status, Some(StatusClass::InProgress));
    }

    #[test]
    fn a_start_date_and_story_points_come_from_the_fields_the_settings_name() {
        let converted = rows(
            vec![policy("start", Some("custom:Start date"), None), policy("work", Some("custom:Story point estimate"), None)],
            &[],
        );
        let first = converted.rows.iter().find(|row| row.key == "PROJ-1").unwrap();
        assert_eq!(first.start, Some(timestamp(2026, 1, 5, 8, 0)));
        assert_eq!(first.work, Some(5 * 3 * 60), "5 points of 3 hours");
        let second = converted.rows.iter().find(|row| row.key == "PROJ-2").unwrap();
        assert_eq!(second.work, None, "no points, no effort");
    }

    #[test]
    fn points_that_are_no_number_are_reported_and_leave_the_effort_unset() {
        let mut issue = issues().remove(1);
        issue["fields"]["customfield_10016"] = json!("nhiều");
        let source = source(vec![policy("work", Some("custom:Story point estimate"), None)]);
        let reads = read_plan(&source, &[], &defs()).unwrap();
        let converted = convert(&[issue], &source, &[], &reads);
        assert_eq!(converted.rows[0].work, None);
        assert_eq!(converted.warnings, vec![PullWarning { key: "PROJ-1".into(), kind: WarningKind::FieldValue }]);
    }

    #[test]
    fn project_custom_fields_come_from_the_site_fields_of_their_names() {
        let fields = vec![
            FieldDef { id: 1, name: "Mảng".into(), kind: FieldKind::Text },
            FieldDef { id: 2, name: "Điểm khó".into(), kind: FieldKind::Number },
            FieldDef { id: 3, name: "Chỉ ở Sheafly".into(), kind: FieldKind::Text },
        ];
        let converted = rows(vec![], &fields);
        let first = converted.rows.iter().find(|row| row.key == "PROJ-1").unwrap();
        assert_eq!(
            first.values,
            vec![("Mảng".to_string(), FieldValue::Text("Backend".into())), ("Điểm khó".to_string(), FieldValue::Number(3.0))]
        );
        // A value that is missing clears; a field Jira lacks is never sent.
        let second = converted.rows.iter().find(|row| row.key == "PROJ-2").unwrap();
        assert!(second.values.contains(&("Mảng".to_string(), FieldValue::Text(String::new()))));
        assert!(converted.rows.iter().all(|row| row.values.iter().all(|(name, _)| name != "Chỉ ở Sheafly")));
        // The unreadable number is reported.
        assert!(converted.warnings.contains(&PullWarning { key: "PROJ-2".into(), kind: WarningKind::FieldValue }));
    }

    #[test]
    fn the_planned_days_are_read_from_the_field_they_are_pushed_to() {
        let converted = rows(vec![policy("planned_finish", None, Some("custom:Kế hoạch"))], &[]);
        let first = converted.rows.iter().find(|row| row.key == "PROJ-1").unwrap();
        assert_eq!(first.planned_finish, Some(timestamp(2026, 2, 3, 17, 0)));
        assert_eq!(first.planned_start, None);
    }

    #[test]
    fn an_issue_without_a_key_or_title_and_a_repeat_are_left_out() {
        let source = source(vec![]);
        let reads = read_plan(&source, &[], &defs()).unwrap();
        let bad = vec![
            json!({ "fields": { "summary": "Không khóa" } }),
            json!({ "key": "X-1", "fields": {} }),
            json!({ "key": "X-2", "fields": { "summary": "Tốt" } }),
            json!({ "key": "X-2", "fields": { "summary": "Lặp" } }),
        ];
        let converted = convert(&bad, &source, &[], &reads);
        assert_eq!(converted.rows.len(), 1);
        assert_eq!(converted.warnings.len(), 3);
    }

    #[test]
    fn a_loop_of_parents_leaves_the_rows_standing_alone() {
        let source = source(vec![]);
        let reads = read_plan(&source, &[], &defs()).unwrap();
        let loop_ = vec![
            json!({ "key": "L-1", "fields": { "summary": "Một", "parent": { "key": "L-2" } } }),
            json!({ "key": "L-2", "fields": { "summary": "Hai", "parent": { "key": "L-1" } } }),
        ];
        let converted = convert(&loop_, &source, &[], &reads);
        assert!(converted.rows.iter().all(|row| row.parent_key.is_none()));
        assert_eq!(converted.rows.len(), 2);
    }

    #[test]
    fn jql_picks_the_project_and_the_filter_is_a_condition_only() {
        let plain = source(vec![]);
        assert_eq!(conditions(&plain).unwrap(), "project = \"PROJ\"");
        let narrowed = Source { filter: " labels = plan OR priority = High ".into(), ..source(vec![]) };
        assert_eq!(conditions(&narrowed).unwrap(), "project = \"PROJ\" AND (labels = plan OR priority = High)");
        for bad in ["status = Done ORDER BY created", "x\u{7}y"] {
            let source = Source { filter: bad.into(), ..source(vec![]) };
            assert_eq!(conditions(&source), Err(TrackerError::BadFilter), "{bad:?}");
        }
        for key in ["", "A B", "A\"B", "A;B", "../x"] {
            let source = Source { project_key: key.into(), ..source(vec![]) };
            assert_eq!(conditions(&source), Err(TrackerError::BadProject), "{key:?}");
        }
    }

    type Posts = Vec<(String, Value)>;

    /// A fake Jira: the search answers in pages by token, and the count answers or fails.
    struct Fake {
        issues: Vec<Value>,
        count: Option<usize>,
        posts: RefCell<Posts>,
    }

    impl Api for Fake {
        fn get(&self, _: &str, _: &[(String, String)]) -> Result<Value, TrackerError> {
            Err(TrackerError::NotFound)
        }

        fn patch(&self, _: &str, _: &[(String, String)]) -> Result<Value, TrackerError> {
            Err(TrackerError::NotFound)
        }

        fn post_json(&self, path: &str, body: &Value) -> Result<Value, TrackerError> {
            self.posts.borrow_mut().push((path.to_string(), body.clone()));
            match path {
                "/rest/api/3/search/approximate-count" => match self.count {
                    Some(count) => Ok(json!({ "count": count })),
                    None => Err(TrackerError::NotFound),
                },
                "/rest/api/3/search/jql" => {
                    let start: usize = body.get("nextPageToken").and_then(Value::as_str).map_or(0, |t| t.parse().unwrap());
                    let size = body["maxResults"].as_u64().unwrap() as usize;
                    let end = (start + size).min(self.issues.len());
                    let mut page = json!({ "issues": self.issues[start..end].to_vec() });
                    if end < self.issues.len() {
                        page["nextPageToken"] = Value::from(end.to_string());
                    } else {
                        page["isLast"] = Value::from(true);
                    }
                    Ok(page)
                }
                _ => Err(TrackerError::NotFound),
            }
        }
    }

    fn many(count: usize) -> Vec<Value> {
        (0..count).map(|i| json!({ "key": format!("PROJ-{i}"), "fields": { "summary": "x" } })).collect()
    }

    type Heard = Vec<(usize, Option<usize>)>;

    fn run(fake: &Fake) -> (Result<Vec<Value>, TrackerError>, Heard) {
        let mut heard = Vec::new();
        let got = fetch(fake, &source(vec![]), &["summary".to_string()], &AtomicBool::new(false), &mut |p| {
            heard.push((p.fetched, p.total))
        });
        (got, heard)
    }

    #[test]
    fn fetch_pages_by_token_and_reports_progress_with_the_count() {
        let fake = Fake { issues: many(250), count: Some(250), posts: RefCell::new(Vec::new()) };
        let (got, heard) = run(&fake);
        assert_eq!(got.unwrap().len(), 250);
        assert_eq!(heard, [(0, Some(250)), (100, Some(250)), (200, Some(250)), (250, Some(250))]);
        let posts = fake.posts.borrow();
        let searches: Vec<&Value> = posts.iter().filter(|(path, _)| path.ends_with("/search/jql")).map(|(_, body)| body).collect();
        assert_eq!(searches.len(), 3);
        assert!(searches[0].get("nextPageToken").is_none(), "no token on the first page");
        assert_eq!(searches[1]["nextPageToken"], "100");
        assert_eq!(searches[0]["jql"], "project = \"PROJ\" ORDER BY created ASC");
        assert_eq!(searches[0]["fields"], json!(["summary"]));
        // The count asks without the ordering.
        let counted = posts.iter().find(|(path, _)| path.ends_with("/approximate-count")).unwrap();
        assert_eq!(counted.1["jql"], "project = \"PROJ\"");
    }

    #[test]
    fn a_jira_that_cannot_count_shows_no_total_but_still_pulls() {
        let fake = Fake { issues: many(120), count: None, posts: RefCell::new(Vec::new()) };
        let (got, heard) = run(&fake);
        assert_eq!(got.unwrap().len(), 120);
        assert_eq!(heard, [(0, None), (100, None), (120, None)]);
    }

    #[test]
    fn too_many_issues_are_refused_and_a_cancel_stops_before_the_first_search() {
        let fake = Fake { issues: many(3), count: Some(20_001), posts: RefCell::new(Vec::new()) };
        assert_eq!(run(&fake).0.unwrap_err(), TrackerError::TooManyIssues);
        let fake = Fake { issues: many(3), count: Some(3), posts: RefCell::new(Vec::new()) };
        let cancel = AtomicBool::new(true);
        let got = fetch(&fake, &source(vec![]), &[], &cancel, &mut |_| {});
        assert_eq!(got.unwrap_err(), TrackerError::Cancelled);
        assert!(fake.posts.borrow().is_empty());
    }

    #[test]
    fn an_answer_without_issues_is_a_bad_response() {
        struct Odd;
        impl Api for Odd {
            fn get(&self, _: &str, _: &[(String, String)]) -> Result<Value, TrackerError> {
                Err(TrackerError::NotFound)
            }
            fn patch(&self, _: &str, _: &[(String, String)]) -> Result<Value, TrackerError> {
                Err(TrackerError::NotFound)
            }
            fn post_json(&self, _: &str, _: &Value) -> Result<Value, TrackerError> {
                Ok(json!({ "errorMessages": ["x"] }))
            }
        }
        let got = fetch(&Odd, &source(vec![]), &[], &AtomicBool::new(false), &mut |_| {});
        assert_eq!(got.unwrap_err(), TrackerError::BadResponse);
    }

    #[test]
    fn values_of_every_shape_read_as_text() {
        assert_eq!(value_text(&json!("  x ")), "x");
        assert_eq!(value_text(&json!(2.5)), "2.5");
        assert_eq!(value_text(&json!({ "value": "BE" })), "BE");
        assert_eq!(value_text(&json!({ "displayName": "An" })), "An");
        assert_eq!(value_text(&json!([{ "value": "a" }, { "name": "b" }, null])), "a, b");
        assert_eq!(value_text(&json!({ "type": "doc", "content": [] })), "", "rich text is left out");
        assert_eq!(value_text(&Value::Null), "");
    }
}

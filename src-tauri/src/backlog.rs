//! The Backlog (Nulab) adapter: API v2, read only.
//!
//! `fetch` lists the issues of one project in pages of 100 and `convert` turns them into
//! the engine's `PullTask` rows. The mapping:
//!
//! - `issueKey` is the key, `summary` the name;
//! - `estimatedHours` is the effort (zero or missing: no estimate);
//! - `startDate` and `dueDate` are dates; the start becomes 08:00 and the due date 17:00 of
//!   that day (the default working day; the engine moves a start past non-working time);
//! - `parentIssueId` is the parent when that issue is in the pull (a parent outside the
//!   filter is ignored), and parents come before their children;
//! - `status` becomes a status class through the source's status map by name, else the
//!   four statuses every Backlog space has (Open, In Progress, Resolved, Closed); a
//!   custom status the map does not list says nothing;
//! - `assignee.mailAddress` is the assignee's email; Backlog sends it only to those allowed
//!   to see it, and an assignee without one is reported as a warning.
//!
//! Backlog has no story points and no "blocks" links, so the source's points factor and
//! `after` are not used here.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use serde_json::Value;
use sheafly_engine::pull::{PullTask, MAX_PULL_ROWS};
use sheafly_engine::source::{Source, StatusClass};
use sheafly_engine::timeutil::timestamp;

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

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Progress {
    pub fetched: usize,
    pub total: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WarningKind {
    /// The issue has an assignee, but Backlog gave no email, so nobody can be matched.
    NoEmail,
    /// The issue could not be read (no key or title) and was left out.
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PullWarning {
    pub key: String,
    pub kind: WarningKind,
}

#[derive(Debug, Default)]
pub struct Converted {
    pub rows: Vec<PullTask>,
    pub warnings: Vec<PullWarning>,
}

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
pub fn convert(issues: &[Value], source: &Source) -> Converted {
    let mut out = Converted::default();
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
        out.rows.push(PullTask {
            key: key.to_string(),
            parent_key: None,
            name: name.to_string(),
            work: minutes_of(issue.get("estimatedHours")),
            start: instant_of(issue.get("startDate"), 8),
            due: instant_of(issue.get("dueDate"), 17),
            status: status_class(issue.get("status"), source),
            assignee_email: email.map(str::to_string),
            values: Vec::new(),
            after: Vec::new(),
        });
    }
    let parents: Vec<Option<usize>> = parent_ids
        .iter()
        .enumerate()
        .map(|(row, parent)| parent.and_then(|id| ids.get(&id).copied()).filter(|p| *p != row))
        .collect();
    let depths = depths(&parents);
    for (row, parent) in parents.iter().enumerate() {
        if let (Some(parent), Some(_)) = (parent, depths[row]) {
            out.rows[row].parent_key = Some(out.rows[*parent].key.clone());
        }
    }
    // A loop of parents (which Backlog does not allow) has no depth: its rows stand alone.
    let mut order: Vec<usize> = (0..out.rows.len()).collect();
    order.sort_by_key(|&row| depths[row].unwrap_or(0));
    let mut rows: Vec<Option<PullTask>> = out.rows.into_iter().map(Some).collect();
    out.rows = order.into_iter().filter_map(|row| rows[row].take()).collect();
    out
}

/// How deep each row is under its parents; `None` for a row on a loop of parents.
fn depths(parents: &[Option<usize>]) -> Vec<Option<usize>> {
    parents
        .iter()
        .map(|&start| {
            let mut depth = 0;
            let mut at = start;
            while let Some(parent) = at {
                depth += 1;
                if depth > parents.len() {
                    return None;
                }
                at = parents[parent];
            }
            Some(depth)
        })
        .collect()
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
        let converted = convert(&issues(), &source());
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
        let converted = convert(&issues(), &source());
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
        let converted = convert(&[a, b, own], &source());
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

//! Writing a push plan to Backlog: `PATCH /api/v2/issues/:key`, one issue at a time.
//!
//! The plan (from the engine's `push::plan`) says what to write; this module only writes
//! it. Dates go as `yyyy-MM-dd` form fields, the assignee as the `assigneeId` of the person
//! whose email the plan names (looked up in the project's users). A day the plan sends to a
//! date custom field of its own (`PushChange::start_field`/`due_field`) goes to
//! `customField_<id>` as `yyyy-MM-dd`. Nothing is ever cleared.
//! A failure of one issue (the source refused it, no such person) does not stop the rest; a
//! failure that would repeat for every issue (a rejected key, a rate limit, no network)
//! does, and the issues not reached are reported as not sent. The days that went through
//! come back as `PushedEntry`s for the engine's `RecordPush`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use sheafly_engine::push::{PushChange, PushedEntry};
use sheafly_engine::Lang;

use crate::backlog_fields::{parse_ref, FieldRef};
use crate::tracker::{Api, TrackerError};

#[derive(Clone, Copy, Debug, Serialize)]
pub struct PushProgress {
    pub done: usize,
    pub total: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PushFailure {
    pub key: String,
    pub message: String,
}

#[derive(Debug, Default, Serialize)]
pub struct PushReport {
    /// Keys of the issues written.
    pub done: Vec<String>,
    /// What was written (days and assignee), for `RecordPush`.
    pub entries: Vec<PushedEntry>,
    pub failed: Vec<PushFailure>,
    /// Keys not reached because the push stopped.
    pub not_sent: Vec<String>,
    /// Why the push stopped early, if it did.
    pub stopped: Option<String>,
}

/// An issue key is safe in a path.
fn key_safe(key: &str) -> bool {
    !key.is_empty() && key.len() <= 100 && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The project's users by lower-cased email, to their ids.
pub fn users(api: &dyn Api, project_key: &str) -> Result<HashMap<String, i64>, TrackerError> {
    let body = api.get(&format!("/api/v2/projects/{project_key}/users"), &[])?;
    let list = body.as_array().ok_or(TrackerError::BadResponse)?;
    Ok(list
        .iter()
        .filter_map(|user| {
            let email = user.get("mailAddress")?.as_str()?.trim().to_lowercase();
            let id = user.get("id")?.as_i64()?;
            (!email.is_empty()).then_some((email, id))
        })
        .collect())
}

/// A failure that would be the same for the next issue.
fn stops_the_push(error: TrackerError) -> bool {
    matches!(
        error,
        TrackerError::RateLimited | TrackerError::Rejected | TrackerError::Offline | TrackerError::Server | TrackerError::NoKey
    )
}

/// The name of the form field a day goes to: the tracker's own, or `customField_<id>`.
fn day_field(native: &str, custom: &Option<String>, ids: &HashMap<String, i64>) -> Result<String, TrackerError> {
    match custom {
        None => Ok(native.to_string()),
        Some(reference) => match parse_ref(reference)? {
            FieldRef::Custom(name) => ids
                .get(&name.to_lowercase())
                .map(|id| format!("customField_{id}"))
                .ok_or(TrackerError::BadField),
            FieldRef::Native(_) => Err(TrackerError::BadField),
        },
    }
}

/// The form fields of one change; the person's id comes from `users`, a custom field's from `ids`.
fn form_of(
    change: &PushChange,
    users: &HashMap<String, i64>,
    ids: &HashMap<String, i64>,
) -> Result<Vec<(String, String)>, TrackerError> {
    let mut form = Vec::new();
    if let Some(start) = &change.start {
        form.push((day_field("startDate", &change.start_field, ids)?, start.to.clone()));
    }
    if let Some(due) = &change.due {
        form.push((day_field("dueDate", &change.due_field, ids)?, due.to.clone()));
    }
    if let Some(assignee) = &change.assignee {
        let id = users.get(&assignee.to.trim().to_lowercase()).ok_or(TrackerError::NoUser)?;
        form.push(("assigneeId".to_string(), id.to_string()));
    }
    Ok(form)
}

/// Write `changes`, in order. `progress` hears after each issue; `cancel` is looked at
/// before each request.
pub fn push_issues(
    api: &dyn Api,
    changes: &[PushChange],
    users: &HashMap<String, i64>,
    ids: &HashMap<String, i64>,
    lang: Lang,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(PushProgress),
) -> PushReport {
    let mut report = PushReport::default();
    let total = changes.len();
    for (index, change) in changes.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            report.stopped = Some(TrackerError::Cancelled.message(lang).to_string());
        }
        if report.stopped.is_some() {
            report.not_sent.push(change.key.clone());
            continue;
        }
        let written = if !key_safe(&change.key) {
            Err(TrackerError::Refused)
        } else {
            form_of(change, users, ids).and_then(|form| api.patch(&format!("/api/v2/issues/{}", change.key), &form))
        };
        match written {
            Ok(_) => {
                report.done.push(change.key.clone());
                // A day sent to a field of its own is recorded under that field's key.
                let day = |change: &Option<sheafly_engine::push::DayChange>| change.as_ref().map(|day| day.to.clone());
                let (start, due) = (day(&change.start), day(&change.due));
                let entry = PushedEntry {
                    key: change.key.clone(),
                    start: start.clone().filter(|_| change.start_field.is_none()),
                    due: due.clone().filter(|_| change.due_field.is_none()),
                    assignee: change.assignee.as_ref().map(|assignee| assignee.to.clone()),
                    planned_start: start.filter(|_| change.start_field.is_some()),
                    planned_finish: due.filter(|_| change.due_field.is_some()),
                };
                if entry.start.is_some()
                    || entry.due.is_some()
                    || entry.assignee.is_some()
                    || entry.planned_start.is_some()
                    || entry.planned_finish.is_some()
                {
                    report.entries.push(entry);
                }
            }
            Err(error) => {
                let message = error.message(lang).to_string();
                if stops_the_push(error) {
                    report.stopped = Some(message.clone());
                }
                report.failed.push(PushFailure { key: change.key.clone(), message });
            }
        }
        progress(PushProgress { done: index + 1, total });
    }
    report
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use serde_json::Value;
    use sheafly_engine::push::{AssigneeChange, DayChange};

    use super::*;

    type Sent = Vec<(String, Vec<(String, String)>)>;

    /// A fake Backlog that records every PATCH and fails the keys it is told to.
    struct Fake {
        sent: RefCell<Sent>,
        fail: HashMap<String, TrackerError>,
    }

    impl Fake {
        fn new(fail: &[(&str, TrackerError)]) -> Self {
            Self {
                sent: RefCell::new(Vec::new()),
                fail: fail.iter().map(|(key, error)| (format!("/api/v2/issues/{key}"), *error)).collect(),
            }
        }
    }

    impl Api for Fake {
        fn get(&self, path: &str, _: &[(String, String)]) -> Result<Value, TrackerError> {
            match path {
                "/api/v2/projects/PROJ/users" => Ok(serde_json::json!([
                    { "id": 5, "userId": "an", "name": "An", "mailAddress": "An@Example.com" },
                    { "id": 6, "userId": "binh", "name": "Bình", "mailAddress": null },
                    { "id": 7, "userId": "chi", "name": "Chi" }
                ])),
                _ => Err(TrackerError::NotFound),
            }
        }

        fn patch(&self, path: &str, form: &[(String, String)]) -> Result<Value, TrackerError> {
            self.sent.borrow_mut().push((path.to_string(), form.to_vec()));
            match self.fail.get(path) {
                Some(error) => Err(*error),
                None => Ok(serde_json::json!({ "issueKey": "ok" })),
            }
        }
    }

    fn day(from: Option<&str>, to: &str) -> DayChange {
        DayChange { from: from.map(str::to_string), to: to.into() }
    }

    fn change(key: &str, start: Option<&str>, due: Option<&str>) -> PushChange {
        PushChange {
            uid: 1,
            key: key.into(),
            name: key.into(),
            start: start.map(|to| day(None, to)),
            due: due.map(|to| day(None, to)),
            assignee: None,
            start_field: None,
            due_field: None,
        }
    }

    fn run(fake: &Fake, changes: &[PushChange], cancel: &AtomicBool) -> (PushReport, Vec<usize>) {
        let users = users(fake, "PROJ").unwrap();
        let mut heard = Vec::new();
        let report = push_issues(fake, changes, &users, &HashMap::new(), Lang::En, cancel, &mut |p| heard.push(p.done));
        (report, heard)
    }

    fn form(fake: &Fake, key: &str) -> Vec<(String, String)> {
        fake.sent.borrow().iter().find(|(path, _)| path.ends_with(key)).unwrap().1.clone()
    }

    #[test]
    fn users_are_found_by_lower_cased_email_and_those_without_one_are_left_out() {
        let found = users(&Fake::new(&[]), "PROJ").unwrap();
        assert_eq!(found, HashMap::from([("an@example.com".to_string(), 5)]));
    }

    #[test]
    fn dates_and_the_assignee_go_as_form_fields_and_nothing_else() {
        let fake = Fake::new(&[]);
        let mut with_person = change("PROJ-2", None, Some("2026-01-06"));
        with_person.assignee = Some(AssigneeChange { from: None, to: "AN@example.com".into(), resource_uid: 3 });
        let (report, heard) = run(&fake, &[change("PROJ-1", Some("2026-01-05"), Some("2026-01-06")), with_person], &AtomicBool::new(false));
        assert_eq!(
            form(&fake, "PROJ-1"),
            vec![("startDate".into(), "2026-01-05".into()), ("dueDate".into(), "2026-01-06".into())]
        );
        assert_eq!(
            form(&fake, "PROJ-2"),
            vec![("dueDate".into(), "2026-01-06".into()), ("assigneeId".into(), "5".into())]
        );
        assert_eq!(report.done, ["PROJ-1", "PROJ-2"]);
        assert_eq!(
            report.entries,
            vec![
                PushedEntry { key: "PROJ-1".into(), start: Some("2026-01-05".into()), due: Some("2026-01-06".into()), assignee: None, planned_start: None, planned_finish: None },
                PushedEntry { key: "PROJ-2".into(), start: None, due: Some("2026-01-06".into()), assignee: Some("AN@example.com".into()), planned_start: None, planned_finish: None },
            ]
        );
        assert_eq!(heard, [1, 2]);
        assert!(report.failed.is_empty() && report.not_sent.is_empty() && report.stopped.is_none());
        // The paths are the issues' own, and no field is ever sent empty.
        assert!(fake.sent.borrow().iter().all(|(path, form)| path.starts_with("/api/v2/issues/PROJ-") && form.iter().all(|(_, v)| !v.is_empty())));
    }

    #[test]
    fn a_day_sent_to_a_date_custom_field_goes_there_and_is_recorded_under_its_own_key() {
        let fake = Fake::new(&[]);
        let ids = HashMap::from([("bắt đầu dự kiến".to_string(), 11)]);
        let mut sent = change("PROJ-1", Some("2026-01-05"), Some("2026-01-06"));
        sent.start_field = Some("custom:Bắt đầu dự kiến".into());
        let users = users(&fake, "PROJ").unwrap();
        let report = push_issues(&fake, &[sent], &users, &ids, Lang::En, &AtomicBool::new(false), &mut |_| {});
        assert_eq!(
            form(&fake, "PROJ-1"),
            vec![("customField_11".into(), "2026-01-05".into()), ("dueDate".into(), "2026-01-06".into())]
        );
        assert_eq!(
            report.entries,
            vec![PushedEntry {
                key: "PROJ-1".into(),
                start: None,
                due: Some("2026-01-06".into()),
                assignee: None,
                planned_start: Some("2026-01-05".into()),
                planned_finish: None,
            }]
        );
    }

    #[test]
    fn a_custom_field_that_is_not_known_fails_the_issue_without_a_request() {
        let fake = Fake::new(&[]);
        let mut sent = change("PROJ-1", Some("2026-01-05"), None);
        sent.start_field = Some("custom:Không có".into());
        let users = users(&fake, "PROJ").unwrap();
        let report = push_issues(&fake, &[sent], &users, &HashMap::new(), Lang::En, &AtomicBool::new(false), &mut |_| {});
        assert!(fake.sent.borrow().is_empty());
        assert_eq!(report.failed.len(), 1);
        assert!(report.failed[0].message.contains("not at the source"));
    }

    #[test]
    fn an_assignee_assigned_only_is_recorded_with_no_days() {
        let fake = Fake::new(&[]);
        let mut only = change("PROJ-1", None, None);
        only.assignee = Some(AssigneeChange { from: None, to: "an@example.com".into(), resource_uid: 3 });
        let (report, _) = run(&fake, &[only], &AtomicBool::new(false));
        assert_eq!(report.done, ["PROJ-1"]);
        assert_eq!(
            report.entries,
            vec![PushedEntry { key: "PROJ-1".into(), start: None, due: None, assignee: Some("an@example.com".into()), planned_start: None, planned_finish: None }]
        );
    }

    #[test]
    fn a_person_the_source_does_not_know_fails_that_issue_without_a_request() {
        let fake = Fake::new(&[]);
        let mut stranger = change("PROJ-1", Some("2026-01-05"), None);
        stranger.assignee = Some(AssigneeChange { from: None, to: "la@example.com".into(), resource_uid: 3 });
        let (report, _) = run(&fake, &[stranger, change("PROJ-2", Some("2026-01-06"), None)], &AtomicBool::new(false));
        assert!(fake.sent.borrow().iter().all(|(path, _)| !path.ends_with("PROJ-1")));
        assert_eq!(report.failed, vec![PushFailure { key: "PROJ-1".into(), message: TrackerError::NoUser.message(Lang::En).into() }]);
        assert_eq!(report.done, ["PROJ-2"]);
    }

    #[test]
    fn a_refused_issue_does_not_stop_the_others() {
        let fake = Fake::new(&[("PROJ-1", TrackerError::Refused)]);
        let (report, _) = run(&fake, &[change("PROJ-1", Some("2026-01-05"), None), change("PROJ-2", Some("2026-01-05"), None)], &AtomicBool::new(false));
        assert_eq!(report.done, ["PROJ-2"]);
        assert_eq!(report.failed.len(), 1);
        assert!(report.stopped.is_none());
        // A refused issue is not remembered as pushed.
        assert_eq!(report.entries.len(), 1);
    }

    #[test]
    fn a_rate_limit_stops_the_push_and_names_what_was_not_sent() {
        let fake = Fake::new(&[("PROJ-2", TrackerError::RateLimited)]);
        let changes: Vec<PushChange> = (1..=4).map(|i| change(&format!("PROJ-{i}"), Some("2026-01-05"), None)).collect();
        let (report, _) = run(&fake, &changes, &AtomicBool::new(false));
        assert_eq!(report.done, ["PROJ-1"]);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.not_sent, ["PROJ-3", "PROJ-4"]);
        assert!(report.stopped.as_deref().unwrap().contains("limiting"));
        assert_eq!(fake.sent.borrow().len(), 2, "no request after the limit");
    }

    #[test]
    fn a_rejected_key_stops_the_push() {
        let fake = Fake::new(&[("PROJ-1", TrackerError::Rejected)]);
        let (report, _) = run(&fake, &[change("PROJ-1", Some("2026-01-05"), None), change("PROJ-2", Some("2026-01-05"), None)], &AtomicBool::new(false));
        assert_eq!(report.not_sent, ["PROJ-2"]);
        assert!(report.entries.is_empty());
    }

    #[test]
    fn cancelling_stops_before_the_next_request() {
        let fake = Fake::new(&[]);
        let cancel = AtomicBool::new(false);
        let users = users(&fake, "PROJ").unwrap();
        let changes: Vec<PushChange> = (1..=3).map(|i| change(&format!("PROJ-{i}"), Some("2026-01-05"), None)).collect();
        let report = push_issues(&fake, &changes, &users, &HashMap::new(), Lang::Vi, &cancel, &mut |p| {
            if p.done == 1 {
                cancel.store(true, Ordering::Relaxed);
            }
        });
        assert_eq!(report.done, ["PROJ-1"]);
        assert_eq!(report.not_sent, ["PROJ-2", "PROJ-3"]);
        assert_eq!(report.stopped.as_deref(), Some("Đã hủy"));
        assert_eq!(fake.sent.borrow().len(), 1);
    }

    #[test]
    fn an_unsafe_key_is_refused_before_any_request() {
        let fake = Fake::new(&[]);
        let (report, _) = run(&fake, &[change("../x", Some("2026-01-05"), None)], &AtomicBool::new(false));
        assert!(fake.sent.borrow().is_empty());
        assert_eq!(report.failed.len(), 1);
    }
}

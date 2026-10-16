//! Writing a push plan to Jira Cloud: `PUT /rest/api/3/issue/:key`, one issue at a time
//! (phase 19, phase 4D-3).
//!
//! The body is `{"fields": {…}}`: a day goes to `duedate` or to a date field as `yyyy-MM-dd`,
//! the assignee as `{"accountId": …}`. The accountId is looked up once at preview from the
//! person's email (shown only when their privacy settings allow it) and, failing that, their
//! name, among the users who can be assigned in the project; a person found by neither is
//! reported as one the source does not have. Nothing is ever cleared. A day goes only to a
//! plain **date** field: a datetime field would need a time of day.
//!
//! Where a day goes when the settings name no field of its own: the due date to `duedate` (or
//! the date field the due date is read from), the start to the date field the start is read
//! from; with none there is no standard start date in Jira, and a push of start days is
//! refused until the settings name one (`planned_start` or the start's source field).
//!
//! Not run against a real site: the body and the user search follow the API documentation and
//! community posts.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

use serde_json::{json, Map, Value};
use sheafly_engine::fields::FieldDef;
use sheafly_engine::push::{PushChange, PushPlan};
use sheafly_engine::source::Source;
use sheafly_engine::Lang;

use crate::backlog_fields::{parse_ref, FieldRef};
use crate::connector::{run_push, PushProgress, PushReport, WriteSetup};
use crate::jira::project_key_ok;
use crate::jira_fields::{self, id_takes_date, read_plan, writable_date};
use crate::tracker::{Api, TrackerError};

/// An issue key is safe in a path.
fn key_safe(key: &str) -> bool {
    !key.is_empty() && key.len() <= 100 && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The accountId of the user who can be assigned in the project and has this email, else this
/// name (exactly one of them); None when nobody does.
pub fn account_of(api: &dyn Api, project_key: &str, email: &str, name: &str) -> Result<Option<String>, TrackerError> {
    let search = |query: &str| -> Result<Vec<Value>, TrackerError> {
        let found = api.get(
            "/rest/api/3/user/assignable/search",
            &[
                ("project".to_string(), project_key.to_string()),
                ("query".to_string(), query.to_string()),
                ("maxResults".to_string(), "50".to_string()),
            ],
        )?;
        Ok(found
            .as_array()
            .ok_or(TrackerError::BadResponse)?
            .iter()
            .filter(|user| user.get("active").and_then(Value::as_bool).unwrap_or(true))
            .cloned()
            .collect())
    };
    let account = |user: &Value| user.get("accountId").and_then(Value::as_str).filter(|id| !id.is_empty()).map(str::to_string);
    let wanted = email.trim().to_lowercase();
    if !wanted.is_empty() {
        let by_email = search(email.trim())?;
        let mut hit = by_email
            .iter()
            .filter(|user| user.get("emailAddress").and_then(Value::as_str).is_some_and(|e| e.trim().to_lowercase() == wanted));
        if let Some(found) = hit.next().and_then(account) {
            return Ok(Some(found));
        }
    }
    let name = name.trim().to_lowercase();
    if name.is_empty() {
        return Ok(None);
    }
    let by_name = search(&name)?;
    let mut hit = by_name
        .iter()
        .filter(|user| user.get("displayName").and_then(Value::as_str).is_some_and(|n| n.trim().to_lowercase() == name));
    match (hit.next(), hit.next()) {
        (Some(user), None) => Ok(account(user)),
        _ => Ok(None),
    }
}

/// The ids the plan needs: for each day the field it goes to (`custom:<lower name>` for a field
/// of its own, `default:start` and `default:due` otherwise) and for each person to assign their
/// accountId (by lower-cased email). An error refuses the whole push before anything is written.
pub fn setup(api: &dyn Api, source: &Source, project_fields: &[FieldDef], plan: &PushPlan) -> Result<WriteSetup, TrackerError> {
    if !project_key_ok(&source.project_key) {
        return Err(TrackerError::BadProject);
    }
    let defs = jira_fields::fields(api)?;
    let reads = read_plan(source, project_fields, &defs)?;
    let mut fields: HashMap<String, String> = HashMap::new();
    let mut resolve = |reference: &Option<String>, default_key: &str, default_id: Option<&String>| -> Result<(), TrackerError> {
        match reference {
            Some(reference) => match parse_ref(reference)? {
                FieldRef::Custom(name) => {
                    let def = writable_date(&defs, &name)?;
                    fields.insert(format!("custom:{}", name.to_lowercase()), def.id.clone());
                    Ok(())
                }
                FieldRef::Native(_) => Err(TrackerError::BadField),
            },
            None => {
                let id = default_id.filter(|id| id_takes_date(&defs, id)).ok_or(TrackerError::BadField)?;
                fields.insert(default_key.to_string(), id.clone());
                Ok(())
            }
        }
    };
    for change in &plan.changes {
        if change.start.is_some() {
            resolve(&change.start_field, "default:start", reads.start.as_ref())?;
        }
        if change.due.is_some() {
            resolve(&change.due_field, "default:due", Some(&reads.due))?;
        }
    }
    let mut people: HashMap<String, String> = HashMap::new();
    for assignee in plan.changes.iter().filter_map(|change| change.assignee.as_ref()) {
        let email = assignee.to.trim().to_lowercase();
        if people.contains_key(&email) {
            continue;
        }
        if let Some(account) = account_of(api, &source.project_key, &assignee.to, &assignee.to_name)? {
            people.insert(email, account);
        }
    }
    Ok(WriteSetup { people, fields })
}

/// The field a day goes to, from the setup.
fn day_field(reference: &Option<String>, default_key: &str, setup: &WriteSetup) -> Result<String, TrackerError> {
    let key = match reference {
        Some(reference) => match parse_ref(reference)? {
            FieldRef::Custom(name) => format!("custom:{}", name.to_lowercase()),
            FieldRef::Native(_) => return Err(TrackerError::BadField),
        },
        None => default_key.to_string(),
    };
    setup.fields.get(&key).cloned().ok_or(TrackerError::BadField)
}

/// The body of one change.
fn body_of(change: &PushChange, setup: &WriteSetup) -> Result<Value, TrackerError> {
    let mut fields = Map::new();
    if let Some(start) = &change.start {
        fields.insert(day_field(&change.start_field, "default:start", setup)?, Value::from(start.to.clone()));
    }
    if let Some(due) = &change.due {
        fields.insert(day_field(&change.due_field, "default:due", setup)?, Value::from(due.to.clone()));
    }
    if let Some(assignee) = &change.assignee {
        let account = setup.people.get(&assignee.to.trim().to_lowercase()).ok_or(TrackerError::NoUser)?;
        fields.insert("assignee".to_string(), json!({ "accountId": account }));
    }
    Ok(json!({ "fields": fields }))
}

/// Write `plan`, in order. `progress` hears after each issue; `cancel` is looked at before each
/// request.
pub fn write(
    api: &dyn Api,
    plan: &PushPlan,
    setup: &WriteSetup,
    lang: Lang,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(PushProgress),
) -> PushReport {
    run_push(&plan.changes, lang, cancel, progress, |change| {
        if !key_safe(&change.key) {
            return Err(TrackerError::Refused);
        }
        let body = body_of(change, setup)?;
        api.put_json(&format!("/rest/api/3/issue/{}", change.key), &body).map(|_| ())
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::sync::atomic::Ordering;

    use sheafly_engine::push::{AssigneeChange, DayChange, PushedEntry};
    use sheafly_engine::source::{Direction, FieldPolicy, OnConflict, SyncSettings, TrackerKind};

    use super::*;

    type Sent = Vec<(String, Value)>;

    /// A fake Jira: the field list, assignable users, and every PUT recorded.
    struct Fake {
        sent: RefCell<Sent>,
        fail: HashMap<String, TrackerError>,
        searches: RefCell<Vec<String>>,
    }

    impl Fake {
        fn new(fail: &[(&str, TrackerError)]) -> Self {
            Self {
                sent: RefCell::new(Vec::new()),
                fail: fail.iter().map(|(key, error)| (format!("/rest/api/3/issue/{key}"), *error)).collect(),
                searches: RefCell::new(Vec::new()),
            }
        }
    }

    impl Api for Fake {
        fn get(&self, path: &str, query: &[(String, String)]) -> Result<Value, TrackerError> {
            match path {
                "/rest/api/3/field" => Ok(json!([
                    { "id": "duedate", "name": "Due date", "schema": { "type": "date" } },
                    { "id": "customfield_10015", "name": "Start date", "schema": { "type": "date" } },
                    { "id": "customfield_10020", "name": "Kế hoạch", "schema": { "type": "date" } },
                    { "id": "customfield_10021", "name": "Giờ hẹn", "schema": { "type": "datetime" } },
                    { "id": "customfield_10030", "name": "Mảng", "schema": { "type": "string" } }
                ])),
                "/rest/api/3/user/assignable/search" => {
                    let query = query.iter().find(|(n, _)| n == "query").map(|(_, v)| v.clone()).unwrap();
                    self.searches.borrow_mut().push(query.clone());
                    Ok(json!([
                        { "accountId": "acc-an", "displayName": "An Nguyễn", "emailAddress": "an@example.com", "active": true },
                        { "accountId": "acc-binh", "displayName": "Bình", "active": true },
                        { "accountId": "acc-chi1", "displayName": "Chi", "active": true },
                        { "accountId": "acc-chi2", "displayName": "chi", "active": true },
                        { "accountId": "acc-off", "displayName": "Nghỉ", "emailAddress": "nghi@example.com", "active": false }
                    ]))
                }
                _ => Err(TrackerError::NotFound),
            }
        }

        fn patch(&self, _: &str, _: &[(String, String)]) -> Result<Value, TrackerError> {
            Err(TrackerError::NotFound)
        }

        fn put_json(&self, path: &str, body: &Value) -> Result<Value, TrackerError> {
            self.sent.borrow_mut().push((path.to_string(), body.clone()));
            match self.fail.get(path) {
                Some(error) => Err(*error),
                None => Ok(Value::Null),
            }
        }
    }

    fn source(fields: Vec<FieldPolicy>) -> Source {
        Source {
            kind: TrackerKind::Jira,
            site: "team.atlassian.net".into(),
            project_key: "PROJ".into(),
            filter: String::new(),
            hours_per_point: 4.0,
            status_map: vec![],
            sync: SyncSettings { fields, ..SyncSettings::default() },
        }
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

    fn day(to: &str) -> Option<DayChange> {
        Some(DayChange { from: None, to: to.into() })
    }

    fn change(key: &str, start: Option<&str>, due: Option<&str>) -> PushChange {
        PushChange {
            uid: 1,
            key: key.into(),
            name: key.into(),
            start: start.and_then(day),
            due: due.and_then(day),
            assignee: None,
            start_field: None,
            due_field: None,
        }
    }

    fn assignee(email: &str, name: &str) -> Option<AssigneeChange> {
        Some(AssigneeChange { from: None, to: email.into(), resource_uid: 3, to_name: name.into() })
    }

    fn plan(changes: Vec<PushChange>) -> PushPlan {
        PushPlan { changes, ..PushPlan::default() }
    }

    #[test]
    fn people_are_found_by_a_visible_email_then_by_a_name_only_one_user_has() {
        let fake = Fake::new(&[]);
        assert_eq!(account_of(&fake, "PROJ", "AN@example.com", "Ai đó").unwrap().as_deref(), Some("acc-an"));
        // Hidden email: the name stands in, case aside.
        assert_eq!(account_of(&fake, "PROJ", "binh@example.com", "BÌNH").unwrap().as_deref(), Some("acc-binh"));
        assert_eq!(account_of(&fake, "PROJ", "binh@example.com", "Bình Trần").unwrap(), None, "a different name");
        // Two users called Chi, an inactive user, and nobody.
        assert_eq!(account_of(&fake, "PROJ", "chi@example.com", "Chi").unwrap(), None);
        assert_eq!(account_of(&fake, "PROJ", "nghi@example.com", "Nghỉ").unwrap(), None, "an inactive user is not offered");
        assert_eq!(account_of(&fake, "PROJ", "la@example.com", "Người lạ").unwrap(), None);
        assert_eq!(account_of(&fake, "PROJ", "la@example.com", "").unwrap(), None);
    }

    #[test]
    fn the_due_date_goes_to_duedate_and_a_start_needs_a_field_the_settings_name() {
        let fake = Fake::new(&[]);
        let due_only = plan(vec![change("PROJ-1", None, Some("2026-01-06"))]);
        let ready = setup(&fake, &source(vec![]), &[], &due_only).unwrap();
        assert_eq!(ready.fields.get("default:due").map(String::as_str), Some("duedate"));

        // No standard start date in Jira: refused until the settings name a field.
        let with_start = plan(vec![change("PROJ-1", Some("2026-01-05"), None)]);
        assert_eq!(setup(&fake, &source(vec![]), &[], &with_start).unwrap_err(), TrackerError::BadField);
        let named = source(vec![policy("start", Some("custom:Start date"), None)]);
        let ready = setup(&fake, &named, &[], &with_start).unwrap();
        assert_eq!(ready.fields.get("default:start").map(String::as_str), Some("customfield_10015"));
    }

    #[test]
    fn planned_days_go_to_the_date_field_that_was_named_and_only_a_plain_date_field() {
        let fake = Fake::new(&[]);
        let mut sent = change("PROJ-1", Some("2026-01-05"), Some("2026-01-06"));
        sent.start_field = Some("custom:kế hoạch".into());
        let ready = setup(&fake, &source(vec![]), &[], &plan(vec![sent.clone()]));
        // The due day has no field of its own and goes to duedate; the start to the named one.
        let ready = ready.unwrap();
        assert_eq!(ready.fields.get("custom:kế hoạch").map(String::as_str), Some("customfield_10020"));
        assert_eq!(ready.fields.get("default:due").map(String::as_str), Some("duedate"));

        for bad in ["custom:Giờ hẹn", "custom:Mảng", "custom:Không có", "duedate"] {
            let mut change = change("PROJ-1", Some("2026-01-05"), None);
            change.start_field = Some(bad.into());
            assert_eq!(setup(&fake, &source(vec![]), &[], &plan(vec![change])).unwrap_err(), TrackerError::BadField, "{bad}");
        }
    }

    #[test]
    fn the_body_has_the_days_and_the_account_and_nothing_else() {
        let fake = Fake::new(&[]);
        let mut sent = change("PROJ-1", Some("2026-01-05"), Some("2026-01-06"));
        sent.start_field = Some("custom:Kế hoạch".into());
        sent.assignee = assignee("an@example.com", "An Nguyễn");
        let plan = plan(vec![sent]);
        let ready = setup(&fake, &source(vec![]), &[], &plan).unwrap();
        let report = write(&fake, &plan, &ready, Lang::En, &AtomicBool::new(false), &mut |_| {});
        let sent = fake.sent.borrow();
        assert_eq!(sent[0].0, "/rest/api/3/issue/PROJ-1");
        assert_eq!(
            sent[0].1,
            json!({ "fields": { "customfield_10020": "2026-01-05", "duedate": "2026-01-06", "assignee": { "accountId": "acc-an" } } })
        );
        assert_eq!(report.done, ["PROJ-1"]);
        assert_eq!(
            report.entries,
            vec![PushedEntry {
                key: "PROJ-1".into(),
                start: None,
                due: Some("2026-01-06".into()),
                assignee: Some("an@example.com".into()),
                planned_start: Some("2026-01-05".into()),
                planned_finish: None,
            }]
        );
    }

    #[test]
    fn a_person_jira_does_not_know_fails_that_issue_without_a_request() {
        let fake = Fake::new(&[]);
        let mut stranger = change("PROJ-1", None, Some("2026-01-06"));
        stranger.assignee = assignee("la@example.com", "Người lạ");
        let plan = plan(vec![stranger, change("PROJ-2", None, Some("2026-01-06"))]);
        let ready = setup(&fake, &source(vec![]), &[], &plan).unwrap();
        assert!(ready.people.is_empty());
        let report = write(&fake, &plan, &ready, Lang::En, &AtomicBool::new(false), &mut |_| {});
        assert_eq!(fake.sent.borrow().len(), 1, "only the other issue was sent");
        assert_eq!(report.done, ["PROJ-2"]);
        assert_eq!(report.failed.len(), 1);
    }

    #[test]
    fn a_rate_limit_stops_the_push_and_names_what_was_not_sent() {
        let fake = Fake::new(&[("PROJ-2", TrackerError::RateLimited)]);
        let changes: Vec<PushChange> = (1..=4).map(|i| change(&format!("PROJ-{i}"), None, Some("2026-01-06"))).collect();
        let plan = plan(changes);
        let ready = setup(&fake, &source(vec![]), &[], &plan).unwrap();
        let report = write(&fake, &plan, &ready, Lang::En, &AtomicBool::new(false), &mut |_| {});
        assert_eq!(report.done, ["PROJ-1"]);
        assert_eq!(report.not_sent, ["PROJ-3", "PROJ-4"]);
        assert_eq!(fake.sent.borrow().len(), 2);
    }

    #[test]
    fn cancelling_stops_before_the_next_request_and_an_unsafe_key_is_refused() {
        let fake = Fake::new(&[]);
        let plan = plan((1..=3).map(|i| change(&format!("PROJ-{i}"), None, Some("2026-01-06"))).collect());
        let ready = setup(&fake, &source(vec![]), &[], &plan).unwrap();
        let cancel = AtomicBool::new(false);
        let report = write(&fake, &plan, &ready, Lang::Vi, &cancel, &mut |p| {
            if p.done == 1 {
                cancel.store(true, Ordering::Relaxed);
            }
        });
        assert_eq!(report.done, ["PROJ-1"]);
        assert_eq!(report.not_sent, ["PROJ-2", "PROJ-3"]);

        let fake = Fake::new(&[]);
        let unsafe_plan = plan_of("../x");
        let report = write(&fake, &unsafe_plan, &WriteSetup::default(), Lang::En, &AtomicBool::new(false), &mut |_| {});
        assert!(fake.sent.borrow().is_empty());
        assert_eq!(report.failed.len(), 1);
    }

    fn plan_of(key: &str) -> PushPlan {
        plan(vec![change(key, None, Some("2026-01-06"))])
    }
}

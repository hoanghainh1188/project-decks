//! A tracker that hides emails (Jira Cloud): the assignee is found by name when there is no
//! email, as long as one person of the project has that name (phase 19, phase 4D-2).

use chrono::Datelike;

use sheafly_engine::command::Command;
use sheafly_engine::model::{ResourceKind, Task};
use sheafly_engine::pull::{diff, diff_with, PullColumn, PullTask};
use sheafly_engine::push::{plan, PushOptions};
use sheafly_engine::source::{Source, SyncSettings, TrackerKind};
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{Document, EngineError, Invalid, Project};

const HOUR: i64 = 60;

fn at(day: u32, hour: u32) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(i64::from(day)))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, 0)
}

/// An (an@example.com), Bình (no email) and two people called Chi.
fn doc() -> Document {
    let mut doc = Document::from_project(Project::new("Tên", at(0, 8)));
    for name in ["An", "Bình", "Chi", "chi"] {
        doc.apply(Command::AddResource { name: name.into(), kind: ResourceKind::Person }).unwrap();
    }
    let an = doc.project.resources[0].uid;
    doc.apply(Command::SetResourceEmail { uid: an, email: "an@example.com".into() }).unwrap();
    doc
}

fn row(email: Option<&str>, name: Option<&str>) -> PullTask {
    PullTask {
        key: "J-1".into(),
        name: "Việc".into(),
        work: Some(8 * HOUR),
        assignee_email: email.map(str::to_string),
        assignee_name: name.map(str::to_string),
        ..PullTask::default()
    }
}

fn pull(doc: &mut Document, row: PullTask) -> Result<(), EngineError> {
    doc.apply(Command::ApplyPull { rows: vec![row], pulled_at: 1, resolutions: Some(vec![]) }).map(|_| ())
}

fn task(doc: &Document) -> &Task {
    doc.project.tasks.iter().find(|task| task.external.is_some()).unwrap()
}

fn assignees(doc: &Document) -> Vec<String> {
    let uid = task(doc).uid;
    doc.project
        .assignments
        .iter()
        .filter(|a| a.task_uid == uid)
        .map(|a| doc.project.resources.iter().find(|r| r.uid == a.resource_uid).unwrap().name.clone())
        .collect()
}

#[test]
fn a_name_finds_the_one_person_who_has_it_whatever_the_letter_case() {
    let mut doc = doc();
    pull(&mut doc, row(None, Some("  BÌNH "))).unwrap();
    assert_eq!(assignees(&doc), ["Bình"]);
}

#[test]
fn an_email_wins_over_a_name() {
    let mut doc = doc();
    pull(&mut doc, row(Some("an@example.com"), Some("Bình"))).unwrap();
    assert_eq!(assignees(&doc), ["An"]);
}

#[test]
fn a_name_nobody_or_two_people_have_is_an_error_on_the_row() {
    let doc = doc();
    for name in ["Không ai", "Chi"] {
        let result = diff(&doc.project, &[row(None, Some(name))], 1);
        assert_eq!(result.errors.len(), 1, "{name}");
        assert_eq!(result.errors[0].column, PullColumn::Assignee);
        assert_eq!(result.errors[0].error, EngineError::Invalid(Invalid::PullUnknownAssignee));
    }
}

#[test]
fn the_same_person_by_name_is_not_a_change_or_a_conflict() {
    let mut doc = doc();
    pull(&mut doc, row(None, Some("Bình"))).unwrap();
    let again = diff_with(&doc.project, &[row(None, Some("Bình"))], 2, Some(&[]));
    assert!(again.is_empty() && again.conflicts.is_empty());
    // The source later names the person by email instead: still the same person.
    let mut doc = self::doc();
    let binh = doc.project.resources[1].uid;
    doc.apply(Command::SetResourceEmail { uid: binh, email: "binh@example.com".into() }).unwrap();
    pull(&mut doc, row(None, Some("Bình"))).unwrap();
    let by_email = diff_with(&doc.project, &[row(Some("binh@example.com"), None)], 2, Some(&[]));
    assert!(by_email.is_empty() && by_email.conflicts.is_empty());
}

#[test]
fn a_push_sees_an_assignee_it_can_only_name() {
    let mut doc = doc();
    // The original pull leaves no base, so nothing counts as changed at the source since.
    doc.apply(Command::ApplyPull { rows: vec![row(None, None)], pulled_at: 1, resolutions: None }).unwrap();
    let an = doc.project.resources[0].uid;
    doc.apply(Command::AssignResource { task_uid: task(&doc).uid, resource_uid: an, units: 1.0 }).unwrap();
    let options = PushOptions { assignee: true, ..PushOptions::default() };

    // Nobody at the source: An is written.
    assert_eq!(plan(&doc.project, &[row(None, None)], options).changes.len(), 1);
    // Someone we cannot identify is on it: left alone unless overwriting.
    let stranger = row(None, Some("Người lạ"));
    assert!(plan(&doc.project, std::slice::from_ref(&stranger), options).changes.is_empty());
    let over = plan(&doc.project, &[stranger], PushOptions { overwrite: true, ..options });
    assert_eq!(over.changes[0].assignee.as_ref().unwrap().from.as_deref(), Some("Người lạ"));
    // An is already on it by name: nothing to write.
    let same = row(None, Some("an"));
    assert!(plan(&doc.project, &[same], PushOptions { overwrite: true, ..options }).changes.is_empty());
}

#[test]
fn the_jira_kind_is_a_valid_source() {
    let mut doc = doc();
    let source = Source {
        kind: TrackerKind::Jira,
        site: "team.atlassian.net".into(),
        project_key: "PROJ".into(),
        filter: "labels = plan".into(),
        hours_per_point: 4.0,
        status_map: vec![],
        sync: SyncSettings::default(),
    };
    doc.apply(Command::SetSource { source: Some(source.clone()) }).unwrap();
    assert_eq!(doc.project.source, Some(source));
    let json = serde_json::to_string(&doc.project.source).unwrap();
    assert!(json.contains("\"kind\":\"jira\""));
}

#[test]
fn a_push_leaves_alone_an_assignee_the_source_gained_since_the_last_sync() {
    let mut doc = doc();
    pull(&mut doc, row(None, None)).unwrap();
    let an = doc.project.resources[0].uid;
    doc.apply(Command::AssignResource { task_uid: task(&doc).uid, resource_uid: an, units: 1.0 }).unwrap();
    let options = PushOptions { assignee: true, overwrite: true, ..PushOptions::default() };
    let result = plan(&doc.project, &[row(None, Some("Người lạ"))], options);
    assert!(result.changes.is_empty());
    assert!(result.skipped.iter().any(|skip| skip.field.as_deref() == Some("assignee")));
}

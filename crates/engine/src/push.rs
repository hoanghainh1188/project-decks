//! Pushing dates back to an external task tracker (phase 19, pha 2).
//!
//! `plan` is a pure function: it compares what the tracker holds now (the rows of a fresh
//! pull, as `PullTask`s) with what the engine computed for the linked tasks, and lists
//! what a push would write, day by day (trackers like Backlog keep dates, not times). The
//! Tauri layer shows that list, and after the user confirms, writes it and tells the
//! engine what went through (`Command::RecordPush`), so a later pull knows those days are
//! its own.
//!
//! Rules: only linked tasks the source still has; nothing is ever cleared at the source;
//! a value already equal is left; with `overwrite` off only empty fields at the source are
//! filled. An assignee is pushed only for a task with exactly one person with an email.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{EngineError, Invalid};
use crate::model::Project;
use crate::pull::PullTask;
use crate::sync::{self, SyncField};
use crate::timeutil::{civil, date_key, parse_date_key};

/// What to push.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushOptions {
    /// The task's start day to the source's start date.
    #[serde(default)]
    pub start: bool,
    /// The task's finish day to the source's due date.
    #[serde(default)]
    pub due: bool,
    /// The task's only assignee to the source's assignee.
    #[serde(default)]
    pub assignee: bool,
    /// Replace values already at the source; off, only empty ones are filled.
    #[serde(default)]
    pub overwrite: bool,
}

/// A day at the source and the day to write.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DayChange {
    pub from: Option<String>,
    pub to: String,
}

/// An assignee at the source and the person to write, by email.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AssigneeChange {
    pub from: Option<String>,
    pub to: String,
    pub resource_uid: u32,
}

/// What a push writes for one issue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PushChange {
    pub uid: u32,
    pub key: String,
    pub name: String,
    pub start: Option<DayChange>,
    pub due: Option<DayChange>,
    pub assignee: Option<AssigneeChange>,
    /// The field at the source to write the start day to, when the source's settings name
    /// one (`planned_start`); none means the tracker's usual start date.
    pub start_field: Option<String>,
    /// The same for the finish day (`planned_finish`).
    pub due_field: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PushSkipReason {
    /// Several people are on the task and the source has room for one.
    ManyAssignees,
    /// The person on the task has no email, so the source cannot find them.
    NoEmail,
    /// The source's value changed since the last sync, so writing over it would lose that
    /// change: pull and settle it first. `PushSkip::field` names the field.
    SourceChanged,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PushSkip {
    pub uid: u32,
    pub key: String,
    pub name: String,
    pub reason: PushSkipReason,
    /// The field a `SourceChanged` skip is about (`start`, `due`, `assignee`); none for the
    /// others, which concern the whole task.
    pub field: Option<String>,
}

/// A linked task whose issue the source did not return.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MissingTask {
    pub uid: u32,
    pub key: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct PushPlan {
    pub changes: Vec<PushChange>,
    pub skipped: Vec<PushSkip>,
    pub missing: Vec<MissingTask>,
}

impl PushPlan {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

fn day_of(instant: i64) -> String {
    date_key(civil(instant).0)
}

/// Whether a value at the source may be written over: empty ones always, others only with
/// `overwrite`.
fn may_write(at_source: &Option<String>, overwrite: bool) -> bool {
    overwrite || at_source.is_none()
}

fn day_change(from: Option<i64>, to: i64, overwrite: bool) -> Option<DayChange> {
    let from = from.map(day_of);
    let to = day_of(to);
    (from.as_deref() != Some(to.as_str()) && may_write(&from, overwrite)).then_some(DayChange { from, to })
}

/// What pushing `options` would write to the issues in `rows`, in project order.
pub fn plan(project: &Project, rows: &[PullTask], options: PushOptions) -> PushPlan {
    let at_source: HashMap<&str, &PullTask> = rows.iter().map(|row| (row.key.trim(), row)).collect();
    let mut plan = PushPlan::default();
    // A field the source's settings do not push is not pushed whatever the options say.
    let sync = project.source.as_ref().map(|source| source.sync.clone()).unwrap_or_default();
    // Where the computed start and finish go: a field of their own (`planned_start`,
    // `planned_finish`) when the settings name one, else the tracker's start and due dates.
    let target = |plain: SyncField, planned: &str| match sync.push_field(planned) {
        Some(field) => (planned.to_string(), Some(field.to_string()), sync.direction(planned).pushes()),
        None => (plain.key(), None, sync.direction(&plain.key()).pushes()),
    };
    let (start_key, start_field, start_ok) = target(SyncField::Start, "planned_start");
    let (due_key, due_field, due_ok) = target(SyncField::Due, "planned_finish");
    let options = PushOptions {
        start: options.start && start_ok,
        due: options.due && due_ok,
        assignee: options.assignee && sync.direction(&SyncField::Assignee.key()).pushes(),
        ..options
    };
    for task in &project.tasks {
        let Some(link) = task.external.as_ref().filter(|link| !link.gone) else { continue };
        let Some(row) = at_source.get(link.key.as_str()) else {
            plan.missing.push(MissingTask { uid: task.uid, key: link.key.clone(), name: task.name.clone() });
            continue;
        };
        let skip = |reason| PushSkip { uid: task.uid, key: link.key.clone(), name: task.name.clone(), reason, field: None };
        // A field the source changed since the last sync is left for a pull to settle.
        let changed_at_source = |key: &str, now: Option<String>| {
            link.base.as_ref().is_some_and(|base| {
                // A planned field nobody has recorded yet has no history to compare with:
                // `overwrite` is the user's say on a value already there.
                let known = !key.starts_with("planned_") || base.source.contains_key(key);
                known && base.source.get(key) != now.as_ref()
            })
        };
        let guard = |key: &str, now: Option<String>, plan: &mut PushPlan| {
            let moved = changed_at_source(key, now);
            if moved {
                plan.skipped.push(PushSkip { field: Some(key.to_string()), ..skip(PushSkipReason::SourceChanged) });
            }
            moved
        };
        let start_now = if start_field.is_some() { row.planned_start } else { row.start };
        let due_now = if due_field.is_some() { row.planned_finish } else { row.due };
        let mut start = if options.start { day_change(start_now, task.start, options.overwrite) } else { None };
        if start.is_some() && guard(&start_key, start_now.map(sync::day_text), &mut plan) {
            start = None;
        }
        let mut due = if options.due { day_change(due_now, task.finish, options.overwrite) } else { None };
        if due.is_some() && guard(&due_key, due_now.map(sync::day_text), &mut plan) {
            due = None;
        }
        let mut assignee = None;
        if options.assignee {
            let people: Vec<u32> = project
                .assignments
                .iter()
                .filter(|assignment| assignment.task_uid == task.uid)
                .map(|assignment| assignment.resource_uid)
                .collect();
            match people.as_slice() {
                [] => {}
                [person] => {
                    let resource = project.resources.iter().find(|resource| resource.uid == *person);
                    match resource.map(|resource| resource.email.trim()).filter(|email| !email.is_empty()) {
                        None => plan.skipped.push(skip(PushSkipReason::NoEmail)),
                        Some(email) => {
                            let from = row.assignee_email.as_deref().map(str::trim).filter(|e| !e.is_empty());
                            let same = from.is_some_and(|from| from.eq_ignore_ascii_case(email));
                            let now = from.map(str::to_lowercase);
                            if !same && (options.overwrite || from.is_none()) && !guard(&SyncField::Assignee.key(), now, &mut plan) {
                                assignee = Some(AssigneeChange {
                                    from: from.map(str::to_string),
                                    to: email.to_string(),
                                    resource_uid: *person,
                                });
                            }
                        }
                    }
                }
                _ => plan.skipped.push(skip(PushSkipReason::ManyAssignees)),
            }
        }
        if start.is_some() || due.is_some() || assignee.is_some() {
            plan.changes.push(PushChange {
                uid: task.uid,
                key: link.key.clone(),
                name: task.name.clone(),
                start_field: start_field.clone().filter(|_| start.is_some()),
                due_field: due_field.clone().filter(|_| due.is_some()),
                start,
                due,
                assignee,
            });
        }
    }
    plan
}

/// What a push wrote for one issue: the days that went through.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushedEntry {
    pub key: String,
    #[serde(default)]
    pub start: Option<String>,
    #[serde(default)]
    pub due: Option<String>,
    /// The email of the person written as the assignee.
    #[serde(default)]
    pub assignee: Option<String>,
    /// The days written to a field of their own (`planned_start`, `planned_finish`).
    #[serde(default)]
    pub planned_start: Option<String>,
    #[serde(default)]
    pub planned_finish: Option<String>,
}

/// `RecordPush`: remember the days written, on the linked tasks, so a pull knows them.
pub(crate) fn record_push(project: &mut Project, entries: &[PushedEntry]) -> Result<(), EngineError> {
    let valid = |day: &Option<String>| day.as_deref().is_none_or(|day| parse_date_key(day).is_some());
    if entries
        .iter()
        .any(|entry| !valid(&entry.start) || !valid(&entry.due) || !valid(&entry.planned_start) || !valid(&entry.planned_finish))
    {
        return Err(Invalid::PushDate.into());
    }
    let mut recorded = false;
    for entry in entries {
        let link = project
            .tasks
            .iter_mut()
            .filter_map(|task| task.external.as_mut())
            .find(|link| link.key == entry.key);
        if let Some(link) = link {
            if entry.start.is_some() {
                link.pushed_start = entry.start.clone();
            }
            if entry.due.is_some() {
                link.pushed_due = entry.due.clone();
            }
            // The source now holds what was written; what Sheafly held stays, so a pull
            // right after finds nothing changed at either side.
            if let Some(base) = link.base.as_mut() {
                if let Some(day) = &entry.start {
                    sync::set(&mut base.source, &SyncField::Start.key(), Some(day.clone()));
                }
                if let Some(day) = &entry.due {
                    sync::set(&mut base.source, &SyncField::Due.key(), Some(day.clone()));
                }
                if let Some(email) = &entry.assignee {
                    sync::set(&mut base.source, &SyncField::Assignee.key(), Some(email.trim().to_lowercase()));
                }
            }
            // The days written to a field of their own are tracked under their own keys, so
            // the next push can tell the source changed them (only on a link that has a base:
            // a base made here alone would stand for history nobody recorded).
            if let Some(base) = link.base.as_mut() {
                if let Some(day) = &entry.planned_start {
                    sync::set(&mut base.source, "planned_start", Some(day.clone()));
                }
                if let Some(day) = &entry.planned_finish {
                    sync::set(&mut base.source, "planned_finish", Some(day.clone()));
                }
            }
            recorded |= entry.start.is_some()
                || entry.due.is_some()
                || entry.assignee.is_some()
                || entry.planned_start.is_some()
                || entry.planned_finish.is_some();
        }
    }
    if recorded {
        Ok(())
    } else {
        Err(Invalid::NothingToRecord.into())
    }
}

//! What a tracker is to the rest of the app (phase 19, phase 4D).
//!
//! A `Connector` turns one tracker's API into the app's own terms: it checks a source, reads
//! its issues as the engine's `PullTask` rows, looks up what a push needs (people, fields),
//! and writes a push plan. The commands (`tracker_commands.rs`) and the engine know nothing
//! of any tracker. The static facts of a tracker (the hosts the app may send a key to, how a
//! request is authorized) are answered by the connector too, and the list of connectors is
//! built into the app: a connector the user could add by file could ask for the API key to be
//! sent to another host.
//!
//! The types here are what every connector reports: progress, warnings, rows, and the report
//! of a push.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

use serde::Serialize;
use sheafly_engine::fields::FieldDef;
use sheafly_engine::pull::PullTask;
use sheafly_engine::push::{PushPlan, PushedEntry};
use sheafly_engine::source::{Source, TrackerKind};
use sheafly_engine::Lang;

use crate::backlog_connector::BacklogConnector;
use crate::jira_connector::JiraConnector;
use crate::tracker::{Api, Credentials, TrackerError};

/// How far a fetch has got: issues so far, and how many there are.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Progress {
    pub fetched: usize,
    pub total: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WarningKind {
    /// The issue has an assignee, but the tracker gave nothing to match the person by.
    NoEmail,
    /// The issue could not be read (no key or title) and was left out.
    Skipped,
    /// A custom field's value does not read as a number, so the field was left as it was.
    FieldValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PullWarning {
    pub key: String,
    pub kind: WarningKind,
}

/// The issues of a pull as rows, with what was noticed on the way.
#[derive(Debug, Default)]
pub struct Converted {
    pub rows: Vec<PullTask>,
    pub warnings: Vec<PullWarning>,
}

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

/// What a push needs besides the plan, looked up once at preview: the tracker's ids of the
/// people the plan assigns (by lower-cased email) and of the custom fields it writes (by
/// lower-cased name). The ids are the tracker's own, kept as text.
#[derive(Clone, Debug, Default)]
pub struct WriteSetup {
    pub people: HashMap<String, String>,
    pub fields: HashMap<String, String>,
}

pub trait Connector: Sync {
    /// Whether `site` is a host of this tracker, the only kind the app sends a key to.
    fn allows_site(&self, site: &str) -> bool;

    /// Whether credentials are an account (an email) and a token, not a single key.
    fn needs_account(&self) -> bool;

    /// The header, name and value, that authorizes a request.
    fn authorize(&self, credentials: &Credentials) -> Result<(String, String), TrackerError>;

    /// Check the key and the project against the tracker, and the sync settings' references
    /// against the project's fields; the project's name.
    fn check(&self, api: &dyn Api, source: &Source, fields: &[FieldDef]) -> Result<String, TrackerError>;

    /// The source's issues as rows. `progress` hears as pages arrive; `cancel` is looked at
    /// before each request (`Err(Cancelled)` when it was set).
    fn pull(
        &self,
        api: &dyn Api,
        source: &Source,
        fields: &[FieldDef],
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Converted, TrackerError>;

    /// The ids the plan needs to be written. An error (a field that is not a date field, no
    /// people list) refuses the whole push before anything is written.
    fn write_setup(
        &self,
        api: &dyn Api,
        source: &Source,
        fields: &[FieldDef],
        plan: &PushPlan,
    ) -> Result<WriteSetup, TrackerError>;

    /// Write the plan, one issue at a time.
    fn write(
        &self,
        api: &dyn Api,
        plan: &PushPlan,
        setup: &WriteSetup,
        lang: Lang,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(PushProgress),
    ) -> PushReport;
}

/// Name each row's parent and put parents before their children, the rest in the order
/// given. `parents[row]` is the index of the row's parent, if it is in the pull. A loop of
/// parents (which trackers do not allow) has no depth: its rows stand alone.
pub fn link_parents(rows: &mut Vec<PullTask>, parents: &[Option<usize>]) {
    let depths: Vec<Option<usize>> = parents
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
        .collect();
    for (row, parent) in parents.iter().enumerate() {
        if let (Some(parent), Some(_)) = (parent, depths[row]) {
            rows[row].parent_key = Some(rows[*parent].key.clone());
        }
    }
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by_key(|&row| depths[row].unwrap_or(0));
    let mut taken: Vec<Option<PullTask>> = std::mem::take(rows).into_iter().map(Some).collect();
    *rows = order.into_iter().filter_map(|row| taken[row].take()).collect();
}

static BACKLOG: BacklogConnector = BacklogConnector;
static JIRA: JiraConnector = JiraConnector;

/// The connector for a kind of tracker.
pub fn connector_for(kind: TrackerKind) -> &'static dyn Connector {
    match kind {
        TrackerKind::Backlog => &BACKLOG,
        TrackerKind::Jira => &JIRA,
    }
}

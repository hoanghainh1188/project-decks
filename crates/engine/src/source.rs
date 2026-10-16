//! The external task tracker a project is pulled from (phase 19).
//!
//! The engine knows no tracker. A project holds one `Source` (which tracker, where, the
//! settings its adapter needs) and each task pulled from it holds an `ExternalLink` with
//! the key it has there. The adapter, in the Tauri layer, turns the tracker's data into
//! the typed rows of `pull.rs`; the credential never reaches the engine or a file.
//!
//! Resources gain an email address here too, since people at the source are matched to
//! people in the project by it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{EngineError, Invalid};
use crate::model::Project;

/// Longest site name (a host name).
pub const MAX_SITE_CHARS: usize = 253;
/// Longest source project key.
pub const MAX_SOURCE_KEY_CHARS: usize = 100;
/// Longest filter text.
pub const MAX_FILTER_CHARS: usize = 500;
/// Statuses the source can map.
pub const MAX_STATUSES: usize = 100;
/// Longest email address.
pub const MAX_EMAIL_CHARS: usize = 254;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackerKind {
    Backlog,
    Jira,
}

/// How far along a task is, as far as the source says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusClass {
    NotStarted,
    InProgress,
    Done,
}

/// A status of the source and what it means here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusMapping {
    pub status: String,
    pub class: StatusClass,
}

fn default_hours_per_point() -> f64 {
    4.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub kind: TrackerKind,
    /// Host name of the tracker's site, such as `team.backlog.com`.
    pub site: String,
    /// The project at the source (its key or id, as the adapter reads it).
    pub project_key: String,
    /// Which issues to pull; the adapter reads it.
    #[serde(default)]
    pub filter: String,
    /// Working hours one story point stands for.
    #[serde(default = "default_hours_per_point")]
    pub hours_per_point: f64,
    /// What each status of the source means; the adapter reads a status not listed.
    #[serde(default)]
    pub status_map: Vec<StatusMapping>,
    /// How each field syncs, and the constants the sync uses (all defaults when absent).
    #[serde(default)]
    pub sync: SyncSettings,
}

/// Which way a field syncs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Pulled from the source and pushed back to it.
    #[default]
    Both,
    /// Pulled only: never written to the source.
    Pull,
    /// Pushed only: never read from the source.
    Push,
    /// Not synced.
    Off,
}

impl Direction {
    pub fn pulls(self) -> bool {
        matches!(self, Self::Both | Self::Pull)
    }

    pub fn pushes(self) -> bool {
        matches!(self, Self::Both | Self::Push)
    }
}

/// What happens when a field changed on both sides since the last sync.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnConflict {
    /// Wait for the user's choice.
    #[default]
    Ask,
    /// The source wins.
    Source,
    /// Sheafly keeps its value.
    Sheafly,
}

/// How one field (or all custom fields) syncs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldPolicy {
    /// `name`, `work`, `start`, `due`, `status`, `assignee`, `field:<id>` for one custom
    /// field, or `fields` for every custom field without a policy of its own. Also
    /// `planned_start` and `planned_finish`: the days Sheafly computes, pushed (never
    /// pulled) to the field named by `push_field`; without one, a push writes them to the
    /// fields of `start` and `due`.
    pub field: String,
    #[serde(default)]
    pub direction: Direction,
    #[serde(default)]
    pub on_conflict: OnConflict,
    /// Which field at the source to read, when not the tracker's usual one; the adapter
    /// reads it (the engine does not look inside).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_field: Option<String>,
    /// Which field at the source to write, when not the tracker's usual one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_field: Option<String>,
}

fn default_in_progress_percent() -> u8 {
    50
}
fn default_start_hour() -> u8 {
    8
}
fn default_due_hour() -> u8 {
    17
}

/// Per-source settings of the sync, beyond the field mapping the adapter knows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncSettings {
    #[serde(default)]
    pub fields: Vec<FieldPolicy>,
    /// The percent complete an in-progress issue gets when the task has none between 0
    /// and 100.
    #[serde(default = "default_in_progress_percent")]
    pub in_progress_percent: u8,
    /// The hour a source's start date becomes (the adapter converts dates).
    #[serde(default = "default_start_hour")]
    pub start_hour: u8,
    /// The hour a source's due date becomes.
    #[serde(default = "default_due_hour")]
    pub due_hour: u8,
}

impl Default for SyncSettings {
    fn default() -> Self {
        Self {
            fields: Vec::new(),
            in_progress_percent: default_in_progress_percent(),
            start_hour: default_start_hour(),
            due_hour: default_due_hour(),
        }
    }
}

impl SyncSettings {
    /// The policy for a field key: its own, else the one for all custom fields, else none.
    fn policy(&self, key: &str) -> Option<&FieldPolicy> {
        self.fields
            .iter()
            .find(|policy| policy.field == key)
            .or_else(|| {
                key.starts_with("field:")
                    .then(|| self.fields.iter().find(|policy| policy.field == "fields"))
                    .flatten()
            })
    }

    /// The field at the source a policy writes to (the policy for exactly this key).
    pub fn push_field(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|policy| policy.field == key)
            .and_then(|policy| policy.push_field.as_deref())
    }

    pub fn direction(&self, key: &str) -> Direction {
        self.policy(key).map_or(Direction::Both, |policy| policy.direction)
    }

    pub fn on_conflict(&self, key: &str) -> OnConflict {
        self.policy(key).map_or(OnConflict::Ask, |policy| policy.on_conflict)
    }
}

/// The issue a task was pulled from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalLink {
    /// The issue key at the source, such as `PROJ-123`.
    pub key: String,
    /// When the last pull that changed this task ran (its `pulled_at`); a pull that finds
    /// nothing to change leaves it, so pulling again changes nothing.
    pub synced_at: i64,
    /// The issue is no longer at the source (deleted, moved, or outside the filter).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gone: bool,
    /// The start and due days (`YYYY-MM-DD`) last written to the source by a push. A pull
    /// that finds the same day there knows it is its own and reads nothing from it, so a
    /// pushed date does not come back as a constraint or a deadline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pushed_start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pushed_due: Option<String>,
    /// What the source and Sheafly said at the last sync, field by field (`sync.rs`); the
    /// base of the three-way comparison that finds conflicts. Absent for a link made
    /// before it existed, and for one a legacy pull (no `resolutions`) last touched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<SyncBase>,
}

/// The value of each synced field as of the last sync, in the canonical text of
/// `sync::SyncField`; a field with no value is absent.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncBase {
    /// What the source held.
    #[serde(default)]
    pub source: BTreeMap<String, String>,
    /// What Sheafly held (the value that corresponds to the source's field).
    #[serde(default)]
    pub local: BTreeMap<String, String>,
}

/// `SetSource`: replace or clear the project's source, checked and tidied.
pub(crate) fn set_source(
    project: &mut Project,
    source: &Option<Source>,
) -> Result<(), EngineError> {
    project.source = match source {
        None => None,
        Some(source) => Some(validate(source)?),
    };
    Ok(())
}

/// The source tidied (site lower-cased, texts trimmed), or why it is refused. The Tauri
/// layer checks a source the same way before it talks to the tracker.
pub fn validate(source: &Source) -> Result<Source, EngineError> {
    let site = source.site.trim().to_lowercase();
    let host_char = |c: char| c.is_ascii_alphanumeric() || c == '.' || c == '-';
    if site.is_empty()
        || site.chars().count() > MAX_SITE_CHARS
        || !site.chars().all(host_char)
        || site.starts_with(['.', '-'])
    {
        return Err(Invalid::SourceSite.into());
    }
    let project_key = source.project_key.trim().to_string();
    if project_key.is_empty() || project_key.chars().count() > MAX_SOURCE_KEY_CHARS {
        return Err(Invalid::SourceProjectKey.into());
    }
    let filter = source.filter.trim().to_string();
    if filter.chars().count() > MAX_FILTER_CHARS {
        return Err(Invalid::SourceFilter.into());
    }
    let points = source.hours_per_point;
    if !points.is_finite() || points <= 0.0 || points > 1000.0 {
        return Err(Invalid::SourceHoursPerPoint.into());
    }
    if source.status_map.len() > MAX_STATUSES {
        return Err(Invalid::SourceStatus.into());
    }
    let mut status_map: Vec<StatusMapping> = Vec::with_capacity(source.status_map.len());
    for mapping in &source.status_map {
        let status = mapping.status.trim().to_string();
        if status.is_empty() || status.chars().count() > MAX_SOURCE_KEY_CHARS {
            return Err(Invalid::SourceStatus.into());
        }
        let lower = status.to_lowercase();
        if status_map.iter().any(|m| m.status.to_lowercase() == lower) {
            return Err(Invalid::SourceStatusRepeated.into());
        }
        status_map.push(StatusMapping {
            status,
            class: mapping.class,
        });
    }
    Ok(Source {
        kind: source.kind,
        site,
        project_key,
        filter,
        hours_per_point: points,
        status_map,
        sync: checked_sync(&source.sync)?,
    })
}

/// Most field policies one source can have.
pub const MAX_FIELD_POLICIES: usize = 100;
/// Longest source field reference.
pub const MAX_SOURCE_FIELD_CHARS: usize = 100;

fn field_key_ok(key: &str) -> bool {
    matches!(
        key,
        "name" | "work" | "start" | "due" | "status" | "assignee" | "fields" | "planned_start" | "planned_finish"
    )
        || key.strip_prefix("field:").is_some_and(|id| id.parse::<u32>().is_ok())
}

fn checked_sync(sync: &SyncSettings) -> Result<SyncSettings, EngineError> {
    if sync.fields.len() > MAX_FIELD_POLICIES {
        return Err(Invalid::SyncPolicy.into());
    }
    if !(1..=99).contains(&sync.in_progress_percent) || sync.start_hour > 23 || sync.due_hour > 23 {
        return Err(Invalid::SyncSetting.into());
    }
    let mut fields: Vec<FieldPolicy> = Vec::with_capacity(sync.fields.len());
    for policy in &sync.fields {
        let tidy = |text: &Option<String>| -> Result<Option<String>, EngineError> {
            match text.as_deref().map(str::trim) {
                None | Some("") => Ok(None),
                Some(text) if text.chars().count() > MAX_SOURCE_FIELD_CHARS || text.chars().any(char::is_control) => {
                    Err(Invalid::SyncPolicy.into())
                }
                Some(text) => Ok(Some(text.to_string())),
            }
        };
        if !field_key_ok(&policy.field) || fields.iter().any(|other| other.field == policy.field) {
            return Err(Invalid::SyncPolicy.into());
        }
        fields.push(FieldPolicy {
            field: policy.field.clone(),
            direction: policy.direction,
            on_conflict: policy.on_conflict,
            source_field: tidy(&policy.source_field)?,
            push_field: tidy(&policy.push_field)?,
        });
    }
    Ok(SyncSettings { fields, ..sync.clone() })
}

/// `DeleteField`: the policy for a custom field goes with it.
pub(crate) fn drop_field(project: &mut Project, id: u32) {
    if let Some(source) = project.source.as_mut() {
        let key = format!("field:{id}");
        source.sync.fields.retain(|policy| policy.field != key);
    }
}

/// `SetResourceEmail`: an address, or empty to clear it. Unique among the project's
/// people, letter case aside, so an address always means one person.
pub(crate) fn set_resource_email(
    project: &mut Project,
    uid: u32,
    email: &str,
) -> Result<(), EngineError> {
    let email = email.trim();
    if !email.is_empty() && !looks_like_email(email) {
        return Err(Invalid::EmailInvalid.into());
    }
    let lower = email.to_lowercase();
    if !email.is_empty()
        && project
            .resources
            .iter()
            .any(|other| other.uid != uid && other.email.to_lowercase() == lower)
    {
        return Err(Invalid::EmailTaken.into());
    }
    let resource = project
        .resources
        .iter_mut()
        .find(|resource| resource.uid == uid)
        .ok_or(EngineError::ResourceNotFound)?;
    resource.email = email.to_string();
    Ok(())
}

/// One `@` with something before it and a dotted domain after it, no spaces. Anything
/// stricter would refuse addresses people really have.
pub(crate) fn looks_like_email(email: &str) -> bool {
    if email.chars().count() > MAX_EMAIL_CHARS || email.chars().any(char::is_whitespace) {
        return false;
    }
    let mut parts = email.split('@');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(local), Some(domain), None) => {
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
        }
        _ => false,
    }
}

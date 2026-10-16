//! The external task tracker a project is pulled from (phase 19).
//!
//! The engine knows no tracker. A project holds one `Source` (which tracker, where, the
//! settings its adapter needs) and each task pulled from it holds an `ExternalLink` with
//! the key it has there. The adapter, in the Tauri layer, turns the tracker's data into
//! the typed rows of `pull.rs`; the credential never reaches the engine or a file.
//!
//! Resources gain an email address here too, since people at the source are matched to
//! people in the project by it.

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
}

/// `SetSource`: replace or clear the project's source, checked and tidied.
pub(crate) fn set_source(
    project: &mut Project,
    source: &Option<Source>,
) -> Result<(), EngineError> {
    project.source = match source {
        None => None,
        Some(source) => Some(checked(source)?),
    };
    Ok(())
}

fn checked(source: &Source) -> Result<Source, EngineError> {
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
    })
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

//! The Tauri commands for task trackers (phase 19).
//!
//! Every network call of a pull or a push runs here, in the shell, through the tracker's
//! `Connector`: the webview sends a source and gets back what pulling or pushing would do. The
//! API key goes in once through `tracker_save_key` to the operating system's credential store
//! and is never read back. Applying a pull and recording a push are ordinary commands
//! (`apply_pull`, `record_push`) sent through `apply_command`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use sheafly_engine::document::now_millis;
use sheafly_engine::pull::PullTask;
use sheafly_engine::push::{self, PushOptions, PushPlan};
use sheafly_engine::source::{self, Source, TrackerKind};
use sheafly_engine::sync::Resolution;
use tauri::{AppHandle, Emitter, State};

use super::{engine_message, lang, lock, AppState};
use crate::connector::{connector_for, PushReport, WriteSetup};
use crate::tracker::{self, Credentials, HttpApi, PullChanges, PullPreview, TrackerError};

/// Save the credentials for a tracker's site in the operating system's credential store.
/// They go in once, from the dialog, and are never sent back to the webview. `key` is the
/// API key or token; `account` is the email for trackers that sign in with one.
#[tauri::command]
pub(crate) fn tracker_save_key(
    state: State<'_, AppState>,
    kind: TrackerKind,
    site: String,
    key: String,
    account: Option<String>,
) -> Result<(), String> {
    let lang = lang(&state);
    let say = |error: TrackerError| error.message(lang).to_string();
    let key = key.trim();
    if key.is_empty() {
        return Err(say(TrackerError::NoKey));
    }
    let credentials = if connector_for(kind).needs_account() {
        let account = account.unwrap_or_default().trim().to_string();
        if account.is_empty() {
            return Err(say(TrackerError::NoKey));
        }
        Credentials::Basic { account, token: key.to_string() }
    } else {
        Credentials::ApiKey(key.to_string())
    };
    tracker::save_key(kind, &site.trim().to_lowercase(), &credentials).map_err(say)
}

/// Whether credentials are saved for the site (they themselves are never read out).
#[tauri::command]
pub(crate) fn tracker_has_key(kind: TrackerKind, site: String) -> bool {
    matches!(tracker::load_key(kind, &site.trim().to_lowercase()), Ok(Some(_)))
}

#[tauri::command]
pub(crate) fn tracker_forget_key(state: State<'_, AppState>, kind: TrackerKind, site: String) -> Result<(), String> {
    tracker::forget_key(kind, &site.trim().to_lowercase()).map_err(|error| error.message(lang(&state)).to_string())
}

/// Check the saved credentials, the project key and the sync settings' field references
/// against the tracker; the project's name.
#[tauri::command(async)]
pub(crate) fn tracker_test(state: State<'_, AppState>, source: Source) -> Result<String, String> {
    let say = |error: TrackerError| error.message(lang(&state)).to_string();
    let source = source::validate(&source).map_err(engine_message(&state))?;
    let api = HttpApi::for_site(source.kind, &source.site).map_err(say)?;
    let fields = lock(&state).project.fields.clone();
    connector_for(source.kind).check(&api, &source, &fields).map_err(say)
}

/// A fresh cancel flag for the tracker call about to start; the one before is set, so a
/// call still running stops.
fn new_tracker_cancel(state: &AppState) -> Arc<AtomicBool> {
    let cancel = Arc::new(AtomicBool::new(false));
    let mut running = state.tracker_cancel.lock().unwrap_or_else(|err| err.into_inner());
    running.store(true, Ordering::Relaxed);
    *running = cancel.clone();
    cancel
}

/// Fetch the source's issues and say what pulling them would do to the project. Reads
/// only: the project changes when the frontend sends the rows in `apply_pull`. Answers
/// `None` when cancelled (`tracker_cancel`, or a newer pull). Progress arrives as
/// `tracker-progress` events.
#[tauri::command(async)]
pub(crate) fn tracker_pull(
    app: AppHandle,
    state: State<'_, AppState>,
    source: Source,
) -> Result<Option<PullPreview>, String> {
    let lang = lang(&state);
    let say = |error: TrackerError| error.message(lang).to_string();
    let source = source::validate(&source).map_err(engine_message(&state))?;
    let cancel = new_tracker_cancel(&state);
    let api = HttpApi::for_site(source.kind, &source.site).map_err(say)?;
    let project = lock(&state).project.clone();
    let pulled = connector_for(source.kind).pull(&api, &source, &project.fields, &cancel, &mut |progress| {
        let _ = app.emit("tracker-progress", progress);
    });
    let converted = match pulled {
        Ok(converted) => converted,
        Err(TrackerError::Cancelled) => return Ok(None),
        Err(error) => return Err(say(error)),
    };
    let pulled_at = now_millis();
    let changes = tracker::changes(&project, &converted.rows, pulled_at, &[], lang);
    Ok(Some(PullPreview {
        rows: converted.rows,
        pulled_at,
        warnings: converted.warnings,
        changes,
    }))
}

/// What a push would write, and the people it cannot find at the source.
#[derive(Serialize)]
pub(crate) struct PushPreview {
    plan: PushPlan,
    /// Emails of the people to assign that no one at the source has.
    unknown_assignees: Vec<String>,
}

/// What `tracker_push` needs to write the plan and to know it is still the right one.
pub(crate) struct PushBase {
    project: sheafly_engine::Project,
    source: Source,
    options: PushOptions,
    plan: PushPlan,
    setup: WriteSetup,
}

/// Fetch the source's issues and list what pushing the computed dates (and assignees)
/// would write. Nothing is written. `None` when cancelled.
#[tauri::command(async)]
pub(crate) fn tracker_push_preview(
    app: AppHandle,
    state: State<'_, AppState>,
    source: Source,
    options: PushOptions,
) -> Result<Option<PushPreview>, String> {
    let lang = lang(&state);
    let say = |error: TrackerError| error.message(lang).to_string();
    let source = source::validate(&source).map_err(engine_message(&state))?;
    let cancel = new_tracker_cancel(&state);
    let api = HttpApi::for_site(source.kind, &source.site).map_err(say)?;
    let connector = connector_for(source.kind);
    let project = lock(&state).project.clone();
    let pulled = connector.pull(&api, &source, &project.fields, &cancel, &mut |progress| {
        let _ = app.emit("tracker-progress", progress);
    });
    let rows = match pulled {
        Ok(converted) => converted.rows,
        Err(TrackerError::Cancelled) => return Ok(None),
        Err(error) => return Err(say(error)),
    };
    let plan = push::plan(&project, &rows, options);
    // The ids the plan needs; a field that cannot be written refuses the whole push before
    // anything is written.
    let setup = connector.write_setup(&api, &source, &project.fields, &plan).map_err(say)?;
    let mut unknown_assignees: Vec<String> = plan
        .changes
        .iter()
        .filter_map(|change| change.assignee.as_ref())
        .map(|assignee| assignee.to.clone())
        .filter(|email| !setup.people.contains_key(&email.trim().to_lowercase()))
        .collect();
    unknown_assignees.sort();
    unknown_assignees.dedup();
    *state.push_base.lock().unwrap_or_else(|err| err.into_inner()) = Some(PushBase {
        project,
        source,
        options,
        plan: plan.clone(),
        setup,
    });
    Ok(Some(PushPreview { plan, unknown_assignees }))
}

/// Write the plan last previewed, one issue at a time, with `tracker-push-progress`
/// events. Refused when the project or the settings changed since the preview, and a
/// preview is good for one push. The report says what went through; the frontend then
/// sends `record_push` with its `entries`.
#[tauri::command(async)]
pub(crate) fn tracker_push(
    app: AppHandle,
    state: State<'_, AppState>,
    source: Source,
    options: PushOptions,
) -> Result<PushReport, String> {
    let lang = lang(&state);
    let say = |error: TrackerError| error.message(lang).to_string();
    let source = source::validate(&source).map_err(engine_message(&state))?;
    let base = state.push_base.lock().unwrap_or_else(|err| err.into_inner()).take();
    let current = lock(&state).project.clone();
    let base = base
        .filter(|base| base.project == current && base.source == source && base.options == options)
        .ok_or_else(|| say(TrackerError::Stale))?;
    let cancel = new_tracker_cancel(&state);
    let api = HttpApi::for_site(source.kind, &source.site).map_err(say)?;
    Ok(connector_for(source.kind).write(&api, &base.plan, &base.setup, lang, &cancel, &mut |progress| {
        let _ = app.emit("tracker-push-progress", progress);
    }))
}

/// What pulling `rows` would do to the project as it is now, for when something changed
/// since the fetch (an email added to a person, an edit) and the rows are still good, or
/// under the choices made for the conflicts.
#[tauri::command(async)]
pub(crate) fn tracker_check(
    state: State<'_, AppState>,
    rows: Vec<PullTask>,
    pulled_at: i64,
    resolutions: Vec<Resolution>,
) -> PullChanges {
    let project = lock(&state).project.clone();
    tracker::changes(&project, &rows, pulled_at, &resolutions, lang(&state))
}

/// Stop the pull or push that is fetching.
#[tauri::command]
pub(crate) fn tracker_cancel(state: State<'_, AppState>) {
    state.tracker_cancel.lock().unwrap_or_else(|err| err.into_inner()).store(true, Ordering::Relaxed);
}

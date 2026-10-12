mod i18n;
mod settings;

use std::fs;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sheafly_engine::autosave::AutosaveSlot;
use sheafly_engine::document::now_millis;
use sheafly_engine::evm::ProjectMetrics;
use sheafly_engine::mspdi::{read_mspdi, write_mspdi};
use sheafly_engine::network::{self, NetworkLayout, NetworkOptions};
use sheafly_engine::store;
use sheafly_engine::{Command, Document, EngineError, Lang, Snapshot};
use tauri::menu::{AboutMetadata, CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager, RunEvent, Runtime, State, WindowEvent};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_dialog::{
    DialogExt, FileDialogBuilder, FilePath, MessageDialogBuilder, MessageDialogButtons,
    MessageDialogKind, MessageDialogResult,
};

struct AppState {
    doc: Mutex<Document>,
    /// True while a native file dialog owns the screen.
    dialog_open: Arc<AtomicBool>,
    /// Where unsaved work goes between saves. `None` when the app has no data folder.
    autosave: Option<AutosaveSlot>,
    /// Bumped on every change to the open document.
    revision: AtomicU64,
    /// The revision the autosave slot holds, or the one that needs no autosave.
    autosaved: AtomicU64,
    /// Set once this run has offered (or found no) autosave from a previous run. Until
    /// then the slot may hold crash work, so it must not be overwritten.
    slot_checked: AtomicBool,
    /// The language of menus, dialogs and messages.
    lang: Mutex<Lang>,
    /// `settings.json`, where a chosen language and the recent files are remembered.
    /// `None` without a config folder.
    settings: Option<PathBuf>,
    /// A project the system asked to open (a double-clicked file) that the frontend has
    /// not opened yet: it waits until no dialog is up, after any autosave offer.
    pending_open: Mutex<VecDeque<String>>,
    /// `settings.json` is read, changed and replaced from several threads; one at a time.
    settings_lock: Mutex<()>,
}

impl AppState {
    /// Starts in the remembered language, else Vietnamese; the frontend sets the system
    /// language right away when nothing is remembered.
    fn new(autosave: Option<AutosaveSlot>, settings: Option<PathBuf>) -> Self {
        let lang = settings.as_deref().and_then(settings::read_language).unwrap_or_default();
        Self {
            doc: Mutex::new(Document::new_default()),
            dialog_open: Arc::new(AtomicBool::new(false)),
            autosave,
            revision: AtomicU64::new(0),
            autosaved: AtomicU64::new(0),
            slot_checked: AtomicBool::new(false),
            lang: Mutex::new(lang),
            settings,
            pending_open: Mutex::new(VecDeque::new()),
            settings_lock: Mutex::new(()),
        }
    }
}

/// How often unsaved changes are written to the autosave slot.
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(30);

/// Undo steps kept by "Gộp lịch sử hoàn tác".
const HISTORY_KEPT: usize = 100;

fn mark_edited(state: &AppState) {
    state.revision.fetch_add(1, Ordering::AcqRel);
}

/// The document was saved, discarded, or replaced: nothing is left to recover. Call with
/// the document lock held, so an autosave tick cannot write the slot back right after.
fn clear_autosave(state: &AppState) {
    state
        .autosaved
        .store(state.revision.load(Ordering::Acquire), Ordering::Release);
    if let Some(Err(err)) = state.autosave.as_ref().map(AutosaveSlot::clear) {
        eprintln!("Sheafly: không xóa được bản tự lưu: {err}");
    }
}

/// Write unsaved changes to the slot if they changed since the last write. Runs on the
/// autosave thread. The counters are read under the lock, so a save or discard that
/// cleared the slot while this tick waited is seen and the slot stays empty.
fn autosave_tick(state: &AppState) {
    let Some(slot) = &state.autosave else {
        return;
    };
    if !state.slot_checked.load(Ordering::Acquire) && slot.pending().is_some() {
        return;
    }
    let doc = lock(state);
    let revision = state.revision.load(Ordering::Acquire);
    if revision == state.autosaved.load(Ordering::Acquire) {
        return;
    }
    if !doc.dirty {
        state.autosaved.store(revision, Ordering::Release);
        return;
    }
    match slot.write(&doc, now_millis()) {
        Ok(()) => state.autosaved.store(revision, Ordering::Release),
        Err(err) => eprintln!("Sheafly: không tự lưu được: {err}"),
    }
}

/// Released when the file dialog command finishes, including cancel and write.
struct DialogPermit(Arc<AtomicBool>);

impl Drop for DialogPermit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn begin_dialog(state: &AppState) -> Option<DialogPermit> {
    state
        .dialog_open
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .ok()
        .map(|_| DialogPermit(Arc::clone(&state.dialog_open)))
}

/// Await the plugin's non-blocking picker. Do not call `blocking_pick_file`:
/// a sync command runs on the macOS main thread, and the blocking helper waits
/// there, so the panel never gets the run loop back.
async fn await_pick<R: Runtime>(dialog: FileDialogBuilder<R>) -> Option<FilePath> {
    let (tx, mut rx) = tauri::async_runtime::channel(1);
    dialog.pick_file(move |path| {
        let _ = tx.try_send(path);
    });
    rx.recv().await.unwrap_or(None)
}

/// Await the plugin's non-blocking save panel. Same deadlock rule as `await_pick`.
async fn await_save<R: Runtime>(dialog: FileDialogBuilder<R>) -> Option<FilePath> {
    let (tx, mut rx) = tauri::async_runtime::channel(1);
    dialog.save_file(move |path| {
        let _ = tx.try_send(path);
    });
    rx.recv().await.unwrap_or(None)
}

/// The current language of menus, dialogs and messages.
fn lang(state: &AppState) -> Lang {
    *state.lang.lock().unwrap_or_else(|err| err.into_inner())
}

/// An engine error as the frontend shows it, in the current language.
fn engine_message(state: &AppState) -> impl Fn(EngineError) -> String + '_ {
    move |err| err.message(lang(state))
}

/// A failed read of a file the user picked, said the way the engine says it.
fn read_failed(state: &AppState) -> impl Fn(std::io::Error) -> String + '_ {
    move |err| EngineError::Store(err.to_string()).message(lang(state))
}

/// A failed write of a file the user picked.
fn write_failed(state: &AppState) -> impl Fn(std::io::Error) -> String + '_ {
    move |err| EngineError::Save(err.to_string()).message(lang(state))
}

/// Edits wait for open dialogs: a "save changes?" answer must apply to what the user saw.
fn ensure_no_dialog(state: &AppState) -> Result<(), String> {
    if state.dialog_open.load(Ordering::Acquire) {
        Err(i18n::text(lang(state)).dialog_busy.into())
    } else {
        Ok(())
    }
}

fn lock(state: &AppState) -> std::sync::MutexGuard<'_, Document> {
    state.doc.lock().unwrap_or_else(|err| err.into_inner())
}

fn path_from_dialog(path: FilePath) -> Option<String> {
    match path {
        FilePath::Path(path) => Some(path.to_string_lossy().to_string()),
        FilePath::Url(url) => url
            .to_file_path()
            .ok()
            .map(|path| path.to_string_lossy().to_string()),
    }
}

/// Project files are saved as `.sheafly`. Files from before the rename end in `.pdproj`
/// ("Sheafly project"); the content is the same, so both open, and a name the user
/// gives with either ending keeps it.
const PROJECT_EXTENSION: &str = "sheafly";
const LEGACY_PROJECT_EXTENSION: &str = "pdproj";

fn ensure_project_suffix(path: String) -> String {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(&format!(".{LEGACY_PROJECT_EXTENSION}")) {
        path
    } else {
        ensure_suffix(path, &format!(".{PROJECT_EXTENSION}"))
    }
}

fn is_project_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(&format!(".{PROJECT_EXTENSION}"))
        || lower.ends_with(&format!(".{LEGACY_PROJECT_EXTENSION}"))
}

/// The project file in a command line: how Windows and Linux open a double-clicked file,
/// and what the single-instance plugin forwards from a second launch.
/// A relative path is taken from `cwd`, the folder the launch came from.
fn project_path_arg<I: IntoIterator<Item = String>>(args: I, cwd: &std::path::Path) -> Option<String> {
    let arg = args.into_iter().skip(1).find(|arg| is_project_file(arg))?;
    let path = std::path::Path::new(&arg);
    Some(if path.is_absolute() { arg } else { cwd.join(path).to_string_lossy().into_owned() })
}

/// Arguments as text; a non-UTF-8 argument cannot be a path we could open anyway.
fn command_line() -> Vec<String> {
    std::env::args_os().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

fn current_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_default()
}

/// Queue a file the system asked to open and tell the frontend, which opens it when no
/// dialog is up (`open_pending`).
fn queue_open(app: &AppHandle, path: String) {
    if let Some(state) = app.try_state::<AppState>() {
        state.pending_open.lock().unwrap_or_else(|err| err.into_inner()).push_back(path);
    } else {
        eprintln!("Sheafly: nhận tệp {path} trước khi app sẵn sàng; bỏ qua");
    }
    let _ = app.emit("open-file", ());
}

fn recent_files(state: &AppState) -> Vec<String> {
    state.settings.as_deref().map(settings::read_recent).unwrap_or_default()
}

/// Put a just opened or saved file first in File → Open Recent. A settings file that
/// cannot be written only costs the list, so it is logged, not shown.
fn remember_recent(state: &AppState, path: &str) {
    let _guard = state.settings_lock.lock().unwrap_or_else(|err| err.into_inner());
    if let Some(settings) = &state.settings
        && let Err(err) = settings::remember_recent(settings, path)
    {
        eprintln!("Sheafly: không ghi được danh sách tệp gần đây: {err}");
    }
}

/// Rebuild the native menu, e.g. after the recent files changed.
fn refresh_menu(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else { return };
    match build_menu(app, lang(&state), &recent_files(&state)) {
        Ok(menu) => {
            let _ = app.set_menu(menu);
        }
        Err(err) => eprintln!("Sheafly: không dựng lại được menu: {err}"),
    }
}

fn ensure_suffix(path: String, suffix: &str) -> String {
    if path.to_ascii_lowercase().ends_with(suffix) {
        path
    } else {
        format!("{path}{suffix}")
    }
}

#[derive(Debug, PartialEq, Eq)]
enum UnsavedChoice {
    Save,
    Discard,
    Cancel,
}

/// The answer to the "save changes?" sheet that was shown with `text`'s labels.
fn unsaved_choice(result: &MessageDialogResult, text: &i18n::Text) -> UnsavedChoice {
    match result {
        MessageDialogResult::Yes => UnsavedChoice::Save,
        MessageDialogResult::No => UnsavedChoice::Discard,
        MessageDialogResult::Custom(label) if label == text.save => UnsavedChoice::Save,
        MessageDialogResult::Custom(label) if label == text.dont_save => UnsavedChoice::Discard,
        _ => UnsavedChoice::Cancel,
    }
}

/// Attach alerts to the main window. Without a parent, rfd falls back to a system
/// user notification on macOS, which can answer with the default button on its own.
fn alert<R: Runtime>(app: &AppHandle<R>, message: impl Into<String>) -> MessageDialogBuilder<R> {
    let builder = app.dialog().message(message);
    match app.get_webview_window("main") {
        Some(window) => builder.parent(&window),
        None => builder,
    }
}

/// Native "save changes?" sheet. Same non-blocking rule as `await_pick`.
async fn ask_unsaved<R: Runtime>(app: &AppHandle<R>, lang: Lang, name: &str) -> UnsavedChoice {
    let (tx, mut rx) = tauri::async_runtime::channel(1);
    let text = i18n::text(lang);
    alert(app, i18n::unsaved_message(lang, name))
        .title(text.unsaved_title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNoCancelCustom(
            text.save.into(),
            text.dont_save.into(),
            text.cancel.into(),
        ))
        .show_with_result(move |result| {
            let _ = tx.try_send(unsaved_choice(&result, text));
        });
    rx.recv().await.unwrap_or(UnsavedChoice::Cancel)
}

fn write_document(state: &AppState, path: String) -> Result<Snapshot, String> {
    let mut doc = lock(state);
    store::save_with_snapshot(&mut doc, &path, now_millis()).map_err(engine_message(state))?;
    remember_recent(state, &path);
    doc.path = Some(path);
    doc.dirty = false;
    clear_autosave(state);
    Ok(doc.snapshot())
}

/// Ask where to save. The caller holds the dialog permit.
async fn pick_save_path<R: Runtime>(app: &AppHandle<R>, state: &AppState) -> Option<String> {
    let suggested = lock(state)
        .project
        .name
        .chars()
        .map(|ch| if ch.is_whitespace() { '-' } else { ch })
        .collect::<String>();
    let picked = await_save(
        app.dialog()
            .file()
            .add_filter("Sheafly", &[PROJECT_EXTENSION])
            .set_title(i18n::text(lang(state)).save_project_title)
            .set_file_name(format!("{suggested}.{PROJECT_EXTENSION}")),
    )
    .await;
    picked.and_then(path_from_dialog).map(ensure_project_suffix)
}

/// Save to the current file, or ask where when there is none. `Ok(false)` means the
/// user cancelled the save panel. The caller holds the dialog permit.
async fn save_held<R: Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<bool, String> {
    let current = lock(state).path.clone();
    let path = match current {
        Some(path) => path,
        None => match pick_save_path(app, state).await {
            Some(path) => path,
            None => return Ok(false),
        },
    };
    write_document(state, path)?;
    Ok(true)
}

/// Whether the open document may be replaced or closed: true when it has no unsaved
/// changes, or the user saved it or chose not to. The caller holds the dialog permit.
async fn confirm_discard<R: Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<bool, String> {
    let name = {
        let doc = lock(state);
        if !doc.dirty {
            return Ok(true);
        }
        doc.project.name.clone()
    };
    match ask_unsaved(app, lang(state), &name).await {
        UnsavedChoice::Cancel => Ok(false),
        UnsavedChoice::Discard => Ok(true),
        UnsavedChoice::Save => save_held(app, state).await,
    }
}

/// Quit once the unsaved-changes question allows it (window close, Cmd+Q, Dock).
async fn quit_if_confirmed<R: Runtime>(app: AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        app.exit(0);
        return;
    };
    let Some(permit) = begin_dialog(&state) else {
        return;
    };
    match confirm_discard(&app, &state).await {
        Ok(true) => {
            {
                let _doc = lock(&state);
                clear_autosave(&state);
            }
            drop(permit);
            app.exit(0);
        }
        Ok(false) => {}
        Err(err) => {
            alert(&app, err)
                .title(i18n::text(lang(&state)).save_failed_title)
                .kind(MessageDialogKind::Error)
                .show(|_| {});
        }
    }
}

// Edits, undo, redo, and large reads run on Tauri's thread pool, not the main thread,
// so scheduling a big project never freezes the window. `src/api.ts` sends them one at
// a time, which keeps the order the user pressed the keys in.
#[tauri::command(async)]
fn project_state(state: State<'_, AppState>) -> Snapshot {
    lock(&state).snapshot()
}

#[tauri::command(async)]
fn project_metrics(state: State<'_, AppState>) -> ProjectMetrics {
    lock(&state).metrics()
}

#[tauri::command(async)]
fn network_layout(state: State<'_, AppState>, options: NetworkOptions) -> NetworkLayout {
    network::layout(&lock(&state).project, &options)
}

#[tauri::command(async)]
fn apply_command(state: State<'_, AppState>, command: Command) -> Result<Snapshot, String> {
    // Check under the lock: `confirm_discard` reads `dirty` under it after the dialog
    // flag is set, so an edit either lands before that read or is refused.
    let mut doc = lock(&state);
    ensure_no_dialog(&state)?;
    let snapshot = doc.apply(command).map_err(engine_message(&state))?;
    mark_edited(&state);
    Ok(snapshot)
}

#[tauri::command(async)]
fn undo(state: State<'_, AppState>) -> Result<Snapshot, String> {
    let mut doc = lock(&state);
    ensure_no_dialog(&state)?;
    let snapshot = doc.undo().map_err(engine_message(&state))?;
    mark_edited(&state);
    Ok(snapshot)
}

#[tauri::command(async)]
fn redo(state: State<'_, AppState>) -> Result<Snapshot, String> {
    let mut doc = lock(&state);
    ensure_no_dialog(&state)?;
    let snapshot = doc.redo().map_err(engine_message(&state))?;
    mark_edited(&state);
    Ok(snapshot)
}

#[tauri::command]
async fn new_project(app: AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(lock(&state).snapshot());
    };
    let go_on = confirm_discard(&app, &state).await?;
    // Saving inside the prompt reorders the recent files.
    refresh_menu(&app);
    if !go_on {
        return Ok(lock(&state).snapshot());
    }
    let current = lang(&state);
    let mut doc = lock(&state);
    *doc = Document::new_in(current);
    clear_autosave(&state);
    Ok(doc.snapshot())
}

#[tauri::command]
async fn open_project(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    // A second chooser while this one is visible would nest the macOS run loop.
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(lock(&state).snapshot());
    };
    let go_on = confirm_discard(&app, &state).await?;
    // Saving inside the prompt reorders the recent files.
    refresh_menu(&app);
    if !go_on {
        return Ok(lock(&state).snapshot());
    }
    let picked = await_pick(
        app.dialog()
            .file()
            .add_filter("Sheafly", &[PROJECT_EXTENSION, LEGACY_PROJECT_EXTENSION])
            .set_title(i18n::text(lang(&state)).open_project_title),
    )
    .await;
    let Some(path) = picked.and_then(path_from_dialog) else {
        return Ok(lock(&state).snapshot());
    };
    open_path_held(&app, &state, path)
}

/// Open `path` in place of the current project. The caller holds the dialog permit and
/// has already asked about unsaved changes.
fn open_path_held(app: &AppHandle, state: &AppState, path: String) -> Result<Snapshot, String> {
    let opened = store::open(&path).map_err(engine_message(state))?;
    let snapshot = {
        let mut doc = lock(state);
        *doc = opened.document;
        clear_autosave(state);
        // A recovered copy is unsaved work from the start.
        if doc.dirty {
            mark_edited(state);
        }
        doc.snapshot()
    };
    remember_recent(state, &path);
    refresh_menu(app);
    // A damaged log or a different engine still opens; say what the user is looking at.
    if let Some(message) = opened.report.message(lang(state)) {
        alert(app, message)
            .title(i18n::text(lang(state)).open_project_title)
            .kind(MessageDialogKind::Warning)
            .show(|_| {});
    }
    Ok(snapshot)
}

/// Open a file we already know the path of (Open Recent, or one the system sent): ask
/// about unsaved changes first, and drop a file that is gone from the recent list.
/// The caller holds the dialog permit.
async fn open_known_path(app: &AppHandle, state: &AppState, path: String) -> Result<Snapshot, String> {
    if !std::path::Path::new(&path).exists() {
        if let Some(settings) = &state.settings {
            let _guard = state.settings_lock.lock().unwrap_or_else(|err| err.into_inner());
            let _ = settings::forget_recent(settings, &path);
        }
        refresh_menu(app);
        return Err(i18n::file_missing(lang(state), &path));
    }
    let go_on = confirm_discard(app, state).await?;
    refresh_menu(app);
    if !go_on {
        return Ok(lock(state).snapshot());
    }
    open_path_held(app, state, path)
}

/// File → Open Recent → the file at `index` (0 is the most recent).
#[tauri::command]
async fn open_recent(app: AppHandle, state: State<'_, AppState>, index: usize) -> Result<Snapshot, String> {
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(lock(&state).snapshot());
    };
    let Some(path) = recent_files(&state).get(index).cloned() else {
        return Ok(lock(&state).snapshot());
    };
    open_known_path(&app, &state, path).await
}

/// Open the next file the system asked to open. `None` when nothing is queued or a
/// dialog is up; the file then stays queued and the frontend asks again later. The
/// permit is taken before the file, so a file is never dropped by a dialog that opens
/// in between.
#[tauri::command]
async fn open_pending(app: AppHandle, state: State<'_, AppState>) -> Result<Option<Snapshot>, String> {
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(None);
    };
    let taken = state.pending_open.lock().unwrap_or_else(|err| err.into_inner()).pop_front();
    let Some(path) = taken else {
        return Ok(None);
    };
    open_known_path(&app, &state, path).await.map(Some)
}

#[tauri::command]
async fn save_project(app: AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    // Saving to the current file shows no dialog, so it needs no permit.
    let current = lock(&state).path.clone();
    if let Some(path) = current {
        let saved = write_document(&state, path);
        refresh_menu(&app);
        return saved;
    }
    let Some(_permit) = begin_dialog(&state) else {
        return Err(i18n::text(lang(&state)).dialog_busy.into());
    };
    save_held(&app, &state).await?;
    refresh_menu(&app);
    Ok(lock(&state).snapshot())
}

#[tauri::command]
async fn save_project_as(app: AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(lock(&state).snapshot());
    };
    match pick_save_path(&app, &state).await {
        Some(path) => {
            let saved = write_document(&state, path);
            refresh_menu(&app);
            saved
        }
        None => Ok(lock(&state).snapshot()),
    }
}

#[tauri::command]
async fn import_mspdi(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(lock(&state).snapshot());
    };
    let go_on = confirm_discard(&app, &state).await?;
    // Saving inside the prompt reorders the recent files.
    refresh_menu(&app);
    if !go_on {
        return Ok(lock(&state).snapshot());
    }
    let picked = await_pick(
        app.dialog()
            .file()
            .add_filter("MSPDI", &["xml"])
            .set_title(i18n::text(lang(&state)).import_mspdi_title),
    )
    .await;
    let Some(path) = picked.and_then(path_from_dialog) else {
        return Ok(lock(&state).snapshot());
    };
    let xml = fs::read_to_string(&path).map_err(read_failed(&state))?;
    let project = read_mspdi(&xml).map_err(engine_message(&state))?;
    let mut doc = lock(&state);
    doc.replace_project(project, None)
        .map_err(engine_message(&state))?;
    doc.dirty = true;
    clear_autosave(&state);
    mark_edited(&state);
    Ok(doc.snapshot())
}

/// A native two-button question on the main window. True for the first button.
async fn ask<R: Runtime>(app: &AppHandle<R>, title: &str, message: String, yes: &str, no: &str) -> bool {
    let (tx, mut rx) = tauri::async_runtime::channel(1);
    let yes_label = yes.to_string();
    alert(app, message)
        .title(title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(yes.into(), no.into()))
        .show_with_result(move |result| {
            let chosen = match result {
                MessageDialogResult::Ok | MessageDialogResult::Yes => true,
                MessageDialogResult::Custom(label) => label == yes_label,
                _ => false,
            };
            let _ = tx.try_send(chosen);
        });
    rx.recv().await.unwrap_or(false)
}

/// After a crash the slot still holds unsaved work: offer it back. Returns the new
/// snapshot when the user restores it.
#[tauri::command]
async fn check_autosave(app: AppHandle, state: State<'_, AppState>) -> Result<Option<Snapshot>, String> {
    let Some(slot) = state.autosave.clone() else {
        return Ok(None);
    };
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(None);
    };
    // Once per run: a reloaded window must not offer old work over newer edits.
    if state.slot_checked.swap(true, Ordering::AcqRel) {
        return Ok(None);
    }
    let Some(pending) = slot.pending() else {
        return Ok(None);
    };
    if pending.is_older_than_file() {
        // The file was saved after this autosave; the slot is left over.
        let _doc = lock(&state);
        clear_autosave(&state);
        return Ok(None);
    }
    let lang = lang(&state);
    let text = i18n::text(lang);
    let source = pending.original_path.as_deref().map(file_name);
    let message = i18n::autosave_offer(lang, &pending.project_name, source.as_deref());
    if !ask(&app, text.restore_title, message, text.restore, text.drop_autosave).await {
        let _doc = lock(&state);
        clear_autosave(&state);
        return Ok(None);
    }
    let opened = match slot.restore() {
        Ok(opened) => opened,
        Err(err) => {
            // Unrecoverable: asking again at every launch would not help.
            let _doc = lock(&state);
            clear_autosave(&state);
            return Err(i18n::restore_failed(lang, &err.message(lang)));
        }
    };
    let snapshot = {
        let mut doc = lock(&state);
        *doc = opened.document;
        mark_edited(&state);
        doc.snapshot()
    };
    if let Some(message) = opened.report.message(lang) {
        alert(&app, message)
            .title(text.restore_title)
            .kind(MessageDialogKind::Warning)
            .show(|_| {});
    }
    Ok(Some(snapshot))
}

fn file_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// Fold old undo steps into the project's starting state, after the user agrees.
#[tauri::command]
async fn compact_history(app: AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(lock(&state).snapshot());
    };
    let lang = lang(&state);
    let text = i18n::text(lang);
    let undoable = lock(&state).cursor;
    if undoable <= HISTORY_KEPT {
        alert(&app, i18n::compact_not_needed(lang, undoable, HISTORY_KEPT))
            .title(text.compact_title)
            .kind(MessageDialogKind::Info)
            .show(|_| {});
        return Ok(lock(&state).snapshot());
    }
    let message = i18n::compact_question(lang, undoable - HISTORY_KEPT, HISTORY_KEPT);
    if !ask(&app, text.compact_title, message, text.compact, text.cancel).await {
        return Ok(lock(&state).snapshot());
    }
    let mut doc = lock(&state);
    doc.compact_history(HISTORY_KEPT).map_err(engine_message(&state))?;
    mark_edited(&state);
    Ok(doc.snapshot())
}

#[tauri::command]
async fn export_mspdi(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(());
    };
    let text = i18n::text(lang(&state));
    let picked = await_save(
        app.dialog()
            .file()
            .add_filter("MSPDI", &["xml"])
            .set_title(text.export_mspdi_title)
            .set_file_name(text.export_mspdi_file),
    )
    .await;
    let Some(path) = picked.and_then(path_from_dialog) else {
        return Ok(());
    };
    let path = ensure_suffix(path, ".xml");
    let xml = {
        let doc = lock(&state);
        write_mspdi(&doc.project).map_err(engine_message(&state))?
    };
    fs::write(path, xml).map_err(write_failed(&state))
}

#[tauri::command]
async fn save_pdf(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    bytes: Vec<u8>,
    file_name: String,
) -> Result<(), String> {
    // The PDF is already rendered. This only shows the save dialog and writes bytes.
    let text = i18n::text(lang(&state));
    if bytes.is_empty() {
        return Err(text.empty_pdf.into());
    }
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(());
    };
    let picked = await_save(
        app.dialog()
            .file()
            .add_filter("PDF", &["pdf"])
            .set_title(text.export_pdf_title)
            .set_file_name(file_name),
    )
    .await;
    let Some(path) = picked.and_then(path_from_dialog) else {
        return Ok(());
    };
    let path = ensure_suffix(path, ".pdf");
    fs::write(path, bytes).map_err(write_failed(&state))
}

/// Text for "About Sheafly". macOS shows the name, version, copyright and
/// credits; the other fields are for Windows and Linux.
fn about_metadata(app: &tauri::AppHandle, text: &i18n::Text) -> AboutMetadata<'static> {
    let version = app.package_info().version.to_string();
    // macOS shows `short_version` as the version and `version` as a build number in
    // parentheses, so set only one; Windows and Linux show `version`.
    let (version, short_version) = if cfg!(target_os = "macos") {
        (None, Some(version))
    } else {
        (Some(version), None)
    };
    AboutMetadata {
        name: Some("Sheafly".into()),
        version,
        short_version,
        comments: Some(text.about_text.into()),
        credits: Some(text.about_text.into()),
        copyright: Some(text.copyright.into()),
        license: Some("MIT".into()),
        ..Default::default()
    }
}

/// Append a table of items to `menu`, labelled in `lang`. Accelerators and ids are the
/// same in every language.
fn append_entries(
    app: &tauri::AppHandle,
    menu: &Submenu<tauri::Wry>,
    entries: &[i18n::Entry],
    lang: Lang,
) -> tauri::Result<()> {
    for entry in entries {
        match entry {
            i18n::Entry::Item { id, vi, en, accelerator } => {
                menu.append(&MenuItem::with_id(app, *id, lang.pick(*vi, *en), true, *accelerator)?)?
            }
            i18n::Entry::Separator => menu.append(&PredefinedMenuItem::separator(app)?)?,
        }
    }
    Ok(())
}

fn submenu(
    app: &tauri::AppHandle,
    title: &str,
    entries: &[i18n::Entry],
    lang: Lang,
) -> tauri::Result<Submenu<tauri::Wry>> {
    let menu = Submenu::new(app, title, true)?;
    append_entries(app, &menu, entries, lang)?;
    Ok(menu)
}

/// The language submenu: each language named in its own language, the current one checked.
/// Choosing one only sends `menu-action` with `lang-vi` or `lang-en`; the frontend then
/// calls `set_language`, which rebuilds the menu.
fn language_menu(app: &tauri::AppHandle, lang: Lang) -> tauri::Result<Submenu<tauri::Wry>> {
    let menu = Submenu::new(app, i18n::text(lang).menu_language, true)?;
    for (id, choice, name) in i18n::LANGUAGES {
        menu.append(&CheckMenuItem::with_id(app, id, name, true, choice == lang, None::<&str>)?)?;
    }
    Ok(menu)
}

/// File → Open Recent: the recent files (by name, with their folder), then Clear Menu.
/// Choosing a file sends `menu-action` `recent:<index>`; the frontend opens it with
/// `open_recent`, like Open… goes through `open_project`.
fn recent_menu(app: &tauri::AppHandle, lang: Lang, recent: &[String]) -> tauri::Result<Submenu<tauri::Wry>> {
    let text = i18n::text(lang);
    let menu = Submenu::new(app, text.open_recent, true)?;
    if recent.is_empty() {
        menu.append(&MenuItem::with_id(app, "recent-empty", text.recent_empty, false, None::<&str>)?)?;
    }
    for (index, file) in recent.iter().enumerate() {
        let path = std::path::Path::new(file);
        let name = path.file_name().map(|name| name.to_string_lossy()).unwrap_or_default();
        let folder = path.parent().map(|dir| dir.to_string_lossy()).unwrap_or_default();
        let label = if folder.is_empty() { name.to_string() } else { format!("{name} — {folder}") };
        // `&` marks a keyboard shortcut in menu labels on Windows and Linux.
        let label = label.replace('&', "&&");
        menu.append(&MenuItem::with_id(app, format!("recent:{index}"), label, true, None::<&str>)?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "recent-clear", text.recent_clear, !recent.is_empty(), None::<&str>)?)?;
    Ok(menu)
}

fn build_menu(app: &tauri::AppHandle, lang: Lang, recent: &[String]) -> tauri::Result<Menu<tauri::Wry>> {
    let text = i18n::text(lang);
    let app_menu = Submenu::with_items(
        app,
        "Sheafly",
        true,
        &[
            &PredefinedMenuItem::about(app, Some(text.about), Some(about_metadata(app, text)))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some(text.hide))?,
            &PredefinedMenuItem::hide_others(app, Some(text.hide_others))?,
            &PredefinedMenuItem::show_all(app, Some(text.show_all))?,
            &PredefinedMenuItem::separator(app)?,
        ],
    )?;
    append_entries(app, &app_menu, std::slice::from_ref(&i18n::QUIT), lang)?;
    let file_menu = submenu(app, text.menu_file, i18n::FILE_MENU, lang)?;
    // After New Project and Open….
    file_menu.insert(&recent_menu(app, lang, recent)?, 2)?;
    let edit_menu = submenu(app, text.menu_edit, i18n::EDIT_MENU, lang)?;
    let view_menu = submenu(app, text.menu_view, i18n::VIEW_MENU, lang)?;
    view_menu.append(&PredefinedMenuItem::separator(app)?)?;
    view_menu.append(&language_menu(app, lang)?)?;
    let project_menu = submenu(app, text.menu_project, i18n::PROJECT_MENU, lang)?;
    let help_menu = submenu(app, text.menu_help, i18n::HELP_MENU, lang)?;
    Menu::with_items(
        app,
        &[&app_menu, &file_menu, &edit_menu, &view_menu, &project_menu, &help_menu],
    )
}

/// The language the user chose and asked to remember, or `None` when there is none yet.
#[tauri::command]
fn get_language(state: State<'_, AppState>) -> Option<String> {
    state
        .settings
        .as_deref()
        .and_then(settings::read_language)
        .map(|lang| lang.code().to_string())
}

/// Switch menus, dialogs and messages to `language` (`"vi"` or `"en"`), and remember it
/// for the next launch when `remember` is true.
#[tauri::command]
fn set_language(
    app: AppHandle,
    state: State<'_, AppState>,
    language: String,
    remember: bool,
) -> Result<(), String> {
    let Some(chosen) = Lang::from_code(&language) else {
        return Err(i18n::unknown_language(lang(&state), &language));
    };
    // Rebuild the menu first, so the stored language never disagrees with it.
    app.set_menu(build_menu(&app, chosen, &recent_files(&state)).map_err(|err| err.to_string())?)
        .map_err(|err| err.to_string())?;
    *state.lang.lock().unwrap_or_else(|err| err.into_inner()) = chosen;
    // The untouched project made at launch, before the language was known, gets its
    // default names again in the chosen language.
    {
        let mut doc = lock(&state);
        if doc.commands.is_empty() && doc.path.is_none() && !doc.dirty {
            *doc = Document::new_in(chosen);
        }
    }
    let _guard = state.settings_lock.lock().unwrap_or_else(|err| err.into_inner());
    match (&state.settings, remember) {
        (Some(path), true) => settings::write_language(path, chosen)
            .map_err(|err| i18n::settings_not_saved(chosen, &err.to_string())),
        _ => Ok(()),
    }
}

/// The licenses file the installer ships (`bundle.resources`), opened in the system's text
/// viewer. The open-source licenses of the bundled packages require it to come with the app.
fn open_third_party_licenses(app: &AppHandle) {
    let opened = app
        .path()
        .resolve(THIRD_PARTY_LICENSES, BaseDirectory::Resource)
        .map_err(|err| err.to_string())
        .and_then(|path| {
            app.opener()
                .open_path(path.to_string_lossy(), None::<&str>)
                .map_err(|err| err.to_string())
        });
    if let Err(err) = opened {
        let lang = app.try_state::<AppState>().map(|state| lang(&state)).unwrap_or_default();
        alert(app, format!("{} ({err})", i18n::text(lang).licenses_unavailable)).show(|_| {});
    }
}

const THIRD_PARTY_LICENSES: &str = "THIRD_PARTY_LICENSES.txt";

/// Whether the open document has unsaved changes. False before `setup` manages the state.
fn is_dirty<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<AppState>().is_some_and(|state| lock(&state).dirty)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // First, so a second launch (a file double-clicked while Sheafly runs, on Windows
        // and Linux) hands its file to this window instead of starting another app, which
        // would also share the autosave slot.
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            if let Some(path) = project_path_arg(argv, std::path::Path::new(&cwd)) {
                queue_open(app, path);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let slot = match app.path().app_data_dir() {
                Ok(dir) => Some(AutosaveSlot::new(dir.join("autosave"))),
                Err(err) => {
                    eprintln!("Sheafly: không có thư mục dữ liệu, tắt tự lưu: {err}");
                    None
                }
            };
            let settings = match app.path().app_config_dir() {
                Ok(dir) => Some(dir.join(settings::FILE_NAME)),
                Err(err) => {
                    eprintln!("Sheafly: không có thư mục cấu hình, không nhớ ngôn ngữ: {err}");
                    None
                }
            };
            let state = AppState::new(slot, settings);
            // Windows and Linux pass a double-clicked file on the command line.
            if let Some(path) = project_path_arg(command_line(), &current_dir()) {
                state.pending_open.lock().unwrap_or_else(|err| err.into_inner()).push_back(path);
            }
            let menu = build_menu(app.handle(), lang(&state), &recent_files(&state))?;
            app.manage(state);
            app.set_menu(menu)?;
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(AUTOSAVE_INTERVAL);
                autosave_tick(&handle.state::<AppState>());
            });
            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id().as_ref() == "quit" {
                tauri::async_runtime::spawn(quit_if_confirmed(app.clone()));
                return;
            }
            if event.id().as_ref() == "third-party-licenses" {
                open_third_party_licenses(app);
                return;
            }
            if event.id().as_ref() == "recent-clear" {
                let state = app.state::<AppState>();
                if let Some(settings) = &state.settings {
                    let _guard = state.settings_lock.lock().unwrap_or_else(|err| err.into_inner());
                    let _ = settings::clear_recent(settings);
                }
                refresh_menu(app);
                return;
            }
            let _ = app.emit("menu-action", event.id().as_ref());
        })
        .on_window_event(|window, event| {
            let app = window.app_handle();
            match event {
                WindowEvent::CloseRequested { api, .. } if is_dirty(app) => {
                    api.prevent_close();
                    tauri::async_runtime::spawn(quit_if_confirmed(app.clone()));
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            project_state,
            project_metrics,
            network_layout,
            apply_command,
            undo,
            redo,
            new_project,
            open_project,
            open_recent,
            open_pending,
            save_project,
            save_project_as,
            import_mspdi,
            export_mspdi,
            save_pdf,
            check_autosave,
            compact_history,
            get_language,
            set_language
        ])
        .build(tauri::generate_context!())
        .expect("error while building Sheafly")
        .run(|app, event| {
            // Quit from the Dock or the system. `app.exit` passes a code and goes through.
            match event {
                RunEvent::ExitRequested { code: None, api, .. }
                    if is_dirty(app) =>
                {
                    api.prevent_exit();
                    tauri::async_runtime::spawn(quit_if_confirmed(app.clone()));
                }
                // macOS sends a double-clicked file (or one dropped on the Dock icon) here,
                // also when it launches the app for it.
                #[cfg(target_os = "macos")]
                RunEvent::Opened { urls } => {
                    let file = urls
                        .iter()
                        .filter_map(|url| url.to_file_path().ok())
                        .map(|path| path.to_string_lossy().into_owned())
                        .find(|path| is_project_file(path));
                    if let Some(path) = file {
                        queue_open(app, path);
                    }
                }
                _ => {}
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_project_file_on_the_command_line() {
        let args = |list: &[&str]| list.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
        let cwd = std::path::Path::new("/work");
        assert_eq!(project_path_arg(args(&["sheafly", "plan.sheafly"]), cwd).as_deref(), Some("/work/plan.sheafly"));
        assert_eq!(project_path_arg(args(&["sheafly"]), cwd), None);
        assert_eq!(project_path_arg(args(&["sheafly", "--flag", "/Plans/Cầu.SHEAFLY"]), cwd).as_deref(), Some("/Plans/Cầu.SHEAFLY"));
        assert_eq!(project_path_arg(args(&["sheafly", "/home/an/old.pdproj"]), cwd).as_deref(), Some("/home/an/old.pdproj"));
        assert_eq!(project_path_arg(args(&["sheafly", "/home/an/notes.txt"]), cwd), None);
        // The program itself is never taken, even if its path looked like a project.
        assert_eq!(project_path_arg(args(&["/opt/x.sheafly"]), cwd), None);
    }

    #[test]
    fn save_keeps_a_legacy_ending_and_adds_sheafly_otherwise() {
        assert_eq!(ensure_project_suffix("/a/plan".into()), "/a/plan.sheafly");
        assert_eq!(ensure_project_suffix("/a/plan.sheafly".into()), "/a/plan.sheafly");
        assert_eq!(ensure_project_suffix("/a/plan.PDPROJ".into()), "/a/plan.PDPROJ");
    }

    #[test]
    fn unsaved_choice_follows_the_button_labels_in_each_language() {
        let custom = |label: &str| MessageDialogResult::Custom(label.into());
        for text in [&i18n::VI, &i18n::EN] {
            assert_eq!(unsaved_choice(&custom(text.save), text), UnsavedChoice::Save);
            assert_eq!(unsaved_choice(&custom(text.dont_save), text), UnsavedChoice::Discard);
            assert_eq!(unsaved_choice(&custom(text.cancel), text), UnsavedChoice::Cancel);
            assert_eq!(unsaved_choice(&MessageDialogResult::Yes, text), UnsavedChoice::Save);
            assert_eq!(unsaved_choice(&MessageDialogResult::No, text), UnsavedChoice::Discard);
            assert_eq!(unsaved_choice(&MessageDialogResult::Cancel, text), UnsavedChoice::Cancel);
        }
        // An English "Save" is not a Vietnamese answer.
        assert_eq!(unsaved_choice(&custom(i18n::EN.save), &i18n::VI), UnsavedChoice::Cancel);
    }

    #[test]
    fn the_language_starts_from_the_settings_file() {
        let dir = std::env::temp_dir().join(format!("sheafly-tauri-lang-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join(settings::FILE_NAME);
        assert_eq!(lang(&AppState::new(None, Some(path.clone()))), Lang::Vi, "no file: Vietnamese");
        assert_eq!(lang(&AppState::new(None, None)), Lang::Vi);

        settings::write_language(&path, Lang::En).unwrap();
        let state = AppState::new(None, Some(path));
        assert_eq!(lang(&state), Lang::En);

        // Engine errors follow the current language.
        let error = lock(&state).apply(Command::RenameTask { uid: 99, name: "x".into() }).unwrap_err();
        assert_eq!(engine_message(&state)(error.clone()), "Task not found");
        *state.lang.lock().unwrap() = Lang::Vi;
        assert_eq!(engine_message(&state)(error), "Không tìm thấy công việc");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn autosave_writes_only_new_unsaved_changes_and_clears_on_save() {
        let dir = std::env::temp_dir().join(format!("sheafly-tauri-autosave-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let state = AppState::new(Some(AutosaveSlot::new(&dir)), None);
        state.slot_checked.store(true, Ordering::Release);
        let slot = state.autosave.clone().unwrap();

        autosave_tick(&state);
        assert!(slot.pending().is_none(), "a clean new project needs no autosave");

        lock(&state)
            .apply(Command::RenameProject { name: "Tự lưu".into() })
            .unwrap();
        mark_edited(&state);
        autosave_tick(&state);
        assert_eq!(slot.pending().unwrap().project_name, "Tự lưu");

        // Nothing new: the slot is not rewritten.
        let written = fs::metadata(dir.join("autosave.pdproj")).unwrap().modified().unwrap();
        autosave_tick(&state);
        assert_eq!(fs::metadata(dir.join("autosave.pdproj")).unwrap().modified().unwrap(), written);

        let path = dir.join("du-an.pdproj").to_string_lossy().to_string();
        write_document(&state, path).unwrap();
        assert!(slot.pending().is_none(), "saving empties the slot");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn autosave_keeps_crash_work_until_it_has_been_offered() {
        let dir = std::env::temp_dir().join(format!("sheafly-tauri-crash-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let slot = AutosaveSlot::new(&dir);
        let mut crashed = Document::new_default();
        crashed.apply(Command::RenameProject { name: "Trước crash".into() }).unwrap();
        slot.write(&crashed, now_millis()).unwrap();

        let state = AppState::new(Some(slot.clone()), None);
        lock(&state)
            .apply(Command::RenameProject { name: "Phiên mới".into() })
            .unwrap();
        mark_edited(&state);
        autosave_tick(&state);
        assert_eq!(slot.pending().unwrap().project_name, "Trước crash");

        state.slot_checked.store(true, Ordering::Release);
        autosave_tick(&state);
        assert_eq!(slot.pending().unwrap().project_name, "Phiên mới");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn edits_wait_while_a_dialog_is_open() {
        let state = AppState::new(None, None);
        assert!(ensure_no_dialog(&state).is_ok());
        let permit = begin_dialog(&state).expect("no dialog yet");
        assert_eq!(ensure_no_dialog(&state), Err(i18n::VI.dialog_busy.to_string()));
        *state.lang.lock().unwrap() = Lang::En;
        assert_eq!(
            ensure_no_dialog(&state),
            Err("A dialog is already open. Close it first.".to_string())
        );
        assert!(begin_dialog(&state).is_none());
        drop(permit);
        assert!(ensure_no_dialog(&state).is_ok());
    }
}

use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use projectdesk_engine::autosave::AutosaveSlot;
use projectdesk_engine::document::now_millis;
use projectdesk_engine::evm::ProjectMetrics;
use projectdesk_engine::mspdi::{read_mspdi, write_mspdi};
use projectdesk_engine::network::{self, NetworkLayout, NetworkOptions};
use projectdesk_engine::store;
use projectdesk_engine::{Command, Document, Snapshot};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, RunEvent, Runtime, State, WindowEvent};
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
}

impl AppState {
    fn new(autosave: Option<AutosaveSlot>) -> Self {
        Self {
            doc: Mutex::new(Document::new_default()),
            dialog_open: Arc::new(AtomicBool::new(false)),
            autosave,
            revision: AtomicU64::new(0),
            autosaved: AtomicU64::new(0),
            slot_checked: AtomicBool::new(false),
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
        eprintln!("ProjectDesk: không xóa được bản tự lưu: {err}");
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
        Err(err) => eprintln!("ProjectDesk: không tự lưu được: {err}"),
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

const DIALOG_BUSY: &str = "Đang mở một hộp thoại. Hãy đóng nó trước.";

/// Edits wait for open dialogs: a "save changes?" answer must apply to what the user saw.
fn ensure_no_dialog(state: &AppState) -> Result<(), String> {
    if state.dialog_open.load(Ordering::Acquire) {
        Err(DIALOG_BUSY.into())
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

fn ensure_suffix(path: String, suffix: &str) -> String {
    if path.to_ascii_lowercase().ends_with(suffix) {
        path
    } else {
        format!("{path}{suffix}")
    }
}

const SAVE_LABEL: &str = "Lưu";
const DISCARD_LABEL: &str = "Không lưu";
const CANCEL_LABEL: &str = "Hủy";

#[derive(Debug, PartialEq, Eq)]
enum UnsavedChoice {
    Save,
    Discard,
    Cancel,
}

fn unsaved_choice(result: &MessageDialogResult) -> UnsavedChoice {
    match result {
        MessageDialogResult::Yes => UnsavedChoice::Save,
        MessageDialogResult::No => UnsavedChoice::Discard,
        MessageDialogResult::Custom(label) if label == SAVE_LABEL => UnsavedChoice::Save,
        MessageDialogResult::Custom(label) if label == DISCARD_LABEL => UnsavedChoice::Discard,
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
async fn ask_unsaved<R: Runtime>(app: &AppHandle<R>, name: &str) -> UnsavedChoice {
    let (tx, mut rx) = tauri::async_runtime::channel(1);
    alert(app, format!("Dự án “{name}” có thay đổi chưa lưu. Lưu trước khi tiếp tục?"))
        .title("Thay đổi chưa lưu")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNoCancelCustom(
            SAVE_LABEL.into(),
            DISCARD_LABEL.into(),
            CANCEL_LABEL.into(),
        ))
        .show_with_result(move |result| {
            let _ = tx.try_send(unsaved_choice(&result));
        });
    rx.recv().await.unwrap_or(UnsavedChoice::Cancel)
}

fn write_document(state: &AppState, path: String) -> Result<Snapshot, String> {
    let mut doc = lock(state);
    store::save_with_snapshot(&mut doc, &path, now_millis()).map_err(|err| err.to_string())?;
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
            .add_filter("ProjectDesk", &["pdproj"])
            .set_title("Lưu dự án")
            .set_file_name(format!("{suggested}.pdproj")),
    )
    .await;
    picked
        .and_then(path_from_dialog)
        .map(|path| ensure_suffix(path, ".pdproj"))
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
    match ask_unsaved(app, &name).await {
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
                .title("Không lưu được dự án")
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
    let snapshot = doc.apply(command).map_err(|err| err.to_string())?;
    mark_edited(&state);
    Ok(snapshot)
}

#[tauri::command(async)]
fn undo(state: State<'_, AppState>) -> Result<Snapshot, String> {
    let mut doc = lock(&state);
    ensure_no_dialog(&state)?;
    let snapshot = doc.undo().map_err(|err| err.to_string())?;
    mark_edited(&state);
    Ok(snapshot)
}

#[tauri::command(async)]
fn redo(state: State<'_, AppState>) -> Result<Snapshot, String> {
    let mut doc = lock(&state);
    ensure_no_dialog(&state)?;
    let snapshot = doc.redo().map_err(|err| err.to_string())?;
    mark_edited(&state);
    Ok(snapshot)
}

#[tauri::command]
async fn new_project(app: AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(lock(&state).snapshot());
    };
    if !confirm_discard(&app, &state).await? {
        return Ok(lock(&state).snapshot());
    }
    let mut doc = lock(&state);
    *doc = Document::new_default();
    clear_autosave(&state);
    Ok(doc.snapshot())
}

#[tauri::command]
async fn open_project(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    // A second chooser while this one is visible would nest the macOS run loop.
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(lock(&state).snapshot());
    };
    if !confirm_discard(&app, &state).await? {
        return Ok(lock(&state).snapshot());
    }
    let picked = await_pick(
        app.dialog()
            .file()
            .add_filter("ProjectDesk", &["pdproj"])
            .set_title("Mở dự án"),
    )
    .await;
    let Some(path) = picked.and_then(path_from_dialog) else {
        return Ok(lock(&state).snapshot());
    };
    let opened = store::open(&path).map_err(|err| err.to_string())?;
    let snapshot = {
        let mut doc = lock(&state);
        *doc = opened.document;
        clear_autosave(&state);
        // A recovered copy is unsaved work from the start.
        if doc.dirty {
            mark_edited(&state);
        }
        doc.snapshot()
    };
    // A damaged log or a different engine still opens; say what the user is looking at.
    if let Some(message) = opened.report.message() {
        alert(&app, message)
            .title("Mở dự án")
            .kind(MessageDialogKind::Warning)
            .show(|_| {});
    }
    Ok(snapshot)
}

#[tauri::command]
async fn save_project(app: AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    // Saving to the current file shows no dialog, so it needs no permit.
    let current = lock(&state).path.clone();
    if let Some(path) = current {
        return write_document(&state, path);
    }
    let Some(_permit) = begin_dialog(&state) else {
        return Err(DIALOG_BUSY.into());
    };
    save_held(&app, &state).await?;
    Ok(lock(&state).snapshot())
}

#[tauri::command]
async fn save_project_as(app: AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(lock(&state).snapshot());
    };
    match pick_save_path(&app, &state).await {
        Some(path) => write_document(&state, path),
        None => Ok(lock(&state).snapshot()),
    }
}

#[tauri::command]
async fn import_mspdi(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(lock(&state).snapshot());
    };
    if !confirm_discard(&app, &state).await? {
        return Ok(lock(&state).snapshot());
    }
    let picked = await_pick(
        app.dialog()
            .file()
            .add_filter("MSPDI", &["xml"])
            .set_title("Nhập MSPDI"),
    )
    .await;
    let Some(path) = picked.and_then(path_from_dialog) else {
        return Ok(lock(&state).snapshot());
    };
    let xml = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let project = read_mspdi(&xml).map_err(|err| err.to_string())?;
    let mut doc = lock(&state);
    doc.replace_project(project, None)
        .map_err(|err| err.to_string())?;
    doc.dirty = true;
    clear_autosave(&state);
    mark_edited(&state);
    Ok(doc.snapshot())
}

const RESTORE_LABEL: &str = "Khôi phục";
const DROP_AUTOSAVE_LABEL: &str = "Bỏ bản tự lưu";
const COMPACT_LABEL: &str = "Gộp";

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
    let source = match &pending.original_path {
        Some(path) => format!(" của tệp “{}”", file_name(path)),
        None => String::new(),
    };
    let message = format!(
        "ProjectDesk đã đóng khi dự án “{}”{source} còn thay đổi chưa lưu. Khôi phục bản tự lưu?",
        pending.project_name
    );
    if !ask(&app, "Khôi phục dự án", message, RESTORE_LABEL, DROP_AUTOSAVE_LABEL).await {
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
            return Err(format!("Không khôi phục được bản tự lưu: {err}"));
        }
    };
    let snapshot = {
        let mut doc = lock(&state);
        *doc = opened.document;
        mark_edited(&state);
        doc.snapshot()
    };
    if let Some(message) = opened.report.message() {
        alert(&app, message)
            .title("Khôi phục dự án")
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
    let undoable = lock(&state).cursor;
    if undoable <= HISTORY_KEPT {
        alert(
            &app,
            format!("Lịch sử hoàn tác mới có {undoable} thao tác, chưa cần gộp (giữ {HISTORY_KEPT} thao tác gần nhất)."),
        )
        .title("Gộp lịch sử hoàn tác")
        .kind(MessageDialogKind::Info)
        .show(|_| {});
        return Ok(lock(&state).snapshot());
    }
    let message = format!(
        "Gộp {} thao tác cũ vào trạng thái gốc và giữ {HISTORY_KEPT} thao tác gần nhất để hoàn tác? \
         Không hoàn tác được việc gộp. Tệp nhỏ lại sau lần lưu tới.",
        undoable - HISTORY_KEPT
    );
    if !ask(&app, "Gộp lịch sử hoàn tác", message, COMPACT_LABEL, CANCEL_LABEL).await {
        return Ok(lock(&state).snapshot());
    }
    let mut doc = lock(&state);
    doc.compact_history(HISTORY_KEPT).map_err(|err| err.to_string())?;
    mark_edited(&state);
    Ok(doc.snapshot())
}

#[tauri::command]
async fn export_mspdi(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(());
    };
    let picked = await_save(
        app.dialog()
            .file()
            .add_filter("MSPDI", &["xml"])
            .set_title("Xuất MSPDI")
            .set_file_name("du-an.xml"),
    )
    .await;
    let Some(path) = picked.and_then(path_from_dialog) else {
        return Ok(());
    };
    let path = ensure_suffix(path, ".xml");
    let xml = {
        let doc = lock(&state);
        write_mspdi(&doc.project).map_err(|err| err.to_string())?
    };
    fs::write(path, xml).map_err(|err| err.to_string())
}

#[tauri::command]
async fn save_pdf(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    bytes: Vec<u8>,
    file_name: String,
) -> Result<(), String> {
    // The PDF is already rendered. This only shows the save dialog and writes bytes.
    if bytes.is_empty() {
        return Err("PDF trống".into());
    }
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(());
    };
    let picked = await_save(
        app.dialog()
            .file()
            .add_filter("PDF", &["pdf"])
            .set_title("Xuất PDF")
            .set_file_name(file_name),
    )
    .await;
    let Some(path) = picked.and_then(path_from_dialog) else {
        return Ok(());
    };
    let path = ensure_suffix(path, ".pdf");
    fs::write(path, bytes).map_err(|err| err.to_string())
}

fn menu_item(
    app: &tauri::AppHandle,
    id: &str,
    label: &str,
    accelerator: Option<&str>,
) -> tauri::Result<MenuItem<tauri::Wry>> {
    MenuItem::with_id(app, id, label, true, accelerator)
}

fn build_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let app_menu = Submenu::with_items(
        app,
        "ProjectDesk",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("Giới thiệu ProjectDesk"), None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some("Ẩn ProjectDesk"))?,
            &PredefinedMenuItem::hide_others(app, Some("Ẩn ứng dụng khác"))?,
            &PredefinedMenuItem::show_all(app, Some("Hiện tất cả"))?,
            &PredefinedMenuItem::separator(app)?,
            // Not the predefined Quit: it would exit without asking about unsaved changes.
            &menu_item(app, "quit", "Thoát ProjectDesk", Some("CmdOrCtrl+Q"))?,
        ],
    )?;
    let file_menu = Submenu::with_items(
        app,
        "Tệp",
        true,
        &[
            &menu_item(app, "new", "Dự án mới", Some("CmdOrCtrl+N"))?,
            &menu_item(app, "open", "Mở…", Some("CmdOrCtrl+O"))?,
            &menu_item(app, "save", "Lưu", Some("CmdOrCtrl+S"))?,
            &menu_item(app, "save-as", "Lưu thành…", Some("CmdOrCtrl+Shift+S"))?,
            &PredefinedMenuItem::separator(app)?,
            &menu_item(app, "import-xml", "Nhập MSPDI…", None)?,
            &menu_item(app, "export-xml", "Xuất MSPDI…", None)?,
            &menu_item(app, "export-pdf", "Xuất PDF", None)?,
        ],
    )?;
    let edit_menu = Submenu::with_items(
        app,
        "Sửa",
        true,
        &[
            &menu_item(app, "undo", "Hoàn tác", Some("CmdOrCtrl+Z"))?,
            &menu_item(app, "redo", "Làm lại", Some("CmdOrCtrl+Shift+Z"))?,
            &PredefinedMenuItem::separator(app)?,
            &menu_item(app, "indent", "Thụt vào", Some("CmdOrCtrl+]"))?,
            &menu_item(app, "outdent", "Thụt ra", Some("CmdOrCtrl+["))?,
            &menu_item(app, "delete", "Xóa công việc", Some("CmdOrCtrl+Backspace"))?,
            &PredefinedMenuItem::separator(app)?,
            &menu_item(app, "recurring", "Công việc lặp…", None)?,
            &menu_item(app, "split-task", "Tách công việc…", None)?,
        ],
    )?;
    let view_menu = Submenu::with_items(
        app,
        "Xem",
        true,
        &[
            &menu_item(app, "view-gantt", "Gantt", None)?,
            &menu_item(app, "view-tracking", "Gantt theo dõi", None)?,
            &menu_item(app, "view-resources", "Bảng nguồn lực", None)?,
            &menu_item(app, "view-usage", "Sử dụng công việc", None)?,
            &menu_item(app, "view-network", "Sơ đồ mạng", None)?,
            &menu_item(app, "view-evm", "Chỉ số EVM và burn-down", None)?,
            &PredefinedMenuItem::separator(app)?,
            &menu_item(app, "scale-day", "Thang ngày", None)?,
            &menu_item(app, "scale-week", "Thang tuần", None)?,
            &menu_item(app, "scale-month", "Thang tháng", None)?,
        ],
    )?;
    let project_menu = Submenu::with_items(
        app,
        "Dự án",
        true,
        &[
            &menu_item(app, "project-info", "Thông tin dự án…", None)?,
            &menu_item(app, "baseline", "Đường cơ sở…", None)?,
            &PredefinedMenuItem::separator(app)?,
            &menu_item(app, "level", "Cân bằng nguồn lực", None)?,
            &menu_item(app, "clear-level", "Xóa độ trễ cân bằng", None)?,
            &PredefinedMenuItem::separator(app)?,
            &menu_item(app, "reschedule-uncompleted", "Dời phần chưa xong sau ngày trạng thái", None)?,
            &PredefinedMenuItem::separator(app)?,
            &menu_item(app, "compact-history", "Gộp lịch sử hoàn tác…", None)?,
        ],
    )?;
    Menu::with_items(
        app,
        &[&app_menu, &file_menu, &edit_menu, &view_menu, &project_menu],
    )
}

/// Whether the open document has unsaved changes. False before `setup` manages the state.
fn is_dirty<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<AppState>().is_some_and(|state| lock(&state).dirty)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let slot = match app.path().app_data_dir() {
                Ok(dir) => Some(AutosaveSlot::new(dir.join("autosave"))),
                Err(err) => {
                    eprintln!("ProjectDesk: không có thư mục dữ liệu, tắt tự lưu: {err}");
                    None
                }
            };
            app.manage(AppState::new(slot));
            let menu = build_menu(app.handle())?;
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
            save_project,
            save_project_as,
            import_mspdi,
            export_mspdi,
            save_pdf,
            check_autosave,
            compact_history
        ])
        .build(tauri::generate_context!())
        .expect("error while building ProjectDesk")
        .run(|app, event| {
            // Quit from the Dock or the system. `app.exit` passes a code and goes through.
            match event {
                RunEvent::ExitRequested { code: None, api, .. }
                    if is_dirty(app) =>
                {
                    api.prevent_exit();
                    tauri::async_runtime::spawn(quit_if_confirmed(app.clone()));
                }
                _ => {}
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsaved_choice_follows_the_button_labels() {
        let custom = |label: &str| MessageDialogResult::Custom(label.into());
        assert_eq!(unsaved_choice(&custom(SAVE_LABEL)), UnsavedChoice::Save);
        assert_eq!(unsaved_choice(&custom(DISCARD_LABEL)), UnsavedChoice::Discard);
        assert_eq!(unsaved_choice(&custom(CANCEL_LABEL)), UnsavedChoice::Cancel);
        assert_eq!(unsaved_choice(&MessageDialogResult::Yes), UnsavedChoice::Save);
        assert_eq!(unsaved_choice(&MessageDialogResult::No), UnsavedChoice::Discard);
        assert_eq!(unsaved_choice(&MessageDialogResult::Cancel), UnsavedChoice::Cancel);
    }

    #[test]
    fn autosave_writes_only_new_unsaved_changes_and_clears_on_save() {
        let dir = std::env::temp_dir().join(format!("projectdesk-tauri-autosave-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let state = AppState::new(Some(AutosaveSlot::new(&dir)));
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
        let dir = std::env::temp_dir().join(format!("projectdesk-tauri-crash-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let slot = AutosaveSlot::new(&dir);
        let mut crashed = Document::new_default();
        crashed.apply(Command::RenameProject { name: "Trước crash".into() }).unwrap();
        slot.write(&crashed, now_millis()).unwrap();

        let state = AppState::new(Some(slot.clone()));
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
        let state = AppState::new(None);
        assert!(ensure_no_dialog(&state).is_ok());
        let permit = begin_dialog(&state).expect("no dialog yet");
        assert_eq!(ensure_no_dialog(&state), Err(DIALOG_BUSY.to_string()));
        assert!(begin_dialog(&state).is_none());
        drop(permit);
        assert!(ensure_no_dialog(&state).is_ok());
    }
}

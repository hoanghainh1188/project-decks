use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use projectdesk_engine::mspdi::{read_mspdi, write_mspdi};
use projectdesk_engine::store;
use projectdesk_engine::{Command, Document, Snapshot};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{Emitter, Runtime, State};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, FilePath};

struct AppState {
    doc: Mutex<Document>,
    /// True while a native file dialog owns the screen.
    dialog_open: Arc<AtomicBool>,
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

#[tauri::command]
fn project_state(state: State<'_, AppState>) -> Snapshot {
    lock(&state).snapshot()
}

#[tauri::command]
fn apply_command(state: State<'_, AppState>, command: Command) -> Result<Snapshot, String> {
    lock(&state)
        .apply(command)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn undo(state: State<'_, AppState>) -> Result<Snapshot, String> {
    lock(&state).undo().map_err(|err| err.to_string())
}

#[tauri::command]
fn redo(state: State<'_, AppState>) -> Result<Snapshot, String> {
    lock(&state).redo().map_err(|err| err.to_string())
}

#[tauri::command]
fn new_project(state: State<'_, AppState>) -> Snapshot {
    let mut doc = lock(&state);
    *doc = Document::new_default();
    doc.snapshot()
}

#[tauri::command]
async fn open_project(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    // A second chooser while this one is visible would nest the macOS run loop.
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(lock(&state).snapshot());
    };
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
    let loaded = store::load(&path).map_err(|err| err.to_string())?;
    let mut doc = lock(&state);
    *doc = loaded;
    Ok(doc.snapshot())
}

#[tauri::command]
async fn save_project(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    let path = lock(&state).path.clone();
    if let Some(path) = path {
        let mut doc = lock(&state);
        store::save(&doc, &path).map_err(|err| err.to_string())?;
        doc.path = Some(path);
        doc.dirty = false;
        return Ok(doc.snapshot());
    }
    save_project_as(app, state).await
}

#[tauri::command]
async fn save_project_as(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Snapshot, String> {
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(lock(&state).snapshot());
    };
    let suggested = lock(&state)
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
    let Some(path) = picked.and_then(path_from_dialog) else {
        return Ok(lock(&state).snapshot());
    };
    let path = ensure_suffix(path, ".pdproj");
    let mut doc = lock(&state);
    store::save(&doc, &path).map_err(|err| err.to_string())?;
    doc.path = Some(path);
    doc.dirty = false;
    Ok(doc.snapshot())
}

#[tauri::command]
async fn import_mspdi(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<Snapshot, String> {
    let Some(_permit) = begin_dialog(&state) else {
        return Ok(lock(&state).snapshot());
    };
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
            &PredefinedMenuItem::quit(app, Some("Thoát ProjectDesk"))?,
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
        ],
    )?;
    Menu::with_items(
        app,
        &[&app_menu, &file_menu, &edit_menu, &view_menu, &project_menu],
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            doc: Mutex::new(Document::new_default()),
            dialog_open: Arc::new(AtomicBool::new(false)),
        })
        .setup(|app| {
            let menu = build_menu(app.handle())?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            let _ = app.emit("menu-action", event.id().as_ref());
        })
        .invoke_handler(tauri::generate_handler![
            project_state,
            apply_command,
            undo,
            redo,
            new_project,
            open_project,
            save_project,
            save_project_as,
            import_mspdi,
            export_mspdi,
            save_pdf
        ])
        .run(tauri::generate_context!())
        .expect("error while running ProjectDesk");
}

//! File safety: atomic saves and format version checks.

use std::fs;

use rusqlite::Connection;

use projectdesk_engine::command::Command;
use projectdesk_engine::store::{load, save, FORMAT_VERSION};
use projectdesk_engine::timeutil::timestamp;
use projectdesk_engine::{Document, Project};

fn temp_dir(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("projectdesk-store-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.to_string_lossy().to_string()
}

fn document() -> Document {
    let mut doc = Document::from_project(Project::new("Lưu", timestamp(2026, 1, 5, 8, 0)));
    doc.apply(Command::RenameTask { uid: 1, name: "Một".into() }).unwrap();
    doc.apply(Command::AddTask { name: "Hai".into(), after_uid: Some(1) }).unwrap();
    doc
}

fn leftovers(dir: &str, file: &str) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .filter(|name| name != file)
        .collect()
}

fn set_meta(path: &str, key: &str, value: Option<&str>) {
    let connection = Connection::open(path).unwrap();
    match value {
        Some(value) => connection
            .execute("UPDATE meta SET value = ?1 WHERE key = ?2", [value, key])
            .unwrap(),
        None => connection.execute("DELETE FROM meta WHERE key = ?1", [key]).unwrap(),
    };
}

#[test]
fn saving_over_a_file_replaces_it_and_leaves_no_temp_file() {
    let dir = temp_dir("replace");
    let path = format!("{dir}/du-an.pdproj");
    let mut doc = document();
    save(&doc, &path).unwrap();

    // A shorter history must not keep rows from the longer one.
    doc.undo().unwrap();
    doc.commands.truncate(doc.cursor);
    save(&doc, &path).unwrap();

    let loaded = load(&path).unwrap();
    assert_eq!(loaded.commands.len(), 1);
    assert_eq!(loaded.project.tasks.len(), 1);
    assert!(leftovers(&dir, "du-an.pdproj").is_empty(), "{:?}", leftovers(&dir, "du-an.pdproj"));
}

#[test]
fn a_failed_save_keeps_the_previous_file() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("failed");
    let path = format!("{dir}/du-an.pdproj");
    save(&document(), &path).unwrap();
    let before = fs::read(&path).unwrap();

    // A read-only folder stops the temp file from being created.
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    let mut changed = document();
    changed.apply(Command::AddTask { name: "Ba".into(), after_uid: Some(2) }).unwrap();
    let result = save(&changed, &path);
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();

    let message = result.unwrap_err().to_string();
    assert!(message.contains("Không lưu được tệp"), "{message}");
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(load(&path).unwrap().project.tasks.len(), 2);
}

#[test]
fn an_unrelated_tmp_file_next_to_the_project_is_left_alone() {
    let dir = temp_dir("sibling");
    let path = format!("{dir}/du-an.pdproj");
    let sibling = format!("{path}.tmp");
    fs::write(&sibling, "của người dùng").unwrap();

    save(&document(), &path).unwrap();

    assert_eq!(fs::read_to_string(&sibling).unwrap(), "của người dùng");
    assert_eq!(leftovers(&dir, "du-an.pdproj"), vec!["du-an.pdproj.tmp".to_string()]);
}

#[test]
fn saving_through_a_symlink_updates_the_target() {
    let dir = temp_dir("symlink");
    let target = format!("{dir}/that.pdproj");
    let link = format!("{dir}/link.pdproj");
    save(&document(), &target).unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let mut changed = document();
    changed.apply(Command::AddTask { name: "Ba".into(), after_uid: Some(2) }).unwrap();
    save(&changed, &link).unwrap();

    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert_eq!(load(&target).unwrap().project.tasks.len(), 3);
}

#[test]
fn saving_keeps_the_file_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("mode");
    let path = format!("{dir}/du-an.pdproj");
    save(&document(), &path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

    save(&document(), &path).unwrap();

    assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
}

#[test]
fn files_from_a_newer_format_are_refused_with_a_clear_message() {
    let dir = temp_dir("newer");
    let path = format!("{dir}/du-an.pdproj");
    save(&document(), &path).unwrap();
    set_meta(&path, "version", Some(&(FORMAT_VERSION + 1).to_string()));

    let message = load(&path).unwrap_err().to_string();
    assert!(message.contains("phiên bản mới hơn"), "{message}");
}

#[test]
fn files_without_a_version_still_open() {
    let dir = temp_dir("legacy");
    let path = format!("{dir}/du-an.pdproj");
    save(&document(), &path).unwrap();
    set_meta(&path, "version", None);

    assert_eq!(load(&path).unwrap().project.tasks.len(), 2);
}

#[test]
fn a_corrupt_version_is_reported() {
    let dir = temp_dir("corrupt");
    let path = format!("{dir}/du-an.pdproj");
    save(&document(), &path).unwrap();
    set_meta(&path, "version", Some("abc"));

    assert!(load(&path).is_err());
}

#[test]
fn a_damaged_command_is_not_blamed_on_a_newer_version() {
    let dir = temp_dir("damaged");
    let path = format!("{dir}/du-an.pdproj");
    save(&document(), &path).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection
        .execute("UPDATE command_log SET payload = '{not json' WHERE seq = 0", [])
        .unwrap();

    let message = load(&path).unwrap_err().to_string();
    assert!(!message.contains("phiên bản mới hơn"), "{message}");
}

#[test]
fn an_unknown_command_points_at_a_newer_version() {
    let dir = temp_dir("unknown");
    let path = format!("{dir}/du-an.pdproj");
    save(&document(), &path).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE command_log SET payload = ?1 WHERE seq = 0",
            [r#"{"type":"teleport_task","uid":1}"#],
        )
        .unwrap();

    let message = load(&path).unwrap_err().to_string();
    assert!(message.contains("phiên bản mới hơn"), "{message}");
}

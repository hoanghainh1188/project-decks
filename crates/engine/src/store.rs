use std::fs::{self, File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};

use crate::command::Command;
use crate::document::{self, Document};
use crate::error::EngineError;
use crate::evm::BurndownSnapshot;
use crate::model::Project;

/// File format version. Bump it when a new `Command` variant or table is added, so an
/// older app refuses the file with a clear message instead of failing halfway through.
pub const FORMAT_VERSION: u32 = 1;

fn store_err(err: impl ToString) -> EngineError {
    EngineError::Store(err.to_string())
}

/// Write the document to `path` atomically: a fresh temp file beside the real file is
/// written in one transaction and then renamed over it, so a crash never leaves a
/// half-written project. A symlinked `path` updates the file it points to.
pub fn save(document: &Document, path: &str) -> Result<(), EngineError> {
    let target = fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
    let temp = create_temp(&target)?;
    let result = write_file(document, &temp)
        .and_then(|()| keep_permissions(&target, &temp))
        .and_then(|()| fs::rename(&temp, &target).map_err(save_err));
    match result {
        Ok(()) => {
            sync_parent(&target);
            Ok(())
        }
        Err(err) => {
            let _ = fs::remove_file(&temp);
            Err(err)
        }
    }
}

fn save_err(err: impl ToString) -> EngineError {
    EngineError::Save(err.to_string())
}

/// A new, uniquely named hidden file next to `target`. Never reuses an existing file.
fn create_temp(target: &Path) -> Result<PathBuf, EngineError> {
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "du-an".into());
    let mut last_error = None;
    for attempt in 0..100 {
        let candidate = dir.join(format!(".{name}.{}-{attempt}.tmp", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&candidate) {
            Ok(_) => return Ok(candidate),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => {
                last_error = Some(err);
                break;
            }
        }
    }
    Err(save_err(last_error.map_or_else(
        || "không tạo được tệp tạm".to_string(),
        |err| err.to_string(),
    )))
}

fn keep_permissions(target: &Path, temp: &Path) -> Result<(), EngineError> {
    match fs::metadata(target) {
        Ok(metadata) => fs::set_permissions(temp, metadata.permissions()).map_err(save_err),
        Err(_) => Ok(()),
    }
}

/// Best effort: make the rename itself durable. The data is already committed.
fn sync_parent(target: &Path) {
    if let Some(dir) = target.parent() {
        if let Ok(handle) = File::open(dir) {
            let _ = handle.sync_all();
        }
    }
}

fn write_file(document: &Document, path: &Path) -> Result<(), EngineError> {
    let mut connection = Connection::open(path).map_err(save_err)?;
    let tx = connection.transaction().map_err(save_err)?;
    tx.execute_batch(
        "CREATE TABLE meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE command_log (
            seq INTEGER PRIMARY KEY,
            payload TEXT NOT NULL
        );
        CREATE TABLE burndown (
            date INTEGER PRIMARY KEY,
            remaining_work INTEGER NOT NULL,
            ev REAL NOT NULL,
            ac REAL NOT NULL
        );",
    )
    .map_err(save_err)?;
    let base = serde_json::to_string(&document.base).map_err(save_err)?;
    let meta = [
        ("version", FORMAT_VERSION.to_string()),
        ("cursor", document.cursor.to_string()),
        ("base", base),
    ];
    for (key, value) in &meta {
        tx.execute("INSERT INTO meta(key, value) VALUES(?1, ?2)", params![key, value])
            .map_err(save_err)?;
    }
    for snapshot in &document.burndown {
        tx.execute(
            "INSERT INTO burndown(date, remaining_work, ev, ac) VALUES(?1, ?2, ?3, ?4)",
            params![snapshot.date, snapshot.remaining_work, snapshot.ev, snapshot.ac],
        )
        .map_err(save_err)?;
    }
    for (seq, command) in document.commands.iter().enumerate() {
        let payload = serde_json::to_string(command).map_err(save_err)?;
        tx.execute(
            "INSERT INTO command_log(seq, payload) VALUES(?1, ?2)",
            params![seq as i64, payload],
        )
        .map_err(save_err)?;
    }
    tx.commit().map_err(save_err)?;
    // Close before the rename so no handle still points at the temp file.
    connection.close().map_err(|(_, err)| save_err(err))
}

/// Refuse files written by a newer app. Files from before the version row open as version 1.
fn check_version(connection: &Connection) -> Result<(), EngineError> {
    let stored: Option<String> = connection
        .query_row("SELECT value FROM meta WHERE key = 'version'", [], |row| row.get(0))
        .optional()
        .map_err(store_err)?;
    let Some(stored) = stored else {
        return Ok(());
    };
    let version: u32 = stored
        .trim()
        .parse()
        .map_err(|_| store_err(format!("phiên bản định dạng không hợp lệ: {stored}")))?;
    if version > FORMAT_VERSION {
        return Err(EngineError::NewerFormat(version));
    }
    Ok(())
}

pub fn load(path: &str) -> Result<Document, EngineError> {
    let connection = Connection::open(path).map_err(store_err)?;
    check_version(&connection)?;
    let base_json: String = connection
        .query_row("SELECT value FROM meta WHERE key = 'base'", [], |row| row.get(0))
        .map_err(|err| EngineError::Store(err.to_string()))?;
    let cursor_text: String = connection
        .query_row("SELECT value FROM meta WHERE key = 'cursor'", [], |row| row.get(0))
        .map_err(|err| EngineError::Store(err.to_string()))?;
    let cursor: usize = cursor_text
        .parse()
        .map_err(|err: std::num::ParseIntError| EngineError::Store(err.to_string()))?;
    let base: Project = serde_json::from_str(&base_json).map_err(|err| EngineError::Store(err.to_string()))?;
    let mut commands = Vec::new();
    let mut statement = connection
        .prepare("SELECT payload FROM command_log ORDER BY seq")
        .map_err(|err| EngineError::Store(err.to_string()))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| EngineError::Store(err.to_string()))?;
    for row in rows {
        let payload = row.map_err(|err| EngineError::Store(err.to_string()))?;
        let command: Command = serde_json::from_str(&payload).map_err(|err| {
            // Only a command name this build does not know points at a newer app.
            if err.to_string().contains("unknown variant") {
                EngineError::UnknownCommand(err.to_string())
            } else {
                store_err(err)
            }
        })?;
        commands.push(command);
    }
    let cursor = cursor.min(commands.len());
    let project = document::replay(&base, &commands[..cursor])?;
    let burndown = load_burndown(&connection)?;
    Ok(Document {
        base,
        project,
        commands,
        cursor,
        path: Some(path.to_string()),
        dirty: false,
        burndown,
    })
}

/// User save: sample burn-down for today (or the status date day), then write the file.
pub fn save_with_snapshot(document: &mut Document, path: &str, now: i64) -> Result<(), EngineError> {
    document.record_burndown(now);
    save(document, path)
}

fn load_burndown(connection: &Connection) -> Result<Vec<BurndownSnapshot>, EngineError> {
    // Files written before phase 10 have no burn-down table.
    let exists: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'burndown'",
            [],
            |row| row.get(0),
        )
        .map_err(|err| EngineError::Store(err.to_string()))?;
    if exists == 0 {
        return Ok(Vec::new());
    }
    let mut statement = connection
        .prepare("SELECT date, remaining_work, ev, ac FROM burndown ORDER BY date")
        .map_err(|err| EngineError::Store(err.to_string()))?;
    let rows = statement
        .query_map([], |row| {
            Ok(BurndownSnapshot {
                date: row.get(0)?,
                remaining_work: row.get(1)?,
                ev: row.get(2)?,
                ac: row.get(3)?,
            })
        })
        .map_err(|err| EngineError::Store(err.to_string()))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| EngineError::Store(err.to_string()))
}

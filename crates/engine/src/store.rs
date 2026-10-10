use rusqlite::{params, Connection};

use crate::command::Command;
use crate::document::{self, Document};
use crate::error::EngineError;
use crate::evm::BurndownSnapshot;
use crate::model::Project;

const VERSION: &str = "1";

pub fn save(document: &Document, path: &str) -> Result<(), EngineError> {
    let connection = Connection::open(path).map_err(|err| EngineError::Store(err.to_string()))?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS command_log (
                seq INTEGER PRIMARY KEY,
                payload TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS burndown (
                date INTEGER PRIMARY KEY,
                remaining_work INTEGER NOT NULL,
                ev REAL NOT NULL,
                ac REAL NOT NULL
            );",
        )
        .map_err(|err| EngineError::Store(err.to_string()))?;
    connection
        .execute("DELETE FROM burndown", [])
        .map_err(|err| EngineError::Store(err.to_string()))?;
    for snapshot in &document.burndown {
        connection
            .execute(
                "INSERT INTO burndown(date, remaining_work, ev, ac) VALUES(?1, ?2, ?3, ?4)",
                params![snapshot.date, snapshot.remaining_work, snapshot.ev, snapshot.ac],
            )
            .map_err(|err| EngineError::Store(err.to_string()))?;
    }
    let base = serde_json::to_string(&document.base).map_err(|err| EngineError::Store(err.to_string()))?;
    connection
        .execute("DELETE FROM command_log", [])
        .map_err(|err| EngineError::Store(err.to_string()))?;
    connection
        .execute(
            "INSERT INTO meta(key, value) VALUES('version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![VERSION],
        )
        .map_err(|err| EngineError::Store(err.to_string()))?;
    connection
        .execute(
            "INSERT INTO meta(key, value) VALUES('cursor', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![document.cursor.to_string()],
        )
        .map_err(|err| EngineError::Store(err.to_string()))?;
    connection
        .execute(
            "INSERT INTO meta(key, value) VALUES('base', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![base],
        )
        .map_err(|err| EngineError::Store(err.to_string()))?;
    for (seq, command) in document.commands.iter().enumerate() {
        let payload = serde_json::to_string(command).map_err(|err| EngineError::Store(err.to_string()))?;
        connection
            .execute(
                "INSERT INTO command_log(seq, payload) VALUES(?1, ?2)",
                params![seq as i64, payload],
            )
            .map_err(|err| EngineError::Store(err.to_string()))?;
    }
    Ok(())
}

pub fn load(path: &str) -> Result<Document, EngineError> {
    let connection = Connection::open(path).map_err(|err| EngineError::Store(err.to_string()))?;
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
        let command: Command = serde_json::from_str(&payload).map_err(|err| EngineError::Store(err.to_string()))?;
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

use rusqlite::{params, Connection};

use crate::command::Command;
use crate::document::{self, Document};
use crate::error::EngineError;
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
            );",
        )
        .map_err(|err| EngineError::Store(err.to_string()))?;
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
    Ok(Document {
        base,
        project,
        commands,
        cursor,
        path: Some(path.to_string()),
        dirty: false,
    })
}

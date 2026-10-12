//! One autosave slot for the open project, so a crash loses at most a few seconds of
//! work. The app picks the folder (its data directory) and the timing; this module
//! writes, finds, restores, and clears the slot.
//!
//! The slot holds a normal `.pdproj` plus a small JSON note naming the file it came
//! from. The note is written last and removed first, so a note always has a project.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::error::EngineError;
use crate::store::{self, Opened};

const PROJECT_FILE: &str = "autosave.pdproj";
const NOTE_FILE: &str = "autosave.json";

/// What the slot holds, read from its note.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pending {
    /// The file the project was opened from or last saved to; `None` for a new project.
    pub original_path: Option<String>,
    pub project_name: String,
    /// Wall-clock time of the autosave, Unix milliseconds.
    pub saved_at: i64,
}

impl Pending {
    /// Whether the original file changed after this autosave, so the slot holds older
    /// work than the file (a save whose clean-up of the slot failed, for example).
    pub fn is_older_than_file(&self) -> bool {
        let Some(path) = &self.original_path else {
            return false;
        };
        let modified = fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok());
        modified.is_some_and(|since_epoch| since_epoch.as_millis() as i64 > self.saved_at)
    }
}

#[derive(Clone, Debug)]
pub struct AutosaveSlot {
    dir: PathBuf,
}

impl AutosaveSlot {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn project_path(&self) -> PathBuf {
        self.dir.join(PROJECT_FILE)
    }

    fn note_path(&self) -> PathBuf {
        self.dir.join(NOTE_FILE)
    }

    /// Write the document into the slot, replacing what was there.
    pub fn write(&self, document: &Document, now: i64) -> Result<(), EngineError> {
        fs::create_dir_all(&self.dir).map_err(save_err)?;
        store::save(document, &self.project_path().to_string_lossy())?;
        let note = Pending {
            original_path: document.path.clone(),
            project_name: document.project.name.clone(),
            saved_at: now,
        };
        let text = serde_json::to_string(&note).map_err(save_err)?;
        write_atomically(&self.note_path(), text.as_bytes())
    }

    /// The slot's contents, if an autosave is waiting. A note without its project, or one
    /// that does not parse, counts as empty.
    pub fn pending(&self) -> Option<Pending> {
        if !self.project_path().is_file() {
            return None;
        }
        let text = fs::read_to_string(self.note_path()).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Open the autosaved project as unsaved changes to the file it came from.
    pub fn restore(&self) -> Result<Opened, EngineError> {
        let pending = self
            .pending()
            .ok_or(EngineError::NoAutosave)?;
        let mut opened = store::open(&self.project_path().to_string_lossy())?;
        let damaged_copy = opened
            .report
            .recovery
            .as_ref()
            .is_some_and(|recovery| recovery.copy);
        // A damaged autosave stays an untitled copy, like any damaged file.
        opened.document.path = if damaged_copy {
            None
        } else {
            pending.original_path
        };
        opened.document.dirty = true;
        Ok(opened)
    }

    /// Empty the slot. Nothing to remove is fine.
    pub fn clear(&self) -> Result<(), EngineError> {
        remove_if_present(&self.note_path())?;
        remove_if_present(&self.project_path())
    }
}

fn save_err(err: impl ToString) -> EngineError {
    EngineError::Save(err.to_string())
}

fn remove_if_present(path: &Path) -> Result<(), EngineError> {
    match fs::remove_file(path) {
        Err(err) if err.kind() != ErrorKind::NotFound => Err(save_err(err)),
        _ => Ok(()),
    }
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), EngineError> {
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, bytes)
        .and_then(|()| fs::rename(&temp, path))
        .map_err(|err| {
            let _ = fs::remove_file(&temp);
            save_err(err)
        })
}

//! `settings.json` in the app config folder, shaped like `{"language":"en"}`.

use std::fs;
use std::io;
use std::path::Path;

use sheafly_engine::Lang;
use serde_json::{Map, Value};

pub const FILE_NAME: &str = "settings.json";

fn read_object(path: &Path) -> Map<String, Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| match value {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .unwrap_or_default()
}

/// The language the user chose, or `None` when the file, the key, or a known value is missing.
pub fn read_language(path: &Path) -> Option<Lang> {
    read_object(path)
        .get("language")
        .and_then(Value::as_str)
        .and_then(Lang::from_code)
}

/// Remember `lang`, keeping any other settings. Writes a temp file beside the real one
/// and renames it over, so a crash never leaves half a file.
pub fn write_language(path: &Path, lang: Lang) -> io::Result<()> {
    let mut settings = read_object(path);
    settings.insert("language".into(), Value::String(lang.code().into()));
    let text = serde_json::to_string_pretty(&Value::Object(settings)).map_err(io::Error::other)?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir)?;
    let temp = dir.join(format!(".{FILE_NAME}.{}.tmp", std::process::id()));
    let written = fs::write(&temp, text).and_then(|()| fs::rename(&temp, path));
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sheafly-settings-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn language_round_trips_through_the_file() {
        let dir = temp_dir("round-trip");
        let path = dir.join(FILE_NAME);
        assert_eq!(read_language(&path), None, "no file yet");

        write_language(&path, Lang::En).unwrap();
        assert_eq!(read_language(&path), Some(Lang::En));
        let stored: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(stored, serde_json::json!({ "language": "en" }));

        write_language(&path, Lang::Vi).unwrap();
        assert_eq!(read_language(&path), Some(Lang::Vi));
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name != FILE_NAME)
            .collect();
        assert!(leftovers.is_empty(), "temp file left behind: {leftovers:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn other_settings_are_kept_and_bad_files_are_ignored() {
        let dir = temp_dir("other");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE_NAME);

        fs::write(&path, r#"{"language":"fr","zoom":2}"#).unwrap();
        assert_eq!(read_language(&path), None, "unknown language");
        write_language(&path, Lang::En).unwrap();
        let stored: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(stored, serde_json::json!({ "language": "en", "zoom": 2 }));

        fs::write(&path, "not json").unwrap();
        assert_eq!(read_language(&path), None);
        fs::write(&path, "{}").unwrap();
        assert_eq!(read_language(&path), None, "no language key");
        let _ = fs::remove_dir_all(&dir);
    }
}

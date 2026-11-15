//! Existing JSON/INI formats stay compatible; unrelated keys and comments survive edits.
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
pub fn atomic_write(path: &Path, content: &[u8]) -> io::Result<()> {
    replace_file(path, content, true)
}
/// Reconstructible embedded assets are verified against their bytes on every
/// start, so they need atomic replacement but no per-file disk flush.
pub fn atomic_cache_write(path: &Path, content: &[u8]) -> io::Result<()> {
    replace_file(path, content, false)
}
fn replace_file(path: &Path, content: &[u8], durable: bool) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No parent directory"))?;
    fs::create_dir_all(directory)?;
    let temporary = directory.join(format!(
        ".{}.{}-{}.tmp",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(content)?;
        if durable {
            file.sync_all()?;
        }
        drop(file);
        for attempt in 0..4 {
            match fs::rename(&temporary, path) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied && attempt < 3 => {
                    std::thread::sleep(std::time::Duration::from_millis(250))
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!()
    })();
    let _ = fs::remove_file(temporary);
    result
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub values: Map<String, Value>,
}
impl Settings {
    pub fn load(path: &Path, defaults: &Map<String, Value>) -> Result<Self, String> {
        let mut values = defaults.clone();
        match fs::read(path) {
            Ok(bytes) => {
                let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
                let actual: Value = serde_json::from_slice(bytes)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                let actual = actual.as_object().ok_or("Settings must be a JSON object")?;
                values.extend(actual.clone());
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        Ok(Self { values })
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let mut bytes = serde_json::to_vec_pretty(&self.values).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        atomic_write(path, &bytes).map_err(|e| e.to_string())
    }
    pub fn text(&self, key: &str) -> &str {
        self.values.get(key).and_then(Value::as_str).unwrap_or("")
    }
    pub fn boolean(&self, key: &str) -> bool {
        self.values
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub struct Ini {
    pub values: BTreeMap<String, String>,
    original: Vec<String>,
}
impl Ini {
    pub fn parse(source: &str) -> Self {
        let mut values = BTreeMap::new();
        let original: Vec<_> = source
            .trim_start_matches('\u{feff}')
            .lines()
            .map(str::to_owned)
            .collect();
        for line in &original {
            let line = line.trim();
            if line.starts_with(['#', ';', '[']) {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                values.insert(key.trim().to_owned(), value.trim().to_owned());
            }
        }
        Self { values, original }
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        match fs::read(path) {
            Ok(bytes) => Ok(Self::parse(&String::from_utf8_lossy(&bytes))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::parse("")),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn render(&self) -> String {
        let mut lines = Vec::new();
        let mut written = BTreeSet::new();
        for line in &self.original {
            let trimmed = line.trim();
            if !trimmed.starts_with(['#', ';', '[']) {
                if let Some((key, _)) = line.split_once('=') {
                    let key = key.trim();
                    if let Some(value) = self.values.get(key) {
                        if written.insert(key.to_owned()) {
                            lines.push(format!("{key}={value}"));
                        }
                        continue;
                    }
                }
            }
            lines.push(line.clone());
        }
        for (key, value) in &self.values {
            if written.insert(key.clone()) {
                lines.push(format!("{key}={value}"));
            }
        }
        lines.join("\n") + "\n"
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        atomic_write(path, self.render().as_bytes()).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ini_keeps_comments_unknown_options_and_uses_the_last_duplicate() {
        let mut ini = Ini::parse(
            "\u{feff}; comment\n[Graphics]\nsharpness=0.5\nunknown=keep\nsharpness=0.7\n",
        );
        assert_eq!(ini.values["sharpness"], "0.7");
        ini.values.insert("sharpness".into(), "0.9".into());
        let result = ini.render();
        assert!(result.contains("; comment\n[Graphics]\n"));
        assert!(result.contains("unknown=keep"));
        assert_eq!(result.matches("sharpness=").count(), 1);
        assert!(result.contains("sharpness=0.9"));
    }
    #[test]
    fn json_updates_keep_future_settings_and_can_replace_an_existing_file() {
        let dir = std::env::temp_dir().join(format!("bb-rust-config-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        atomic_write(&path, br#"{"future":{"x":2},"fullscreen":false}"#).unwrap();
        let mut settings = Settings::load(&path, &Map::new()).unwrap();
        settings.values.insert("fullscreen".into(), true.into());
        settings.save(&path).unwrap();
        let loaded = Settings::load(&path, &Map::new()).unwrap();
        assert_eq!(loaded.values["future"]["x"], 2);
        assert!(loaded.boolean("fullscreen"));
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
}

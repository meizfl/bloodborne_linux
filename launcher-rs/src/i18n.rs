//! Initial Russian/English resource selection. The remaining source languages
//! are tracked in MIGRATION.md; locale precedence matches the existing launcher.
use std::path::Path;
pub fn select(settings: &Path) -> Result<String, String> {
    let mut requested = std::env::var("BB_UI_LANGUAGE").unwrap_or_default();
    if requested.is_empty() {
        match std::fs::read(settings) {
            Ok(raw) => {
                let raw = raw.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&raw);
                let value: serde_json::Value = serde_json::from_slice(raw)
                    .map_err(|e| format!("{}: {e}", settings.display()))?;
                requested = value["ui_language"].as_str().unwrap_or("").to_owned();
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    if requested == "ru" || requested == "en" {
        return Ok(requested);
    }
    for key in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        if let Ok(value) = std::env::var(key) {
            if !value.is_empty() {
                return Ok(if value.to_lowercase().starts_with("ru") {
                    "ru"
                } else {
                    "en"
                }
                .into());
            }
        }
    }
    Ok("en".into())
}

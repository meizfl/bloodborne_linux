use std::path::Path;
use std::process::Command;
pub fn label() -> String {
    let system = match std::env::consts::OS {
        "linux" => "Linux",
        "windows" => "Windows",
        "macos" => "macOS",
        other => other,
    };
    format!("{system} · {}", std::env::consts::ARCH)
}
fn open(argument: &std::ffi::OsStr) -> Result<(), String> {
    #[cfg(windows)]
    let mut command = Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    command.arg(argument);
    crate::bootstrap::desktop_environment(&mut command);
    for (variable, marker) in [
        ("FONTCONFIG_FILE", "BB_BUNDLED_FONTCONFIG"),
        ("LIBDECOR_PLUGIN_DIR", "BB_BUNDLED_LIBDECOR"),
        ("XLOCALEDIR", "BB_BUNDLED_XLOCALE"),
    ] {
        if std::env::var_os(marker).is_some()
            && std::env::var_os(marker) == std::env::var_os(variable)
        {
            command.env_remove(variable);
        }
        command.env_remove(marker);
    }
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
pub fn open_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Unsupported web link".into());
    }
    open(std::ffi::OsStr::new(url))
}
pub fn open_path(path: &Path) -> Result<(), String> {
    open(path.as_os_str())
}

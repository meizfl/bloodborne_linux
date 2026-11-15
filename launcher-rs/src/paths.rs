use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Paths {
    pub install: PathBuf,
    pub data: PathBuf,
    pub runtime: PathBuf,
}
impl Paths {
    pub fn detect() -> Result<Self, String> {
        let install = std::env::var_os("BB_PORT_DIR")
            .map(PathBuf::from)
            .unwrap_or(
                std::env::current_exe()
                    .map_err(|e| e.to_string())?
                    .parent()
                    .ok_or("Executable has no parent")?
                    .to_owned(),
            );
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .map(PathBuf::from)
            .ok_or("User home is unavailable")?;
        let data = std::env::var_os("BB_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| Self::default_data(&home));
        let runtime = std::env::var_os("BB_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| install.join("game_runtime"));
        Ok(Self {
            install,
            data,
            runtime,
        })
    }
    fn default_data(home: &Path) -> PathBuf {
        if cfg!(windows) {
            std::env::var_os("LOCALAPPDATA")
                .or_else(|| std::env::var_os("APPDATA"))
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join("AppData/Local"))
                .join("Bloodborne")
        } else {
            home.join(".local/share/Bloodborne")
        }
    }
    pub fn settings(&self) -> PathBuf {
        self.data.join("settings.json")
    }
    pub fn ini(&self) -> PathBuf {
        std::env::var_os("BB_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|| self.data.join("bbport.ini"))
    }
    pub fn probe(&self) -> PathBuf {
        self.runtime.join(if cfg!(windows) {
            "bb-probe.exe"
        } else {
            "bb-probe"
        })
    }
    pub fn game(&self) -> PathBuf {
        std::env::var_os("BB_GAME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| self.install.join("Game"))
    }
}

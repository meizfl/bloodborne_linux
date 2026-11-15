//! The shipped executable embeds its toolkit. Linux re-execs itself once so
//! the dynamic loader sees the private search path before opening Tcl/Tk.
use crate::{paths::Paths, resources};
pub fn toolkit() -> Result<(), String> {
    if !resources::ASSETS
        .iter()
        .any(|(name, _)| name.starts_with("toolkit/lib/"))
    {
        return Ok(());
    }
    let paths = Paths::detect()?;
    let root = resources::extract(&paths.data.join("cache"))?.join("toolkit");
    let lib = root.join("lib");
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        if std::env::var_os("BB_RS_BOOTSTRAP").as_deref() != Some(root.as_os_str()) {
            let mut command =
                std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
            command.args(std::env::args_os().skip(1));
            let previous = std::env::var_os("LD_LIBRARY_PATH");
            let mut search = vec![lib.clone()];
            if let Some(previous) = &previous {
                search.extend(std::env::split_paths(previous));
            }
            command.env(
                "LD_LIBRARY_PATH",
                std::env::join_paths(search).map_err(|e| e.to_string())?,
            );
            command.env("BB_RS_BOOTSTRAP", &root);
            command.env(
                "BB_RS_ORIGINAL_LD_LIBRARY_PATH",
                previous.unwrap_or_default(),
            );
            return Err(command.exec().to_string());
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        extern "system" {
            fn SetDllDirectoryW(path: *const u16) -> i32;
        }
        let wide: Vec<u16> = lib.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { SetDllDirectoryW(wide.as_ptr()) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    std::env::set_var("BB_TK_LIB_DIR", lib);
    std::env::set_var("TCL_LIBRARY", root.join("tcl9.1"));
    std::env::set_var("TK_LIBRARY", root.join("tk9.2"));
    Ok(())
}
/// Browsers and desktop helpers must get the user's original loader environment.
pub fn desktop_environment(command: &mut std::process::Command) {
    if std::env::var_os("BB_RS_BOOTSTRAP").is_some() {
        match std::env::var_os("BB_RS_ORIGINAL_LD_LIBRARY_PATH").filter(|v| !v.is_empty()) {
            Some(value) => {
                command.env("LD_LIBRARY_PATH", value);
            }
            None => {
                command.env_remove("LD_LIBRARY_PATH");
            }
        }
        command
            .env_remove("TCL_LIBRARY")
            .env_remove("TK_LIBRARY")
            .env_remove("BB_TK_LIB_DIR");
    }
    command
        .env_remove("BB_RS_BOOTSTRAP")
        .env_remove("BB_RS_ORIGINAL_LD_LIBRARY_PATH");
}

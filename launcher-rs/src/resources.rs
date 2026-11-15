use std::path::{Path, PathBuf};
include!(concat!(env!("OUT_DIR"), "/assets.rs"));
pub fn extract(directory: &Path) -> Result<PathBuf, String> {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for (name, bytes) in ASSETS {
        name.hash(&mut hash);
        bytes.hash(&mut hash);
    }
    let root = directory.join(format!("rust-assets-{:016x}", hash.finish()));
    for (name, bytes) in ASSETS {
        let path = root.join(name);
        if std::fs::read(&path).ok().as_deref() != Some(*bytes) {
            crate::config::atomic_cache_write(&path, bytes).map_err(|e| e.to_string())?;
        }
    }
    Ok(root)
}

pub fn register_fonts(directory: &Path) -> Result<(), String> {
    #[cfg(unix)]
    unsafe {
        use std::ffi::{c_char, c_int, c_void, CString};
        let library = libloading::Library::new("libfontconfig.so.1").map_err(|e| e.to_string())?;
        let current: libloading::Symbol<unsafe extern "C" fn() -> *mut c_void> = library
            .get(b"FcConfigGetCurrent\0")
            .map_err(|e| e.to_string())?;
        let add: libloading::Symbol<unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int> =
            library
                .get(b"FcConfigAppFontAddFile\0")
                .map_err(|e| e.to_string())?;
        let config = current();
        for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().and_then(|x| x.to_str()) != Some("ttf") {
                continue;
            }
            let path =
                CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
            if add(config, path.as_ptr()) == 0 {
                return Err("Cannot register embedded font".into());
            }
        }
    }
    #[cfg(windows)]
    unsafe {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "gdi32")]
        extern "system" {
            fn AddFontResourceExW(
                path: *const u16,
                flags: u32,
                reserved: *mut std::ffi::c_void,
            ) -> i32;
        }
        for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().and_then(|x| x.to_str()) != Some("ttf") {
                continue;
            }
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            if AddFontResourceExW(wide.as_ptr(), 0x10, std::ptr::null_mut()) == 0 {
                return Err("Cannot register embedded font".into());
            }
        }
    }
    Ok(())
}

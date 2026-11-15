//! Windows directory junctions need no Developer Mode/symlink privilege.
use std::{ffi::c_void, os::windows::ffi::OsStrExt, path::Path};
#[link(name = "kernel32")]
extern "system" {
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *mut c_void,
        creation: u32,
        flags: u32,
        template: *mut c_void,
    ) -> *mut c_void;
    fn DeviceIoControl(
        file: *mut c_void,
        code: u32,
        input: *const c_void,
        input_size: u32,
        output: *mut c_void,
        output_size: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}
struct Handle(*mut c_void);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub(super) fn create(source: &Path, destination: &Path) -> Result<(), String> {
    let source = std::fs::canonicalize(source).map_err(|e| e.to_string())?;
    let raw: Vec<u16> = source.as_os_str().encode_wide().collect();
    let prefix: Vec<u16> = r"\??\".encode_utf16().collect();
    let verbatim: Vec<u16> = r"\\?\".encode_utf16().collect();
    let unc: Vec<u16> = r"\\?\UNC\".encode_utf16().collect();
    let ordinary_unc: Vec<u16> = r"\\".encode_utf16().collect();
    let mut substitute = prefix;
    if raw.starts_with(&unc) {
        substitute.extend(r"UNC\".encode_utf16());
        substitute.extend_from_slice(&raw[unc.len()..]);
    } else if raw.starts_with(&verbatim) {
        substitute.extend_from_slice(&raw[verbatim.len()..]);
    } else if raw.starts_with(&ordinary_unc) {
        substitute.extend(r"UNC\".encode_utf16());
        substitute.extend_from_slice(&raw[2..]);
    } else {
        substitute.extend_from_slice(&raw);
    }
    let subst_len = substitute.len() * 2;
    let print_len = raw.len() * 2;
    let data_len = 8 + subst_len + 2 + print_len + 2;
    if data_len + 8 > 16 * 1024 {
        return Err("Junction target exceeds Windows reparse-point limit".into());
    }
    let mut data = Vec::with_capacity(data_len + 8);
    data.extend_from_slice(&0xa0000003u32.to_le_bytes());
    data.extend_from_slice(&(data_len as u16).to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    for v in [0, subst_len, subst_len + 2, print_len] {
        data.extend_from_slice(&(v as u16).to_le_bytes());
    }
    for v in substitute
        .into_iter()
        .chain(Some(0))
        .chain(raw)
        .chain(Some(0))
    {
        data.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::create_dir(destination).map_err(|e| e.to_string())?;
    let result = (|| {
        let wide: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                0x40000000,
                7,
                std::ptr::null_mut(),
                3,
                0x02200000,
                std::ptr::null_mut(),
            )
        };
        if handle == (-1isize as *mut c_void) {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let handle = Handle(handle);
        let mut returned = 0;
        if unsafe {
            DeviceIoControl(
                handle.0,
                0x000900a4,
                data.as_ptr().cast(),
                data.len() as u32,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir(destination);
    }
    result
}

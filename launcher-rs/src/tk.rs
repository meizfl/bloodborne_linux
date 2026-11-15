//! In-process Tcl/Tk 9 bindings. All GUI access stays on the interpreter thread.
//! Commands cross the ABI as Tcl objects, never interpolated user strings.
use libloading::Library;
use std::ffi::{c_char, c_int, c_void, CString};
use std::marker::PhantomData;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

#[repr(C)]
struct Interp {
    _private: [u8; 0],
}
#[repr(C)]
struct Obj {
    _private: [u8; 0],
}
type ObjProc = unsafe extern "C" fn(*mut c_void, *mut Interp, isize, *const *mut Obj) -> c_int;
type DeleteProc = unsafe extern "C" fn(*mut c_void);

type Result<T> = std::result::Result<T, String>;
struct Api {
    _libraries: Vec<Library>,
    create: unsafe extern "C" fn() -> *mut Interp,
    delete: unsafe extern "C" fn(*mut Interp),
    init: unsafe extern "C" fn(*mut Interp) -> c_int,
    tk_init: unsafe extern "C" fn(*mut Interp) -> c_int,
    eval_objv: unsafe extern "C" fn(*mut Interp, isize, *const *mut Obj, c_int) -> c_int,
    new_string: unsafe extern "C" fn(*const c_char, isize) -> *mut Obj,
    get_string: unsafe extern "C" fn(*mut Obj, *mut isize) -> *const c_char,
    get_result: unsafe extern "C" fn(*mut Interp) -> *mut Obj,
    set_result: unsafe extern "C" fn(*mut Interp, *mut Obj),
    increment: unsafe extern "C" fn(*mut Obj),
    decrement: unsafe extern "C" fn(*mut Obj),
    create_command: unsafe extern "C" fn(
        *mut Interp,
        *const c_char,
        Option<ObjProc>,
        *mut c_void,
        Option<DeleteProc>,
    ) -> *mut c_void,
    do_event: unsafe extern "C" fn(c_int) -> c_int,
    main_windows: unsafe extern "C" fn() -> c_int,
}

static API: OnceLock<Arc<Api>> = OnceLock::new();

#[cfg(unix)]
unsafe fn open(path: &Path) -> Result<Library> {
    use libloading::os::unix::{Library as UnixLibrary, RTLD_GLOBAL, RTLD_NOW};
    // Tk and its extensions resolve symbols from the owning Tcl interpreter.
    UnixLibrary::open(Some(path), RTLD_NOW | RTLD_GLOBAL)
        .map(Into::into)
        .map_err(|e| e.to_string())
}
#[cfg(windows)]
unsafe fn open(path: &Path) -> Result<Library> {
    Library::new(path).map_err(|e| e.to_string())
}

impl Api {
    unsafe fn load(directory: &Path) -> Result<Arc<Self>> {
        if let Some(api) = API.get() {
            return Ok(api.clone());
        }
        #[cfg(target_os = "linux")]
        let names = ("libtcl9.1.so", "libtcl9tk9.2.so");
        #[cfg(windows)]
        let names = ("tcl91.dll", "tcl9tk92.dll");
        #[cfg(not(any(target_os = "linux", windows)))]
        let names = ("libtcl9.1.dylib", "libtcl9tk9.2.dylib");
        let tcl = open(&directory.join(names.0))?;
        let tk = open(&directory.join(names.1))?;
        let version: libloading::Symbol<
            unsafe extern "C" fn(*mut c_int, *mut c_int, *mut c_int, *mut c_int),
        > = tcl.get(b"Tcl_GetVersion\0").map_err(|e| e.to_string())?;
        let (mut major, mut minor, mut patch, mut kind) = (0, 0, 0, 0);
        version(&mut major, &mut minor, &mut patch, &mut kind);
        if major != 9 {
            return Err(format!("Tcl 9 ABI required; found {major}.{minor}"));
        }
        macro_rules! function {
            ($lib:expr, $name:literal) => {
                *$lib
                    .get(concat!($name, "\0").as_bytes())
                    .map_err(|e| e.to_string())?
            };
        }
        let find: unsafe extern "C" fn(*const c_char) = function!(tcl, "Tcl_FindExecutable");
        let executable = CString::new(
            std::env::current_exe()
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .as_bytes(),
        )
        .map_err(|e| e.to_string())?;
        find(executable.as_ptr());
        let api = Arc::new(Self {
            create: function!(tcl, "Tcl_CreateInterp"),
            delete: function!(tcl, "Tcl_DeleteInterp"),
            init: function!(tcl, "Tcl_Init"),
            tk_init: function!(tk, "Tk_Init"),
            eval_objv: function!(tcl, "Tcl_EvalObjv"),
            new_string: function!(tcl, "Tcl_NewStringObj"),
            get_string: function!(tcl, "Tcl_GetStringFromObj"),
            get_result: function!(tcl, "Tcl_GetObjResult"),
            set_result: function!(tcl, "Tcl_SetObjResult"),
            increment: function!(tcl, "Tcl_IncrRefCount"),
            decrement: function!(tcl, "Tcl_DecrRefCount"),
            create_command: function!(tcl, "Tcl_CreateObjCommand2"),
            do_event: function!(tcl, "Tcl_DoOneEvent"),
            main_windows: function!(tk, "Tk_GetNumMainWindows"),
            _libraries: vec![tk, tcl],
        });
        // The toolkit keeps process-global exit hooks, so its libraries remain loaded.
        let _ = API.set(api.clone());
        Ok(api)
    }
    unsafe fn string(&self, object: *mut Obj) -> String {
        let mut length = 0;
        let data = (self.get_string)(object, &mut length);
        if data.is_null() || length <= 0 {
            return String::new();
        }
        String::from_utf8_lossy(std::slice::from_raw_parts(data.cast(), length as usize))
            .into_owned()
    }
}

pub struct Tk {
    api: Arc<Api>,
    interpreter: *mut Interp,
    // Tcl interpreters must not move to another thread, even behind a Mutex.
    _thread: PhantomData<Rc<()>>,
}

type Handler = dyn FnMut(&[String]) -> Result<String>;
struct Command {
    api: Arc<Api>,
    handler: std::cell::RefCell<Box<Handler>>,
}
unsafe extern "C" fn command(
    data: *mut c_void,
    interp: *mut Interp,
    count: isize,
    values: *const *mut Obj,
) -> c_int {
    let command = &*data.cast::<Command>();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let arguments: Vec<String> = std::slice::from_raw_parts(values, count as usize)
            .iter()
            .skip(1)
            .map(|value| command.api.string(*value))
            .collect();
        let mut handler = command
            .handler
            .try_borrow_mut()
            .map_err(|_| "Recursive GUI callback".to_string())?;
        handler(&arguments)
    }))
    .unwrap_or_else(|_| Err("Rust GUI callback panicked".into()));
    let (status, text) = match result {
        Ok(text) => (0, text),
        Err(text) => (1, text),
    };
    let value = (command.api.new_string)(text.as_ptr().cast(), text.len() as isize);
    (command.api.set_result)(interp, value);
    status
}
unsafe extern "C" fn delete_command(data: *mut c_void) {
    drop(Box::from_raw(data.cast::<Command>()));
}

impl Tk {
    pub fn has_windows(&self) -> bool {
        unsafe { (self.api.main_windows)() > 0 }
    }
    pub fn step(&self) {
        unsafe {
            (self.api.do_event)(0);
        }
    }
    pub fn snapshot(&self, path: &Path) -> Result<()> {
        use std::ffi::c_ulong;
        use std::io::Write;
        #[repr(C)]
        struct Image {
            width: c_int,
            height: c_int,
            xoffset: c_int,
            format: c_int,
            data: *const u32,
        }
        type Read = unsafe extern "C" fn(
            *mut c_void,
            c_ulong,
            c_int,
            c_int,
            u32,
            u32,
            c_ulong,
            c_int,
        ) -> *mut Image;
        type Free = unsafe extern "C" fn(*mut Image) -> c_int;
        if self.call(&["tk", "windowingsystem"])? != "wayland" {
            return Err("Framebuffer capture requires native Wayland".into());
        }
        let id = self.call(&["winfo", "id", "."])?;
        let id = u64::from_str_radix(id.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
        let width: u32 = self
            .call(&["winfo", "width", "."])?
            .parse::<u32>()
            .map_err(|e| e.to_string())?;
        let height: u32 = self
            .call(&["winfo", "height", "."])?
            .parse::<u32>()
            .map_err(|e| e.to_string())?;
        unsafe {
            let read: Read = *self.api._libraries[0]
                .get(b"XGetImage\0")
                .map_err(|e| e.to_string())?;
            let free: Free = *self.api._libraries[0]
                .get(b"XDestroyImage\0")
                .map_err(|e| e.to_string())?;
            let image = read(
                std::ptr::null_mut(),
                id as c_ulong,
                0,
                0,
                width,
                height,
                c_ulong::MAX,
                2,
            );
            if image.is_null() {
                return Err("Cannot read Tk framebuffer".into());
            }
            let mut bytes = Vec::with_capacity(width as usize * height as usize * 3);
            for index in 0..(width as usize * height as usize) {
                let pixel = (*image).data.add(index).read_unaligned();
                bytes.extend_from_slice(&[(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8]);
            }
            free(image);
            let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
            write!(file, "P6\n{width} {height}\n255\n").map_err(|e| e.to_string())?;
            file.write_all(&bytes).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn new(libraries: &Path) -> Result<Self> {
        unsafe {
            let api = Api::load(libraries)?;
            let interpreter = (api.create)();
            if interpreter.is_null() {
                return Err("Cannot allocate Tcl interpreter".into());
            }
            let tk = Self {
                api,
                interpreter,
                _thread: PhantomData,
            };
            tk.check((tk.api.init)(interpreter))?;
            tk.check((tk.api.tk_init)(interpreter))?;
            Ok(tk)
        }
    }
    fn check(&self, code: c_int) -> Result<String> {
        let result = unsafe { self.api.string((self.api.get_result)(self.interpreter)) };
        if code == 0 {
            Ok(result)
        } else {
            Err(result)
        }
    }
    pub fn call<S: AsRef<str>>(&self, arguments: &[S]) -> Result<String> {
        unsafe {
            let objects: Vec<_> = arguments
                .iter()
                .map(|value| {
                    let value = value.as_ref();
                    let object = (self.api.new_string)(value.as_ptr().cast(), value.len() as isize);
                    (self.api.increment)(object);
                    object
                })
                .collect();
            let status = (self.api.eval_objv)(
                self.interpreter,
                objects.len() as isize,
                objects.as_ptr(),
                0,
            );
            let result = self.check(status);
            for object in objects {
                (self.api.decrement)(object);
            }
            result
        }
    }
    pub fn register<F>(&self, name: &str, handler: F) -> Result<()>
    where
        F: FnMut(&[String]) -> Result<String> + 'static,
    {
        let name = CString::new(name).map_err(|e| e.to_string())?;
        let data = Box::into_raw(Box::new(Command {
            api: self.api.clone(),
            handler: std::cell::RefCell::new(Box::new(handler)),
        }));
        let result = unsafe {
            (self.api.create_command)(
                self.interpreter,
                name.as_ptr(),
                Some(command),
                data.cast(),
                Some(delete_command),
            )
        };
        if result.is_null() {
            unsafe {
                drop(Box::from_raw(data));
            }
            return Err("Cannot register Rust command".into());
        }
        Ok(())
    }
    pub fn library_directory() -> PathBuf {
        std::env::var_os("BB_TK_LIB_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/opt/tk92/lib"))
    }
}
impl Drop for Tk {
    fn drop(&mut self) {
        unsafe {
            (self.api.delete)(self.interpreter);
        }
    }
}

//! Faithful migration renderer for the reference widget tree and image assets.
//! Callback provenance stays explicit until the matching Rust action is ported.
use crate::{paths::Paths, tk::Tk};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;

type Result<T> = std::result::Result<T, String>;
#[derive(Debug)]
pub struct Event {
    pub callback: String,
    pub arguments: Vec<String>,
}
pub struct Ui {
    pub tk: Rc<Tk>,
    pub manifest: Value,
    pub events: Rc<RefCell<VecDeque<Event>>>,
    pub current_page: String,
    pub assets: PathBuf,
    pub canvas_ids: HashMap<(String, i64), String>,
}
impl Ui {
    fn string(&self, value: &Value) -> Result<String> {
        Ok(match value {
            Value::Null => String::new(),
            Value::String(v) => v.clone(),
            Value::Bool(v) => if *v { "1" } else { "0" }.into(),
            Value::Number(v) => v.to_string(),
            Value::Array(v) => {
                let mut command = vec!["list".to_owned()];
                for item in v {
                    command.push(self.string(item)?);
                }
                self.tk.call(&command)?
            }
            _ => return Err(format!("Unexpected toolkit value: {value}")),
        })
    }
    fn options(&self, value: &Value, arguments: &mut Vec<String>) -> Result<()> {
        for (key, value) in value
            .as_object()
            .ok_or("Widget options must be an object")?
        {
            // An unspecified nullable ttk option inherits its style. Explicitly
            // setting it to an empty Tcl object can suppress that inheritance.
            if value.is_null()
                || (value.as_str() == Some("")
                    && arguments.get(1).map(String::as_str) != Some("create"))
            {
                continue;
            }
            let option = if key.starts_with('-') {
                key.clone()
            } else {
                format!("-{key}")
            };
            arguments.push(option);
            arguments.push(self.string(value)?);
        }
        Ok(())
    }
    pub fn load(tk: Rc<Tk>, assets: &Path, paths: &Paths, language: &str) -> Result<Self> {
        let file = assets.join("ui").join(language).join("ui.json");
        let text = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
        let mut manifest: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        fn expand(value: &mut Value, paths: &Paths) {
            match value {
                Value::String(text) => {
                    *text = text
                        .replace("@PLATFORM@", &crate::platform::label().to_uppercase())
                        .replace("@INSTALL@", &paths.install.to_string_lossy())
                        .replace("@DATA@", &paths.data.to_string_lossy())
                        .replace("@GAME@", &paths.game().to_string_lossy())
                }
                Value::Array(values) => {
                    for value in values {
                        expand(value, paths);
                    }
                }
                Value::Object(values) => {
                    for value in values.values_mut() {
                        expand(value, paths);
                    }
                }
                _ => {}
            }
        }
        expand(&mut manifest, paths);
        let events = Rc::new(RefCell::new(VecDeque::new()));
        let mut ui = Self {
            tk,
            manifest,
            events,
            current_page: String::new(),
            assets: assets.to_owned(),
            canvas_ids: HashMap::new(),
        };
        ui.create(language)?;
        Ok(ui)
    }
    fn create(&mut self, language: &str) -> Result<()> {
        self.tk.call(&["ttk::setTheme", "clam"])?;
        self.tk.call(&["wm", "title", ".", "Bloodborne — bbport"])?;
        self.tk.call(&[
            "wm",
            "geometry",
            ".",
            self.manifest["geometry"]
                .as_str()
                .ok_or("Missing geometry")?,
        ])?;
        self.tk
            .call(&[".", "configure", "-background", "#08090a"])?;
        for font in self.manifest["fonts"].as_array().ok_or("Missing fonts")? {
            let name = font["name"].as_str().ok_or("Missing font name")?;
            let mut command = vec!["font".into(), "create".into(), name.into()];
            self.options(&font["options"], &mut command)?;
            if self.tk.call(&command).is_err() {
                command[1] = "configure".into();
                self.tk.call(&command)?;
            }
        }
        for image in self.manifest["images"].as_array().ok_or("Missing images")? {
            let file = self
                .assets
                .join("ui")
                .join(language)
                .join(image["file"].as_str().ok_or("Missing image file")?);
            self.tk.call(&[
                "image",
                "create",
                "photo",
                image["name"].as_str().ok_or("Missing image name")?,
                "-file",
                &file.to_string_lossy(),
            ])?;
        }
        for (name, metadata) in self.manifest["callbacks"]
            .as_object()
            .ok_or("Missing callbacks")?
        {
            if let Some(widget) = metadata["widget"].as_str() {
                let function = metadata["name"].as_str().unwrap_or("");
                let method = if function.ends_with("/yview") {
                    Some("yview")
                } else if function.ends_with("/xview") {
                    Some("xview")
                } else if function == "Scrollbar/set" {
                    Some("set")
                } else {
                    None
                };
                if let Some(method) = method {
                    let tk = Rc::downgrade(&self.tk);
                    let widget = widget.to_owned();
                    self.tk.register(name, move |arguments| {
                        let mut command = vec![widget.clone(), method.to_owned()];
                        command.extend_from_slice(arguments);
                        tk.upgrade().ok_or("Toolkit was closed")?.call(&command)
                    })?;
                    continue;
                }
            }
            let queue = self.events.clone();
            let key = name.clone();
            self.tk.register(name, move |arguments| {
                queue.borrow_mut().push_back(Event {
                    callback: key.clone(),
                    arguments: arguments.to_vec(),
                });
                Ok(String::new())
            })?;
        }
        for (name, value) in self.manifest["tcl_variables"]
            .as_object()
            .ok_or("Missing variables")?
        {
            self.tk
                .call(&["set".to_owned(), name.clone(), self.string(value)?])?;
        }
        for command in self.manifest["styles"]
            .as_array()
            .ok_or("Missing style commands")?
        {
            let arguments: Result<Vec<String>> = command
                .as_array()
                .ok_or("Bad style command")?
                .iter()
                .map(|v| self.string(v))
                .collect();
            self.tk
                .call(&arguments?)
                .map_err(|e| format!("Style {command}: {e}"))?;
        }
        for widget in self.manifest["widgets"]
            .as_array()
            .ok_or("Missing widgets")?
        {
            let mut command = vec![
                widget["command"]
                    .as_str()
                    .ok_or("Missing widget command")?
                    .into(),
                widget["path"].as_str().ok_or("Missing widget path")?.into(),
            ];
            self.options(&widget["options"], &mut command)?;
            self.tk
                .call(&command)
                .map_err(|e| format!("Widget {}: {e}", widget["path"]))?;
            if let Some(contents) = widget["contents"].as_object() {
                let path = widget["path"].as_str().unwrap();
                if let Some(index) = contents.get("current").and_then(Value::as_i64) {
                    if index >= 0 {
                        self.tk.call(&[path, "current", &index.to_string()])?;
                    } else {
                        self.tk
                            .call(&[path, "set", contents["text"].as_str().unwrap_or("")])?;
                    }
                } else if let Some(text) = contents.get("text").and_then(Value::as_str) {
                    let state = widget["options"]["state"].as_str().unwrap_or("normal");
                    if state != "normal" {
                        self.tk.call(&[path, "configure", "-state", "normal"])?;
                    }
                    let index = if widget["command"] == "text" {
                        "1.0"
                    } else {
                        "0"
                    };
                    self.tk.call(&[path, "insert", index, text])?;
                    if state != "normal" {
                        self.tk.call(&[path, "configure", "-state", state])?;
                    }
                } else if let Some(items) = contents.get("items").and_then(Value::as_array) {
                    for item in items {
                        self.tk
                            .call(&[path, "insert", "end", item.as_str().unwrap_or("")])?;
                    }
                }
            }
            if let Some(states) = widget["states"].as_array() {
                let persistent: Vec<Value> = states
                    .iter()
                    .filter(|state| {
                        matches!(
                            state.as_str(),
                            Some("disabled" | "readonly" | "selected" | "alternate" | "invalid")
                        )
                    })
                    .cloned()
                    .collect();
                if !persistent.is_empty() {
                    self.tk.call(&[
                        widget["path"].as_str().unwrap(),
                        "state",
                        &self.string(&Value::Array(persistent))?,
                    ])?;
                }
            }
            for manager in ["pack", "grid"] {
                if let Some(propagate) = widget["propagate"][manager].as_bool() {
                    self.tk.call(&[
                        manager,
                        "propagate",
                        widget["path"].as_str().unwrap(),
                        if propagate { "1" } else { "0" },
                    ])?;
                }
            }
        }
        for item in self.manifest["canvas_items"]
            .as_array()
            .ok_or("Missing canvas items")?
        {
            let canvas = item["canvas"].as_str().ok_or("Missing canvas")?;
            let mut command = vec![
                canvas.into(),
                "create".into(),
                item["kind"].as_str().ok_or("Missing item kind")?.into(),
            ];
            for coordinate in item["coords"]
                .as_array()
                .ok_or("Missing item coordinates")?
            {
                command.push(self.string(coordinate)?);
            }
            self.options(&item["options"], &mut command)?;
            let id = self
                .tk
                .call(&command)
                .map_err(|e| format!("Canvas {canvas}: {e}"))?;
            self.canvas_ids.insert(
                (canvas.into(), item["id"].as_i64().ok_or("Missing item id")?),
                id,
            );
        }
        for menu in self.manifest["menus"].as_array().ok_or("Missing menus")? {
            let mut command = vec![
                menu["path"].as_str().unwrap().into(),
                "add".into(),
                menu["kind"].as_str().unwrap().into(),
            ];
            self.options(&menu["options"], &mut command)?;
            self.tk.call(&command)?;
        }
        for layout in self.manifest["layout"].as_array().ok_or("Missing layout")? {
            let kind = layout[0].as_str().ok_or("Missing geometry manager")?;
            let path = layout[1].as_str().ok_or("Missing geometry widget")?;
            if let Some(axis) = kind.strip_prefix("grid-") {
                let mut command = vec![
                    "grid".into(),
                    format!("{axis}configure"),
                    path.into(),
                    self.string(&layout[2])?,
                ];
                let config = layout[3].as_array().ok_or("Missing grid configuration")?;
                for value in config {
                    command.push(self.string(value)?);
                }
                self.tk.call(&command)?;
            } else {
                // Page mapping belongs to navigation, not to a captured stacking state.
                if kind == "place"
                    && self.manifest["pages"]
                        .as_object()
                        .unwrap()
                        .values()
                        .any(|v| v.as_str() == Some(path))
                {
                    continue;
                }
                let mut command = vec![kind.into(), path.into()];
                self.options(&layout[2], &mut command)?;
                self.tk.call(&command)?;
            }
        }
        self.show_page("play")?;
        self.tk.call(&["update", "idletasks"])?;
        Ok(())
    }
    pub fn show_page(&mut self, name: &str) -> Result<()> {
        let pages = self.manifest["pages"].as_object().ok_or("Missing pages")?;
        let selected = pages
            .get(name)
            .and_then(Value::as_str)
            .ok_or("Unknown page")?;
        for page in pages.values().filter_map(Value::as_str) {
            if page != selected {
                self.tk.call(&["place", "forget", page])?;
            }
        }
        self.tk.call(&[
            "place",
            selected,
            "-x",
            "0",
            "-y",
            "0",
            "-relwidth",
            "1",
            "-relheight",
            "1",
        ])?;
        self.current_page = name.into();
        let names: Vec<String> = self.manifest["nav_widgets"]
            .as_object()
            .ok_or("Missing navigation widgets")?
            .keys()
            .cloned()
            .collect();
        for key in names {
            self.nav_paint(&key, false)?;
        }
        Ok(())
    }
    pub fn nav_paint(&self, name: &str, hover: bool) -> Result<()> {
        let nav = &self.manifest["nav_widgets"][name];
        let canvas = nav["canvas"].as_str().unwrap();
        let items = &nav["items"];
        let selected = self.current_page == name;
        for item in ["pill", "bar"] {
            if let Some(id) = items[item]
                .as_i64()
                .and_then(|id| self.canvas_ids.get(&(canvas.into(), id)))
            {
                if item == "pill" {
                    let image = if selected {
                        self.manifest["nav_pills"]["on"].as_str().unwrap_or("")
                    } else if hover {
                        self.manifest["nav_pills"]["hover"].as_str().unwrap_or("")
                    } else {
                        ""
                    };
                    self.tk.call(&[
                        canvas,
                        "itemconfigure",
                        id,
                        "-image",
                        image,
                        "-state",
                        if image.is_empty() { "hidden" } else { "normal" },
                    ])?;
                } else {
                    self.tk.call(&[
                        canvas,
                        "itemconfigure",
                        id,
                        "-state",
                        if selected { "normal" } else { "hidden" },
                    ])?;
                }
            }
        }
        for (item, color) in [
            (
                "icon",
                if selected {
                    "#c2a878"
                } else if hover {
                    "#bdb8ae"
                } else {
                    "#8a8c90"
                },
            ),
            (
                "label",
                if selected {
                    "#ece7dd"
                } else if hover {
                    "#d9d4ca"
                } else {
                    "#bdb8ae"
                },
            ),
        ] {
            if let Some(id) = items[item]
                .as_i64()
                .and_then(|id| self.canvas_ids.get(&(canvas.into(), id)))
            {
                self.tk
                    .call(&[canvas, "itemconfigure", id, "-fill", color])?;
            }
        }
        Ok(())
    }
}

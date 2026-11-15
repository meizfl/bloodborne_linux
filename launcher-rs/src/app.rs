//! Rust controller. No Python callbacks execute in this process.
use crate::{
    config::{Ini, Settings},
    paths::Paths,
    scroll::Spring,
    ui::{Event, Ui},
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
type Result<T> = std::result::Result<T, String>;

pub struct App {
    pub ui: Ui,
    pub paths: Paths,
    pub settings: Settings,
    pub ini: Ini,
    pending: BTreeSet<String>,
    canvas: BTreeMap<String, String>,
    spring: Spring,
    scroll_page: String,
    scroll_time: Instant,
    scroll_timer: Option<String>,
}
impl App {
    pub fn new(ui: Ui, paths: Paths) -> Result<Self> {
        let mut defaults = ui.manifest["app_defaults"]
            .as_object()
            .ok_or("Missing application defaults")?
            .clone();
        defaults.insert(
            "game_dir".into(),
            paths.game().to_string_lossy().to_string().into(),
        );
        let mut settings = Settings::load(&paths.settings(), &defaults)?;
        let mut ini = Ini::load(&paths.ini())?;
        for (key, value) in ui.manifest["ini_defaults"]
            .as_object()
            .ok_or("Missing graphics defaults")?
        {
            ini.values
                .entry(key.clone())
                .or_insert_with(|| value.as_str().unwrap_or("").to_owned());
        }
        if settings.text("play_with").is_empty() {
            let device = if ini.values.get("mouse_look").map(String::as_str) == Some("1") {
                "keyboard"
            } else {
                "controller"
            };
            settings.values.insert("play_with".into(), device.into());
        }
        let mut app = Self {
            ui,
            paths,
            settings,
            ini,
            pending: BTreeSet::new(),
            canvas: BTreeMap::new(),
            spring: Spring::default(),
            scroll_page: String::new(),
            scroll_time: Instant::now(),
            scroll_timer: None,
        };
        app.initialize()?;
        Ok(app)
    }
    fn initialize(&mut self) -> Result<()> {
        for (key, metadata) in self.ui.manifest["variables"].as_object().unwrap() {
            let name = metadata["name"].as_str().unwrap();
            let value = if metadata["store"] == "ini" {
                self.ini.values.get(key).cloned().unwrap_or_default()
            } else {
                match self.settings.values.get(key) {
                    Some(Value::Bool(v)) => if *v { "1" } else { "0" }.into(),
                    Some(Value::String(v)) => v.clone(),
                    Some(v) => v.to_string(),
                    None => String::new(),
                }
            };
            self.ui.tk.call(&["set", name, &value])?;
        }
        for item in self.ui.manifest["canvas_items"].as_array().unwrap() {
            if item["kind"] != "window" {
                continue;
            }
            let canvas = item["canvas"].as_str().unwrap();
            for (page, path) in self.ui.manifest["pages"].as_object().unwrap() {
                if canvas.starts_with(&format!("{}.", path.as_str().unwrap())) {
                    self.canvas.insert(page.clone(), canvas.into());
                }
            }
        }
        let queue = self.ui.events.clone();
        self.ui.tk.register("bb::variable", move |arguments| {
            if let Some(key) = arguments.first() {
                queue.borrow_mut().push_back(Event {
                    callback: format!("@variable/{key}"),
                    arguments: Vec::new(),
                });
            }
            Ok(String::new())
        })?;
        let queue = self.ui.events.clone();
        self.ui.tk.register("bb::animate", move |_| {
            queue.borrow_mut().push_back(Event {
                callback: "@animate".into(),
                arguments: Vec::new(),
            });
            Ok(String::new())
        })?;
        for (key, metadata) in self.ui.manifest["variables"].as_object().unwrap() {
            let script = self.ui.tk.call(&["list", "bb::variable", key])?;
            self.ui.tk.call(&[
                "trace",
                "add",
                "variable",
                metadata["name"].as_str().unwrap(),
                "write",
                &script,
            ])?;
        }
        for binding in self.ui.manifest["bindings"].as_array().unwrap() {
            self.ui.tk.call(&[
                "bind",
                binding[0].as_str().unwrap(),
                binding[1].as_str().unwrap(),
                binding[2].as_str().unwrap(),
            ])?;
        }
        // Rust owns lifecycle rather than the reference's Python close callback.
        self.ui
            .tk
            .call(&["wm", "protocol", ".", "WM_DELETE_WINDOW", "destroy ."])?;
        self.sync_choices()?;
        self.drain()?;
        Ok(())
    }
    fn number(arguments: &[String], index: usize) -> i32 {
        arguments
            .get(index)
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    }
    fn sync_choices(&self) -> Result<()> {
        for metadata in self.ui.manifest["callbacks"].as_object().unwrap().values() {
            if metadata["name"] != "Launcher/choice/show" {
                continue;
            }
            let args = &metadata["arguments"];
            let (Some(variable), Some(widget), Some(values)) = (
                args["var"].as_str(),
                args["box"].as_str(),
                args["values"].as_array(),
            ) else {
                continue;
            };
            let current = self.ui.tk.call(&["set", variable])?;
            let index = values
                .iter()
                .position(|v| v.as_str() == Some(&current))
                .unwrap_or(0);
            self.ui.tk.call(&[widget, "current", &index.to_string()])?;
        }
        Ok(())
    }
    fn update_variable(&mut self, key: &str) -> Result<()> {
        let metadata = &self.ui.manifest["variables"][key];
        let value = self
            .ui
            .tk
            .call(&["set", metadata["name"].as_str().ok_or("Unknown variable")?])?;
        if metadata["store"] == "ini" {
            self.ini.values.insert(key.into(), value);
            self.ini.save(&self.paths.ini())?;
        } else {
            let value = if self.ui.manifest["app_defaults"][key].is_boolean() {
                Value::Bool(value == "1" || value == "true")
            } else {
                Value::String(value)
            };
            self.settings.values.insert(key.into(), value);
            self.settings.save(&self.paths.settings())?;
        }
        self.sync_choices()
    }
    pub fn navigate(&mut self, page: &str) -> Result<()> {
        self.spring.cancel();
        if let Some(timer) = self.scroll_timer.take() {
            self.ui.tk.call(&["after", "cancel", &timer])?;
        }
        self.ui.show_page(page)
    }
    fn wheel(&mut self, arguments: &[String]) -> Result<()> {
        let widget = arguments.get(14).map(String::as_str).unwrap_or("");
        if widget.contains("popdown") {
            return Ok(());
        }
        if self.ui.tk.call(&["winfo", "class", widget]).ok().as_deref() == Some("Text") {
            return Ok(());
        }
        if !self.canvas.contains_key(&self.ui.current_page) {
            return Ok(());
        }
        if self.scroll_page != self.ui.current_page {
            self.spring.cancel();
            self.scroll_page = self.ui.current_page.clone();
        }
        self.spring.input(
            -f64::from(Self::number(arguments, 18)) / 120.0
                * 64.0
                * self.ui.manifest["dpi"].as_f64().unwrap_or(1.0),
        );
        if self.scroll_timer.is_none() {
            self.scroll_time = Instant::now();
            self.scroll_timer = Some(self.ui.tk.call(&["after", "8", "bb::animate"])?);
        }
        Ok(())
    }
    fn animate(&mut self) -> Result<()> {
        self.scroll_timer = None;
        if self.scroll_page != self.ui.current_page {
            self.spring.cancel();
            return Ok(());
        }
        let now = Instant::now();
        let pixels = self
            .spring
            .step(now.duration_since(self.scroll_time).as_secs_f64());
        self.scroll_time = now;
        if let Some(canvas) = self.canvas.get(&self.ui.current_page) {
            let canvas = canvas.as_str();
            if pixels != 0 {
                let before = self.ui.tk.call(&[canvas, "yview"])?;
                self.ui
                    .tk
                    .call(&[canvas, "yview", "scroll", &pixels.to_string(), "units"])?;
                if self.ui.tk.call(&[canvas, "yview"])? == before {
                    self.spring.cancel();
                }
            }
            if self.spring.active() {
                self.scroll_timer = Some(self.ui.tk.call(&["after", "8", "bb::animate"])?);
            }
        }
        Ok(())
    }
    fn dispatch(&mut self, event: Event) -> Result<()> {
        if let Some(key) = event.callback.strip_prefix("@variable/") {
            return self.update_variable(key);
        }
        if event.callback == "@animate" {
            return self.animate();
        }
        let Some(metadata) = self.ui.manifest["callbacks"].get(&event.callback).cloned() else {
            return Ok(());
        };
        let name = metadata["name"].as_str().unwrap_or("");
        let action = |key: &str| {
            metadata["actions"]
                .as_array()
                .is_some_and(|items| items.iter().any(|v| v.as_str() == Some(key)))
        };
        let args = &metadata["arguments"];
        if let Some(variable) = metadata["set_variable"].as_str() {
            let value = metadata["set_value"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| metadata["set_value"].to_string());
            self.ui.tk.call(&["set", variable, &value])?;
            return Ok(());
        }
        if action("nav_paint") {
            let page = metadata["defaults"][0]
                .as_str()
                .ok_or("Missing navigation target")?;
            return self.ui.nav_paint(page, action("hover"));
        }
        if action("navigate") {
            let page = metadata["defaults"][0]
                .as_str()
                .ok_or("Missing navigation target")?;
            return self.navigate(page);
        }
        if name == "HelpMixin/topic_card/toggle" {
            let body = args["body"].as_str().ok_or("Missing help card body")?;
            let arrow = args["arrow"].as_str().ok_or("Missing help card arrow")?;
            let shown = self.ui.tk.call(&["winfo", "ismapped", body])? == "1";
            if shown {
                self.ui.tk.call(&["pack", "forget", body])?;
            } else {
                let dpi = self.ui.manifest["dpi"].as_f64().unwrap_or(1.0);
                let padx = format!(
                    "{} {}",
                    (52.0 * dpi).round() as i32,
                    (18.0 * dpi).round() as i32
                );
                let pady = format!("0 {}", (12.0 * dpi).round() as i32);
                self.ui
                    .tk
                    .call(&["pack", body, "-fill", "x", "-padx", &padx, "-pady", &pady])?;
            }
            self.ui
                .tk
                .call(&[arrow, "configure", "-text", if shown { "›" } else { "⌄" }])?;
            return Ok(());
        }
        if name == "Launcher/link_row/lambda"
            && action("page_link")
            && metadata["has_command"] != true
        {
            if let Some(page) = args["page"].as_str() {
                return self.navigate(page);
            }
        }
        if action("enter") || action("leave") {
            let widget = args
                .get("w")
                .or_else(|| args.get("m"))
                .and_then(Value::as_str)
                .or_else(|| metadata["defaults"][0].as_str())
                .filter(|w| w.starts_with('.'));
            if let Some(widget) = widget {
                let mut command = vec![widget.to_owned(), "configure".into()];
                for pair in metadata["appearance"].as_array().into_iter().flatten() {
                    command.push(pair[0].as_str().ok_or("Missing appearance option")?.into());
                    command.push(pair[1].as_str().ok_or("Missing appearance color")?.into());
                }
                if command.len() > 2 {
                    self.ui.tk.call(&command)?;
                    return Ok(());
                }
            }
        }
        if name == "Launcher/wheel" || name == "Launcher/combobox_wheel" {
            return self.wheel(&event.arguments);
        }
        if name == "Launcher/wrap_within/fit" {
            let label = args["label"].as_str().ok_or("Missing wrap target")?;
            let width = Self::number(&event.arguments, 7);
            let requested = self
                .ui
                .tk
                .call(&["winfo", "reqwidth", label])?
                .parse::<i32>()
                .unwrap_or(0);
            if width != requested && width > 0 {
                let dpi = self.ui.manifest["dpi"].as_f64().unwrap_or(1.0);
                let max = (args["wrap"].as_f64().unwrap_or(640.0) * dpi) as i32;
                let width = (width - (8.0 * dpi) as i32)
                    .min(max)
                    .max((120.0 * dpi) as i32);
                self.ui
                    .tk
                    .call(&[label, "configure", "-wraplength", &width.to_string()])?;
            }
            return Ok(());
        }
        if name == "Launcher/scrolled_page/lambda" {
            let canvas = args["canvas"].as_str().ok_or("Missing scroll canvas")?;
            if let Some(id) = args["window"]
                .as_i64()
                .and_then(|id| self.ui.canvas_ids.get(&(canvas.into(), id)))
            {
                let width = Self::number(&event.arguments, 7);
                if width > 0 {
                    self.ui.tk.call(&[
                        canvas,
                        "itemconfigure",
                        id,
                        "-width",
                        &width.to_string(),
                    ])?;
                }
            } else {
                let bounds = self.ui.tk.call(&[canvas, "bbox", "all"])?;
                self.ui
                    .tk
                    .call(&[canvas, "configure", "-scrollregion", &bounds])?;
            }
            return Ok(());
        }
        if name == "Launcher/choice/show" {
            return self.sync_choices();
        }
        if name == "Launcher/choice/lambda" && action("select_choice") {
            let widget = args["box"].as_str().ok_or("Missing choice widget")?;
            let index = self
                .ui
                .tk
                .call(&[widget, "current"])?
                .parse::<usize>()
                .map_err(|e| e.to_string())?;
            let value = args["values"][index].as_str().ok_or("Invalid selection")?;
            self.ui.tk.call(&[
                "set",
                args["var"].as_str().ok_or("Missing choice variable")?,
                value,
            ])?;
            return Ok(());
        }
        if name == "Launcher/folder/browse" {
            let variable = args["var"].as_str().ok_or("Missing folder variable")?;
            let initial = self.ui.tk.call(&["set", variable])?;
            let path = self.ui.tk.call(&[
                "tk_chooseDirectory",
                "-title",
                args["prompt"].as_str().unwrap_or("Bloodborne"),
                "-initialdir",
                if initial.is_empty() {
                    self.paths.install.to_str().unwrap_or("")
                } else {
                    &initial
                },
            ])?;
            if !path.is_empty() {
                self.ui.tk.call(&["set", variable, &path])?;
            }
            return Ok(());
        }
        if action("open_folder") {
            let path = self
                .ui
                .tk
                .call(&["set", args["var"].as_str().ok_or("Missing path variable")?])?;
            return crate::platform::open_path(std::path::Path::new(&path));
        }
        if action("open_url") {
            if let Some(url) = args["url"]
                .as_str()
                .or_else(|| metadata["defaults"][0].as_str())
            {
                return crate::platform::open_url(url);
            }
        }
        if name == "Launcher/collect" {
            self.settings.save(&self.paths.settings())?;
            return self.ini.save(&self.paths.ini());
        }
        // This is a migration preview, never a silent success for an unported action.
        if self.pending.insert(name.into()) {
            eprintln!("Rust migration pending handler: {name}");
        }
        Ok(())
    }
    pub fn drain(&mut self) -> Result<()> {
        loop {
            let event = self.ui.events.borrow_mut().pop_front();
            let Some(event) = event else {
                break;
            };
            self.dispatch(event)?;
        }
        Ok(())
    }
    /// Development QA drives native Tk callbacks, not mirrored mock handlers.
    pub fn check_interactions(&mut self) -> Result<Value> {
        self.navigate("help")?;
        self.ui.tk.call(&["update"])?;
        self.drain()?;
        let (callback, args) = self.ui.manifest["callbacks"]
            .as_object()
            .unwrap()
            .iter()
            .find(|(_, v)| v["name"] == "HelpMixin/topic_card/toggle")
            .map(|(k, v)| (k.clone(), v["arguments"].clone()))
            .ok_or("No help toggle")?;
        let body = args["body"].as_str().unwrap();
        let arrow = args["arrow"].as_str().unwrap();
        for (expected, marker) in [("pack", "⌄"), ("", "›")] {
            self.ui.tk.call(&[&callback])?;
            self.drain()?;
            self.ui.tk.call(&["update"])?;
            self.drain()?;
            if self.ui.tk.call(&["winfo", "manager", body])? != expected
                || self.ui.tk.call(&[arrow, "cget", "-text"])? != marker
            {
                return Err("Help toggle did not update native widgets".into());
            }
        }
        let (callback, widget) = self.ui.manifest["callbacks"]
            .as_object()
            .unwrap()
            .iter()
            .find(|(_, v)| {
                v["name"] == "Launcher/build_controls/lambda"
                    && v["actions"]
                        .as_array()
                        .is_some_and(|items| items.iter().any(|v| v == "control_hover"))
            })
            .map(|(k, v)| (k.clone(), v["defaults"][0].as_str().unwrap().to_owned()))
            .ok_or("No control hover")?;
        self.ui.tk.call(&[&callback])?;
        self.drain()?;
        if self.ui.tk.call(&[widget.as_str(), "cget", "-background"])? != "#25272b" {
            return Err("Control hover did not update its native widget".into());
        }
        let (callback, args) = self.ui.manifest["callbacks"]
            .as_object()
            .unwrap()
            .iter()
            .find(|(_, v)| {
                v["name"] == "Launcher/choice/lambda"
                    && v["arguments"]["values"]
                        .as_array()
                        .is_some_and(|v| v.len() > 1)
                    && v["actions"]
                        .as_array()
                        .is_some_and(|items| items.iter().any(|v| v == "select_choice"))
            })
            .map(|(k, v)| (k.clone(), v["arguments"].clone()))
            .ok_or("No combobox handler")?;
        let selected = args["values"][1].as_str().ok_or("Missing choice")?;
        self.ui
            .tk
            .call(&[args["box"].as_str().unwrap(), "current", "1"])?;
        self.ui.tk.call(&[&callback])?;
        self.drain()?;
        if self.ui.tk.call(&["set", args["var"].as_str().unwrap()])? != selected {
            return Err("Combobox did not update its setting".into());
        }
        Ok(
            serde_json::json!({"help_expand_collapse":true,"control_hover":true,"combobox_setting":true}),
        )
    }
    pub fn run(&mut self) -> Result<()> {
        while self.ui.tk.has_windows() {
            self.drain()?;
            if self.ui.tk.has_windows() {
                self.ui.tk.step();
            }
        }
        self.settings.save(&self.paths.settings())?;
        self.ini.save(&self.paths.ini())?;
        Ok(())
    }
}

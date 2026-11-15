//! Compatible keyboard/mouse/controller input.ini model. Unknown lines survive edits.
pub mod capture;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::OnceLock,
};
type Result<T> = std::result::Result<T, String>;
fn spec() -> &'static Value {
    static SPEC: OnceLock<Value> = OnceLock::new();
    SPEC.get_or_init(|| {
        serde_json::from_str(include_str!("../resources/input-spec.json"))
            .expect("Embedded input specification")
    })
}
fn names(key: &str) -> Vec<&'static str> {
    spec()[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect()
}
fn has(key: &str, name: &str) -> bool {
    spec()[key]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_str() == Some(name))
}
pub fn parts(value: &str) -> Vec<&str> {
    value.split(',').map(str::trim).collect()
}
fn formatted(value: &str) -> String {
    parts(value).join(", ")
}
pub fn bindable(output: &str, value: &str) -> bool {
    let parts = parts(value);
    !parts.is_empty()
        && parts.len() <= 3
        && parts.iter().copied().collect::<BTreeSet<_>>().len() == parts.len()
        && parts.iter().all(|p| {
            has("KEY_NAMES", p) || (!has("STICK_OUTPUTS", output) && has("MOUSE_BUTTON_NAMES", p))
        })
}
fn compact(raw: &str) -> String {
    raw.chars().filter(|c| !c.is_whitespace()).collect()
}
fn assignment(body: &str) -> Option<(&str, &str)> {
    let (a, b) = body.split_once('=')?;
    Some((a.split(':').next()?, b.split(':').next()?))
}
fn controller_line(raw: &str) -> Option<(String, String)> {
    let line = compact(raw);
    let body = line.split('#').next().unwrap_or("");
    let (a, b) = assignment(body)?;
    if !has("CONTROLLER_OUTPUTS", a)
        || (!has("CONTROLLER_NAMES", b) && spec()["CONTROLLER_ALIASES"].get(b).is_none())
    {
        return None;
    }
    Some((
        a.into(),
        spec()["CONTROLLER_ALIASES"][b].as_str().unwrap_or(b).into(),
    ))
}
#[derive(Clone, Debug)]
pub enum Edit {
    First(Option<String>),
    All(Vec<String>),
}
#[derive(Debug)]
pub struct Input {
    pub lines: Vec<String>,
    pub bindings: BTreeMap<String, Vec<String>>,
    pub controller: BTreeMap<String, Option<String>>,
    pub mouse: Value,
    pub toggle_key: String,
    pub reload_key: String,
}
impl Input {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => Ok(Self::parse(&s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(Self::parse(spec()["DEFAULT_INPUT_INI"].as_str().unwrap()))
            }
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn parse(text: &str) -> Self {
        let mut result = Self {
            lines: text
                .trim_start_matches('\u{feff}')
                .lines()
                .map(str::to_owned)
                .collect(),
            bindings: names("KEYBOARD_OUTPUTS")
                .into_iter()
                .map(|n| (n.into(), Vec::new()))
                .collect(),
            controller: names("CONTROLLER_OUTPUTS")
                .into_iter()
                .map(|n| (n.into(), None))
                .collect(),
            mouse: spec()["MOUSE_SETTINGS_DEFAULTS"].clone(),
            toggle_key: "f7".into(),
            reload_key: "f8".into(),
        };
        let mut seen = BTreeSet::new();
        for raw in &result.lines {
            let line = compact(raw);
            let (body, comment) = line.split_once('#').unwrap_or((&line, ""));
            if let Some((a, b)) = assignment(comment) {
                if a == "mouse_to_joystick" && ["left", "right"].contains(&b) {
                    result.mouse["stick"] = b.into();
                }
            }
            if let Some((a, b)) = assignment(body) {
                if a == "hotkey_toggle_mouse_to_joystick" && has("KEY_NAMES", b) {
                    result.toggle_key = b.into();
                } else if a == "hotkey_reload_inputs" && has("KEY_NAMES", b) {
                    result.reload_key = b.into();
                } else if !mouse_line(a, b, &mut result.mouse)
                    && result.bindings.contains_key(a)
                    && bindable(a, b)
                {
                    result.bindings.get_mut(a).unwrap().push(formatted(b));
                }
            }
            if let Some((a, b)) = controller_line(raw) {
                if seen.insert(a.clone()) {
                    result.controller.insert(a, Some(b));
                }
            }
        }
        result
    }
    pub fn render_layout(&self) -> String {
        let layout = spec()["DS3_LAYOUT"].as_str().unwrap();
        let header = layout.lines().next().unwrap_or("");
        let mut out = Vec::new();
        for raw in &self.lines {
            if raw.trim() == header {
                continue;
            }
            let line = compact(raw);
            let commented = line.starts_with('#');
            let body = line.trim_start_matches('#').split('#').next().unwrap_or("");
            if let Some((a, b)) = assignment(body) {
                if [
                    "leftjoystick_halfmode",
                    "rightjoystick_halfmode",
                    "mouse_to_joystick",
                    "mouse_camera",
                ]
                .contains(&a)
                {
                    continue;
                }
                if !commented && has("KEYBOARD_OUTPUTS", a) && bindable(a, b) {
                    continue;
                }
            }
            out.push(raw.clone());
        }
        while out.last().is_some_and(|s| s.trim().is_empty()) {
            out.pop();
        }
        out.push(String::new());
        out.extend(layout.lines().map(str::to_owned));
        out.join("\n") + "\n"
    }
    pub fn layout_name(&self) -> &'static str {
        let normalize = |mut b: BTreeMap<String, Vec<String>>| {
            for values in b.values_mut() {
                values.sort();
            }
            b
        };
        let current = normalize(self.bindings.clone());
        let default = Self::parse(spec()["DEFAULT_INPUT_INI"].as_str().unwrap());
        if current == normalize(Self::parse(&default.render_layout()).bindings) {
            "ds3"
        } else if current == normalize(default.bindings) {
            "default"
        } else {
            "own"
        }
    }
    pub fn render_mouse(&self, settings: &Value) -> Result<String> {
        const SIMPLE: [(&str, &str); 5] = [
            ("invert_x", "mouse_invert_x"),
            ("invert_y", "mouse_invert_y"),
            ("sensitivity_y", "mouse_sensitivity_y"),
            ("camera", "mouse_camera"),
            ("camera_sensitivity", "mouse_camera_sensitivity"),
        ];
        let stick = settings["stick"]
            .as_str()
            .filter(|v| ["left", "right"].contains(v))
            .ok_or("Invalid mouse stick")?;
        let enabled = settings["enabled"]
            .as_bool()
            .ok_or("Invalid mouse enabled flag")?;
        let toggle = if enabled {
            format!("mouse_to_joystick = {stick}")
        } else {
            format!("# mouse_to_joystick = {stick}")
        };
        let params = format!(
            "mouse_movement_params = {}, {}, {}",
            general(
                settings["deadzone_offset"]
                    .as_f64()
                    .ok_or("Invalid mouse deadzone")?
            ),
            general(settings["speed"].as_f64().ok_or("Invalid mouse speed")?),
            general(
                settings["speed_offset"]
                    .as_f64()
                    .ok_or("Invalid mouse offset")?
            )
        );
        let simple_line = |key: &str, name: &str| -> Result<String> {
            let value = &settings[key];
            let text = if let Some(b) = value.as_bool() {
                if b {
                    "on".into()
                } else {
                    "off".into()
                }
            } else if let Some(s) = value.as_str() {
                s.into()
            } else {
                general(value.as_f64().ok_or("Invalid mouse sensitivity")?)
            };
            Ok(format!("{name} = {text}"))
        };
        let mut seen = BTreeSet::new();
        let mut written_toggle = false;
        let mut written_params = false;
        let mut out = Vec::new();
        for raw in &self.lines {
            let line = compact(raw);
            let (body, comment) = line.split_once('#').unwrap_or((&line, ""));
            if [body, comment]
                .iter()
                .any(|s| assignment(s).is_some_and(|(a, _)| a == "mouse_to_joystick"))
            {
                if !written_toggle {
                    out.push(toggle.clone());
                    written_toggle = true;
                }
                continue;
            }
            if let Some((key, name)) = SIMPLE
                .iter()
                .find(|(_, name)| assignment(body).is_some_and(|(a, _)| a == *name))
            {
                if seen.insert(*key) {
                    out.push(simple_line(key, name)?);
                }
                continue;
            }
            if !written_params
                && assignment(body).is_some_and(|(a, _)| a == "mouse_movement_params")
            {
                out.push(params.clone());
                written_params = true;
                continue;
            }
            out.push(raw.clone());
        }
        if !written_toggle {
            out.push(toggle);
        }
        if !written_params {
            out.push(params);
        }
        for (key, name) in SIMPLE {
            if !seen.contains(key) && settings[key] != spec()["MOUSE_SETTINGS_DEFAULTS"][key] {
                out.push(simple_line(key, name)?);
            }
        }
        Ok(out.join("\n") + "\n")
    }
    pub fn render_keyboard(&self, edits: &BTreeMap<String, Edit>) -> Result<String> {
        for (output, edit) in edits {
            if !has("KEYBOARD_OUTPUTS", output) {
                return Err(format!("Unknown keyboard output: {output}"));
            }
            let values = match edit {
                Edit::First(v) => v.iter().collect::<Vec<_>>(),
                Edit::All(v) => v.iter().collect(),
            };
            if values.iter().any(|v| !bindable(output, v)) {
                return Err(format!("Invalid binding for {output}"));
            }
        }
        let mut written = BTreeSet::new();
        let mut out = Vec::new();
        for raw in &self.lines {
            let line = compact(raw);
            let hit = assignment(line.split('#').next().unwrap_or(""));
            if let Some((a, b)) = hit {
                if let Some(edit) = edits.get(a) {
                    if bindable(a, b) {
                        if matches!(edit, Edit::All(_)) || !written.contains(a) {
                            if written.insert(a.to_owned()) {
                                match edit {
                                    Edit::First(Some(v)) => {
                                        out.push(format!("{a} = {}", formatted(v)))
                                    }
                                    Edit::First(None) => {}
                                    Edit::All(v) => out.extend(
                                        v.iter().map(|v| format!("{a} = {}", formatted(v))),
                                    ),
                                }
                            }
                            continue;
                        }
                    }
                }
            }
            out.push(raw.clone());
        }
        for output in names("KEYBOARD_OUTPUTS") {
            if !written.contains(output) {
                if let Some(edit) = edits.get(output) {
                    match edit {
                        Edit::First(Some(v)) => out.push(format!("{output} = {}", formatted(v))),
                        Edit::First(None) => {}
                        Edit::All(v) => {
                            out.extend(v.iter().map(|v| format!("{output} = {}", formatted(v))))
                        }
                    }
                }
            }
        }
        Ok(out.join("\n") + "\n")
    }
    pub fn save_keyboard(&self, path: &Path, edits: &BTreeMap<String, Edit>) -> Result<()> {
        crate::config::atomic_write(path, self.render_keyboard(edits)?.as_bytes())
            .map_err(|e| e.to_string())
    }
    pub fn render_controller(&self, edits: &BTreeMap<String, Option<String>>) -> Result<String> {
        for (output, name) in edits {
            if !has("CONTROLLER_OUTPUTS", output)
                || name.as_ref().is_some_and(|n| !has("CONTROLLER_NAMES", n))
            {
                return Err("Invalid controller binding".into());
            }
        }
        let mut written = BTreeSet::new();
        let mut out = Vec::new();
        let mut last = None;
        for raw in &self.lines {
            let hit = controller_line(raw);
            if let Some((a, _)) = &hit {
                if let Some(value) = edits.get(a) {
                    if written.insert(a.clone()) {
                        if let Some(value) = value {
                            out.push(format!("{a} = {value}"));
                            last = Some(out.len());
                        }
                        continue;
                    }
                }
            }
            out.push(raw.clone());
            if hit.is_some() {
                last = Some(out.len());
            }
        }
        let new: Vec<_> = names("CONTROLLER_OUTPUTS")
            .into_iter()
            .filter(|n| !written.contains(*n))
            .filter_map(|n| {
                edits
                    .get(n)
                    .and_then(Option::as_ref)
                    .map(|v| format!("{n} = {v}"))
            })
            .collect();
        let at = last.unwrap_or(out.len());
        out.splice(at..at, new);
        Ok(out.join("\n") + "\n")
    }
}
/// Python's .3g spelling, including its exponent threshold and trailing zeros.
fn general(value: f64) -> String {
    let scientific = format!("{value:.2e}");
    let Some((mantissa, power)) = scientific.split_once('e') else {
        return scientific;
    };
    let exponent = power.parse::<i32>().unwrap_or(0);
    fn trim(s: &str) -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').into()
        } else {
            s.into()
        }
    }
    if !(-4..3).contains(&exponent) {
        format!("{}e{exponent:+03}", trim(mantissa))
    } else {
        let precision = (2 - exponent).max(0) as usize;
        trim(&format!("{value:.precision$}"))
    }
}
pub fn keysym(key: &str) -> Option<String> {
    let name = spec()["TK_KEYSYM_TO_NAME"][key]
        .as_str()
        .or_else(|| spec()["TK_SHIFTED_KEYSYM_TO_NAME"][key].as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| key.to_lowercase());
    if has("KEY_NAMES", &name) {
        Some(name)
    } else {
        None
    }
}
pub fn button(number: u32) -> Option<&'static str> {
    spec()["TK_BUTTON_NUM_TO_NAME"][number.to_string()].as_str()
}
pub fn display(part: &str, russian: bool) -> String {
    let value = &spec()["DISPLAY_NAMES"][part];
    if let Some(s) = value.as_str() {
        s.into()
    } else if let Some(v) = value.as_array() {
        v[usize::from(russian)].as_str().unwrap_or(part).into()
    } else {
        part.to_uppercase()
    }
}
pub fn reserved(key: &str, toggle: &str, reload: &str) -> bool {
    key == toggle || key == reload || has("ALWAYS_RESERVED", key)
}
fn mouse_line(output: &str, value: &str, settings: &mut Value) -> bool {
    match output {
        "mouse_to_joystick" if ["left", "right"].contains(&value) => {
            settings["enabled"] = true.into();
            settings["stick"] = value.into();
            true
        }
        "mouse_invert_x" | "mouse_invert_y" if ["on", "off"].contains(&value) => {
            settings[if output == "mouse_invert_x" {
                "invert_x"
            } else {
                "invert_y"
            }] = (value == "on").into();
            true
        }
        "mouse_camera" if ["direct", "stick"].contains(&value) => {
            settings["camera"] = value.into();
            true
        }
        "mouse_sensitivity_y" | "mouse_camera_sensitivity" => {
            let Ok(v) = value.parse::<f64>() else {
                return false;
            };
            let (key, lo, hi) = if output == "mouse_sensitivity_y" {
                ("sensitivity_y", 0.1, 10.0)
            } else {
                ("camera_sensitivity", 0.05, 20.0)
            };
            if !(lo..=hi).contains(&v) {
                return false;
            }
            settings[key] = json!(v);
            true
        }
        "mouse_movement_params" => {
            let v: std::result::Result<Vec<f64>, _> = value.split(',').map(str::parse).collect();
            let Ok(v) = v else {
                return false;
            };
            if v.len() != 3
                || !(0.0..=1.0).contains(&v[0])
                || !(0.01..=20.0).contains(&v[1])
                || !(0.0..=1.0).contains(&v[2])
            {
                return false;
            }
            for (k, v) in ["deadzone_offset", "speed", "speed_offset"].iter().zip(v) {
                settings[*k] = json!(v);
            }
            true
        }
        _ => false,
    }
}
pub fn conflicts(
    bindings: &BTreeMap<String, Vec<Option<String>>>,
) -> BTreeMap<String, BTreeMap<usize, BTreeSet<String>>> {
    let items: Vec<_> = bindings
        .iter()
        .flat_map(|(o, v)| {
            v.iter().enumerate().filter_map(move |(i, v)| {
                v.as_ref()
                    .filter(|v| !v.is_empty())
                    .map(|v| (o, i, parts(v)))
            })
        })
        .collect();
    let mut result = BTreeMap::<String, BTreeMap<usize, BTreeSet<String>>>::new();
    for (n, (a, ia, pa)) in items.iter().enumerate() {
        for (b, ib, pb) in items.iter().skip(n + 1) {
            let same = pa.iter().copied().collect::<BTreeSet<_>>() == pb.iter().copied().collect();
            let inside = (pa.len() == 1 && has("MODIFIER_NAMES", pa[0]) && pb.contains(&pa[0]))
                || (pb.len() == 1 && has("MODIFIER_NAMES", pb[0]) && pa.contains(&pb[0]));
            if a != b && (same || inside) {
                result
                    .entry((*a).clone())
                    .or_default()
                    .entry(*ia)
                    .or_default()
                    .insert((*b).clone());
                result
                    .entry((*b).clone())
                    .or_default()
                    .entry(*ib)
                    .or_default()
                    .insert((*a).clone());
            }
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mouse_numbers_layout_and_shifted_keys_match_the_runtime() {
        assert_eq!(general(1000.0), "1e+03");
        assert_eq!(general(0.000012345), "1.23e-05");
        assert_eq!(general(100.0), "100");
        assert_eq!(general(1.0), "1");
        assert_eq!(keysym("exclam").as_deref(), Some("1"));
        assert_eq!(keysym("Shift_L").as_deref(), Some("lshift"));
        assert_eq!(button(8), Some("sidebuttonback"));
        assert!(reserved("insert", "f7", "f8"));
        let model = Input::parse(spec()["DEFAULT_INPUT_INI"].as_str().unwrap());
        assert_eq!(model.layout_name(), "default");
        assert_eq!(Input::parse(&model.render_layout()).layout_name(), "ds3");
        assert!(model.render_layout().contains("cross = cross"));
    }
    #[test]
    fn keyboard_edits_preserve_controller_comments_and_other_slots() {
        let input=Input::parse("# custom\ncross = cross\ncross = e # first\ncross = q\nfuture = anything\n# mouse_to_joystick = right\n");
        assert_eq!(input.bindings["cross"], ["e", "q"]);
        assert_eq!(input.controller["cross"].as_deref(), Some("cross"));
        assert_eq!(input.mouse["enabled"], false);
        let edit = BTreeMap::from([("cross".into(), Edit::All(vec!["lctrl, leftbutton".into()]))]);
        let out = input.render_keyboard(&edit).unwrap();
        assert!(out.contains("cross = cross\n"));
        assert!(out.contains("future = anything\n"));
        assert!(out.contains("cross = lctrl, leftbutton\n"));
        assert!(!out.contains("cross = q"));
        let first = BTreeMap::from([("cross".into(), Edit::First(None))]);
        assert!(input
            .render_keyboard(&first)
            .unwrap()
            .contains("cross = q\n"));
        assert!(!bindable("axis_left_x_plus", "leftbutton"));
    }
    #[test]
    fn controller_edits_keep_the_second_binding_and_keyboard_line() {
        let input = Input::parse("cross = cross\ncross = triangle\ncross = e\n# end\n");
        let edits = BTreeMap::from([
            ("cross".into(), Some("square".into())),
            ("options".into(), Some("options".into())),
        ]);
        assert_eq!(
            input.render_controller(&edits).unwrap(),
            "cross = square\ncross = triangle\noptions = options\ncross = e\n# end\n"
        );
    }
    #[test]
    fn modifier_alone_conflicts_but_plain_combo_members_do_not() {
        let b = BTreeMap::from([
            ("circle".into(), vec![Some("lshift".into())]),
            ("r1".into(), vec![Some("leftbutton".into())]),
            ("r2".into(), vec![Some("lshift, leftbutton".into())]),
        ]);
        let c = conflicts(&b);
        assert!(c["circle"][&0].contains("r2"));
        assert!(!c.contains_key("r1"));
    }
}

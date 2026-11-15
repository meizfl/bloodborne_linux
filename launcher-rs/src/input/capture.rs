//! One-shot input capture; UI and Windows polling feed the same state machine.
use super::*;
#[derive(Debug)]
pub enum Event {
    Press(String),
    Release(String),
    Button(u32),
    Wheel(i32, bool),
}
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Waiting,
    Modifiers(Vec<String>),
    Accepted(String),
    Cancelled,
    Unsupported,
    Reserved(String),
    KeyboardOnly,
}
pub struct Capture {
    axis: bool,
    modifiers: Vec<String>,
    toggle: String,
    reload: String,
    finished: bool,
}
impl Capture {
    pub fn new(output: &str, toggle: &str, reload: &str) -> Self {
        Self {
            axis: has("STICK_OUTPUTS", output),
            modifiers: Vec::new(),
            toggle: toggle.into(),
            reload: reload.into(),
            finished: false,
        }
    }
    fn apply(&mut self, name: &str) -> Outcome {
        let mut parts = self.modifiers.clone();
        if !parts.iter().any(|m| m == name) {
            parts.push(name.into());
        }
        let parts = &parts[parts.len().saturating_sub(3)..];
        self.finished = true;
        if let Some(key) = parts
            .iter()
            .find(|p| reserved(p, &self.toggle, &self.reload))
        {
            Outcome::Reserved(key.clone())
        } else {
            Outcome::Accepted(parts.join(", "))
        }
    }
    pub fn event(&mut self, event: Event) -> Outcome {
        if self.finished {
            return Outcome::Waiting;
        }
        match event {
            Event::Press(key) if key == "Escape" => {
                self.finished = true;
                Outcome::Cancelled
            }
            Event::Press(key) => {
                let Some(name) = keysym(&key) else {
                    return Outcome::Unsupported;
                };
                if has("MODIFIER_NAMES", &name) {
                    if !self.modifiers.contains(&name) && self.modifiers.len() < 2 {
                        self.modifiers.push(name);
                        return Outcome::Modifiers(self.modifiers.clone());
                    }
                    Outcome::Waiting
                } else {
                    self.apply(&name)
                }
            }
            Event::Release(key) => {
                if keysym(&key)
                    .as_ref()
                    .is_some_and(|key| self.modifiers.contains(key))
                {
                    let name = self.modifiers.last().unwrap().clone();
                    self.apply(&name)
                } else {
                    Outcome::Waiting
                }
            }
            Event::Button(number) => {
                let Some(name) = button(number) else {
                    return Outcome::Waiting;
                };
                if self.axis {
                    Outcome::KeyboardOnly
                } else {
                    self.apply(name)
                }
            }
            Event::Wheel(delta, horizontal) => {
                if delta == 0 {
                    return Outcome::Waiting;
                }
                if self.axis {
                    return Outcome::KeyboardOnly;
                }
                let name = if horizontal {
                    if delta > 0 {
                        "mousewheelright"
                    } else {
                        "mousewheelleft"
                    }
                } else if delta > 0 {
                    "mousewheelup"
                } else {
                    "mousewheeldown"
                };
                self.apply(name)
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modifiers_and_mouse_make_one_combination() {
        let mut capture = Capture::new("r2", "f7", "f8");
        assert_eq!(
            capture.event(Event::Press("Shift_L".into())),
            Outcome::Modifiers(vec!["lshift".into()])
        );
        assert_eq!(
            capture.event(Event::Button(1)),
            Outcome::Accepted("lshift, leftbutton".into())
        );
        assert_eq!(capture.event(Event::Button(2)), Outcome::Waiting);
    }
    #[test]
    fn invalid_axis_buttons_do_not_end_capture_but_reserved_keys_do() {
        let mut capture = Capture::new("axis_left_x_plus", "f7", "f8");
        assert_eq!(capture.event(Event::Button(1)), Outcome::KeyboardOnly);
        assert_eq!(
            capture.event(Event::Press("XF86AudioPlay".into())),
            Outcome::Unsupported
        );
        assert_eq!(
            capture.event(Event::Press("Insert".into())),
            Outcome::Reserved("insert".into())
        );
        assert_eq!(capture.event(Event::Press("d".into())), Outcome::Waiting);
    }
    #[test]
    fn modifier_release_wheel_and_escape() {
        let mut capture = Capture::new("circle", "f7", "f8");
        capture.event(Event::Press("Control_L".into()));
        assert_eq!(
            capture.event(Event::Release("Control_L".into())),
            Outcome::Accepted("lctrl".into())
        );
        let mut capture = Capture::new("r3", "f7", "f8");
        assert_eq!(
            capture.event(Event::Wheel(-120, true)),
            Outcome::Accepted("mousewheelleft".into())
        );
        let mut capture = Capture::new("cross", "f7", "f8");
        assert_eq!(
            capture.event(Event::Press("Escape".into())),
            Outcome::Cancelled
        );
    }
}

//! The viewers' mouse bindings as the person set them (Settings, Mouse and trackpad; the table is host/src/mouse.rs, shared by path). The hub's
//! data carries the three settings; `apply` keeps them here, so a viewer page, which holds no hub data, asks `plot()` or `solid()` on each event.
//! This is the one place a viewer learns what an input means: no viewer names a mouse button of its own (tests below grep for it).
use crate::mouse::{Button, Kind, Mods, Scheme};
use gpui::{Modifiers, MouseButton};
use std::sync::Mutex;

/// The three buttons a viewer listens to; the table decides what each does.
pub const BUTTONS: [MouseButton; 3] = [MouseButton::Left, MouseButton::Middle, MouseButton::Right];
static SET: Mutex<(String, String, bool)> = Mutex::new((String::new(), String::new(), false));

/// Take the hub's `mouse` object ({"2d", "3d", "invert"}); nothing there leaves what was.
pub fn apply(m: &serde_json::Value) {
    if !m.is_object() { return }
    if let Ok(mut s) = SET.lock() { *s = (m["2d"].as_str().unwrap_or("").to_string(), m["3d"].as_str().unwrap_or("").to_string(), crate::mouse::inverted(m["invert"].as_str().unwrap_or(""))); }
}
pub fn scheme(kind: Kind) -> Scheme { let t = SET.lock().map(|s| if kind == Kind::Plot { s.0.clone() } else { s.1.clone() }).unwrap_or_default(); Scheme::parse(kind, &t) }
pub fn plot() -> Scheme { scheme(Kind::Plot) }
pub fn solid() -> Scheme { scheme(Kind::Solid) }
/// The person's zoom direction.
pub fn invert() -> bool { SET.lock().map_or(false, |s| s.2) }
/// The table's button for gpui's (a navigation button is none).
pub fn button(b: MouseButton) -> Option<Button> { match b { MouseButton::Left => Some(Button::Left), MouseButton::Middle => Some(Button::Middle), MouseButton::Right => Some(Button::Right), _ => None } }
/// The held keys; Cmd counts as Ctrl on a Mac, where Ctrl alone is the right button's.
pub fn mods(m: &Modifiers) -> Mods { Mods { shift: m.shift, ctrl: m.control || (cfg!(target_os = "macos") && m.platform), alt: m.alt } }
/// A scroll event as the table takes it: lines or pixels, and the delta.
pub fn delta(d: &gpui::ScrollDelta) -> (bool, f32, f32) { match d { gpui::ScrollDelta::Lines(l) => (true, l.x, l.y), gpui::ScrollDelta::Pixels(p) => (false, f32::from(p.x), f32::from(p.y)) } }
/// What "the person moved the view" must do, for every viewer: the single hook (a follow mode pauses on it, issue 1229).
pub fn drag_zoom_factor(dy: f32, invert: bool) -> f32 { (dy * if invert { 0.01 } else { -0.01 }).clamp(-1.0, 1.0).exp() }

#[cfg(test)]
mod tests {
    use super::*;
    /// No viewer names a mouse button for itself: what a button does comes from the table. (Left is allowed where it is not a binding: a tool's own button.)
    #[test] fn the_viewers_name_no_middle_or_right_button_and_no_modifier_of_their_own() {
        for f in ["screens/plotview.rs", "viewers/solid.rs"] {
            let src = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(f)).unwrap();
            let body = src.split("#[cfg(test)]").next().unwrap();
            for bad in ["MouseButton::Middle", "MouseButton::Right", "modifiers.control", "modifiers.shift", "modifiers.alt", "e.modifiers.control"] { assert!(!body.contains(bad), "{f} handles {bad} itself: it goes through mousecfg"); }
            assert!(body.contains("mousecfg::"), "{f} reads the table");
        }
    }
    #[test] fn a_hub_object_sets_the_schemes() {
        apply(&serde_json::json!({"2d": "legacy", "3d": "kicad;wheel=pan", "invert": "on"}));
        assert_eq!(plot().base, crate::mouse::Preset::Legacy); assert!(invert());
        assert_eq!(solid().wheels[0], crate::mouse::Action::Pan);
        apply(&serde_json::json!({"2d": "", "3d": "", "invert": ""}));
        assert_eq!(plot().base, crate::mouse::Preset::Kicad); assert!(!invert());
    }
}

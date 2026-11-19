//! The mouse and trackpad bindings of the built-in viewers, as one pure table every client shares: the hub draws the Settings page from it and
//! the desktop client includes this very file (native/gpui/src/main.rs `#[path]`) and asks it what each press, drag, scroll and pinch means.
//! The viewers hold no buttons of their own (the guard test in native/gpui/src/mouse_tests.rs greps for them): "the person panned or
//! zoomed" is decided here, in `Scheme::drag` and `Scheme::scroll`, and nowhere else.
//!
//! Two viewer kinds have a scheme each: the flat plot (a board or schematic, `Kind::Plot`) and the 3D view (a board's 3D model or a STEP part,
//! `Kind::Solid`). A scheme maps every input (a button's drag, the same drag with Shift, Ctrl or Alt held, the wheel, the wheel with a modifier)
//! to one action, so two actions never fight for one input; choosing an action for an input replaces what it did. Select is a click on one button.
//!
//! Stored as a setting, one text a kind (`mouse.2d`, `mouse.3d`): a preset name, then what the person changed, `kicad;middle=zoom;wheel=pan`.
//! An empty or unknown text is the KiCad preset. Presets, and where each value comes from: docs/MOUSE-CONTROLS.md.
//! Std only: this file compiles in both crates.

#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Kind { Plot, Solid }
impl Kind {
    /// The setting's key.
    pub fn key(self) -> &'static str { match self { Kind::Plot => "mouse.2d", Kind::Solid => "mouse.3d" } }
}
pub const INVERT_KEY: &str = "mouse.invert";

#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Action { None, Pan, PanH, PanV, Zoom, Rotate, Roll }
impl Action {
    pub fn name(self) -> &'static str { match self { Action::None => "none", Action::Pan => "pan", Action::PanH => "pan-h", Action::PanV => "pan-v", Action::Zoom => "zoom", Action::Rotate => "rotate", Action::Roll => "roll" } }
    pub fn from_name(s: &str) -> Option<Action> { [Action::None, Action::Pan, Action::PanH, Action::PanV, Action::Zoom, Action::Rotate, Action::Roll].into_iter().find(|a| a.name() == s) }
    /// The words Settings shows for it (catalogued).
    pub fn label(self) -> &'static str { match self { Action::None => "nothing", Action::Pan => "pan", Action::PanH => "pan sideways", Action::PanV => "pan up and down", Action::Zoom => "zoom", Action::Rotate => "rotate", Action::Roll => "roll" } }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Button { Left, Middle, Right }
impl Button { fn name(self) -> &'static str { match self { Button::Left => "left", Button::Middle => "middle", Button::Right => "right" } } }

/// Which of Shift, Ctrl (Cmd on a Mac) and Alt are held.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)] pub struct Mods { pub shift: bool, pub ctrl: bool, pub alt: bool }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] enum M { None, Shift, Ctrl, Alt }
impl Mods {
    pub const NONE: Mods = Mods { shift: false, ctrl: false, alt: false };
    /// The one modifier that counts for a wheel: KiCad's order, Shift before Ctrl before Alt.
    fn wheel(self) -> M { if self.shift { M::Shift } else if self.ctrl { M::Ctrl } else if self.alt { M::Alt } else { M::None } }
    /// The one that counts for a drag: Alt before Shift before Ctrl, as the viewers always had it.
    fn drag(self) -> M { if self.alt { M::Alt } else if self.shift { M::Shift } else if self.ctrl { M::Ctrl } else { M::None } }
}

/// The inputs a person can bind, by their setting names: drags first (`Scheme::drags`), then wheels (`Scheme::wheels`).
pub const DRAGS: [&str; 6] = ["left", "middle", "right", "shift-left", "ctrl-left", "alt-left"];
pub const WHEELS: [&str; 4] = ["wheel", "shift-wheel", "ctrl-wheel", "alt-wheel"];
/// What an input can be set to: a drag, a wheel (the 3D view's rotate and roll only turn the model).
pub fn drag_actions(kind: Kind) -> &'static [Action] { match kind { Kind::Plot => &[Action::None, Action::Pan, Action::Zoom], Kind::Solid => &[Action::None, Action::Pan, Action::Zoom, Action::Rotate, Action::Roll] } }
pub const WHEEL_ACTIONS: [Action; 5] = [Action::None, Action::Zoom, Action::Pan, Action::PanH, Action::PanV];
pub const SELECTS: [&str; 4] = ["left", "middle", "right", "none"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Preset { Kicad, Legacy }
impl Preset {
    pub fn name(self) -> &'static str { match self { Preset::Kicad => "kicad", Preset::Legacy => "legacy" } }
}

/// One viewer kind's bindings.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Scheme {
    pub base: Preset,
    pub drags: [Action; 6],
    pub wheels: [Action; 4],
    /// The button whose click (a press and release that did not move) selects; None: no click selects.
    pub select: Option<Button>,
    /// A trackpad's two-finger scroll (pixel deltas) pans, and with Ctrl it is the pinch and zooms; off, it is the wheel like any other.
    pub scroll_pans: bool,
    /// A trackpad pinch zooms.
    pub pinch: bool,
}

impl Scheme {
    /// A preset for a kind.
    pub fn preset(p: Preset, kind: Kind) -> Scheme {
        use Action::*;
        match (p, kind) {
            // KiCad 10 defaults (common/settings/common_settings.cpp): wheel zooms, Ctrl+wheel pans sideways, Shift+wheel up and down; middle and right drag pan; the left button selects
            (Preset::Kicad, Kind::Plot) => Scheme { base: p, drags: [None, Pan, Pan, None, None, None], wheels: [Zoom, PanV, PanH, None], select: Some(Button::Left), scroll_pans: true, pinch: true },
            // the 3D viewer (common/hidpi_gl_3d_canvas.cpp): the left drag turns the model, middle and right drag pan, the wheel zooms, Ctrl and Shift pan
            (Preset::Kicad, Kind::Solid) => Scheme { base: p, drags: [Rotate, Pan, Pan, None, None, None], wheels: [Zoom, PanV, PanH, None], select: Some(Button::Left), scroll_pans: false, pinch: true },
            // what the viewers did before the presets: the left drag pans a plot and turns a model, the wheel zooms, a two-finger scroll pans a plot
            (Preset::Legacy, Kind::Plot) => Scheme { base: p, drags: [Pan, None, None, None, None, None], wheels: [Zoom, None, Zoom, None], select: Some(Button::Left), scroll_pans: true, pinch: true },
            (Preset::Legacy, Kind::Solid) => Scheme { base: p, drags: [Rotate, Pan, Pan, Pan, Pan, Roll], wheels: [Zoom, None, Zoom, None], select: Some(Button::Left), scroll_pans: false, pinch: true },
        }
    }
    /// What a stored text means (see the module): a preset, then `key=value` changes. Anything unknown is left out.
    pub fn parse(kind: Kind, text: &str) -> Scheme {
        let mut parts = text.split(';').map(str::trim);
        let base = match parts.next() { Some("legacy") => Preset::Legacy, _ => Preset::Kicad };
        let mut s = Scheme::preset(base, kind);
        for kv in parts { if let Some((k, v)) = kv.split_once('=') { s.set(kind, k.trim(), v.trim()); } }
        s
    }
    /// Change one input by its setting name; false when the name or value is not one this kind has.
    pub fn set(&mut self, kind: Kind, key: &str, value: &str) -> bool {
        if let Some(i) = DRAGS.iter().position(|d| *d == key) {
            match Action::from_name(value) { Some(a) if drag_actions(kind).contains(&a) => { self.drags[i] = a; true } _ => false }
        } else if let Some(i) = WHEELS.iter().position(|d| *d == key) {
            match Action::from_name(value) { Some(a) if WHEEL_ACTIONS.contains(&a) => { self.wheels[i] = a; true } _ => false }
        } else if key == "select" {
            match value { "left" => self.select = Some(Button::Left), "middle" => self.select = Some(Button::Middle), "right" => self.select = Some(Button::Right), "none" => self.select = None, _ => return false }
            true
        } else if key == "scroll" { match value { "pan" => self.scroll_pans = true, "wheel" => self.scroll_pans = false, _ => return false } true }
        else if key == "pinch" { match value { "zoom" => self.pinch = true, "off" => self.pinch = false, _ => return false } true }
        else { false }
    }
    /// The stored text: the preset, and every input that differs from it.
    pub fn text(&self, kind: Kind) -> String {
        let base = Scheme::preset(self.base, kind);
        let mut t = self.base.name().to_string();
        for (i, k) in DRAGS.iter().enumerate() { if self.drags[i] != base.drags[i] { t.push_str(&format!(";{k}={}", self.drags[i].name())); } }
        for (i, k) in WHEELS.iter().enumerate() { if self.wheels[i] != base.wheels[i] { t.push_str(&format!(";{k}={}", self.wheels[i].name())); } }
        if self.select != base.select { t.push_str(&format!(";select={}", self.select.map_or("none", Button::name))); }
        if self.scroll_pans != base.scroll_pans { t.push_str(if self.scroll_pans { ";scroll=pan" } else { ";scroll=wheel" }); }
        if self.pinch != base.pinch { t.push_str(if self.pinch { ";pinch=zoom" } else { ";pinch=off" }); }
        t
    }
    /// Whether anything differs from its preset.
    pub fn custom(&self, kind: Kind) -> bool { *self != Scheme::preset(self.base, kind) }
    /// The setting text after one input is changed: what Settings' buttons press. The input that had the action loses nothing else: one input, one action.
    pub fn edited(kind: Kind, text: &str, key: &str, value: &str) -> String { let mut s = Scheme::parse(kind, text); s.set(kind, key, value); s.text(kind) }
    /// The actions nothing reaches, among those a viewer cannot do without (Settings says so beside the table).
    pub fn unreachable(&self, kind: Kind) -> Vec<Action> {
        let need: &[Action] = match kind { Kind::Plot => &[Action::Pan, Action::Zoom], Kind::Solid => &[Action::Pan, Action::Zoom, Action::Rotate] };
        need.iter().copied().filter(|a| {
            let by_pan = |x: Action| matches!(x, Action::Pan | Action::PanH | Action::PanV);
            let on = |x: &Action| if *a == Action::Pan { by_pan(*x) } else { x == a };
            !(self.drags.iter().any(on) || self.wheels.iter().any(on) || (*a == Action::Pan && self.scroll_pans) || (*a == Action::Zoom && self.pinch))
        }).collect()
    }

    /// What a drag of this button with these modifiers held does.
    pub fn drag(&self, b: Button, m: Mods) -> Action {
        let name = |mm: M| match (mm, b) { (M::None, Button::Left) => Some(0), (M::None, Button::Middle) => Some(1), (M::None, Button::Right) => Some(2), (M::Shift, Button::Left) => Some(3), (M::Ctrl, Button::Left) => Some(4), (M::Alt, Button::Left) => Some(5), _ => None };
        name(m.drag()).map_or(Action::None, |i| self.drags[i])
    }
    /// Whether a click of this button, with no modifier held, selects.
    pub fn selects(&self, b: Button, m: Mods) -> bool { self.select == Some(b) && m == Mods::NONE }
    /// What a wheel or two-finger scroll does. `lines` says the delta is in lines (a mouse wheel's notches), else in pixels (a trackpad);
    /// `(x, y)` is the delta as the system gave it, `invert` the person's zoom direction.
    pub fn scroll(&self, m: Mods, lines: bool, x: f32, y: f32, invert: bool) -> Scroll {
        let sign = if invert { -1.0 } else { 1.0 };
        if !lines && self.scroll_pans {
            // a trackpad's two fingers pan; with Ctrl held the system means the pinch (a browser's convention, and the platform's), which zooms
            return if m.ctrl { Scroll::Zoom { lines, y: y * sign, ctrl: true } } else { Scroll::Pan { dx: -x, dy: -y } };
        }
        let k = if lines { PAN_LINE } else { 1.0 };
        match self.wheels[match m.wheel() { M::None => 0, M::Shift => 1, M::Ctrl => 2, M::Alt => 3 }] {
            Action::Zoom => Scroll::Zoom { lines, y: y * sign, ctrl: m.ctrl },
            Action::Pan => Scroll::Pan { dx: -x * k, dy: -y * k },
            Action::PanH => Scroll::Pan { dx: -(if x != 0.0 { x } else { y }) * k, dy: 0.0 },
            Action::PanV => Scroll::Pan { dx: 0.0, dy: -y * k },
            _ => Scroll::None,
        }
    }
    /// A pinch's zoom factor, scaled by `factor` (over 1 closer): 1 when pinch is off.
    pub fn pinch_factor(&self, factor: f32, invert: bool) -> f32 { if !self.pinch { 1.0 } else if invert && factor > 0.0 { 1.0 / factor } else { factor } }
}
/// Pixels a mouse wheel's notch pans by, for a wheel bound to pan.
pub const PAN_LINE: f32 = 40.0;

/// What `Scheme::scroll` decided: nothing, a zoom by the raw delta (the viewer applies its own gain: `lines` says which unit, `ctrl` that Ctrl was held), or a pan by pixels.
#[derive(Clone, Copy, PartialEq, Debug)] pub enum Scroll { None, Zoom { lines: bool, y: f32, ctrl: bool }, Pan { dx: f32, dy: f32 } }

/// What `mouse.invert` stores, as a bool.
pub fn inverted(v: &str) -> bool { v == "on" }

/// The Settings page's rows for one kind, as `(input key, label, current action name, [(action name, value to store)])`: the buttons are
/// every action the input can take, each carrying the whole setting text it would make (so a press is the generic `setting key=value`).
pub fn rows(kind: Kind, text: &str) -> Vec<(&'static str, Action, Vec<(Action, String)>)> {
    let s = Scheme::parse(kind, text);
    let mut v = Vec::new();
    for (i, k) in DRAGS.iter().enumerate() { v.push((*k, s.drags[i], drag_actions(kind).iter().map(|a| (*a, Scheme::edited(kind, text, k, a.name()))).collect())); }
    for (i, k) in WHEELS.iter().enumerate() { v.push((*k, s.wheels[i], WHEEL_ACTIONS.iter().map(|a| (*a, Scheme::edited(kind, text, k, a.name()))).collect())); }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use Action::*;
    const L: Mods = Mods::NONE;
    const SH: Mods = Mods { shift: true, ctrl: false, alt: false };
    const CT: Mods = Mods { shift: false, ctrl: true, alt: false };
    const AL: Mods = Mods { shift: false, ctrl: false, alt: true };

    #[test] fn kicad_plot_is_kicads_own_defaults() {
        let s = Scheme::parse(Kind::Plot, "");
        assert_eq!(s.base, Preset::Kicad);
        assert_eq!((s.drag(Button::Middle, L), s.drag(Button::Right, L), s.drag(Button::Left, L)), (Pan, Pan, None));
        assert!(s.selects(Button::Left, L) && !s.selects(Button::Right, L) && !s.selects(Button::Left, SH));
        assert_eq!(s.scroll(L, true, 0.0, 1.0, false), Scroll::Zoom { lines: true, y: 1.0, ctrl: false });
        assert_eq!(s.scroll(CT, true, 0.0, 1.0, false), Scroll::Pan { dx: -PAN_LINE, dy: 0.0 }, "Ctrl+wheel pans sideways");
        assert_eq!(s.scroll(SH, true, 0.0, 1.0, false), Scroll::Pan { dx: 0.0, dy: -PAN_LINE }, "Shift+wheel pans up and down");
    }
    #[test] fn kicad_solid_turns_with_the_left_drag_and_pans_with_the_others() {
        let s = Scheme::parse(Kind::Solid, "kicad");
        assert_eq!((s.drag(Button::Left, L), s.drag(Button::Middle, L), s.drag(Button::Right, L)), (Rotate, Pan, Pan));
        assert_eq!(s.drag(Button::Left, SH), None); assert_eq!(s.drag(Button::Left, AL), None);
        assert!(matches!(s.scroll(L, true, 0.0, 1.0, false), Scroll::Zoom { .. }));
    }
    #[test] fn legacy_is_what_the_viewers_did_before() {
        let p = Scheme::parse(Kind::Plot, "legacy");
        assert_eq!((p.drag(Button::Left, L), p.drag(Button::Middle, L), p.drag(Button::Right, L)), (Pan, None, None));
        assert!(p.selects(Button::Left, L));
        // a trackpad's pixels pan, a mouse's lines zoom, a pinch (pixels + ctrl) zooms, with or without Ctrl on a mouse
        assert_eq!(p.scroll(L, false, 3.0, 4.0, false), Scroll::Pan { dx: -3.0, dy: -4.0 });
        assert!(matches!(p.scroll(CT, false, 0.0, 4.0, false), Scroll::Zoom { ctrl: true, .. }));
        assert!(matches!(p.scroll(L, true, 0.0, 4.0, false), Scroll::Zoom { lines: true, .. }));
        assert!(matches!(p.scroll(CT, true, 0.0, 4.0, false), Scroll::Zoom { lines: true, ctrl: true, .. }));
        let s = Scheme::parse(Kind::Solid, "legacy");
        assert_eq!((s.drag(Button::Left, L), s.drag(Button::Left, AL), s.drag(Button::Left, SH), s.drag(Button::Left, CT)), (Rotate, Roll, Pan, Pan));
        assert_eq!((s.drag(Button::Right, L), s.drag(Button::Middle, L)), (Pan, Pan));
        assert!(matches!(s.scroll(L, false, 0.0, 4.0, false), Scroll::Zoom { lines: false, .. }), "the 3D view's pixels still zoom");
        assert_eq!(s.scroll(SH, true, 0.0, 1.0, false), Scroll::None);
        // Alt with Shift was a roll, as before
        assert_eq!(s.drag(Button::Left, Mods { shift: true, ctrl: false, alt: true }), Roll);
    }
    #[test] fn a_remap_takes_effect_and_is_stored_as_the_difference() {
        let t = Scheme::edited(Kind::Plot, "kicad", "middle", "zoom");
        assert_eq!(t, "kicad;middle=zoom");
        let s = Scheme::parse(Kind::Plot, &t);
        assert_eq!(s.drag(Button::Middle, L), Zoom); assert_eq!(s.drag(Button::Right, L), Pan);
        assert!(s.custom(Kind::Plot));
        // putting it back is the preset again
        assert_eq!(Scheme::edited(Kind::Plot, &t, "middle", "pan"), "kicad");
        // wheel with a modifier, and the select button
        let s = Scheme::parse(Kind::Plot, &Scheme::edited(Kind::Plot, "legacy", "alt-wheel", "pan-h"));
        assert!(matches!(s.scroll(AL, true, 0.0, 1.0, false), Scroll::Pan { dy, .. } if dy == 0.0));
        let s = Scheme::parse(Kind::Solid, &Scheme::edited(Kind::Solid, "kicad", "select", "right"));
        assert!(s.selects(Button::Right, L) && !s.selects(Button::Left, L));
    }
    #[test] fn one_input_has_one_action_and_nonsense_is_refused() {
        let mut s = Scheme::preset(Preset::Kicad, Kind::Plot);
        assert!(!s.set(Kind::Plot, "left", "rotate"), "a flat plot cannot rotate");
        assert!(!s.set(Kind::Plot, "wheel", "roll") && !s.set(Kind::Plot, "nope", "pan") && !s.set(Kind::Plot, "left", "nope"));
        assert!(Scheme::preset(Preset::Kicad, Kind::Solid).set(Kind::Solid, "left", "rotate"));
        // setting an input replaces its action: nothing else moves
        let t = Scheme::edited(Kind::Plot, "kicad", "left", "pan");
        let s = Scheme::parse(Kind::Plot, &t);
        assert_eq!((s.drag(Button::Left, L), s.drag(Button::Middle, L)), (Pan, Pan));
        assert_eq!(Scheme::parse(Kind::Plot, "garbage;x;y=z;left=rotate"), Scheme::preset(Preset::Kicad, Kind::Plot));
    }
    #[test] fn unreachable_actions_are_named() {
        let mut s = Scheme::preset(Preset::Kicad, Kind::Solid);
        assert!(s.unreachable(Kind::Solid).is_empty());
        s.set(Kind::Solid, "left", "none"); assert_eq!(s.unreachable(Kind::Solid), vec![Rotate]);
        let mut s = Scheme::preset(Preset::Kicad, Kind::Plot);
        s.set(Kind::Plot, "wheel", "none"); s.set(Kind::Plot, "pinch", "off"); assert_eq!(s.unreachable(Kind::Plot), vec![Zoom]);
    }
    #[test] fn invert_flips_the_zoom_direction_of_a_wheel_and_a_pinch() {
        let s = Scheme::parse(Kind::Plot, "kicad");
        assert_eq!(s.scroll(L, true, 0.0, 2.0, true), Scroll::Zoom { lines: true, y: -2.0, ctrl: false });
        assert_eq!(s.pinch_factor(1.25, true), 0.8); assert_eq!(s.pinch_factor(1.25, false), 1.25);
        let mut off = s.clone(); off.set(Kind::Plot, "pinch", "off"); assert_eq!(off.pinch_factor(1.25, false), 1.0);
    }
    #[test] fn a_trackpad_pans_with_two_fingers_and_zooms_with_the_pinch() {
        let s = Scheme::parse(Kind::Plot, "kicad");
        assert_eq!(s.scroll(L, false, 2.0, 5.0, false), Scroll::Pan { dx: -2.0, dy: -5.0 });
        assert!(matches!(s.scroll(CT, false, 0.0, 5.0, false), Scroll::Zoom { .. }));
        // turned off, the scroll is the wheel like any other
        let w = Scheme::parse(Kind::Plot, "kicad;scroll=wheel");
        assert!(matches!(w.scroll(L, false, 2.0, 5.0, false), Scroll::Zoom { .. }));
    }
    #[test] fn the_page_has_a_button_per_action_carrying_the_whole_text() {
        let r = rows(Kind::Solid, "legacy");
        assert_eq!(r.len(), DRAGS.len() + WHEELS.len());
        let (k, cur, btns) = &r[1]; assert_eq!((*k, *cur), ("middle", Pan));
        assert!(btns.iter().any(|(a, t)| *a == Zoom && t == "legacy;middle=zoom"));
        for (_, _, b) in &r { for (_, t) in b { assert_eq!(Scheme::edited(Kind::Solid, t, "wheel", "zoom"), Scheme::edited(Kind::Solid, &Scheme::parse(Kind::Solid, t).text(Kind::Solid), "wheel", "zoom")); } }
    }
}

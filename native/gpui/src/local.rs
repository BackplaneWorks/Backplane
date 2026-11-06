//! Settings that belong to this device: kept in a file beside the client's other per-device state, read at start, written when one changes, and
//! NEVER sent to the hub (nothing in this file builds a `Command`; the hub-wide settings are the hub's, shown apart in the dialog).
//! Path: `$BACKPLANE_GPUI_LOCAL` (tests), else `$XDG_CONFIG_HOME/backplane-gpui/local-settings.json`, else `~/.config/backplane-gpui/...`.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

pub const THEMES: [(&str, &str); 3] = [("hub", "Follow the hub"), ("dark", "Dark"), ("light", "Light")];
pub const FONTS: [(&str, &str); 3] = [("Noto Sans Mono CJK JP", "Noto Sans Mono"), ("JetBrains Mono", "JetBrains Mono"), (".SystemUIFont", "System UI")];
pub const VIEWS: [(&str, &str); 2] = [("last", "Where I left off"), ("list", "The thread list")];
pub const TEXT_PCT: (u32, u32, u32) = (80, 160, 10);   // min, max, step
pub const SIDEBAR_PX: (u32, u32, u32) = (180, 420, 24);
/// The viewer pane's width: 0 is the default (VIEWER_PX_DEFAULT); a drag on its left edge sets it, a double click puts it back.
pub const VIEWER_PX: (u32, u32) = (380, 1400);
pub const VIEWER_PX_DEFAULT: u32 = 700;
/// (action, label, default keystroke): the keys the client binds itself.
pub const SHORTCUTS: [(&str, &str, &str); 4] = [("send", "Send the message", "enter"), ("close", "Close a popup", "escape"), ("sidebar", "Fold or unfold the sidebar", "ctrl-b"), ("search", "Search the sidebar", "ctrl-k")];
/// Keystrokes a shortcut can be set to (a pick, not free text: a bad keystroke would unbind the action).
pub const KEYS: [&str; 8] = ["enter", "ctrl-enter", "escape", "ctrl-b", "ctrl-k", "ctrl-shift-b", "ctrl-shift-k", "alt-s"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Local {
    pub theme: String, pub text_pct: u32, pub font: String, pub sidebar_px: u32, pub viewer_px: u32, pub sidebar_open: bool,
    pub notifications: bool, pub default_view: String, pub shortcuts: BTreeMap<String, String>,
    /// Starred models ("provider/model"), listed first in the model menu; kept on this device.
    pub favs: Vec<String>,
}
impl Default for Local {
    fn default() -> Self { Local { theme: "hub".into(), text_pct: 100, font: FONTS[0].0.into(), sidebar_px: 264, viewer_px: 0, sidebar_open: true, notifications: true, default_view: "last".into(), favs: vec![], shortcuts: BTreeMap::new() } }
}

pub fn path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("BACKPLANE_GPUI_LOCAL").filter(|p| !p.is_empty()) { return Some(p.into()) }
    let base = std::env::var_os("XDG_CONFIG_HOME").filter(|p| !p.is_empty()).map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("backplane-gpui/local-settings.json"))
}

fn step(v: u32, dir: i32, (lo, hi, by): (u32, u32, u32)) -> u32 { (if dir < 0 { v.saturating_sub(by) } else { v + by }).clamp(lo, hi) }
fn cycle<'a>(list: &[(&'a str, &str)], now: &str) -> &'a str { let i = list.iter().position(|(k, _)| *k == now).map_or(0, |i| i + 1); list[i % list.len()].0 }

impl Local {
    /// Read from `path`; a missing or damaged file is the defaults, and values out of range are brought into range.
    pub fn read(path: &std::path::Path) -> Local {
        let mut l: Local = std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        l.text_pct = l.text_pct.clamp(TEXT_PCT.0, TEXT_PCT.1); l.sidebar_px = l.sidebar_px.clamp(SIDEBAR_PX.0, SIDEBAR_PX.1);
        if l.viewer_px != 0 { l.viewer_px = l.viewer_px.clamp(VIEWER_PX.0, VIEWER_PX.1); }
        if !THEMES.iter().any(|(k, _)| *k == l.theme) { l.theme = "hub".into() }
        if !VIEWS.iter().any(|(k, _)| *k == l.default_view) { l.default_view = "last".into() }
        if l.font.is_empty() { l.font = FONTS[0].0.into() }
        l.shortcuts.retain(|a, k| SHORTCUTS.iter().any(|(s, _, _)| s == a) && KEYS.contains(&k.as_str()));
        l
    }
    pub fn load() -> Local { path().map(|p| Local::read(&p)).unwrap_or_default() }
    /// Written beside and renamed; a failure is said once on stderr and changes nothing else.
    pub fn save(&self) { if let Some(p) = path() { self.save_to(&p) } }
    pub fn save_to(&self, p: &std::path::Path) {
        let r = (|| -> std::io::Result<()> {
            if let Some(d) = p.parent() { std::fs::create_dir_all(d)?; }
            let tmp = p.with_extension(format!("tmp{}", std::process::id()));
            std::fs::write(&tmp, serde_json::to_vec_pretty(self).map_err(std::io::Error::from)?)?; std::fs::rename(&tmp, p)
        })();
        if let Err(e) = r { eprintln!("local settings not saved: {e}") }
    }
    pub fn key(&self, action: &str) -> String { self.shortcuts.get(action).cloned().or_else(|| SHORTCUTS.iter().find(|(a, _, _)| *a == action).map(|(_, _, k)| k.to_string())).unwrap_or_default() }
    /// Apply a change by name (the dialog's buttons are `local:<name>` with a value). False for a name or value this layer does not know.
    pub fn set(&mut self, name: &str, value: &str) -> bool {
        match name {
            "theme" if THEMES.iter().any(|(k, _)| *k == value) => self.theme = value.into(),
            "text" => self.text_pct = step(self.text_pct, if value == "-" { -1 } else { 1 }, TEXT_PCT),
            "text-reset" => self.text_pct = 100,
            "font" if FONTS.iter().any(|(k, _)| *k == value) => self.font = value.into(),
            "sidebar-width" => self.sidebar_px = step(self.sidebar_px, if value == "-" { -1 } else { 1 }, SIDEBAR_PX),
            "sidebar-open" => self.sidebar_open = value == "on",
            "notifications" => self.notifications = value == "on",
            "default-view" if VIEWS.iter().any(|(k, _)| *k == value) => self.default_view = value.into(),
            "shortcut" => { // "<action>\u{1f}<key>", or just "<action>" to take the next key in the list
                let (a, k) = value.split_once('\u{1f}').unwrap_or((value, ""));
                if !SHORTCUTS.iter().any(|(s, _, _)| *s == a) { return false }
                let key = if k.is_empty() { let now = self.key(a); KEYS[(KEYS.iter().position(|x| *x == now).map_or(0, |i| i + 1)) % KEYS.len()].to_string() } else if KEYS.contains(&k) { k.to_string() } else { return false };
                self.shortcuts.insert(a.into(), key);
            }
            "shortcuts-reset" => self.shortcuts.clear(),
            "fav" if !value.is_empty() && value.len() < 200 => { if let Some(i) = self.favs.iter().position(|f| f == value) { self.favs.remove(i); } else { self.favs.push(value.into()); self.favs.truncate(200); } }
            _ => return false,
        }
        true
    }
    pub fn font_label(&self) -> &str { FONTS.iter().find(|(k, _)| *k == self.font).map_or("Noto Sans Mono", |(_, l)| l) }
    /// Whether the picture is light: this device's choice, else the hub's.
    pub fn light(&self, hub_light: bool) -> bool { match self.theme.as_str() { "light" => true, "dark" => false, _ => hub_light } }
}
pub fn next_font(now: &str) -> &'static str { cycle(&FONTS, now) }
pub fn next_theme(now: &str) -> &'static str { cycle(&THEMES, now) }

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> PathBuf { std::env::temp_dir().join(format!("bp-local-{}-{n}.json", std::process::id())) }
    #[test] fn defaults_are_a_working_client_and_a_damaged_file_is_the_defaults() {
        let p = tmp("dmg"); std::fs::write(&p, b"{ not json").unwrap(); assert_eq!(Local::read(&p), Local::default());
        assert_eq!(Local::read(&tmp("none")), Local::default());
    }
    #[test] fn a_change_survives_a_save_and_a_read_and_values_out_of_range_are_brought_in() {
        let p = tmp("rt");
        let mut l = Local::default(); assert!(l.set("theme", "light")); assert!(l.set("text", "+")); assert!(l.set("sidebar-width", "-")); assert!(l.set("notifications", "off")); assert!(l.set("shortcut", "sidebar\u{1f}alt-s")); l.save_to(&p);
        let back = Local::read(&p); assert_eq!(back, l); assert_eq!((back.text_pct, back.sidebar_px, back.key("sidebar")), (110, 240, "alt-s".to_string()));
        std::fs::write(&p, br#"{"text_pct":9999,"sidebar_px":1,"theme":"neon","shortcuts":{"send":"ctrl-q","nope":"enter","close":"ctrl-b"}}"#).unwrap();
        let b = Local::read(&p); assert_eq!((b.text_pct, b.sidebar_px, b.theme.as_str()), (160, 180, "hub")); assert_eq!(b.key("send"), "enter", "an unknown key is the default"); assert_eq!(b.key("close"), "ctrl-b");
    }
    #[test] fn unknown_names_and_values_change_nothing() {
        let mut l = Local::default(); let before = l.clone();
        assert!(!l.set("theme", "neon") && !l.set("font", "Comic") && !l.set("shortcut", "send\u{1f}ctrl-q") && !l.set("shortcut", "nope") && !l.set("nope", "x")); assert_eq!(l, before);
    }
    #[test] fn steps_stop_at_the_ends_and_the_theme_and_font_cycle() {
        let mut l = Local::default(); for _ in 0..20 { l.set("text", "+"); } assert_eq!(l.text_pct, 160); for _ in 0..20 { l.set("text", "-"); } assert_eq!(l.text_pct, 80);
        assert_eq!(next_theme("hub"), "dark"); assert_eq!(next_theme("light"), "hub"); assert_eq!(next_font(FONTS[2].0), FONTS[0].0);
        assert!(Local { theme: "light".into(), ..Default::default() }.light(false) && !Local { theme: "dark".into(), ..Default::default() }.light(true) && Local::default().light(true));
    }
    /// Local settings stay on the device: this module builds no hub command and imports none.
    #[test] fn this_module_never_talks_to_the_hub() {
        let src = include_str!("local.rs"); let code = src.split("#[cfg(test)]").next().unwrap();
        for word in ["Command", "tx.try_send", "hub::"] { assert!(!code.replace("NEVER sent to the hub (nothing in this file builds a `Command`;", "").contains(word), "{word}"); }
    }
}

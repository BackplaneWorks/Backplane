//! Settings that belong to this device: kept in a file beside the client's other per-device state, read at start, written when one changes, and
//! NEVER sent to the hub (nothing in this file builds a `Command`; the hub-wide settings are the hub's, shown apart in the dialog).
//! Path: `$BACKPLANE_GPUI_LOCAL` (tests), else `$XDG_CONFIG_HOME/backplane-gpui/local-settings.json`, else `~/.config/backplane-gpui/...`.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

/* i18n-ok: labels the settings page draws through tr; font names */ pub const THEMES: [(&str, &str); 3] = [("hub", "Follow the hub"), ("dark", "Dark"), ("light", "Light")];
/// The 3D viewers' background (Mech and 3D board share it): follow the theme, or light, dark, or a colour of the person's own (docs/VIEWER-BACKGROUND.md).
/* i18n-ok: labels the settings page draws through tr */ pub const VIEWER_BGS: [(&str, &str); 4] = [("auto", "Auto"), ("light", "Light"), ("dark", "Dark"), ("custom", "Custom")];
/* i18n-ok: labels the settings page draws through tr; colour names */ pub const VIEWER_BG_SWATCHES: [(&str, u32); 8] = [("Slate", 0x8a94a6), ("Graphite", 0x5a5f66), ("Sand", 0xd9cfb8), ("Sky", 0xa9c7e8), ("Mint", 0xa8d8c0), ("Rose", 0xe2b4bd), ("Plum", 0x6b5b7b), ("Navy", 0x2f4468)];
/* i18n-ok: labels the settings page draws through tr; font names */ pub const FONTS: [(&str, &str); 3] = [("Noto Sans Mono CJK JP", "Noto Sans Mono"), ("JetBrains Mono", "JetBrains Mono"), (".SystemUIFont", "System UI")];
/* i18n-ok: labels the settings page draws through tr; font names */ pub const VIEWS: [(&str, &str); 2] = [("last", "Where I left off"), ("list", "The thread list")];
pub const TEXT_PCT: (u32, u32, u32) = (80, 160, 5);   // min, max, step (the slider's stops)
pub const SIDEBAR_PX: (u32, u32, u32) = (180, 420, 24);
/// The viewer pane's width: 0 is the default (VIEWER_PX_DEFAULT); a drag on its left edge sets it, a double click puts it back.
pub const VIEWER_PX: (u32, u32) = (380, 1400);
pub const VIEWER_PX_DEFAULT: u32 = 700;
/// Keystrokes a shortcut can be set to (a pick, not free text: a bad keystroke would unbind the action).
pub const KEYS: [&str; 8] = ["enter", "ctrl-enter", "escape", "ctrl-b", "ctrl-k", "ctrl-shift-b", "ctrl-shift-k", "alt-s"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Local {
    /// The interface language: "auto" (the system's), "en" or "ja" (`i18n::CHOICES`).
    pub lang: String,
    pub theme: String, pub viewer_bg: String, pub viewer_bg_rgb: u32, pub text_pct: u32, pub font: String, pub sidebar_px: u32, pub viewer_px: u32, pub sidebar_open: bool,
    pub notifications: bool, pub default_view: String, pub shortcuts: BTreeMap<String, String>,
    /// Starred models ("provider/model"), listed first in the model menu; kept on this device.
    pub favs: Vec<String>,
    /// Split's panes put away on this device ("sch", "pcb", "3d" words, as 0.14's split.hide) and where its two dividers sit
    /// (the schematic's share beside the board and 3D, the board's share beside the 3D; 0 is the default).
    pub split_hide: String, pub split_main: f32, pub split_side: f32,
    /// Streamer mode (streamer.rs): on by the person's switch; and whether it turns on by itself while the screen is shared (a portal /
    /// PipeWire screen cast, where the platform can tell) or while a known recording app runs (an opt-in guess).
    pub streamer: bool, pub streamer_share: bool, pub streamer_apps: bool,
}
impl Default for Local {
    fn default() -> Self { Local { lang: "auto".into(), theme: "hub".into(), viewer_bg: "auto".into(), viewer_bg_rgb: VIEWER_BG_SWATCHES[0].1, text_pct: 100, font: FONTS[0].0.into(), sidebar_px: 264, viewer_px: 0, sidebar_open: true, notifications: true, default_view: "last".into(), favs: vec![], shortcuts: BTreeMap::new(), split_hide: String::new(), split_main: 0., split_side: 0., streamer: false, streamer_share: true, streamer_apps: false } }
}

pub fn path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("BACKPLANE_GPUI_LOCAL").filter(|p| !p.is_empty()) { return Some(p.into()) }
    let base = crate::plat::config_base()?;
    Some(base.join("backplane-gpui/local-settings.json"))
}

fn step(v: u32, dir: i32, (lo, hi, by): (u32, u32, u32)) -> u32 { (if dir < 0 { v.saturating_sub(by) } else { v + by }).clamp(lo, hi) }
/// `v` at the nearest stop of a range (`lo` + a whole number of steps), within it.
pub fn snap(v: u32, (lo, hi, by): (u32, u32, u32)) -> u32 { (lo + (v.clamp(lo, hi) - lo + by / 2) / by * by).min(hi) }
fn cycle<'a>(list: &[(&'a str, &str)], now: &str) -> &'a str { let i = list.iter().position(|(k, _)| *k == now).map_or(0, |i| i + 1); list[i % list.len()].0 }

impl Local {
    /// Read from `path`; a missing or damaged file is the defaults, and values out of range are brought into range.
    pub fn read(path: &std::path::Path) -> Local {
        let mut l: Local = std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        l.text_pct = l.text_pct.clamp(TEXT_PCT.0, TEXT_PCT.1); l.sidebar_px = l.sidebar_px.clamp(SIDEBAR_PX.0, SIDEBAR_PX.1);
        if l.viewer_px != 0 { l.viewer_px = l.viewer_px.clamp(VIEWER_PX.0, VIEWER_PX.1); }
        if !crate::i18n::CHOICES.iter().any(|(k, _)| *k == l.lang) { l.lang = "auto".into() }
        if !THEMES.iter().any(|(k, _)| *k == l.theme) { l.theme = "hub".into() }
        if !VIEWER_BGS.iter().any(|(k, _)| *k == l.viewer_bg) { l.viewer_bg = "auto".into() }
        l.viewer_bg_rgb &= 0xff_ffff;
        if !VIEWS.iter().any(|(k, _)| *k == l.default_view) { l.default_view = "last".into() }
        if l.font.is_empty() { l.font = FONTS[0].0.into() }
        for r in [&mut l.split_main, &mut l.split_side] { if !(r.is_finite() && *r > 0. && *r < 1.) { *r = 0. } }
        l.shortcuts.retain(|a, k| crate::keys::rebindable(a) && crate::keys::choices(a).contains(k));
        l
    }
    /// What the app starts with; the language in force is set from it here, once.
    pub fn load() -> Local { let l = path().map(|p| Local::read(&p)).unwrap_or_default(); crate::i18n::set_choice(&l.lang); l }
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
    pub fn key(&self, action: &str) -> String { self.shortcuts.get(action).cloned().or_else(|| crate::keys::find(action).map(|x| x.key.to_string())).unwrap_or_default() }
    /// Apply a change by name (the dialog's buttons are `local:<name>` with a value). False for a name or value this layer does not know.
    pub fn set(&mut self, name: &str, value: &str) -> bool {
        match name {
            "lang" if crate::i18n::CHOICES.iter().any(|(k, _)| *k == value) => self.lang = value.into(),
            "theme" if THEMES.iter().any(|(k, _)| *k == value) => self.theme = value.into(),
            "viewer-bg" if VIEWER_BGS.iter().any(|(k, _)| *k == value) => self.viewer_bg = value.into(),
            "viewer-bg-color" => { let Ok(c) = u32::from_str_radix(value.trim_start_matches('#'), 16) else { return false }; if value.trim_start_matches('#').len() != 6 { return false } self.viewer_bg_rgb = c; self.viewer_bg = "custom".into(); }
            "viewer-bg-shade" => { // a step lighter ("+") or darker ("-") of the custom colour
                let k = if value == "-" { 0.9 } else { 1.0 / 0.9 };
                let c = self.viewer_bg_rgb; self.viewer_bg_rgb = [16, 8, 0].iter().fold(0u32, |a, sh| a | ((((c >> sh) & 255) as f32 * k).round().clamp(0.0, 255.0) as u32) << sh); self.viewer_bg = "custom".into();
            }
            // "-"/"+" a step, or a percentage (the slider), snapped to a stop and kept in the range
            "text" => self.text_pct = match value { "-" | "+" => step(self.text_pct, if value == "-" { -1 } else { 1 }, TEXT_PCT), v => { let Ok(n) = v.parse::<u32>() else { return false }; snap(n, TEXT_PCT) } },
            "text-reset" => self.text_pct = 100,
            "font" if FONTS.iter().any(|(k, _)| *k == value) => self.font = value.into(),
            "sidebar-width" => self.sidebar_px = step(self.sidebar_px, if value == "-" { -1 } else { 1 }, SIDEBAR_PX),
            "sidebar-open" => self.sidebar_open = value == "on",
            "notifications" => self.notifications = value == "on",
            "streamer" => self.streamer = if value == "toggle" { !self.streamer } else { value == "on" },
            "streamer-share" => self.streamer_share = value == "on",
            "streamer-apps" => self.streamer_apps = value == "on",
            "default-view" if VIEWS.iter().any(|(k, _)| *k == value) => self.default_view = value.into(),
            "shortcut" => { // "<action>\u{1f}<key>", or just "<action>" to take the next key in the list
                let (a, k) = value.split_once('\u{1f}').unwrap_or((value, ""));
                if !crate::keys::rebindable(a) { return false }
                let ch = crate::keys::choices(a); let key = if k.is_empty() { let now = self.key(a); ch[(ch.iter().position(|x| *x == now).map_or(0, |i| i + 1)) % ch.len()].clone() } else if ch.iter().any(|x| x == k) { k.to_string() } else { return false };
                self.shortcuts.insert(a.into(), key);
            }
            "shortcuts-reset" => self.shortcuts.clear(),
            "fav" if !value.is_empty() && value.len() < 200 => { if let Some(i) = self.favs.iter().position(|f| f == value) { self.favs.remove(i); } else { self.favs.push(value.into()); self.favs.truncate(200); } }
            _ => return false,
        }
        true
    }
    /// The 3D viewers' background for the theme now (`light`: the resolved theme).
    pub fn viewer_backdrop(&self, light: bool) -> crate::viewers::raster::Backdrop { crate::viewers::raster::Backdrop::resolve(&self.viewer_bg, self.viewer_bg_rgb, light) }
    pub fn font_label(&self) -> &str { FONTS.iter().find(|(k, _)| *k == self.font).map_or("Noto Sans Mono", |(_, l)| l) }
    /// Whether the picture is light: this device's choice, else the hub's.
    pub fn light(&self, hub_light: bool) -> bool { match self.theme.as_str() { "light" => true, "dark" => false, _ => hub_light } }
}
pub fn next_font(now: &str) -> &'static str { cycle(&FONTS, now) }
pub fn next_theme(now: &str) -> &'static str { cycle(&THEMES, now) }

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> crate::tmp::TempFile { crate::tmp::file(&format!("local-{n}"), "local.json") }
    #[test] fn defaults_are_a_working_client_and_a_damaged_file_is_the_defaults() {
        let p = tmp("dmg"); std::fs::write(&p, b"{ not json").unwrap(); assert_eq!(Local::read(&p), Local::default());
        assert_eq!(Local::read(&tmp("none")), Local::default());
    }
    #[test] fn the_language_is_a_choice_of_the_three_and_anything_else_is_automatic() {
        let mut l = Local::default(); assert_eq!(l.lang, "auto");
        assert!(l.set("lang", "ja") && l.lang == "ja"); assert!(l.set("lang", "en") && l.lang == "en"); assert!(!l.set("lang", "fr") && l.lang == "en", "a language the client has not is refused");
        let p = tmp("lang"); std::fs::write(&p, br#"{"lang":"klingon"}"#).unwrap(); assert_eq!(Local::read(&p).lang, "auto");
        std::fs::write(&p, br#"{"lang":"ja"}"#).unwrap(); assert_eq!(Local::read(&p).lang, "ja");
    }
    #[test] fn a_change_survives_a_save_and_a_read_and_values_out_of_range_are_brought_in() {
        let p = tmp("rt");
        let mut l = Local::default(); assert!(l.set("theme", "light")); assert!(l.set("text", "+")); assert!(l.set("sidebar-width", "-")); assert!(l.set("notifications", "off")); assert!(l.set("shortcut", "sidebar\u{1f}alt-s")); l.save_to(&p);
        let back = Local::read(&p); assert_eq!(back, l); assert_eq!((back.text_pct, back.sidebar_px, back.key("sidebar")), (105, 240, "alt-s".to_string()));
        std::fs::write(&p, br#"{"text_pct":9999,"sidebar_px":1,"theme":"neon","shortcuts":{"send":"ctrl-q","nope":"enter","close":"ctrl-b"}}"#).unwrap();
        let b = Local::read(&p); assert_eq!((b.text_pct, b.sidebar_px, b.theme.as_str()), (160, 180, "hub")); assert_eq!(b.key("send"), "enter", "an unknown key is the default"); assert_eq!(b.key("close"), "ctrl-b");
    }
    #[test] fn unknown_names_and_values_change_nothing() {
        let mut l = Local::default(); let before = l.clone();
        assert!(!l.set("theme", "neon") && !l.set("font", "Comic") && !l.set("shortcut", "send\u{1f}ctrl-q") && !l.set("shortcut", "nope") && !l.set("nope", "x")); assert_eq!(l, before);
    }
    #[test] fn the_viewer_background_follows_the_theme_or_the_choice() {
        let mut l = Local::default(); use crate::viewers::raster::Backdrop as B;
        assert_eq!((l.viewer_backdrop(true), l.viewer_backdrop(false)), (B::LIGHT, B::DARK), "auto follows the theme");
        assert!(l.set("viewer-bg", "light")); assert_eq!(l.viewer_backdrop(false), B::LIGHT); assert!(l.set("viewer-bg", "dark")); assert_eq!(l.viewer_backdrop(true), B::DARK);
        assert!(l.set("viewer-bg-color", "#c8a000")); assert_eq!(l.viewer_bg, "custom");
        let b = l.viewer_backdrop(false); assert_eq!((b.top, b.bottom), ([0xc8, 0xa0, 0x00], [176, 141, 0]));
        assert!(!l.set("viewer-bg-color", "zzzzzz") && !l.set("viewer-bg-color", "fff") && !l.set("viewer-bg", "neon"));
        l.viewer_bg_rgb = 0x808080; assert!(l.set("viewer-bg-shade", "+")); assert!(l.viewer_bg_rgb & 255 > 0x80); assert!(l.set("viewer-bg-shade", "-")); assert!(l.set("viewer-bg-shade", "-")); assert!(l.viewer_bg_rgb & 255 < 0x80);
        let p = tmp("vbg"); l.save_to(&p); assert_eq!(Local::read(&p), l); std::fs::write(&p, br#"{"viewer_bg":"neon","viewer_bg_rgb":4294967295}"#).unwrap(); let r = Local::read(&p); assert_eq!((r.viewer_bg.as_str(), r.viewer_bg_rgb), ("auto", 0xffffff)); let _ = std::fs::remove_file(&p);
    }
    #[test] fn steps_stop_at_the_ends_and_the_theme_and_font_cycle() {
        let mut l = Local::default(); for _ in 0..20 { l.set("text", "+"); } assert_eq!(l.text_pct, 160); for _ in 0..20 { l.set("text", "-"); } assert_eq!(l.text_pct, 80);
        for (v, want) in [("100", 100), ("123", 125), ("122", 120), ("9999", 160), ("0", 80), ("157", 155), ("158", 160)] { assert!(l.set("text", v)); assert_eq!(l.text_pct, want, "{v}"); }
        assert!(!l.set("text", "big"), "not a number, no change"); assert_eq!(l.text_pct, 160);
        assert_eq!(next_theme("hub"), "dark"); assert_eq!(next_theme("light"), "hub"); assert_eq!(next_font(FONTS[2].0), FONTS[0].0);
        assert!(Local { theme: "light".into(), ..Default::default() }.light(false) && !Local { theme: "dark".into(), ..Default::default() }.light(true) && Local::default().light(true));
    }
    /// Local settings stay on the device: this module builds no hub command and imports none.
    #[test] fn this_module_never_talks_to_the_hub() {
        let src = include_str!("local.rs"); let code = src.split("#[cfg(test)]").next().unwrap();
        for word in ["Command", "tx.try_send", "hub::"] { assert!(!code.replace("NEVER sent to the hub (nothing in this file builds a `Command`;", "").contains(word), "{word}"); }
    }
}

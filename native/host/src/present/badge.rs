//! A project's badge: a colour and an icon (settings `project.color.<id>`, `project.icon.<id>`), and the editor the project menu opens (`look`, `look-set`).
//! 0.14's tints and icons (core/badge.bend); a project with no colour set takes one from its id. Every client draws what this module says.
use serde_json::{json, Value};

pub const TINTS: &[(&str, &str)] = &[("blue", "#5b9cf5"), ("teal", "#3fbfb0"), ("green", "#5cc46a"), ("lime", "#a6c84a"), ("amber", "#e0b040"), ("orange", "#e8854a"),
    ("red", "#e5646a"), ("pink", "#e06fb4"), ("violet", "#a483e8"), ("slate", "#8a97a8")];
pub const ICONS: &[&str] = &["folder", "circuit-board", "cpu", "zap", "radio", "box", "layers", "code", "terminal", "globe", "wrench", "flask-conical", "book", "star",
    // added for the picked icons (agents/iconpick.rs); assets/icons.json draws each
    "usb", "plug", "presentation", "gauge", "file-text", "image"];

/// The colour a project takes when none is set: from its id, so it stays put (Badge.auto).
pub fn auto(id: &str) -> &'static str { TINTS[(id.bytes().map(|b| b as usize).sum::<usize>() % 10).min(TINTS.len() - 1)].0 }
/// A set colour when it is one of the tints, else the automatic one.
pub fn color_of<'a>(set: &'a str, id: &str) -> &'a str { TINTS.iter().find(|(n, _)| *n == set).map(|(n, _)| *n).unwrap_or_else(|| auto(id)) }
pub fn css_of(name: &str) -> &'static str { TINTS.iter().find(|(n, _)| *n == name).map_or("#8a97a8", |(_, c)| c) }
pub fn icon_of(set: &str) -> &'static str { ICONS.iter().copied().find(|i| *i == set).unwrap_or("folder") }
/// The icon a project shows: the person's when they chose one (`set`), else the model's pick (`project.pick.<id>`: "<icon>" or "<icon> <tint>"), else the folder.
pub fn icon_eff(set: &str, pick: &str) -> &'static str { if ICONS.contains(&set) { icon_of(set) } else { icon_of(pick.split(' ').next().unwrap_or("")) } }
/// The colour a project shows: the person's, else the pick's tint, else the one from its id.
pub fn color_eff<'a>(set: &'a str, pick: &'a str, id: &str) -> &'a str { if TINTS.iter().any(|(n, _)| *n == set) { set } else { color_of(pick.split(' ').nth(1).unwrap_or(""), id) } }

/// The setting a `look-set` press writes: `project.color.<id>=<tint>` or `project.icon.<id>=<icon>` (an empty value takes the default back); None for anything else.
pub fn setting_of(v: &str) -> Option<(String, String)> {
    let (k, val) = v.split_once('=')?;
    let (kind, id) = if let Some(id) = k.strip_prefix("project.color.") { ("color", id) } else if let Some(id) = k.strip_prefix("project.icon.") { ("icon", id) } else { return None };
    let ok = id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') && !id.is_empty()
        && (val.is_empty() || if kind == "color" { TINTS.iter().any(|(n, _)| *n == val) } else { ICONS.contains(&val) });
    ok.then(|| (k.to_string(), val.to_string()))
}

/// `badge.list` (data/1): the tints and icons a project may take.
pub fn list() -> Value { json!({"colors": TINTS.iter().map(|(n, css)| json!({"name": n, "css": css})).collect::<Vec<_>>(), "icons": ICONS}) }

/// The project's badge editor while it is open: every choice carries the press it makes.
/// `color_set` and `icon_set` are what the project shows (`color_eff`, `icon_eff`); `again` is the Pick again press (`look-again`, `project.pick`).
pub fn editor(id: &str, color_set: &str, icon_set: &str) -> Value {
    let (c, i) = (color_of(color_set, id), icon_of(icon_set));
    json!({"colors": TINTS.iter().map(|(n, css)| json!({"label": n, "css": css, "on": *n == c, "value": format!("project.color.{id}={n}")})).collect::<Vec<_>>(),
        "icons": ICONS.iter().map(|n| json!({"label": n, "on": *n == i, "value": format!("project.icon.{id}={n}")})).collect::<Vec<_>>(),
        "again": {"label": "Pick again", "value": id}})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_project_without_a_colour_takes_one_from_its_id_and_a_set_one_wins() {
        assert_eq!(auto("p1"), auto("p1")); assert!(TINTS.iter().any(|(n, _)| *n == auto("p1")));
        assert_eq!(color_of("pink", "p1"), "pink"); assert_eq!(color_of("nonsense", "p1"), auto("p1")); assert_eq!(icon_of("cpu"), "cpu"); assert_eq!(icon_of("x"), "folder");
    }
    #[test] fn the_persons_choice_wins_over_the_pick_and_the_pick_over_the_default() {
        assert_eq!((icon_eff("cpu", "usb teal"), color_eff("pink", "usb teal", "p1")), ("cpu", "pink"));
        assert_eq!((icon_eff("", "usb teal"), color_eff("", "usb teal", "p1")), ("usb", "teal"));
        assert_eq!((icon_eff("", "presentation"), color_eff("", "presentation", "p1")), ("presentation", auto("p1")), "a pick with no tint keeps the colour from the id");
        assert_eq!((icon_eff("skull", ""), color_eff("mauve", "", "p1")), ("folder", auto("p1")));
        assert_eq!(editor("p1", "teal", "usb")["again"], json!({"label": "Pick again", "value": "p1"}));
    }
    #[test] fn only_a_tint_or_an_icon_of_the_list_is_written() {
        assert_eq!(setting_of("project.color.p1=teal"), Some(("project.color.p1".into(), "teal".into())));
        assert_eq!(setting_of("project.icon.p1="), Some(("project.icon.p1".into(), String::new())));
        for bad in ["project.color.p1=mauve", "project.icon.p1=skull", "agent.provider=codex", "project.color.=red", "project.color.a b=red", "nothing"] { assert_eq!(setting_of(bad), None, "{bad}"); }
    }
    #[test] fn the_list_names_every_tint_and_icon() { let l = list(); assert_eq!((l["colors"].as_array().unwrap().len(), l["icons"].as_array().unwrap().len()), (TINTS.len(), ICONS.len())); assert_eq!(l["colors"][0], json!({"name": "blue", "css": "#5b9cf5"})); }
    #[test] fn the_editor_marks_the_current_choice_and_carries_each_press() {
        let e = editor("p1", "", "cpu");
        assert_eq!(e["colors"].as_array().unwrap().iter().filter(|c| c["on"] == true).count(), 1);
        assert_eq!(e["icons"].as_array().unwrap().iter().find(|c| c["on"] == true).unwrap()["label"], "cpu");
        assert_eq!(e["colors"][1]["value"], "project.color.p1=teal");
    }
}

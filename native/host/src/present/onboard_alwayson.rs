//! Setup's step 4, "Where your uberbot lives": its rows and presses. The step's frame (title, the two-line explanation, Next) is
//! present/onboard.rs's; everything under the explanation is here. Lane F (always-on Boat) owns this file; this is its stub: the three choices,
//! with An always-on Boat and Another machine of yours saying they are coming.
//!
//! The interface onboard.rs calls (keep these signatures):
//! - `rows(f, session, c)`: the rows under the explanation, in the Settings row shape (`onboard::row`, `btn`, fields named `obaon*`).
//! - `done(f)`: a choice is made (the step's mark).
//! - `next(f, session, ask)`: Next was pressed on this step; records the preselected choice when none is saved.
//! - `act(f, session, c, action, value, ask)`: a press whose action starts `ob-aon-`; Some(session patch) when it is this step's.
//! Fields named `obaon*` are kept as drafts `@f.obaon*` by onboard::act (`bfield`), so `draft(session, "@f.obaon...")` reads them.
use super::onboard::{btn, draft, row, Ask, Client, Facts};
use serde_json::{json, Value};

/// The choice made: "here", "boat" or "machine"; empty until one is.
pub const CHOICE: &str = "alwayson.choice";
/// The three options, in order: id, label, what it means.
pub const OPTIONS: &[(&str, &str, &str)] = &[
    ("here", "Stay on this computer", "Your uberbot runs here, while this computer is on. You can move it later in Settings, Machines."),
    ("boat", "An always-on Boat", "A small server on your own Boat account runs your uberbot day and night. You pay Boat for it."),
    ("machine", "Another machine of yours", "A home server or a desktop that stays on runs your uberbot."),
];

fn chosen<'a>(f: &'a Facts) -> &'a str { f.st.settings.get(CHOICE).map(String::as_str).unwrap_or("") }
/// The option on screen: the one pressed in this view, else the one saved, else Stay on this computer (preselected).
fn picked<'a>(f: &'a Facts, session: &'a Value) -> &'a str {
    let d = draft(session, "@ob.aon");
    if !d.is_empty() { d } else if !chosen(f).is_empty() { chosen(f) } else { "here" }
}

pub fn rows(f: &Facts, session: &Value, _c: &Client) -> Vec<Value> {
    let on = picked(f, session);
    let mut v = vec![];
    for (id, label, note) in OPTIONS {
        v.push(row(label, note, vec![btn(if *id == on { "Chosen" } else { "Choose" }, "ob-aon-pick", id, *id == on)]));
        if *id == on && *id != "here" { v.push(row("Coming soon", "This build cannot set it up yet. Stay on this computer for now.", vec![])); }
    }
    v
}

pub fn done(f: &Facts) -> bool { !chosen(f).is_empty() }

pub fn next(f: &Facts, session: &Value, ask: &mut Ask) {
    // the stub saves only Stay on this computer: the others cannot be set up yet
    if chosen(f).is_empty() && picked(f, session) == "here" { ask.put(CHOICE, "here"); }
}

pub fn act(_f: &Facts, _session: &Value, _c: &Client, action: &str, value: &str, ask: &mut Ask) -> Option<Value> {
    match action {
        "ob-aon-pick" if OPTIONS.iter().any(|o| o.0 == value) => { if value == "here" { ask.put(CHOICE, "here"); } Some(json!({"drafts": {"@ob.aon": value}})) }
        _ => None,
    }
}

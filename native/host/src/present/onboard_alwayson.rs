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
fn other_of(f: &Facts) -> Value { f.st.settings.get(crate::net::alwayson::other::PICKED).and_then(|t| serde_json::from_str(t).ok()).unwrap_or(Value::Null) }
fn linked_of(f: &Facts) -> Value {
    let m = if f.machines.is_string() { serde_json::from_str::<Value>(f.machines.as_str().unwrap_or("")).unwrap_or(Value::Null) } else { f.machines.clone() };
    m["linked"].clone()
}

pub fn rows(f: &Facts, session: &Value, _c: &Client) -> Vec<Value> {
    use crate::net::alwayson::rows as ar;
    let on = picked(f, session);
    let mut v = vec![];
    for (id, label, note) in OPTIONS {
        v.push(row(label, note, vec![btn(if *id == on { "Chosen" } else { "Choose" }, "ob-aon-pick", id, *id == on)]));
        if *id != on { continue }
        match *id {
            "boat" => v.extend(ar::boat_rows(&ar::view_of(f.info))),
            "machine" => v.extend(ar::other_rows(&other_of(f), &linked_of(f), &|k| format!("obaon{k}"), &|k| draft(session, &format!("@f.obaon{k}")).to_string())),
            _ => {}
        }
        // the sign-in panel of the machine whose Sign in there was pressed, under its row
        if *id != "here" { v.extend(super::signin_there::open_rows(f.info)) }
    }
    v
}

/// Stay on this computer is the default (D2): with nothing saved the step is done, as with a choice made. A Boat or another machine
/// being set up does not undo that: the uberbot stays here until it is moved.
pub fn done(_f: &Facts) -> bool { true }

/// Next: Stay on this computer is recorded at once; a Boat once it is made (make.rs records `boat`), another machine once one is picked
/// (other.rs records `machine`). Until then Next leaves the step not done, so setup comes back to it.
pub fn next(f: &Facts, session: &Value, ask: &mut Ask) {
    if chosen(f).is_empty() && picked(f, session) == "here" { ask.put(CHOICE, "here"); }
}

pub fn act(_f: &Facts, session: &Value, _c: &Client, action: &str, value: &str, ask: &mut Ask) -> Option<Value> {
    match action {
        "ob-aon-pick" if OPTIONS.iter().any(|o| o.0 == value) => { if value == "here" { ask.put(CHOICE, "here"); } Some(json!({"drafts": {"@ob.aon": value}})) }
        // the Boat's and the other machine's presses: to the hub as Settings, Machines sends them
        a if a.starts_with("alwayson-") => {
            let fields = json!({"addr": draft(session, "@f.obaonaddr"), "code": draft(session, "@f.obaoncode")});
            ask.send("alwayson.act", json!({"action": a, "value": value, "fields": fields}));
            Some(if a == "alwayson-other-pair" { json!({"drafts": {"@f.obaonaddr": "", "@f.obaoncode": ""}}) } else { json!({}) })
        }
        // Sign it in to your agents (lane G's panel: `machine-signin*`) and Move your uberbot there (`alwayson-move`: net/alwayson makes the uberbot first when there is none, then the home move)
        a if a.starts_with("machine-signin") => {
            let fields: serde_json::Map<String, Value> = super::signin_there::FIELDS.iter().map(|k| (k.to_string(), json!(draft(session, &format!("@f.{k}"))))).collect();
            ask.send("machines.page.act", json!({"action": a, "value": value, "fields": fields}));
            Some(json!({}))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_boat_and_machine_presses_go_to_the_hub() {
        let st = crate::state::State::default(); let info = json!({}); let f = Facts::of(&st, &info);
        let c = Client { phone: false, os: "linux".into(), launcher: false };
        let session = json!({"drafts": {"@f.obaonaddr": "http://nas:3797", "@f.obaoncode": "ABCD"}});
        let mut ask = Ask { hub: "h", rid: 0, cmds: vec![] };
        assert!(act(&f, &session, &c, "alwayson-other-pair", "", &mut ask).is_some());
        let cmd = ask.cmds.last().unwrap().to_string();
        assert!(cmd.contains("alwayson.act") && cmd.contains("http://nas:3797") && cmd.contains("ABCD"), "{cmd}");
        assert!(act(&f, &session, &c, "machine-signin-open", "p1", &mut ask).is_some()); assert!(ask.cmds.last().unwrap().to_string().contains("machines.page.act"));
        assert!(act(&f, &session, &c, "alwayson-move", "p1", &mut ask).is_some()); assert!(ask.cmds.last().unwrap().to_string().contains("alwayson.act"), "the move makes the uberbot first when there is none");
        assert!(act(&f, &session, &c, "kicad-path", "", &mut ask).is_none());
        let r = rows(&f, &json!({"drafts": {"@ob.aon": "boat"}}), &c);
        assert!(r.iter().any(|x| x["label"] == "Connect your Boat account"), "{r:?}");
        let r = rows(&f, &json!({"drafts": {"@ob.aon": "machine"}}), &c);
        assert!(r.iter().any(|x| x["label"] == "Install Backplane there"));
        assert!(done(&f), "staying here is the default: nothing saved is done");
        let r = rows(&f, &json!({}), &c); assert_eq!((r[0]["buttons"][0]["label"].as_str(), r[0]["buttons"][0]["on"].as_bool()), (Some("Chosen"), Some(true)), "the option in effect says Chosen");
    }
}

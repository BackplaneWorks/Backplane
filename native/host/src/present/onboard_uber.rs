//! Setup's "Your uberbot" step (before Where your uberbot lives): the person names the uberbot and sees its cat. The cat row (`cat_row`) is
//! drawn by the desktop as one control (native/gpui/src/screens/catpick.rs): the name field with the cat beside it, the colours, Different cat.
//! While the name is typed the window works out the look itself, each letter (bots/cat.rs, the same file in both crates), so nothing waits on
//! the hub; `look` set in the row is a picked one. Say hello (the step's Next until it is made, or Enter in the name) makes the uberbot with
//! that name, colour and pick; when the bot is there the row's `spawn` names it and the window plays the cat's arrival once.
//! The bot's own Settings tab draws the same row (bots/api.rs `catpick`), its presses `bot-cat` (present/screen.rs).
//!
//! Drafts: `@f.obubername` (the name typed, from `bfield`), `@f.obubercustom` (a custom colour typed), `@ob.uber.color` ("#rrggbb", "none"
//! for the coat's own, unset: as kept), `@ob.uber.look` (a number: picked; "auto": the name's; unset: as kept), `@ob.uber.err` (the last
//! refusal), `@ob.uber.made` ("1": Say hello was pressed in this view, so its arrival plays).
use super::onboard::{draft, Ask, Facts, Step};
use crate::bots::cat;
use serde_json::{json, Value};

/// The name the field starts with on a home with no uberbot.
pub const DEFAULT_NAME: &str = "Bub";

/// The cat control. `field`/`name`: the name box and its text; `look`: a picked look, None for the name's; `color` "#rrggbb" or "";
/// `act`: the action its presses send, with values `color\u{1f}#rrggbb` (or empty: the coat's own), `custom`, `shuffle`, `auto`, `go`;
/// `go`: the label of a button that saves the name (None: there is none, the step's Next does it); `spawn`: the bot that just arrived.
pub fn cat_row(field: &str, custom: &str, name: &str, custom_text: &str, look: Option<u64>, color: &str, act: &str, go: Option<&str>, spawn: &str) -> Value {
    let presets: Vec<Value> = cat::PRESETS.iter().map(|(n, c)| json!({"name": n, "color": cat::color_hex(*c)})).collect();
    json!({"kind": "cat", "label": "Name", "note": "", "act": act, "look": look, "color": color, "presets": presets, "picked": look.is_some(),
        "fields": [{"name": field, "hint": "Name", "secret": false, "value": name}, {"name": custom, "hint": "#rrggbb", "secret": false, "value": custom_text}],
        "go": go.map(|g| json!({"label": g})), "spawn": spawn,
        "words": {"color": "Colour", "own": "Its own", "custom": "Custom", "use": "Use", "shuffle": "Different cat", "auto": "The name's cat"}})
}

/// Whether a name may be a bot's: well formed and free (its own bot, `me`, may keep it in any case).
pub fn name_problem(f: &Facts, name: &str, me: Option<&str>) -> Option<String> {
    if !crate::bots::rpc::name_ok(name) { return Some("Use 1 to 24 letters, digits, - or _.".into()) }
    let taken = f.st.bots_nav.iter().any(|(id, b)| Some(id.as_str()) != me && b.name.eq_ignore_ascii_case(name));
    taken.then(|| format!("A bot named {name} already exists."))
}

fn uber<'a>(f: &'a Facts) -> Option<(&'a str, &'a crate::state::BotNav)> {
    let u = f.st.settings.get("uber.bot").map(String::as_str).unwrap_or("");
    f.st.bots_nav.get_key_value(u).map(|(k, b)| (k.as_str(), b))
}
/// What the step shows: the name, the look (None: the name's), the colour.
fn shown(f: &Facts, session: &Value) -> (String, Option<u64>, String) {
    let u = uber(f);
    let name = if session["drafts"]["@f.obubername"].is_string() { draft(session, "@f.obubername").to_string() } else { u.map_or(DEFAULT_NAME.to_string(), |(_, b)| b.name.clone()) };
    let look = match draft(session, "@ob.uber.look") { "auto" => None, "" => u.filter(|(_, b)| !b.auto).map(|(_, b)| b.look), n => n.parse().ok() };
    let color = match draft(session, "@ob.uber.color") { "none" => String::new(), "" => u.map_or(String::new(), |(_, b)| b.color.clone()), c => c.to_string() };
    (name.trim().to_string(), look, color)
}

pub fn step(f: &Facts, session: &Value) -> Step {
    let (name, look, color) = shown(f, session);
    let u = uber(f);
    let spawn = if draft(session, "@ob.uber.made") == "1" { u.map_or("", |(id, _)| id) } else { "" };
    let mut r = cat_row("obubername", "obubercustom", &name, draft(session, "@f.obubercustom"), look, &color, "ob-uber", u.map(|_| "Save"), spawn);
    let err = draft(session, "@ob.uber.err");
    if !err.is_empty() { r["note"] = json!(err); r["tone"] = json!("warn"); }
    else if u.is_some() && spawn.is_empty() { r["note"] = json!("Your uberbot is here. A new name or colour is saved with Save."); }
    Step { id: "uber", group: "", title: "Your uberbot".into(),
        body: "Your uberbot is the assistant you talk to first. Give it a name: the name picks its cat, and the same name gives the same cat on every device.".into(),
        rows: vec![r], choices: vec![], panel: String::new(), done: u.is_some(), task: true, accept: true }
}

/// The footer's Next on this step: Say hello until the uberbot is made.
pub fn next_label(f: &Facts) -> Option<&'static str> { uber(f).is_none().then_some("Say hello") }

/// Say hello / Save: the uberbot made, or the one there renamed and recoloured. Some(patch) when it went to the hub, Err(patch) refused.
fn go(f: &Facts, session: &Value, ask: &mut Ask) -> Result<Value, Value> {
    let (name, look, color) = shown(f, session);
    let u = uber(f);
    if let Some(e) = name_problem(f, &name, u.map(|(id, _)| id)) { return Err(json!({"drafts": {"@ob.uber.err": e}})) }
    match u {
        None => {
            let mut p = json!({"name": name, "uber": true, "color": color});
            if let Some(l) = look { p["look"] = json!(l) }
            ask.send("bots.create", p);
            Ok(json!({"drafts": {"@ob.uber.err": "", "@ob.uber.made": "1"}}))
        }
        Some((id, _)) => {
            let mut p = json!({"bot": id, "name": name, "color": color});
            match look { Some(l) => p["look"] = json!(l), None => p["auto"] = json!(true) }
            ask.send("bots.edit", p);
            Ok(json!({"drafts": {"@ob.uber.err": "", "@ob.uber.color": Value::Null, "@ob.uber.look": Value::Null, "@f.obubername": Value::Null}}))
        }
    }
}

/// Next on this step: with no uberbot yet it is Say hello (the step stays, so the cat's arrival is seen); None: Next goes on as usual.
pub fn next(f: &Facts, session: &Value, ask: &mut Ask) -> Option<Value> {
    if uber(f).is_some() { return None }
    Some(match go(f, session, ask) { Ok(p) | Err(p) => p })
}

/// `ob-uber` presses. None for a value that is not this step's.
pub fn act(f: &Facts, session: &Value, value: &str, ask: &mut Ask) -> Option<Value> {
    let (name, look, _) = shown(f, session);
    Some(match value.split_once('\u{1f}').unwrap_or((value, "")) {
        ("color", "") => json!({"drafts": {"@ob.uber.color": "none", "@ob.uber.err": ""}}),
        ("color", c) => json!({"drafts": {"@ob.uber.color": cat::color_hex(cat::parse_color(c)?), "@ob.uber.err": ""}}),
        ("custom", _) => match cat::parse_color(draft(session, "@f.obubercustom")) {
            Some(c) => json!({"drafts": {"@ob.uber.color": cat::color_hex(c), "@ob.uber.err": ""}}),
            None => json!({"drafts": {"@ob.uber.err": "Type a colour like #6fb3f2."}}),
        },
        ("shuffle", _) => json!({"drafts": {"@ob.uber.look": cat::shuffled(look.unwrap_or_else(|| cat::look_of(&name))).to_string(), "@ob.uber.err": ""}}),
        ("auto", _) => json!({"drafts": {"@ob.uber.look": "auto", "@ob.uber.err": ""}}),
        ("go", _) => match go(f, session, ask) { Ok(p) | Err(p) => p },
        _ => return None,
    })
}

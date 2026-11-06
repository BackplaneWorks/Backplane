//! The new-bot and new-room forms (docs/bots.md; Bend's `Bots.form`/`Rooms.form` in client.bend, the phones' `bot-new`, `bot-create`, `room-new`, `room-pick`, `room-save`, `form-close`):
//! the form is open while the session's scratch `@bnew` / `@rnew` is "1"; its typed fields are `@f.name`, `@f.persona`, `@f.rname` (the `bfield` action), the provider `@bnew.provider`, the
//! members `@rnew.members` (bot ids, one a line). Create and Save are the hub's `bots.create` and `bots.room`. The screen's `newBot` / `newRoom` objects come from here.
use crate::state::State;
use serde_json::{json, Value};

pub struct Out { pub patch: Value, pub cmds: Vec<Value> }
const PROVIDERS: &[&str] = &["claude", "codex", "grok", "muse", "antigravity", "opencode", "pi", "omp"];
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn draft<'a>(session: &'a Value, k: &str) -> &'a str { s(&session["drafts"], k) }
fn raw(id: &str) -> &str { id.rsplit('|').next().unwrap_or(id) }
fn on(session: &Value, k: &str) -> bool { draft(session, k) == "1" }
fn members(session: &Value) -> Vec<String> { draft(session, "@rnew.members").lines().filter(|l| !l.is_empty()).map(String::from).collect() }

fn send(session: &Value, hub: &str, m: &str, p: Value, cmds: &mut Vec<Value>, rid: &mut u64) {
    let _ = session;
    cmds.push(json!({"type": "send", "hub": hub, "text": json!({"id": *rid, "m": m, "p": p}).to_string()}));
    *rid += 1;
}

/// None for what is not a form action.
pub fn act(st: &State, hub: &str, session: &Value, action: &str, value: &str) -> Option<Out> {
    let mut rid = session["rid"].as_u64().unwrap_or(1);
    let mut cmds = vec![];
    let provider = { let p = st.settings.get("agent.provider").map(String::as_str).unwrap_or("claude"); if PROVIDERS.contains(&p) { p } else { "claude" } };
    let patch = match action {
        "bot-new" => json!({"drafts": {"@bnew": "1", "@rnew": "", "@f.name": "", "@f.persona": "", "@bnew.provider": provider}}),
        "room-new" => json!({"drafts": {"@rnew": "1", "@bnew": "", "@f.rname": "", "@rnew.members": ""}}),
        "form-close" => match value { "@bnew" => json!({"drafts": {"@bnew": ""}}), "@rnew" => json!({"drafts": {"@rnew": ""}}), "@onew" => json!({"drafts": {"@onew": ""}}), _ => return None },
        "bfield" => {
            let (name, text) = value.split_once('\u{1f}')?;
            match name {
                "name" | "persona" | "rname" | "rpost" | "oname" | "ocron" | "oprompt" | "todonew" => json!({"drafts": {format!("@f.{name}"): text}}),
                n if n.starts_with("todo.") && n.len() > 5 && n.len() < 100 => json!({"drafts": {format!("@f.{n}"): text}}),
                "provider" if PROVIDERS.contains(&text) => json!({"drafts": {"@bnew.provider": text}}),
                _ => return None,
            }
        }
        "room-pick" => {
            let id = raw(value); if id.is_empty() { return None }
            let mut m = members(session);
            if let Some(i) = m.iter().position(|x| x == id) { m.remove(i); } else { m.push(id.to_string()); }
            json!({"drafts": {"@rnew.members": m.join("\n")}})
        }
        "bot-create" if on(session, "@bnew") => {
            let name = draft(session, "@f.name").trim();
            if name.is_empty() { return Some(Out { patch: json!({}), cmds }) }
            send(session, hub, "bots.create", json!({"name": name, "persona": draft(session, "@f.persona"), "provider": draft(session, "@bnew.provider")}), &mut cmds, &mut rid);
            json!({"drafts": {"@bnew": "", "@f.name": "", "@f.persona": ""}})
        }
        "room-save" if on(session, "@rnew") => {
            let m = members(session);
            if m.is_empty() { return Some(Out { patch: json!({}), cmds }) }
            let name = draft(session, "@f.rname").trim();
            send(session, hub, "bots.room", json!({"room": "", "name": if name.is_empty() { "room" } else { name }, "members": m.join("\n")}), &mut cmds, &mut rid);
            json!({"drafts": {"@rnew": "", "@f.rname": "", "@rnew.members": ""}})
        }
        _ => return None,
    };
    let mut patch = patch;
    if !cmds.is_empty() { patch["rid"] = json!(rid); }
    Some(Out { patch, cmds })
}

/// A bot page's routine form (Bend's `Bots.routine.edit`, `Bots.routine.save`; `routine-edit` fills it, `bfield` oname / ocron / oprompt type into it): `bot` is the bot's id,
/// `page` its page (the routines it lists). A schedule the hub reads as cron is sent as `cron`, anything else as `at` (a one-time time): the hub's `bots.routine` says what it cannot read.
pub fn routine_act(page: &Value, bot: &str, hub: &str, session: &Value, action: &str, value: &str) -> Option<Out> {
    let mut rid = session["rid"].as_u64().unwrap_or(1);
    let mut cmds = vec![];
    let patch = match action {
        "routine-edit" => {
            let r = page["routines"].as_array().and_then(|rs| rs.iter().find(|r| s(r, "id") == value));
            if !value.is_empty() && r.is_none() { return None }
            let g = |k: &str| r.map_or("", |r| s(r, k));
            let when = if g("cron").is_empty() { g("label") } else { g("cron") };
            json!({"drafts": {"@onew": "1", "@f.oid": value, "@f.oname": g("name"), "@f.ocron": when, "@f.oprompt": g("prompt")}})
        }
        "routine-save" if on(session, "@onew") => {
            let when = draft(session, "@f.ocron").trim();
            if when.is_empty() { return Some(Out { patch: json!({}), cmds }) }
            let name = draft(session, "@f.oname");
            let key = if crate::bots::clock::Cron::parse(when).is_some() { "cron" } else { "at" };
            let mut p = json!({"bot": bot, "routine": draft(session, "@f.oid"), "name": if name.is_empty() { "routine" } else { name }, "prompt": draft(session, "@f.oprompt"), "on": true});
            p[key] = json!(when);
            send(session, hub, "bots.routine", p, &mut cmds, &mut rid);
            json!({"drafts": {"@onew": "", "@f.oid": "", "@f.oname": "", "@f.ocron": "", "@f.oprompt": ""}, "rid": rid})
        }
        _ => return None,
    };
    Some(Out { patch, cmds })
}
/// The bot page's `routine` object (MBots.routine.form).
pub fn routine_form(session: &Value) -> Value {
    let when = draft(session, "@f.ocron").trim();
    let hint = if crate::bots::clock::Cron::parse(when).is_some() { crate::bots::sched_text::when_cron(when) } else { String::new() };
    json!({"open": on(session, "@onew"), "id": draft(session, "@f.oid"), "name": draft(session, "@f.oname"), "cron": draft(session, "@f.ocron"), "prompt": draft(session, "@f.oprompt"), "hint": hint})
}

/// The screen's `newBot` object: None while the form is shut.
pub fn new_bot(session: &Value) -> Option<Value> {
    if !on(session, "@bnew") { return None }
    let cur = draft(session, "@bnew.provider");
    Some(json!({"hint": "Name", "name": draft(session, "@f.name"), "persona": draft(session, "@f.persona"),
        "providers": PROVIDERS.iter().map(|p| json!({"label": p, "provider": p, "on": *p == cur})).collect::<Vec<_>>()}))
}
/// The screen's `newRoom` object: the bots of this machine to pick from.
pub fn new_room(st: &State, hub: &str, session: &Value) -> Option<Value> {
    if !on(session, "@rnew") { return None }
    let have = members(session);
    let mut bots: Vec<_> = st.bots_nav.iter().filter(|(id, b)| !id.contains('~') && !b.thread.is_empty()).collect();
    bots.sort_by(|(ia, a), (ib, b)| a.created.cmp(&b.created).then(ia.cmp(ib)));
    Some(json!({"name": draft(session, "@f.rname"),
        "picks": bots.iter().map(|(id, b)| json!({"id": format!("{hub}|{id}"), "name": b.name, "on": have.iter().any(|h| h == *id)})).collect::<Vec<_>>()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sess(d: Value) -> Value { json!({"rid": 5, "drafts": d}) }
    #[test] fn the_new_bot_form_opens_takes_fields_and_creates() {
        let st = State::default();
        let o = act(&st, "h", &sess(json!({})), "bot-new", "").unwrap();
        let mut s = sess(json!({})); crate::session::patch::merge(&mut s, &o.patch);
        assert!(new_bot(&s).is_some() && new_room(&st, "h", &s).is_none());
        for (k, v) in [("name", "Rex"), ("persona", "dog"), ("provider", "codex")] { let o = act(&st, "h", &s, "bfield", &format!("{k}\u{1f}{v}")).unwrap(); crate::session::patch::merge(&mut s, &o.patch); }
        let nb = new_bot(&s).unwrap(); assert_eq!(nb["name"], "Rex"); assert!(nb["providers"].as_array().unwrap().iter().any(|p| p["provider"] == "codex" && p["on"] == true));
        let o = act(&st, "h", &s, "bot-create", "").unwrap();
        let req: Value = serde_json::from_str(o.cmds[0]["text"].as_str().unwrap()).unwrap();
        assert_eq!((req["m"].as_str(), req["p"]["name"].as_str(), req["p"]["provider"].as_str(), req["id"].as_u64()), (Some("bots.create"), Some("Rex"), Some("codex"), Some(5)));
        crate::session::patch::merge(&mut s, &o.patch); assert!(new_bot(&s).is_none() && s["rid"] == 6);
    }
    #[test] fn a_room_needs_a_member_and_a_blank_name_is_room() {
        let st = State::default(); let mut s = sess(json!({"@rnew": "1"}));
        assert!(act(&st, "h", &s, "room-save", "").unwrap().cmds.is_empty());
        for id in ["h|b1", "h|b2", "h|b1"] { let o = act(&st, "h", &s, "room-pick", id).unwrap(); crate::session::patch::merge(&mut s, &o.patch); }
        let o = act(&st, "h", &s, "room-save", "").unwrap();
        let req: Value = serde_json::from_str(o.cmds[0]["text"].as_str().unwrap()).unwrap();
        assert_eq!((req["m"].as_str(), req["p"]["name"].as_str(), req["p"]["members"].as_str()), (Some("bots.room"), Some("room"), Some("b2")));
    }
    #[test] fn a_routine_is_filled_saved_as_cron_or_at() {
        let page = json!({"routines": [{"id": "o1", "name": "tidy", "cron": "0 9 * * *", "prompt": "tidy up"}]});
        let mut s = sess(json!({}));
        let o = routine_act(&page, "b1", "h", &s, "routine-edit", "o1").unwrap(); crate::session::patch::merge(&mut s, &o.patch);
        let f = routine_form(&s); assert_eq!((f["open"].as_bool(), f["name"].as_str(), f["cron"].as_str()), (Some(true), Some("tidy"), Some("0 9 * * *")));
        let o = routine_act(&page, "b1", "h", &s, "routine-save", "").unwrap();
        let req: Value = serde_json::from_str(o.cmds[0]["text"].as_str().unwrap()).unwrap();
        assert_eq!((req["m"].as_str(), req["p"]["routine"].as_str(), req["p"]["cron"].as_str(), req["p"]["at"].is_null()), (Some("bots.routine"), Some("o1"), Some("0 9 * * *"), true));
        crate::session::patch::merge(&mut s, &o.patch); assert_eq!(routine_form(&s)["open"], false);
        let mut s = sess(json!({"@onew": "1", "@f.ocron": "2030-01-01T09:00"}));
        let o = routine_act(&page, "b1", "h", &s, "routine-save", "").unwrap(); crate::session::patch::merge(&mut s, &o.patch);
        let req: Value = serde_json::from_str(o.cmds[0]["text"].as_str().unwrap()).unwrap();
        assert_eq!((req["p"]["at"].as_str(), req["p"]["cron"].is_null(), req["p"]["name"].as_str()), (Some("2030-01-01T09:00"), true, Some("routine")));
        assert!(routine_act(&page, "b1", "h", &sess(json!({"@onew": "1"})), "routine-save", "").unwrap().cmds.is_empty());
    }
    #[test] fn a_form_action_with_no_form_open_is_not_ours() {
        let st = State::default(); assert!(act(&st, "h", &sess(json!({})), "bot-create", "").is_none() && act(&st, "h", &sess(json!({})), "form-close", "x").is_none());
    }
}

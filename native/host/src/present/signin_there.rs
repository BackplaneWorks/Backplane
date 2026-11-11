//! Sign in there: one machine's panel of sign-ins (docs/ALWAYS-ON.md, section 3), drawn from the info `signin.there` (settings/accounts_there.rs) by
//! every client: the desktop in Settings, Machines and in setup, the phones through `machines.page`. A pure function of that info; every press is a
//! `machine-signin*` action that goes to the hub as `machines.page.act {action, value, fields}` (fields `sikeys` and `sicode`).
//!
//! Entry points for other pages (setup's always-on step): `rows(link, info)` for a machine, `open_button(link)` for its button, and the
//! actions in ACTIONS. A value is `<link>\u{1f}<provider>[\u{1f}<key>]`.
use serde_json::{json, Value};

/// Every action this panel's buttons send (all through `machines.page.act`).
/// `secret-save` is a masked box's Send: its value is packed (`@signin\u{1f}<link>\u{1f}<provider>`, `@signin-key\u{1f}...`, `@signin-sync`) and the
/// typed text rides in `fields` of that one request (settings/accounts_there.rs `save`); a client never keeps it as a draft.
pub const ACTIONS: &[&str] = &["machine-signin-open", "machine-signin-close", "machine-signin", "machine-signin-shell", "machine-signin-key", "machine-signin-keymode",
    "machine-signin-stop", "machine-signin-install", "machine-signin-check", "machine-signin-sync", "machine-signin-sync-cancel", "secret-save"];
/// The fields its rows carry: both masked boxes (`secret` and `vault`), typed text the client holds until its Send.
#[allow(dead_code)] // for setup's step (lane F), which draws these rows
pub const FIELDS: &[&str] = &["sikeys", "sicode"];

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn btn(label: &str, action: &str, value: &str, on: bool) -> Value { json!({"label": label, "action": action, "value": value, "on": on}) }
fn row(label: &str, note: &str, buttons: Vec<Value>) -> Value { json!({"label": label, "note": note, "buttons": buttons}) }
fn toned(mut r: Value, tone: &str) -> Value { if !tone.is_empty() { r["tone"] = json!(tone) } r }
/// A masked box: what is typed stays in the client (never a draft, never sent back by the hub) and leaves once, with its Send.
fn masked(name: &str, hint: &str) -> Value { json!({"name": name, "hint": hint, "secret": true, "vault": true, "value": ""}) }
fn val(link: &str, prov: &str) -> String { format!("{link}\u{1f}{prov}") }

/// The panel's info as a value (it is kept as JSON text).
pub fn of(info: &Value) -> Value { info["signin.there"].as_str().and_then(|t| serde_json::from_str(t).ok()).unwrap_or(Value::Null) }
/// The button that opens a machine's panel.
pub fn open_button(link: &str) -> Value { btn("Sign in there", "machine-signin-open", link, false) }

/// What one provider's row says and offers. `p` is the machine's answer for it, `name` the machine's name.
pub fn provider_row(link: &str, name: &str, p: &Value, keymode: bool) -> Value {
    let (id, title) = (s(p, "id"), s(p, "title"));
    let v = val(link, id);
    let sess = &p["session"]; let inst = &p["install"];
    if sess["running"] == true {
        let mut note = vec![];
        if !s(sess, "code").is_empty() { note.push(format!("Code: {}", s(sess, "code"))) }
        if !s(p, "hint").is_empty() { note.push(s(p, "hint").to_string()) }
        let lines: Vec<&str> = sess["lines"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        if !lines.is_empty() { note.push(lines.join("\n")) }
        let mut b = vec![];
        if !s(sess, "url").is_empty() { b.push(btn("Open the link", "open-url", s(sess, "url"), true)); b.push(btn("Copy the link", "copy", s(sess, "url"), false)) }
        if !s(sess, "code").is_empty() { b.push(btn("Copy the code", "copy", s(sess, "code"), false)) }
        b.push(btn("This is a key", "machine-signin-keymode", &v, keymode));
        b.push(btn(if keymode { "Send the key" } else { "Send" }, "secret-save", &format!("{}\u{1f}{v}", if keymode { "@signin-key" } else { "@signin" }), false));
        for (k, l) in [("enter", "Enter"), ("up", "↑"), ("down", "↓"), ("esc", "Esc")] { b.push(btn(l, "machine-signin-key", &format!("{v}\u{1f}{k}"), false)) }
        b.push(btn("Stop", "machine-signin-stop", &v, false));
        let mut r = toned(row(&format!("{title}: signing in on {name}"), &note.join("\n"), b), "warn");
        r["fields"] = json!([masked("sikeys", if keymode { "The key: kept in that machine's vault and typed from there" } else { "Type there (a code, an answer), then Send" })]);
        return r
    }
    if s(inst, "state") == "running" { return toned(row(title, &format!("Installing on {name}: {}", s(inst, "note")), vec![]), "warn") }
    let ended = s(sess, "state");
    let after = if ended.starts_with("exited") && ended != "exited 0" { format!(" The last sign-in there ended ({ended}).") } else { String::new() };
    if p["installed"] != true {
        let failed = if s(inst, "state") == "failed" { format!(" The install failed: {}", s(inst, "note")) } else { String::new() };
        let b = if p["installable"] == true { vec![btn("Install there", "machine-signin-install", &v, true)] } else { vec![btn("Open a terminal there", "machine-signin-shell", &v, false)] };
        return toned(row(title, &format!("Not installed on {name}.{failed}"), b), if failed.is_empty() { "dim" } else { "bad" })
    }
    let ver = if s(p, "version").is_empty() { String::new() } else { format!(" ({})", s(p, "version")) };
    let start = |label: &str| btn(label, "machine-signin", &v, false);
    match s(p, "signed") {
        "yes" => toned(row(title, &format!("Signed in on {name}{}.{after}", if s(p, "who").is_empty() { String::new() } else { format!(" as {}", s(p, "who")) }), vec![start("Sign in again")]), "ok"),
        "no" => row(title, &format!("Installed on {name}{ver}, not signed in.{after}"), vec![{ let mut b = start("Sign in there"); b["on"] = json!(true); b }]),
        _ => row(title, &format!("Installed on {name}{ver}. Backplane cannot ask it whether it is signed in.{after}"), vec![start("Sign in there"), btn("Open a terminal there", "machine-signin-shell", &v, false)]),
    }
}

/// The Claude sync token's row: made here, it reaches every linked machine by itself; or the sign-in making it, with its link and the code box.
pub fn sync_row(link: &str, name: &str, local: &Value, there: &Value) -> Value {
    let _ = link;
    let a = &local["attempt"];
    if a["busy"] == true {
        let mut b = vec![]; if !s(a, "url").is_empty() { b.push(btn("Open the link", "open-url", s(a, "url"), true)) }
        if a["paste"] == true { b.push(btn("Send the code", "secret-save", "@signin-sync", false)) }
        b.push(btn("Cancel", "machine-signin-sync-cancel", "", false));
        let note = if s(a, "error").is_empty() { s(a, "text").to_string() } else { format!("{} {}", s(a, "text"), s(a, "error")) };
        let mut r = toned(row("Claude sync token", &note, b), "warn");
        if a["paste"] == true { r["fields"] = json!([masked("sicode", "The code the page shows")]) }
        return r
    }
    let mine: Vec<&Value> = local["sync"].as_array().into_iter().flatten().filter(|x| s(x, "state") == "connected").collect();
    let theirs = |id: &str| there["sync"].as_array().into_iter().flatten().find(|x| s(x, "id") == id).map(|x| s(x, "state").to_string());
    if mine.is_empty() {
        let failed = if a["done"] == false && !s(a, "state").is_empty() && s(a, "state") != "cancelled" { format!(" {}", s(a, "text")) } else { String::new() };
        return row("Claude sync token", &format!("One approval in your browser here. The token reaches {name} and your other linked machines by itself.{failed}"), vec![btn("Make a sync token", "machine-signin-sync", "", true)])
    }
    let lines: Vec<String> = mine.iter().map(|x| {
        let who = if s(x, "email").is_empty() { s(x, "label").to_string() } else { format!("{} ({})", s(x, "label"), s(x, "email")) };
        match theirs(s(x, "id")).as_deref() { Some("connected") => format!("{who}: in use on {name}"), Some(_) => format!("{who}: on {name}, being checked"), None => format!("{who}: on its way to {name}") }
    }).collect();
    let all = mine.iter().all(|x| theirs(s(x, "id")).as_deref() == Some("connected"));
    toned(row("Claude sync token", &lines.join("\n"), vec![]), if all { "ok" } else { "" })
}

/// The panel for machine `link`: nothing when another machine's (or none) is open; else its heading, what went wrong, the sync token and one row a provider.
pub fn rows(link: &str, info: &Value) -> Vec<Value> {
    let p = of(info);
    if s(&p, "link") != link || link.is_empty() { return vec![] }
    let name = if s(&p, "name").is_empty() { "that machine" } else { s(&p, "name") };
    let mut r = vec![row(&format!("Sign in on {name}"), "Each agent signs in there once, run there and shown here. Account files stay on their machine; only Claude sync tokens travel.",
        vec![btn("Check again", "machine-signin-open", link, false), btn("Close", "machine-signin-close", link, false)])];
    if !s(&p, "error").is_empty() { r.push(toned(row("Could not reach it", s(&p, "error"), vec![btn("Try again", "machine-signin-open", link, true)]), "bad")) }
    let data = &p["data"];
    if data["providers"].as_array().is_none() { if s(&p, "error").is_empty() { r.push(toned(row("Asking it", "Checking what is installed there.", vec![]), "dim")) } return r }
    r.push(sync_row(link, name, &p["local"], data));
    let keymode = |id: &str| p["keymode"].as_array().map_or(false, |a| a.iter().any(|x| x.as_str() == Some(id)));
    // (Claude's own sign-in there is for a person who wants a separate account there: the sync token above is the one approval otherwise)
    for pv in data["providers"].as_array().into_iter().flatten() { r.push(provider_row(link, name, pv, keymode(s(pv, "id")))) }
    r
}
/// The open panel's rows, whichever machine it is for (Settings, Machines puts them under the linked machines).
pub fn open_rows(info: &Value) -> Vec<Value> { let p = of(info); rows(s(&p, "link"), info) }
/// Whether the open panel changes by itself now (a sign-in or an install runs there, the sync token's sign-in here): the phones read the page
/// again every 2 s while it does, as the desktop's presenter redraws on the info.
pub fn busy(info: &Value) -> bool { let p = of(info); !s(&p, "link").is_empty() && crate::settings::accounts_there::busy(&p) }

#[cfg(test)]
mod tests {
    use super::*;
    fn info(data: Value, local: Value, error: &str) -> Value { json!({"signin.there": json!({"link":"p1","name":"boat-hub","data":data,"local":local,"error":error}).to_string()}) }
    fn labels(r: &[Value]) -> Vec<String> { r.iter().map(|x| x["label"].as_str().unwrap().to_string()).collect() }
    fn actions(r: &Value) -> Vec<String> { r["buttons"].as_array().unwrap().iter().map(|b| b["action"].as_str().unwrap().to_string()).collect() }
    fn data() -> Value { json!({"providers":[
        {"id":"claude","title":"Claude","installed":true,"version":"2.1.322","signed":"yes","who":"a@b.c","installable":true,"session":null},
        {"id":"codex","title":"Codex","installed":true,"version":"0.167.0","signed":"no","installable":true,"hint":"Open the link","session":{"state":"running","running":true,"url":"https://auth.openai.com/codex/device","code":"ABCD-EFGH","lines":["Enter this code"]}},
        {"id":"agy","title":"Antigravity","installed":true,"signed":"unknown","installable":false,"session":{"state":"exited 1","running":false,"lines":[]}},
        {"id":"grok","title":"Grok","installed":false,"installable":true,"install":{"state":"running","note":"Downloading the installer"}},
        {"id":"muse","title":"Muse","installed":false,"installable":false}],
        "sync":[{"id":"s1","state":"connected"}]}) }

    #[test] fn only_the_open_machines_panel_is_drawn() {
        let i = info(data(), json!({}), "");
        assert!(rows("p2", &i).is_empty()); assert!(rows("", &json!({})).is_empty());
        let r = rows("p1", &i);
        assert_eq!(labels(&r), ["Sign in on boat-hub", "Claude sync token", "Claude", "Codex: signing in on boat-hub", "Antigravity", "Grok", "Muse"]);
        assert_eq!(open_rows(&i).len(), r.len());
        assert!(r.iter().flat_map(|x| x["buttons"].as_array().unwrap()).all(|b| ACTIONS.contains(&b["action"].as_str().unwrap()) || matches!(b["action"].as_str().unwrap(), "open-url" | "copy")), "every press is the panel's or the client's own");
    }
    #[test] fn each_provider_says_signed_in_sign_in_there_or_not_installed() {
        let r = rows("p1", &info(data(), json!({}), ""));
        assert_eq!((r[2]["note"].as_str().unwrap(), r[2]["tone"].as_str().unwrap()), ("Signed in on boat-hub as a@b.c.", "ok"));
        assert_eq!(r[2]["buttons"][0]["value"], "p1\u{1f}claude");
        assert!(r[4]["note"].as_str().unwrap().contains("cannot ask it") && r[4]["note"].as_str().unwrap().contains("ended (exited 1)"), "{}", r[4]["note"]);
        assert_eq!(actions(&r[4]), ["machine-signin", "machine-signin-shell"]);
        assert_eq!(r[5]["note"], "Installing on boat-hub: Downloading the installer");
        assert_eq!((r[6]["note"].as_str().unwrap(), actions(&r[6])), ("Not installed on boat-hub.", vec!["machine-signin-shell".to_string()]), "no installer of its own: a terminal there");
        let mut d = data(); d["providers"][3]["install"] = json!({"state":"failed","note":"Could not download"});
        let r = rows("p1", &info(d, json!({}), "")); assert_eq!((actions(&r[5]), r[5]["tone"].as_str().unwrap()), (vec!["machine-signin-install".to_string()], "bad"));
    }
    #[test] fn a_sign_in_running_there_shows_its_link_code_and_a_box_to_type_in() {
        let r = &rows("p1", &info(data(), json!({}), ""))[3];
        let a = actions(r);
        assert_eq!(&a[..3], ["open-url", "copy", "copy"]); assert!(a.contains(&"secret-save".to_string()) && a.last().unwrap() == "machine-signin-stop");
        assert!(r["note"].as_str().unwrap().starts_with("Code: ABCD-EFGH\nOpen the link\nEnter this code"), "{}", r["note"]);
        assert_eq!(r["fields"][0], json!({"name":"sikeys","hint":"Type there (a code, an answer), then Send","secret":true,"vault":true,"value":""}), "masked, held by the client, never a draft");
        assert!(r["buttons"].as_array().unwrap().iter().any(|b| b["action"] == "secret-save" && b["value"] == "@signin\u{1f}p1\u{1f}codex"));
        let mut i = info(data(), json!({}), ""); let mut p = of(&i); p["keymode"] = json!(["codex"]); i["signin.there"] = json!(p.to_string());
        let k = &rows("p1", &i)[3];
        assert!(k["buttons"].as_array().unwrap().iter().any(|b| b["action"] == "machine-signin-keymode" && b["on"] == true), "the toggle is lit");
        assert!(k["buttons"].as_array().unwrap().iter().any(|b| b["action"] == "secret-save" && b["value"] == "@signin-key\u{1f}p1\u{1f}codex"), "its Send goes through the vault there");
        assert!(r["buttons"].as_array().unwrap().iter().any(|b| b["value"] == "p1\u{1f}codex\u{1f}enter"));
    }
    #[test] fn the_sync_token_is_made_here_and_said_to_reach_that_machine() {
        let none = sync_row("p1", "boat-hub", &json!({"sync":[]}), &json!({}));
        assert_eq!(actions(&none), ["machine-signin-sync"]);
        let making = sync_row("p1", "boat-hub", &json!({"attempt":{"busy":true,"paste":true,"url":"https://claude.com/x","text":"Open the link and sign in."}}), &json!({}));
        assert_eq!(actions(&making), ["open-url", "secret-save", "machine-signin-sync-cancel"]); assert_eq!((making["fields"][0]["name"].as_str(), making["fields"][0]["vault"].as_bool()), (Some("sicode"), Some(true)));
        let local = json!({"sync":[{"id":"s1","label":"Claude sync 1","email":"a@b.c","state":"connected"}]});
        let going = sync_row("p1", "boat-hub", &local, &json!({"sync":[]})); assert_eq!(going["note"], "Claude sync 1 (a@b.c): on its way to boat-hub");
        let there = sync_row("p1", "boat-hub", &local, &json!({"sync":[{"id":"s1","state":"connected"}]}));
        assert_eq!((there["note"].as_str().unwrap(), there["tone"].as_str().unwrap()), ("Claude sync 1 (a@b.c): in use on boat-hub", "ok"));
    }
    #[test] fn busy_only_while_the_open_panel_runs_something() {
        assert!(busy(&info(data(), json!({}), "")), "Codex signs in there");
        let mut d = data(); d["providers"][1]["session"]["running"] = json!(false); d["providers"][3]["install"] = json!(null);
        assert!(!busy(&info(d, json!({}), ""))); assert!(!busy(&json!({})));
    }
    #[test] fn a_machine_that_cannot_be_reached_says_so_and_offers_to_try_again() {
        let r = rows("p1", &info(Value::Null, json!({}), "that machine is away"));
        assert_eq!(labels(&r), ["Sign in on boat-hub", "Could not reach it"]); assert_eq!(r[1]["tone"], "bad");
        let r = rows("p1", &info(Value::Null, json!({}), "")); assert_eq!(labels(&r), ["Sign in on boat-hub", "Asking it"]);
    }
}

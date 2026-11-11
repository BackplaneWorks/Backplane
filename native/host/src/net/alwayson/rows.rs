//! The always-on rows, a pure function of the info `alwayson` (mod.rs `Shared::view`): setup's step 4 (present/onboard_alwayson.rs) and Settings,
//! Machines draw the same ones. Row shape: `{label, note, buttons:[{label, action, value, on}], fields?, tone?}`. Presses are `alwayson-*` (to the
//! hub as `alwayson.act`), `secret-save` (the client puts the key straight into the vault, as on the API keys page), `machine-*` (Settings,
//! Machines' own: sign in there is lane G's `machine-signin-open`, the move is `machine-home`), and the clients' own `open-url` and `copy`.
use serde_json::{json, Value};

fn btn(label: &str, action: &str, value: &str, on: bool) -> Value { json!({"label": label, "action": action, "value": value, "on": on}) }
fn row(label: &str, note: &str, buttons: Vec<Value>) -> Value { json!({"label": label, "note": note, "buttons": buttons}) }
fn tone(mut r: Value, t: &str) -> Value { r["tone"] = json!(t); r }
fn field(name: &str, hint: &str, value: &str) -> Value { json!({"name": name, "hint": hint, "secret": false, "value": value}) }
/// The masked box: the client sends it with `secret.set` (name, purpose) and the value never reaches a draft or a log.
fn vault_box(name: &str, purpose: &str, hint: &str) -> (Value, Value) {
    (json!({"name": "sval", "hint": hint, "secret": true, "vault": true, "value": ""}), btn("Save", "secret-save", &format!("{name}\u{1f}{purpose}\u{1f}"), true))
}
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn hours(h: f64) -> String { let n = h.round() as i64; if n >= 1000 { format!("{},{:03}", n / 1000, n % 1000) } else if h < 10.0 { format!("{h:.1}") } else { n.to_string() } }
fn day(iso: &str) -> &str { iso.get(..10).unwrap_or(iso) }

/// The view from the info line ("" or broken: nothing known yet).
pub fn view_of(info: &Value) -> Value { info["alwayson"].as_str().and_then(|t| serde_json::from_str(t).ok()).unwrap_or(Value::Null) }

/// Sub-step 1, the account: the account's line, or the ways to connect it.
pub fn account_rows(v: &Value) -> Vec<Value> {
    let a = &v["account"];
    let mut r = vec![];
    if a.is_object() && !s(a, "email").is_empty() {
        let plan = if s(a, "plan").is_empty() { String::new() } else { format!("{} plan. ", s(a, "plan")) };
        let left = if a["hours"].as_f64().unwrap_or(0.0) > 0.0 { format!("{} of {} plan hours left until {}.", hours(a["hours_left"].as_f64().unwrap_or(0.0)), hours(a["hours"].as_f64().unwrap_or(0.0)), day(s(a, "period_end"))) } else { String::new() };
        r.push(row(&format!("Boat account: {}", s(a, "email")), &format!("{plan}{left} Boat bills this account; Backplane does not."), vec![btn("Forget", "alwayson-forget", "", false)]));
        if a["can_start"] == false { r.push(tone(row("Boat will not start a sandbox now", s(a, "blocked"), vec![btn("Open Boat", "open-url", "https://boat.dev/dashboard?tab=billing", false)]), "warn")) }
        let lacks: Vec<&str> = a["missing"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        if !lacks.is_empty() {
            let what = if lacks.contains(&"sandbox.create") { "This key cannot make a Boat" } else { "This key cannot run an always-on Boat" };
            r.push(tone(row(what, &format!("It lacks {}. Make a key with the command below, or on boat.dev, Dashboard, API Keys, with these actions: {}. Then Forget this one and paste the new key.", lacks.join(", "), super::boat::ACTIONS.join(", ")),
                vec![btn("Copy the command", "copy", &super::boat::key_command(), true), btn("Forget this key", "alwayson-forget", "", false)]), "bad"));
        } else if a["actions"].as_array().map_or(false, |x| x.iter().any(|v| v == "*")) && v["login_err"].as_str().map_or(true, str::is_empty) && a["session"] != true {
            r.push(tone(row("This key can do everything on your account", "The always-on Boat needs less. A key with only what it needs is safer: make one with the command below, then Forget this one and paste the new key.",
                vec![btn("Copy the command", "copy", &super::boat::key_command(), false)]), "warn"));
        }
        return r
    }
    if v["key"] == true && v["account_err"].as_str().map_or(true, str::is_empty) { return vec![row("Boat account", "Reading your account.", vec![])] }
    if let Some(e) = v["account_err"].as_str().filter(|e| !e.is_empty()) { r.push(tone(row("Boat account", e, vec![btn("Try again", "alwayson-account", "", false), btn("Forget the key", "alwayson-forget", "", false)]), "bad")) }
    let l = &v["login"];
    if l.is_object() && !s(l, "url").is_empty() {
        r.push(row("Finish signing in to Boat", &format!("Open the page and sign in{}. This page carries on by itself.", if s(l, "code").is_empty() { String::new() } else { format!("; Boat shows the code {}", s(l, "code")) }),
            vec![btn("Open the sign-in page", "open-url", s(l, "url"), true), btn("Cancel", "alwayson-login-cancel", "", false)]));
    } else {
        r.push(row("Connect your Boat account", "Your own Boat account runs the always-on machine, and Boat bills it. Sign in, or paste an API key below.",
            vec![btn("Sign in with GitHub", "alwayson-login", "github", true), btn("Sign in with Google", "alwayson-login", "google", false)]));
    }
    if let Some(e) = v["login_err"].as_str().filter(|e| !e.is_empty()) { r.push(tone(row("Boat sign-in", e, vec![]), "warn")) }
    let (f, b) = vault_box(super::boat::KEY, "Boat account for the always-on machine", "Boat API key, boat_...");
    let mut k = row("Or paste an API key", &format!("Make a key with only what the always-on Boat needs, with Boat's command: {}. Or on boat.dev, Dashboard, API Keys: New key, expiry a year, with these actions: {}. It is kept in this computer's vault.", super::boat::key_command(), super::boat::ACTIONS.join(", ")),
        vec![b, btn("Copy the command", "copy", &super::boat::key_command(), false), btn("Open Boat", "open-url", "https://boat.dev/dashboard?tab=api-keys", false)]);
    k["fields"] = json!([f]);
    r.push(k);
    r
}

/// Sub-step 2, before it is made: sizes with their cost, the memory it needs, dictation there, how it is reached, Make it.
pub fn make_rows(v: &Value) -> Vec<Value> {
    let mut r = vec![];
    for z in v["sizes"].as_array().into_iter().flatten() {
        let on = z["picked"] == true;
        let mut x = row(s(z, "line").split(':').next().unwrap_or(""), s(z, "line").split_once(": ").map_or("", |p| p.1), vec![btn(if on { "Chosen" } else { "Choose" }, "alwayson-size", s(z, "id"), on)]);
        if z["fits"] == false { x["note"] = json!(format!("{} Too small for what it would run.", s(&x, "note"))); x["tone"] = json!("warn") }
        r.push(x);
    }
    r.push(row("Memory", s(v, "need"), vec![]));
    let d = v["dictation"] != false;
    r.push(row("Hear dictation there", "Installs local dictation on the Boat, so phones and the ring are heard there. About 0.5 GB to download, and 0.7 to 1.2 GB of memory while it listens.",
        vec![btn("On", "alwayson-dictation", "on", d), btn("Off", "alwayson-dictation", "off", !d)]));
    if v["network"] == "tailnet" {
        r.push(row("Reached over your tailnet", "This computer is on Tailscale: the Boat joins your tailnet, so only your devices reach it.", vec![btn("Use Boat's public address instead", "alwayson-network", "public", false)]));
        if v["ts_key"] != true {
            let (f, b) = vault_box(super::TS_KEY, "Tailscale auth key for the always-on Boat", "Tailscale auth key, tskey-auth-...");
            let mut k = row("Tailscale auth key", "Make one in Tailscale's admin console, Settings, Keys: not reusable, pre-approved. It is sent to the Boat once and deleted there.", vec![b, btn("Open Tailscale", "open-url", "https://login.tailscale.com/admin/settings/keys", false)]);
            k["fields"] = json!([f]);
            r.push(k);
        }
    } else {
        r.push(row("Reached at Boat's public address", "A fixed https address on boat.dev. Anyone can reach it, so everything but pairing and hooks needs this computer's or a paired device's credential.",
            vec![if v["tailnet"] == true { btn("Use my tailnet", "alwayson-network", "tailnet", false) } else { Value::Null }].into_iter().filter(|b| !b.is_null()).collect()));
    }
    let lacks = v["account"]["missing"].as_array().map_or(false, |m| !m.is_empty());
    let ready = v["account"]["email"].as_str().map_or(false, |e| !e.is_empty()) && (v["network"] != "tailnet" || v["ts_key"] == true) && !lacks;
    let why = if lacks { "This key cannot make and run the Boat: see above." } else if v["network"] == "tailnet" && v["ts_key"] != true { "Paste the Tailscale auth key first." } else { "Connect your Boat account first." };
    r.push(row("Make it", if ready { "Takes about five minutes. You can leave this page; it carries on." } else { why }, if ready { vec![btn("Make it", "alwayson-make", "", true)] } else { vec![] }));
    r
}

/// The run: one row with its steps, and where it failed with Try again and Delete it.
pub fn progress_rows(v: &Value) -> Vec<Value> {
    let steps = v["steps"].as_array().cloned().unwrap_or_default();
    if steps.is_empty() { return vec![] }
    let mark = |st: &str| match st { "done" => "✓", "run" => "…", "fail" => "✗", "skip" => "-", _ => "·" };
    let line = steps.iter().map(|x| format!("{} {}", mark(s(x, "state")), s(x, "label"))).collect::<Vec<_>>().join("   ");
    let failed = steps.iter().find(|x| x["state"] == "fail");
    let running = v["running"] == true;
    let title = if running { "Making your always-on Boat" } else if failed.is_some() { "Your always-on Boat is not finished" } else { "Your always-on Boat is ready" };
    let mut r = vec![row(title, &line, vec![])];
    if let Some(f) = failed {
        let mut b = vec![btn("Try again", "alwayson-retry", "", true)];
        if !s(&v["boat"], "id").is_empty() { b.push(btn(if v["busy"] == "delete" { "Delete it: press again" } else { "Delete it" }, "alwayson-delete", "", false)) }
        r.push(tone(row(&format!("Failed at {}", s(f, "label")), s(f, "note"), b), "bad"));
        if v["busy"] == "delete" { r.push(row("Delete it", &delete_note(v), vec![btn("Cancel", "alwayson-cancel", "", false)])) }
    } else if let Some(x) = steps.iter().find(|x| x["state"] == "run") { if !s(x, "note").is_empty() { r[0]["note"] = json!(format!("{line}\n{}", s(x, "note"))) } }
    r
}
fn delete_note(v: &Value) -> String {
    format!("Deletes the Boat {} and everything on it: its hub's home, and the bots and threads there. Its link to this computer goes too, and Boat stops billing for it.", s(&v["boat"], "id"))
}

/// Sub-steps 3 and 4 once a machine is linked: sign it in to the agents (present/signin_there.rs: its button opens that machine's panel),
/// move the uberbot there (`home.move`, net/homemove.rs).
pub fn after_rows(link: &str, name: &str) -> Vec<Value> {
    if link.is_empty() { return vec![] }
    vec![
        row("Sign it in to your agents", &format!("{name} signs in to Claude, Codex and the others on its own. Account files never travel."), vec![crate::present::signin_there::open_button(link)]),
        row("Move your uberbot there", "One press: Bub, the bots, rooms, reminders and phone alerts move, and Bub's chat shows the move.", vec![btn(&format!("Move it to {name}"), "machine-home", link, false)]),
    ]
}

/// Step 4's "An always-on Boat": the account, then making it (or its progress), then the two sub-steps after it.
pub fn boat_rows(v: &Value) -> Vec<Value> {
    let mut r = account_rows(v);
    let b = &v["boat"];
    let made = b["made"] == true;
    let prog = progress_rows(v);
    if !prog.is_empty() { r.extend(prog) } else if !made && v["account"]["email"].as_str().map_or(false, |e| !e.is_empty()) { r.extend(make_rows(v)) }
    if made { r.extend(status_rows(v, crate::hub::now_ms() / 1000)) }
    r.extend(after_rows(s(b, "link"), s(b, "name")));
    r
}

/// Settings, Machines: the always-on Boat's status, build, cost this month and plan hours, Resize, Restart, Delete (two presses).
pub fn status_rows(v: &Value, now_s: u64) -> Vec<Value> {
    let b = &v["boat"]; if s(b, "id").is_empty() { return vec![] }
    let st = &v["status"]; let c = &v["cost"]; let a = &v["account"];
    let state = match (s(st, "hub"), s(st, "state")) { ("up", _) => "Running".to_string(), ("down", "archived") => "Stopped".into(), ("down", "") => "Not answering".into(), ("down", x) => format!("Not answering, Boat says {x}"), _ => "Checking".into() };
    let size = if s(st, "size").is_empty() { s(b, "size") } else { s(st, "size") };
    let mut note = format!("{state} · {} · {}", size, if s(b, "network") == "tailnet" { "on your tailnet" } else { "public address" });
    if !s(b, "build").is_empty() { note.push_str(&format!(" · build {}", s(b, "build"))) }
    if let Some(d) = c["dollars"].as_f64() { note.push_str(&format!(" · ${d:.2} this month, {} h", hours(c["hours"].as_f64().unwrap_or(0.0)))) }
    if a["hours"].as_f64().unwrap_or(0.0) > 0.0 { note.push_str(&format!(" · {} plan hours left", hours(a["hours_left"].as_f64().unwrap_or(0.0)))) }
    if !s(b, "url").is_empty() { note.push_str(&format!(" · {}", s(b, "url"))) }
    let armed = v["busy"] == "delete";
    let mut r = vec![row(&format!("Always-on: {}", if s(b, "name").is_empty() { s(b, "id") } else { s(b, "name") }), &note,
        vec![btn("Restart", "alwayson-restart", "", false), btn(if armed { "Delete it: press again" } else { "Delete" }, "alwayson-delete", "", armed)])];
    // bp-hub, made by hand before this existed: shown, never driven from here (scripts/boat-hub.sh)
    if b["adopted"] == true { r[0]["buttons"] = json!([]); r[0]["note"] = json!(format!("{} · made by hand: scripts/boat-hub.sh runs it", s(&r[0], "note"))); return r }
    let down = v["down_since"].as_u64().unwrap_or(0);
    if down > 0 && now_s >= down + 900 { r[0]["tone"] = json!("bad"); r[0]["note"] = json!(format!("{note} · down for {} min", (now_s - down) / 60)) }
    if armed { r.push(row("Delete it", &delete_note(v), vec![btn("Cancel", "alwayson-cancel", "", false)])) }
    let busy = s(v, "busy");
    if let Some(to) = busy.strip_prefix("resize:") { r.push(row("Resizing", &format!("Stopping it and starting it again as {to}. The hub there is away for a minute or two."), vec![])) }
    else {
        let cur = size;
        let chips: Vec<Value> = super::sizes::SIZES.iter().map(|z| btn(z.label, "alwayson-resize", z.id, z.id == cur)).collect();
        r.push(row("Resize", "Stops the Boat and starts it again on the new size; its disk stays.", chips));
    }
    if !s(v, "note").is_empty() { r.push(row("", s(v, "note"), vec![])) }
    r
}

/// Step 4's "Another machine of yours": install there, pair by address and code (or pick a linked machine), and what it is.
/// `fname`: the field name a logical field is drawn with (setup keeps `obaon*` drafts, Settings `aon*`); `draft` what was typed.
pub fn other_rows(picked: &Value, linked: &Value, fname: &dyn Fn(&str) -> String, draft: &dyn Fn(&str) -> String) -> Vec<Value> {
    let mut r = vec![row("Install Backplane there", &format!("On that machine: {}. Then open Settings, Machines, Make a code.", super::other::INSTALL), vec![btn("Copy", "copy", super::other::INSTALL, false)])];
    let mut p = row("Pair with it", "Its address and the code it shows. You approve the request on that machine.", vec![btn("Pair", "alwayson-other-pair", "", true)]);
    p["fields"] = json!([field(&fname("addr"), "Address (https://box.example.ts.net or http://box:3787)", &draft("addr")), field(&fname("code"), "Code", &draft("code"))]);
    r.push(p);
    let chosen = s(picked, "link");
    for l in linked.as_array().into_iter().flatten().filter(|l| s(l, "state") == "paired") {
        let on = s(l, "id") == chosen;
        let note = if !on { format!("{} · linked", s(l, "url")) } else if picked["server"] == true { "Always-on: it runs as a server.".into() } else if picked["asked"] == false { "It did not answer: it runs while that machine is on.".into() } else { "It runs while that machine is on.".into() };
        r.push(row(s(l, "name"), &note, vec![btn(if on { "Chosen" } else { "Use this one" }, "alwayson-other-pick", s(l, "id"), on)]));
    }
    if !chosen.is_empty() { r.extend(after_rows(chosen, s(picked, "name"))) }
    r
}

/// This machine's own switch: it stays on (other machines then call it always-on).
pub fn stays_on_row(on: bool, systemd: bool) -> Value {
    if systemd { return row("This machine stays on", "Its hub runs as a system service, so other machines can use it as the always-on one.", vec![]) }
    row("This machine stays on", "Say so when this computer never sleeps (a home server): other machines can then use it as the always-on one.", vec![btn("On", "alwayson-other-stayson", "on", on), btn("Off", "alwayson-other-stayson", "off", !on)])
}

/// Settings, Machines' always-on part.
pub fn machines_rows(info: &Value, stays_on: bool) -> Vec<Value> {
    let v = view_of(info);
    let mut r = vec![];
    if v.is_object() {
        if s(&v["boat"], "id").is_empty() && v["steps"].as_array().map_or(true, |a| a.is_empty()) {
            r.push(row("Always-on machine", "None. An always-on Boat on your own Boat account keeps your uberbot, reminders, bots and phone alerts running while this computer sleeps.", vec![]));
            r.extend(account_rows(&v));
            if v["account"]["email"].as_str().map_or(false, |e| !e.is_empty()) { r.extend(make_rows(&v)) }
        } else {
            r.extend(progress_rows(&v));
            if v["boat"]["made"] == true { r.extend(status_rows(&v, crate::hub::now_ms() / 1000)) }
            r.extend(after_rows(s(&v["boat"], "link"), s(&v["boat"], "name")));
        }
    }
    r.push(stays_on_row(stays_on, super::other::under_systemd()));
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view(extra: Value) -> Value {
        let mut v = json!({"key": false, "account": null, "account_err": "", "login": null, "login_err": "", "sizes": super::super::sizes::list(0.036, true, 2, &[], ""),
            "need": super::super::sizes::need_line(true, 2), "dictation": true, "network": "tailnet", "tailnet": true, "ts_key": false, "steps": [], "running": false, "boat": null, "status": null, "down_since": 0, "cost": null, "busy": "", "note": ""});
        for (k, x) in extra.as_object().unwrap() { v[k] = x.clone() }
        v
    }
    fn acts(r: &[Value]) -> Vec<String> { r.iter().flat_map(|x| x["buttons"].as_array().cloned().unwrap_or_default()).map(|b| format!("{}:{}", s(&b, "action"), s(&b, "value"))).collect() }
    #[test] fn with_no_account_it_offers_the_sign_in_and_the_masked_key_box() {
        let r = boat_rows(&view(json!({})));
        let a = acts(&r);
        assert!(a.contains(&"alwayson-login:github".to_string()) && a.contains(&"secret-save:boat.api_key\u{1f}Boat account for the always-on machine\u{1f}".to_string()), "{a:?}");
        let f = r.iter().find_map(|x| x["fields"].as_array().cloned()).unwrap();
        assert_eq!((f[0]["secret"].as_bool(), f[0]["vault"].as_bool(), f[0]["value"].as_str()), (Some(true), Some(true), Some("")), "the key is never in a draft");
        assert!(!a.iter().any(|x| x.starts_with("alwayson-make")), "nothing to make before the account");
    }
    #[test] fn a_pasted_key_that_cannot_create_says_so_before_make_it() {
        let acc = json!({"email": "a@b.c", "hours": 10.0, "hours_left": 5.0, "can_start": true, "missing": ["sandbox.create", "sandbox.stop", "sandbox.resume", "sandbox.delete-own"], "actions": ["sandbox.read", "exec"]});
        let r = boat_rows(&view(json!({"account": acc, "ts_key": true})));
        let bad = r.iter().find(|x| x["tone"] == "bad").expect("a red row");
        assert_eq!(s(bad, "label"), "This key cannot make a Boat"); assert!(s(bad, "note").starts_with("It lacks sandbox.create, sandbox.stop"));
        assert!(acts(&r).contains(&format!("copy:{}", super::super::boat::key_command())));
        assert!(!acts(&r).contains(&"alwayson-make:".to_string()), "no Make it with that key");
        let ok = json!({"email": "a@b.c", "hours": 10.0, "hours_left": 5.0, "can_start": true, "missing": [], "actions": super::super::boat::ACTIONS});
        let r = boat_rows(&view(json!({"account": ok, "ts_key": true}))); assert!(acts(&r).contains(&"alwayson-make:".to_string()) && !r.iter().any(|x| x["tone"] == "bad" || x["tone"] == "warn"));
        let admin = json!({"email": "a@b.c", "can_start": true, "missing": [], "actions": ["*"]});
        let r = boat_rows(&view(json!({"account": admin, "ts_key": true}))); assert!(r.iter().any(|x| x["tone"] == "warn" && s(x, "label").contains("everything")), "an admin key is usable but said to be more than needed");
        let none = boat_rows(&view(json!({})));
        assert!(none.iter().any(|x| s(x, "note").contains("boat api-key create backplane-always-on --ttl 365d --actions sandbox.create")), "the key row says how to make the key");
    }
    #[test] fn a_failed_sign_in_says_so_and_the_key_box_stays() {
        let r = boat_rows(&view(json!({"login_err": "Boat's sign-in did not start: Boat answered 404. Paste an API key instead."})));
        assert!(r.iter().any(|x| x["tone"] == "warn" && s(x, "note").contains("Paste an API key instead")));
        assert!(acts(&r).iter().any(|a| a.starts_with("secret-save:boat.api_key")));
    }
    #[test] fn with_an_account_it_shows_the_plan_the_sizes_and_the_tailnet_key() {
        let acc = json!({"email": "a@b.c", "plan": "Standard", "hours": 2777.8, "hours_left": 2413.0, "period_end": "2026-12-03T17:19:55.000Z", "types": [], "can_start": true});
        let r = boat_rows(&view(json!({"account": acc})));
        assert_eq!(s(&r[0], "label"), "Boat account: a@b.c"); assert!(s(&r[0], "note").contains("2,413 of 2,778 plan hours left until 2026-12-03"), "{}", r[0]);
        let small = r.iter().find(|x| s(x, "label") == "Small").unwrap(); assert_eq!(s(small, "note"), "2 vCPU, 4 GB, about $13 a month on your Boat plan."); assert_eq!(small["buttons"][0]["on"], true);
        assert!(r.iter().any(|x| s(x, "label") == "Hear dictation there" && x["buttons"][0]["on"] == true), "on by default");
        assert!(acts(&r).iter().any(|a| a.starts_with("secret-save:tailscale.boat")), "the tailnet key is asked in the masked box");
        assert!(!acts(&r).contains(&"alwayson-make:".to_string()), "not before the tailnet key");
        let r = boat_rows(&view(json!({"account": acc, "ts_key": true})));
        assert!(acts(&r).contains(&"alwayson-make:".to_string()));
    }
    #[test] fn the_run_is_one_row_and_a_failure_names_its_step_with_try_again_and_delete_it() {
        let steps = json!([{"id":"create","label":"Make the Boat","state":"done","note":"bx_1"},{"id":"ready","label":"Start it","state":"done","note":""},{"id":"build","label":"Install Backplane","state":"fail","note":"the upload failed: HTTP 500"}]);
        let r = progress_rows(&view(json!({"steps": steps, "boat": {"id": "bx_1"}})));
        assert_eq!(s(&r[0], "note"), "✓ Make the Boat   ✓ Start it   ✗ Install Backplane");
        assert_eq!(s(&r[1], "label"), "Failed at Install Backplane"); assert_eq!(r[1]["tone"], "bad");
        assert_eq!(acts(&r[1..2]), ["alwayson-retry:", "alwayson-delete:"]);
    }
    #[test] fn the_made_boat_shows_status_cost_and_hours_with_resize_restart_and_a_two_press_delete() {
        let v = view(json!({"account": {"email": "a@b.c", "hours": 2777.8, "hours_left": 2400.0}, "boat": {"id": "bx_1", "name": "Backplane always-on", "size": "small", "network": "tailnet", "url": "https://bp.ts.net", "made": true, "link": "p1"},
            "status": {"hub": "up", "state": "ready", "size": "small"}, "cost": {"dollars": 1.234, "hours": 68.5}}));
        let r = status_rows(&v, 1000);
        assert_eq!(s(&r[0], "note"), "Running · small · on your tailnet · $1.23 this month, 69 h · 2,400 plan hours left · https://bp.ts.net");
        assert_eq!(acts(&r[..1]), ["alwayson-restart:", "alwayson-delete:"]);
        assert!(acts(&r).contains(&"alwayson-resize:large".to_string()));
        let mut armed = v.clone(); armed["busy"] = json!("delete");
        let r = status_rows(&armed, 1000); assert_eq!(s(&r[0]["buttons"][1], "label"), "Delete it: press again"); assert!(s(&r[1], "note").contains("Deletes the Boat bx_1 and everything on it"));
        let mut down = v.clone(); down["status"]["hub"] = json!("down"); down["down_since"] = json!(100);
        assert_eq!(status_rows(&down, 1100)[0]["tone"], "bad");
        let b = boat_rows(&v); assert!(acts(&b).contains(&"machine-signin-open:p1".to_string()) && acts(&b).contains(&"machine-home:p1".to_string()), "sign in there and move it there");
    }
    #[test] fn another_machine_is_always_on_only_when_it_says_it_is_a_server() {
        let linked = json!([{"id": "p1", "name": "nas", "url": "http://nas:3797", "state": "paired"}, {"id": "p2", "name": "x", "url": "u", "state": "unpaired"}]);
        let n = |k: &str| format!("obaon{k}"); let d = |_: &str| String::new();
        let r = other_rows(&Value::Null, &linked, &n, &d);
        assert!(acts(&r).contains(&format!("copy:{}", super::super::other::INSTALL)) && acts(&r).contains(&"alwayson-other-pick:p1".to_string()) && !acts(&r).contains(&"alwayson-other-pick:p2".to_string()));
        assert_eq!(r[1]["fields"][0]["name"], "obaonaddr");
        let r = other_rows(&json!({"link": "p1", "name": "nas", "server": false, "asked": true}), &linked, &n, &d);
        assert!(r.iter().any(|x| s(x, "note") == "It runs while that machine is on."));
        let r = other_rows(&json!({"link": "p1", "name": "nas", "server": true, "asked": true}), &linked, &n, &d);
        assert!(r.iter().any(|x| s(x, "note") == "Always-on: it runs as a server.") && acts(&r).contains(&"machine-home:p1".to_string()));
    }
}

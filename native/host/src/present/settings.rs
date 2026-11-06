//! The Settings dialog's rows (Bend's `Mob.settings.*` in src/mobile/screen.bend, with the desktop menu `Settings.menu.pages` of client.bend) and the actions that drive it
//! (`flag settings`, `settings-page`, `setting k=v`, `hook-url`, `hook-secret`). A page is a pure function of the state's settings and the session's drafts, listed in `PAGES`
//! only once ported: the menu offers exactly those, so no row is a promise the hub cannot keep. Buttons are the same `{label, action, value, on}` the window draws
//! (screens/settings.rs): a chip is a `setting` action with `key=value`, which sends `setting.set` (settings/set.rs).
use crate::state::State;
use serde_json::{json, Value};
use std::sync::OnceLock;

static VERSION: OnceLock<String> = OnceLock::new();
/// The hub's version, said on the About page (set once when the presenter binds to the hub).
pub fn set_version(v: &str) { let _ = VERSION.set(v.to_string()); }

/// The ported pages in the order the desktop menu lists them (Settings.menu.pages): page id and title.
pub const PAGES: &[(&str, &str)] = &[("appearance", "Appearance"), ("threads", "Threads"), ("agents", "Agents"), ("ordering", "Ordering"), ("network", "Network and updates"), ("timezone", "Time zone"), ("about", "About"), ("privacy", "Privacy")];

pub struct Out { pub patch: Value, pub cmds: Vec<Value> }

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn draft<'a>(session: &'a Value, k: &str) -> &'a str { s(&session["drafts"], k) }
fn set<'a>(st: &'a State, k: &str, dflt: &'a str) -> &'a str { st.settings.get(k).map(String::as_str).unwrap_or(dflt) }
pub fn is_open(session: &Value) -> bool { session["open"]["settings"].as_bool().unwrap_or(false) }
pub fn page(session: &Value) -> &str { let p = draft(session, "@settings.page"); if PAGES.iter().any(|(id, _)| *id == p) { p } else { "" } }

// ---- rows ----
fn btn(label: &str, action: &str, value: &str, on: bool) -> Value { json!({"label": label, "action": action, "value": value, "on": on}) }
fn row(label: &str, note: &str, buttons: Vec<Value>) -> Value { json!({"label": label, "note": note, "buttons": buttons}) }
fn row_f(label: &str, note: &str, fields: Vec<Value>, buttons: Vec<Value>) -> Value { json!({"label": label, "note": note, "fields": fields, "buttons": buttons}) }
fn field(name: &str, hint: &str, value: &str) -> Value { json!({"name": name, "hint": hint, "secret": false, "value": value}) }
/// Mob.set.chips: one button per value, `setting key=value`, lit when it is the current one; the label is the value as VLabel.choice says it ("" is "default").
fn chips(values: &[&str], cur: &str, key: &str) -> Vec<Value> {
    values.iter().map(|v| btn(if v.is_empty() { "default" } else { v }, "setting", &format!("{key}={v}"), *v == cur)).collect()
}
fn onoff(on: bool) -> &'static str { if on { "on" } else { "off" } }

fn appearance(st: &State) -> Vec<Value> {
    vec![
        row("Theme", "The window and the web page (phones unset follow their own system).", chips(&["dark", "light"], set(st, "ui.theme", ""), "ui.theme")),
    ]
}

/// Settle.note: the worker-grace sentence with its minutes
fn settle_note(minutes: &str) -> String {
    let n = minutes.trim().parse::<u64>().ok();
    format!("Workers settle after {} idle {} once completed, with no work or interaction pending and their report delivered. Unread personal replies stay visible. History is kept. 0 disables the timer.",
        minutes, if n == Some(1) { "minute" } else { "minutes" })
}
/// Att.mb: files.max.mb clamped to 1..=4000, default 512
fn files_mb(st: &State) -> u64 { set(st, "files.max.mb", "").trim().parse::<u64>().ok().map_or(512, |n| n.clamp(1, 4000)) }
fn url_ok(u: &str) -> bool { (u.starts_with("http://") || u.starts_with("https://")) && u.len() <= 2000 && !u.chars().any(|c| c.is_whitespace() || c.is_control()) }

fn threads(st: &State, session: &Value) -> Vec<Value> {
    let idle = set(st, "settle.idle.minutes", "15");
    let url = if session["drafts"]["@f.whurl"].is_string() { draft(session, "@f.whurl") } else { set(st, "push.webhook.url", "") };
    let sec = if session["drafts"]["@f.whsec"].is_string() { draft(session, "@f.whsec") } else { set(st, "push.webhook.secret", "") };
    let bad = draft(session, "@hook.err") == "bad";
    let mut v = vec![
        row("Worker grace (minutes)", &settle_note(idle), chips(&["0", "5", "10", "15", "30", "60"], idle, "settle.idle.minutes")),
        row("Personal grace (minutes)", "Read, completed personal conversations settle after this quiet period (240 minutes is four hours). 0 disables the timer.", chips(&["0", "60", "120", "240", "480"], set(st, "settle.personal.minutes", "240"), "settle.personal.minutes")),
        row("Short grace for read conversations", "Use the worker grace for personal conversations once their reply is read and the work is complete.", chips(&["on", "off"], onoff(set(st, "settle.done", "off") == "on"), "settle.done")),
        row("Continue after a restart", "Turns a restart cuts short go on by themselves.", chips(&["on", "off"], onoff(set(st, "restart.continue", "on") != "off"), "restart.continue")),
        row("Largest attachment (MB)", &format!("A file sent with a message, or shown from this hub, may be up to {} MB. Files stream from disk, so a large one costs disk space, not memory.", files_mb(st)),
            chips(&["128", "512", "1024", "2048", "4000"], &files_mb(st).to_string(), "files.max.mb")),
        row("Webhook", "Posts when Notify me for would. A settle or a failure, an ask, a merge, a reminder or a note.", vec![]),
        row_f("Address", if bad { "The address must be http or https." } else { "An http or https address. Empty turns the webhook off." }, vec![field("whurl", "https://example.com/hook", url)], vec![btn("Save", "hook-url", "", false)]),
        row_f("Secret file", "A path to the file that holds the secret. Empty uses the hub's default file. The secret itself is never shown.", vec![field("whsec", "Path to the secret file", sec)], vec![btn("Save", "hook-secret", "", false)]),
        row("Notify me for", "Phone and desktop alerts. The rest stays in the app as unread.", vec![]),
    ];
    for (kind, name, note) in [("needs", "Needs me", "Approvals, questions and anything an agent is blocked on until you act."),
        ("settled", "Settled", "Work you gave a thread is finished, failed, or its branch merged. Once per piece of work."),
        ("reminders", "Reminders", "Reminders and digests you scheduled."), ("all", "Everything", "Also every turn end and room exchange, as before.")] {
        let on = match set(st, &format!("notify.{kind}"), "") { "on" => true, "off" => false, _ => kind != "all" };
        v.push(row(name, note, chips(&["on", "off"], onoff(on), &format!("notify.{kind}"))));
    }
    v
}

fn about(st: &State) -> Vec<Value> {
    let v = VERSION.get().map(String::as_str).unwrap_or("");
    let _ = st;
    vec![row(&format!("Backplane {v}"), "Updates are made by the installer or package this build came from.", vec![])]
}

const PROVIDERS: &[&str] = &["claude", "codex", "grok", "muse", "antigravity", "opencode", "pi", "omp"];
/// Settings > Agents, as far as the hub knows it: the default provider and model for new threads and thinking. Provider install and sign-in, the fallback hierarchy and Antigravity's account are not on this page yet.
fn agents(st: &State, info: &Value) -> Vec<Value> {
    let models: Value = info["models"].as_str().and_then(|s| serde_json::from_str(s).ok()).unwrap_or(Value::Null);
    let prov = set(st, "agent.provider", "claude");
    let cli = models["about"][prov]["cli"].as_str().unwrap_or("");
    let model = set(st, "agent.model", "");
    let mut ids: Vec<&str> = vec![""];
    ids.extend(models[prov].as_array().into_iter().flatten().filter_map(Value::as_str).take(12));
    if !ids.contains(&model) { ids.push(model); }
    let label = models["about"][prov]["labels"][if model.is_empty() { "default" } else { model }].as_str().unwrap_or("");
    vec![
        row("Provider and model for new threads", "Each thread keeps its own; change it from the composer.", vec![]),
        row("Provider", &if cli.is_empty() && prov != "claude" { format!("{prov} is not installed") } else { cli.to_string() }, chips(PROVIDERS, prov, "agent.provider")),
        row("Model", &format!("{}: {}", if model.is_empty() { "default" } else { model }, label), chips(&ids, model, "agent.model")),
        row("Show thinking", "Ask agents for their thinking and show it in the thread, folded (Claude, Codex and Pi; a Claude session restarts to take it)", chips(&["on", "off"], set(st, "agent.thinking", "off"), "agent.thinking")),
    ]
}

fn ordering(st: &State) -> Vec<Value> {
    let enabled = set(st, "order.enabled", "on") != "off";
    let mut v = vec![
        row("How it works", "With Ordering on, a project whose design is ready shows Buy it: in the Ready to order bar above the composer, and with the thread's tools. Buy it opens a buy thread. Its agent checks that every board and part is ready, asks you anything missing (quantities, finishes, where to ship), exports the manufacturing files into the project's mfg/ folder and sends them to Backplane's order server, which measures them and prices the exact order: manufacturing (boards, assembly, components and parts, with Backplane's fee included), shipping, and duties and taxes. The quote shows in the thread, and nothing is charged until you press Buy now and pay on Stripe's checkout page. A quote holds for 3 days. Prices are estimates: if the real cost turns out higher, you are asked before anything more is charged and can have a full refund instead; if it is lower, the difference is refunded. Once paid, the thread shows the order's progress (placed, shipped, tracking). With Ordering off, nothing about your projects leaves this machine for ordering. Price estimates add a rough \"from $X\" to Buy it, worked out from each part's size and options without sending any files.", vec![]),
        row("Ordering", "Order boards and parts from Backplane. Off: no prices or uploads.", chips(&["on", "off"], onoff(enabled), "order.enabled")),
    ];
    if enabled {
        v.push(row("Price estimates", "Buy it shows a price from each part's size and options. No files are sent.", chips(&["on", "off"], onoff(set(st, "order.estimates", "on") != "off"), "order.estimates")));
    } else { v.push(row("Price estimates", "Comes with Ordering.", vec![])); }
    v
}

fn network(st: &State, info: &Value) -> Vec<Value> {
    let i = |k: &str| info[k].as_str().unwrap_or("");
    let told = if !i("tailscale").is_empty() { i("tailscale") } else if i("tailnet") == "off" || i("tailnet").is_empty() { "off" } else if i("pair").is_empty() { "unavailable" } else { "reachable" };
    let state = match told { "reachable" => i("tailnet"), "unavailable" => "not connected", t => t };
    let note = match told {
        "off" => format!("Listening on {} only: started with --no-tailscale.", i("listen")),
        "reachable" => "Also on your tailnet: your own devices get in, others need the pairing token.".to_string(),
        "unavailable" => format!("Listening on {} only. Sign in to Tailscale, then restart Backplane.", i("listen")),
        _ => "Looking for Tailscale.".to_string(),
    };
    let link = i("pair");
    let version = VERSION.get().map(String::as_str).unwrap_or("");
    let line = if i("update") == "package" || i("update").is_empty() { "Updated by its package." } else { "Up to date, checked hourly." };
    vec![
        row(&format!("Tailscale: {state}"), &note, vec![]),
        row("Pairing link", if link.is_empty() { "Shown once Backplane listens on your tailnet." } else { link }, vec![]),
        row("Update channel", "Stable takes releases; nightly also takes the daily builds.", chips(&["stable", "nightly"], set(st, "update.channel", "stable"), "update.channel")),
        row(&format!("Backplane {version}"), line, vec![]),
    ]
}

/// Zone.person's source: a zone chosen here, else the phone's, else this machine's
fn zone_of(st: &State) -> (String, &'static str) {
    for (k, src) in [("person.tz", "set"), ("person.tz.device", "phone"), ("tz.machine", "machine")] {
        let v = set(st, k, ""); if !v.is_empty() { return (v.to_string(), src) }
    }
    ("UTC".to_string(), "none")
}
/// Zone.valid, as far as a name can be told from its text: Area/City words, or UTC/GMT; the hub checks the zone itself
fn zone_ok(z: &str) -> bool {
    !z.is_empty() && z.len() <= 64 && z.chars().all(|c| c.is_ascii_alphanumeric() || "/_-+".contains(c)) && (z.contains('/') || matches!(z, "UTC" | "GMT" | "Etc"))
}
fn timezone(st: &State, session: &Value) -> Vec<Value> {
    let (zone, src) = zone_of(st);
    let source = match src { "set" => "set", "phone" => "from your phone", "machine" => "this machine", _ => "nothing reported yet" };
    let auto = !set(st, "person.tz", "").is_empty();
    let from = if !set(st, "person.tz.device", "").is_empty() { "phone" } else if !set(st, "tz.machine", "").is_empty() { "machine" } else { "utc" };
    let tip = match (auto, from) {
        (true, "phone") => "Follow your phone's time zone again", (true, "machine") => "Follow this machine's time zone again", (true, _) => "Go back to UTC until your phone or this machine reports a time zone",
        (false, "phone") => "Automatic: following your phone's time zone", (false, "machine") => "Automatic: following this machine's time zone", (false, _) => "Automatic: UTC until your phone or this machine reports a time zone",
    };
    let typed = draft(session, "@f.tzname");
    let mut v = vec![row_f("Time zone", &format!("{zone} · {source}"), vec![field("tzname", "A zone name, e.g. Europe/Helsinki", typed)], vec![btn("Set", "tz-set", "", true)])];
    if auto { v.push(row("Automatic", tip, vec![btn("Automatic", "tz-auto", "", false)])); } else { v.push(row("Automatic", tip, vec![])); }
    if draft(session, "@tz.err") == "bad" { v.push(row("", "That is not a time zone name (like Europe/Helsinki)", vec![])); }
    v
}

fn privacy(st: &State, info: &Value) -> Vec<Value> {
    let share = set(st, "telemetry.share", ""); let usage = set(st, "telemetry.usage.asked", "");
    let on = (share.is_empty() || share == "on") && usage != "0";
    let id = if on { info["telemetry.id"].as_str().unwrap_or("") } else { "" };
    let mut note = "All telemetry: installs, updates, daily activity, daily counts of threads, messages, model tokens, bot traffic, boards created and completed board orders; random install id, version, package channel, OS, architecture and UTC day. Used for product and aggregate investor metrics. Board/order data is integer counts only: no design paths, hashes, dimensions, layers, part numbers, BOMs, netlists or prices. No content, files, names or credentials. Off disables every category. Local diagnostics stay on this device unless you choose to export them. This build of the hub sends nothing yet. Privacy policy: https://backplane.works/privacy".to_string();
    if !id.is_empty() { note.push_str(&format!(" Install id: {id}")); }
    let n = set(st, "telemetry.reset", "").parse::<u64>().unwrap_or(0) + 1;
    vec![row("Share anonymous usage", &note, vec![btn("On", "setting", "telemetry.share=on", on), btn("Off", "setting", "telemetry.share=off", !on), btn("Reset id", "setting", &format!("telemetry.reset={n}"), false)])]
}

/// The rows shown while the dialog is open: the page menu (no page chosen) or one page, its first row the way back.
pub fn view(st: &State, session: &Value, info: &Value) -> Value {
    let p = page(session);
    let rows = if p.is_empty() {
        PAGES.iter().map(|(id, title)| row(title, "", vec![btn(&format!("{title} ›"), "settings-page", id, false)])).collect()
    } else {
        let title = PAGES.iter().find(|(id, _)| *id == p).map_or("", |(_, t)| *t);
        let mut r = vec![row(title, "", vec![btn("‹ Settings", "settings-page", "", false)])];
        r.extend(match p { "appearance" => appearance(st), "threads" => threads(st, session), "about" => about(st), "agents" => agents(st, info), "ordering" => ordering(st), "network" => network(st, info), "timezone" => timezone(st, session), "privacy" => privacy(st, info), _ => vec![] });
        r
    };
    json!({"rows": rows})
}

// ---- actions ----
fn send(session: &Value, hub: &str, m: &str, p: Value, extra: &mut Vec<Value>, rid: &mut u64) {
    let _ = session;
    extra.push(json!({"type": "send", "hub": hub, "text": json!({"id": *rid, "m": m, "p": p}).to_string()}));
    *rid += 1;
}

/// Ui.act's settings cases. None for what is not Settings'.
pub fn act(hub: &str, session: &Value, action: &str, value: &str) -> Option<Out> {
    let mut rid = session["rid"].as_u64().unwrap_or(1);
    let mut cmds = vec![];
    let patch = match action {
        "flag" if value == "settings" => json!({"open": {"settings": !is_open(session)}}),
        "settings-page" => {
            let p = if PAGES.iter().any(|(id, _)| *id == value) { value } else { "" };
            // opening Threads fills the webhook fields from the saved settings (Settings.seed): the drafts are dropped so the saved values show
            json!({"drafts": {"@settings.page": p, "@f.whurl": Value::Null, "@f.whsec": Value::Null, "@hook.err": "", "@f.tzname": "", "@tz.err": ""}})
        }
        "bfield" => {
            let (name, text) = value.split_once('\u{1f}')?;
            if !matches!(name, "whurl" | "whsec" | "tzname") { return None }
            json!({"drafts": {format!("@f.{name}"): text}})
        }
        "setting" => {
            let (k, v) = value.split_once('=')?;
            if k.is_empty() { return None }
            send(session, hub, "setting.set", json!({"key": k, "value": v}), &mut cmds, &mut rid);
            json!({})
        }
        "hook-url" => {
            let raw = if session["drafts"]["@f.whurl"].is_string() { draft(session, "@f.whurl").trim().to_string() } else { return Some(Out { patch: json!({}), cmds }) };
            if raw.is_empty() || url_ok(&raw) {
                send(session, hub, "setting.set", json!({"key": "push.webhook.url", "value": raw}), &mut cmds, &mut rid);
                json!({"drafts": {"@hook.err": "", "@f.whurl": raw}})
            } else { json!({"drafts": {"@hook.err": "bad"}}) }
        }
        "hook-secret" => {
            if !session["drafts"]["@f.whsec"].is_string() { return Some(Out { patch: json!({}), cmds }) }
            let raw = draft(session, "@f.whsec").trim().to_string();
            send(session, hub, "setting.set", json!({"key": "push.webhook.secret", "value": raw}), &mut cmds, &mut rid);
            json!({"drafts": {"@f.whsec": raw}})
        }
        "tz-set" => {
            let z = draft(session, "@f.tzname").trim().to_string();
            if !zone_ok(&z) { json!({"drafts": {"@tz.err": "bad"}}) }
            else { send(session, hub, "zone.set", json!({"zone": z}), &mut cmds, &mut rid); json!({"drafts": {"@tz.err": "", "@f.tzname": ""}}) }
        }
        "tz-auto" => { send(session, hub, "zone.set", json!({"zone": "auto"}), &mut cmds, &mut rid); json!({"drafts": {"@tz.err": "", "@f.tzname": ""}}) }
        _ => return None,
    };
    let mut patch = patch;
    if rid != session["rid"].as_u64().unwrap_or(1) { patch["rid"] = json!(rid); }
    Some(Out { patch, cmds })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn st(kv: &[(&str, &str)]) -> State { let mut s = State::new(); for (k, v) in kv { s.settings.insert(k.to_string(), v.to_string()); } s }
    fn labels(v: &Value) -> Vec<String> { v["rows"].as_array().unwrap().iter().map(|r| r["label"].as_str().unwrap().to_string()).collect() }

    #[test] fn the_menu_lists_the_ported_pages_each_a_settings_page_button() {
        let v = view(&st(&[]), &json!({"open": {"settings": true}, "drafts": {}}), &json!({}));
        assert_eq!(labels(&v), ["Appearance", "Threads", "Agents", "Ordering", "Network and updates", "Time zone", "About", "Privacy"]);
        assert_eq!(v["rows"][1]["buttons"][0]["action"], "settings-page"); assert_eq!(v["rows"][1]["buttons"][0]["value"], "threads");
    }
    #[test] fn a_page_leads_with_the_way_back_and_marks_the_current_chip() {
        let v = view(&st(&[("settle.idle.minutes", "30"), ("restart.continue", "off")]), &json!({"drafts": {"@settings.page": "threads"}}), &json!({}));
        assert_eq!(v["rows"][0]["buttons"][0]["value"], ""); assert_eq!(v["rows"][0]["buttons"][0]["action"], "settings-page");
        let grace = &v["rows"][1]; assert_eq!(grace["label"], "Worker grace (minutes)");
        let on: Vec<&str> = grace["buttons"].as_array().unwrap().iter().filter(|b| b["on"] == true).map(|b| b["label"].as_str().unwrap()).collect();
        assert_eq!(on, ["30"]); assert!(grace["note"].as_str().unwrap().contains("after 30 idle minutes"));
        let restart = v["rows"].as_array().unwrap().iter().find(|r| r["label"] == "Continue after a restart").unwrap();
        assert_eq!(restart["buttons"][1]["on"], true, "off is lit");
    }
    #[test] fn a_chip_sends_setting_set_with_the_next_rid() {
        let o = act("h", &json!({"rid": 5}), "setting", "ui.theme=light").unwrap();
        assert_eq!(o.cmds[0]["text"], json!(r#"{"id":5,"m":"setting.set","p":{"key":"ui.theme","value":"light"}}"#)); assert_eq!(o.patch["rid"], 6);
        assert_eq!(o.cmds[0]["hub"], "h"); assert!(act("h", &json!({"rid": 5}), "setting", "no-equals").is_none());
    }
    #[test] fn the_page_opens_shuts_and_an_unknown_page_is_the_menu() {
        assert_eq!(act("h", &json!({}), "flag", "settings").unwrap().patch["open"]["settings"], true);
        assert_eq!(act("h", &json!({"open": {"settings": true}}), "flag", "settings").unwrap().patch["open"]["settings"], false);
        assert_eq!(act("h", &json!({}), "settings-page", "nonsense").unwrap().patch["drafts"]["@settings.page"], "");
        assert_eq!(page(&json!({"drafts": {"@settings.page": "voice"}})), "", "an unported page is the menu");
    }
    #[test] fn the_webhook_address_saves_only_when_http_or_empty() {
        let bad = act("h", &json!({"rid": 1, "drafts": {"@f.whurl": "ftp://x"}}), "hook-url", "").unwrap();
        assert!(bad.cmds.is_empty()); assert_eq!(bad.patch["drafts"]["@hook.err"], "bad");
        let ok = act("h", &json!({"rid": 1, "drafts": {"@f.whurl": " https://e.com/h "}}), "hook-url", "").unwrap();
        assert_eq!(ok.cmds.len(), 1); assert!(ok.cmds[0]["text"].as_str().unwrap().contains(r#""value":"https://e.com/h""#));
        let off = act("h", &json!({"rid": 1, "drafts": {"@f.whurl": ""}}), "hook-url", "").unwrap();
        assert_eq!(off.cmds.len(), 1, "empty turns it off");
        assert!(act("h", &json!({"rid": 1, "drafts": {}}), "hook-url", "").unwrap().cmds.is_empty(), "nothing typed, nothing sent");
        let t = view(&st(&[]), &json!({"drafts": {"@settings.page": "threads", "@hook.err": "bad"}}), &json!({}));
        assert!(t["rows"].as_array().unwrap().iter().any(|r| r["note"] == "The address must be http or https."));
    }
    #[test] fn typed_text_is_kept_in_the_session_and_shown_back() {
        let o = act("h", &json!({}), "bfield", "whurl\u{1f}https://a").unwrap();
        assert_eq!(o.patch["drafts"]["@f.whurl"], "https://a");
        let v = view(&st(&[("push.webhook.url", "https://old")]), &json!({"drafts": {"@settings.page": "threads", "@f.whurl": "https://new"}}), &json!({}));
        let r = v["rows"].as_array().unwrap().iter().find(|r| r["label"] == "Address").unwrap();
        assert_eq!(r["fields"][0]["value"], "https://new");
        let saved = view(&st(&[("push.webhook.url", "https://old")]), &json!({"drafts": {"@settings.page": "threads"}}), &json!({}));
        assert_eq!(saved["rows"].as_array().unwrap().iter().find(|r| r["label"] == "Address").unwrap()["fields"][0]["value"], "https://old");
    }
    fn page_rows(s: &State, page: &str, info: Value) -> Vec<Value> { view(s, &json!({"drafts": {"@settings.page": page}}), &info)["rows"].as_array().unwrap().clone() }
    #[test] fn ordering_hides_its_estimates_when_off() {
        let on = page_rows(&st(&[]), "ordering", json!({})); assert_eq!(on[3]["buttons"][0]["on"], true);
        let off = page_rows(&st(&[("order.enabled", "off")]), "ordering", json!({}));
        assert!(off[3]["buttons"].as_array().unwrap().is_empty() && off[3]["note"] == "Comes with Ordering.");
    }
    #[test] fn network_says_what_the_hub_reports_about_tailscale() {
        let r = page_rows(&st(&[]), "network", json!({"tailscale": "off", "listen": "127.0.0.1:3787", "update": "package"}));
        assert_eq!(r[1]["label"], "Tailscale: off"); assert_eq!(r[1]["note"], "Listening on 127.0.0.1:3787 only: started with --no-tailscale.");
        assert_eq!(r[2]["note"], "Shown once Backplane listens on your tailnet.");
        let r = page_rows(&st(&[("update.channel", "nightly")]), "network", json!({"tailscale": "reachable", "tailnet": "m.t.ts.net", "pair": "backplane://x"}));
        assert_eq!(r[1]["label"], "Tailscale: m.t.ts.net"); assert_eq!(r[2]["note"], "backplane://x"); assert_eq!(r[3]["buttons"][1]["on"], true);
    }
    #[test] fn the_zone_says_where_it_comes_from_and_a_bad_name_is_not_sent() {
        let r = page_rows(&st(&[("tz.machine", "Europe/Helsinki")]), "timezone", json!({}));
        assert_eq!(r[1]["note"], "Europe/Helsinki · this machine");
        let bad = act("h", &json!({"rid": 1, "drafts": {"@f.tzname": "Not a zone!"}}), "tz-set", "").unwrap(); assert!(bad.cmds.is_empty() && bad.patch["drafts"]["@tz.err"] == "bad");
        let ok = act("h", &json!({"rid": 1, "drafts": {"@f.tzname": "Europe/Helsinki"}}), "tz-set", "").unwrap();
        assert_eq!(ok.cmds[0]["text"], json!(r#"{"id":1,"m":"zone.set","p":{"zone":"Europe/Helsinki"}}"#));
        assert_eq!(act("h", &json!({"rid": 2}), "tz-auto", "").unwrap().cmds[0]["text"], json!(r#"{"id":2,"m":"zone.set","p":{"zone":"auto"}}"#));
    }
    #[test] fn privacy_reads_the_choice_and_resets_with_the_next_number() {
        let r = page_rows(&st(&[("telemetry.share", "off"), ("telemetry.reset", "3")]), "privacy", json!({"telemetry.id": "abc"}));
        let b = r[1]["buttons"].as_array().unwrap();
        assert_eq!((b[0]["on"].as_bool(), b[1]["on"].as_bool()), (Some(false), Some(true))); assert_eq!(b[2]["value"], "telemetry.reset=4");
        assert!(!r[1]["note"].as_str().unwrap().contains("Install id"), "no id while off");
    }
    #[test] fn agents_offers_the_hubs_models_with_default_first_and_the_current_lit() {
        let info = json!({"models": json!({"claude": ["fable", "opus"], "about": {"claude": {"cli": "", "labels": {"default": "Claude Code's own default", "opus": "Opus 5.5"}}}}).to_string()});
        let r = page_rows(&st(&[("agent.model", "opus")]), "agents", info);
        let m = r[3]["buttons"].as_array().unwrap().iter().map(|b| (b["label"].as_str().unwrap().to_string(), b["on"].as_bool().unwrap())).collect::<Vec<_>>();
        assert_eq!(m, [("default".to_string(), false), ("fable".to_string(), false), ("opus".to_string(), true)]);
        assert_eq!(r[3]["note"], "opus: Opus 5.5"); assert_eq!(r[2]["buttons"][0]["on"], true, "claude is the default provider");
        let r = page_rows(&st(&[("agent.provider", "codex")]), "agents", json!({}));
        assert_eq!(r[2]["note"], "codex is not installed");
    }
}

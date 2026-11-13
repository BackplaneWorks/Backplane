//! The Settings dialog's rows and the actions that drive it
//! (`flag settings`, `settings-page`, `setting k=v`, `hook-url`, `hook-secret`). A page is a pure function of the state's settings and the session's drafts, listed in `PAGES`
//! only once ported: the menu offers exactly those, so no row is a promise the hub cannot keep. Buttons are the same `{label, action, value, on}` the window draws
//! (screens/settings.rs): a chip is a `setting` action with `key=value`, which sends `setting.set` (settings/set.rs).
use crate::state::State;
use serde_json::{json, Value};
use std::sync::OnceLock;

static VERSION: OnceLock<String> = OnceLock::new();
/// The hub's version, said on the About page (set once when the presenter binds to the hub).
pub fn set_version(v: &str) { let _ = VERSION.set(v.to_string()); }
pub fn version() -> &'static str { VERSION.get().map(String::as_str).unwrap_or("") }

/// The ported pages in the order the desktop menu lists them (Settings.menu.pages): page id and title.
pub const PAGES: &[(&str, &str)] = &[("appearance", "Appearance"), ("threads", "Threads"), ("notifications", "Notifications"), ("agents", "Agents"), ("mcp", "MCP servers"), ("writing", "Writing"), ("cad", "CAD"), ("ordering", "Ordering"), ("network", "Network and updates"), ("devices", "Devices"), ("machines", "Machines"), ("voice", "Voice"), ("ring", "Ring"), ("keys", "API keys"), ("timezone", "Time zone"), ("rules", "Rules and task board"), ("usage", "Usage"), ("setup", "Setup"), ("import", "Import from 0.14"), ("about", "About"), ("privacy", "Privacy"), ("developer", "Developer")];

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

/// The channel a stored value means: 0.14's "nightly" is beta now, and the unknown is stable (`update::rel::channel`, the rule the updater itself follows).
fn update_channel(raw: &str) -> &'static str { match raw.trim() { "beta" | "nightly" => "beta", _ => "stable" } }
/// What to say about updates, and whether Check now is offered, from info `update.status` (`update/svc.rs`). One sentence for every client: the phones get it in `update.status`.
pub fn update_line(status: &str) -> (String, bool) {
    let v: Value = serde_json::from_str(status).unwrap_or_default();
    let g = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let (avail, note) = (g("available"), g("note"));
    match g("state").as_str() {
        "" | "package" => ("Updated by its package.".to_string(), false),
        "off" => ("Updates are off on this machine.".to_string(), false),
        "checking" => ("Checking for updates.".to_string(), false),
        "downloading" => (format!("Downloading {avail}."), false),
        "waiting" => (format!("{avail} installs when running turns finish."), false),
        "installing" => (format!("Installing {avail}. Backplane restarts once it is ready."), false),
        "failed" => (if note.is_empty() { "The last update failed.".to_string() } else { note }, true),
        _ => (if note.is_empty() { "Up to date, checked hourly.".to_string() } else { note }, true),
    }
}
fn update_row(version: &str, status: &str) -> Value {
    let (line, check) = update_line(status);
    // waiting on running turns: Update now goes ahead anyway (update.now; the turns continue after the restart)
    let waiting = serde_json::from_str::<Value>(status).map_or(false, |v| v["state"] == "waiting");
    let mut b = if check { vec![btn("Check now", "update-check", "", false)] } else { vec![] };
    if waiting { b.push(btn("Update now", "update-now", "", false)); }
    row(&format!("Backplane {version}"), &line, b)
}

fn appearance(st: &State) -> Vec<Value> {
    vec![
        row("Theme", "The window and the web page (phones unset follow their own system).", chips(&["dark", "light"], set(st, "ui.theme", ""), "ui.theme")),
        row("Thread list", "Projects groups threads by project; active lists what is going on, what needs you first.", chips(&["projects", "active"], if set(st, "sidebar.view", "") == "active" { "active" } else { "projects" }, "sidebar.view")),
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
        row("Personal grace (minutes)", "Read, completed threads of your own settle after this quiet period. 0 (the default) never settles them; threads that bots or launches started settle on their own.", chips(&["0", "60", "120", "240", "480"], set(st, "settle.personal.minutes", "0"), "settle.personal.minutes")),
        row("Short grace for read conversations", "Use the worker grace for personal conversations once their reply is read and the work is complete.", chips(&["on", "off"], onoff(set(st, "settle.done", "off") == "on"), "settle.done")),
        row("Continue after a restart", "Turns a restart cuts short go on by themselves.", chips(&["on", "off"], onoff(set(st, "restart.continue", "on") != "off"), "restart.continue")),
        // thread.env (commands/plan.rs): what the + and a phone's new thread get; a thread asked for with its own env keeps it
        row("New threads work in", "auto: a thread in a git project gets its own worktree and branch, so threads do not edit the same files; elsewhere the project folder. local: always the project folder.",
            chips(&["auto", "local"], if set(st, "thread.env", "auto") == "local" { "local" } else { "auto" }, "thread.env")),
        row("Largest attachment (MB)", &format!("A file sent with a message, or shown from this hub, may be up to {} MB. Files stream from disk, so a large one costs disk space, not memory.", files_mb(st)),
            chips(&["128", "512", "1024", "2048", "4000"], &files_mb(st).to_string(), "files.max.mb")),
        row("Webhook", "Posts when Notify me for would. A settle or a failure, an ask, a merge, a reminder or a note.", vec![]),
        row_f("Address", if bad { "The address must be http or https." } else { "An http or https address. Empty turns the webhook off." }, vec![field("whurl", "https://example.com/hook", url)], vec![btn("Save", "hook-url", "", false)]),
        row_f("Secret file", "A path to the file that holds the secret. Empty uses the hub's default file. The secret itself is never shown.", vec![field("whsec", "Path to the secret file", sec)], vec![btn("Save", "hook-secret", "", false)]),
        row("Notify me for", "Which alerts reach you, quiet hours and phones are under Notifications.", vec![btn("Notifications ›", "settings-page", "notifications", false)]),
    ];
    v
}

/// The quiet window's two ends as `(from, to)`: the setting's own, else the default; None when quiet hours are off.
pub fn quiet_ends(st: &State) -> Option<(String, String)> {
    let raw = set(st, crate::push::QUIET, "").trim();
    let spec = if raw.is_empty() { crate::push::QUIET_DEFAULT } else { raw };
    let (a, b) = spec.split_once('-')?; crate::push::window(spec)?; Some((a.trim().to_string(), b.trim().to_string()))
}
/// What `notify.quiet` may become from two typed times: `HH:MM-HH:MM` the push gate reads, else None (the page says so).
pub fn quiet_spec(from: &str, to: &str) -> Option<String> {
    let ok = |t: &str| t.len() == 5 && t.as_bytes()[2] == b':';
    let (from, to) = (from.trim(), to.trim());
    (ok(from) && ok(to)).then(|| format!("{from}-{to}")).filter(|s| crate::push::window(s).is_some())
}

/// Settings > Notifications: which alerts reach the person (the hub decides each one: `state/turnend.rs`), and what the phones' push may do: quiet hours, whether an
/// urgent ask passes them, and each phone's mute. `info["push.devices"]` is the hub's list of phones that hear a push (`push::devices_info`): `[{h, name, mute}]`, `h` a
/// handle that is not the token. Every client draws these rows (the desktop through the presenter, the phones through `notify.page`).
pub fn notifications(st: &State, session: &Value, info: &Value) -> Vec<Value> {
    let mut v = vec![row("Notify me for", "Phone and desktop alerts. The rest stays in the app as unread.", vec![])];
    for (kind, name, note) in [("needs", "Needs me", "Approvals, questions and anything an agent is blocked on until you act."),
        ("settled", "Settled", "Work you gave a thread is finished, failed, or its branch merged. Once per piece of work."),
        ("reminders", "Reminders", "Reminders and digests you scheduled."), ("all", "Everything", "Also every other turn that completes or fails, not only the work you gave a thread.")] {
        let on = match set(st, &format!("notify.{kind}"), "") { "on" => true, "off" => false, _ => kind != "all" };
        v.push(row(name, note, chips(&["on", "off"], onoff(on), &format!("notify.{kind}"))));
    }
    v.push(row("Phones", "Only your uberbot pushes to a phone, and a phone window open on the thread is not pushed to.", vec![]));
    let ends = quiet_ends(st);
    v.push(row("Quiet hours", "A push waits until the end of quiet hours, in your time zone, and goes then as one.",
        vec![btn("on", "notify-quiet", "on", ends.is_some()), btn("off", "notify-quiet", "off", ends.is_none())]));
    if let Some((from, to)) = ends {
        let f = if session["drafts"]["@f.qfrom"].is_string() { draft(session, "@f.qfrom").to_string() } else { from };
        let t = if session["drafts"]["@f.qto"].is_string() { draft(session, "@f.qto").to_string() } else { to };
        let bad = draft(session, "@quiet.err") == "bad";
        v.push(row_f("Quiet from and until", if bad { "Times are HH:MM, 24 hours, and not the same." } else { "HH:MM, 24 hours." }, vec![field("qfrom", "22:00", &f), field("qto", "07:00", &t)], vec![btn("Save", "notify-quiet", "save", false)]));
    }
    let urgent = set(st, crate::push::QUIET_URGENT, "") == "on";
    v.push(row("Urgent asks come through", "An approval or a question that blocks an agent is pushed in quiet hours, and to a muted phone.", chips(&["on", "off"], onoff(urgent), crate::push::QUIET_URGENT)));
    let devices = info["push.devices"].as_str().and_then(|d| serde_json::from_str::<Vec<Value>>(d).ok()).unwrap_or_default();
    if devices.is_empty() { v.push(row("iPhones that hear a push", "None yet. An iPhone registers when its app opens with alerts allowed. An Android phone hears alerts while its app is connected, and follows the switches above.", vec![])) }
    for d in &devices {
        let (h, mute) = (s(d, "h"), d["mute"] == true);
        v.push(row(s(d, "name"), if mute { "Muted: hears only what urgent asks let through." } else { "Hears alerts." },
            vec![btn("hears", "notify-mute", &format!("{h}=off"), !mute), btn("muted", "notify-mute", &format!("{h}=on"), mute)]));
    }
    v
}

/// Settings > Developer: replay what a new person sees. Both act on the hub's settings, so every client sees the result.
fn developer(st: &State) -> Vec<Value> {
    vec![row("Replay first run", "Clears setup's progress and opens it at the first step, as on a new hub.", vec![btn("Replay setup", "ob-replay", "", true)]),
         row("Star suggestion", &crate::state::fam::star::standing(set(st, crate::state::fam::star::KEY, "")), vec![btn("Show it now", "dev-star", "", false)])]
}
fn about(st: &State, info: &Value) -> Vec<Value> {
    let v = VERSION.get().map(String::as_str).unwrap_or("");
    let _ = st;
    let mut r = vec![row(&format!("Backplane {v}"), "Updates are under Network.", vec![])];
    // what this window's Browser tab is (a desktop client says; a phone says nothing)
    match info["client"]["browser"].as_str() {
        Some("frames") => r.push(row("Browser", "Frame mode: the hub's browser draws each page and this window shows it.", vec![])),
        Some("cef") => r.push(row("Browser", "Embedded Chromium in this window.", vec![])),
        _ => {}
    }
    // where the third-party notices are (licenses/ beside the programs, in every release archive): a desktop client names its own and opens it;
    // any other client is told the hub's
    match info["client"]["licenses"].as_str().filter(|p| !p.is_empty()) {
        Some(p) => r.push(row("Open-source notices", &format!("The licences of the third-party software in Backplane are in {p}."), vec![btn("Open folder", "open-licenses", "", false)])),
        None => if let Some(p) = licenses_dir() { r.push(row("Open-source notices", &format!("The licences of the third-party software in Backplane are in {} on the hub's machine.", p.display()), vec![])) },
    }
    r
}
/// licenses/ beside the hub's own program, when this install has it (every release archive does; a dev build may not)
fn licenses_dir() -> Option<std::path::PathBuf> {
    let me = std::env::current_exe().ok()?; let me = std::fs::canonicalize(&me).unwrap_or(me);
    let d = me.parent()?.join("licenses"); d.join("README.md").is_file().then_some(d)
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
        row("How it works", "With Ordering on, a project whose design is ready shows Buy it in the thread's header. Buy it opens a buy thread. Its agent checks that every board and part is ready, asks you anything missing (quantities, finishes, where to ship), exports the manufacturing files into the project's mfg/ folder and sends them to Backplane's order server, which measures them and prices the exact order: manufacturing (boards, assembly, components and parts, with Backplane's fee included), shipping, and duties and taxes. The quote shows in the thread, and nothing is charged until you press Buy now and pay on Stripe's checkout page. A quote holds for 3 days. Prices are estimates: if the real cost turns out higher, you are asked before anything more is charged and can have a full refund instead; if it is lower, the difference is refunded. Once paid, the thread shows the order's progress (placed, shipped, tracking). With Ordering off, nothing about your projects leaves this machine for ordering. Price estimates add a rough \"from $X\" to Buy it, worked out from each part's size and options without sending any files.", vec![]),
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
    vec![
        row(&format!("Tailscale: {state}"), &note, vec![]),
        // no tailnet link: pairing still works on the local network or this computer, from Devices (where the links are made since 0.15)
        if link.is_empty() { row("Pairing link", PAIR_ELSEWHERE, vec![btn("Devices", "settings-page", "devices", false)]) } else { row("Pairing link", link, vec![]) },
        row("Update channel", "Stable takes releases; beta also takes prereleases.", chips(&["stable", "beta"], update_channel(set(st, "update.channel", "stable")), "update.channel")),
        update_row(version, &i("update.status")),
    ].into_iter().chain(boat_hub_rows(i("boathub.cost"), crate::hub::now_ms() / 1000)).collect()
}

fn hours(h: f64) -> String { let n = h.round() as i64; if n.abs() >= 1000 { format!("{},{:03}", n / 1000, (n % 1000).abs()) } else if h < 10.0 { format!("{h:.1}") } else { n.to_string() } }
fn ago(s: u64) -> String { match s { 0..=89 => "just now".into(), 90..=5399 => format!("{} min ago", (s + 30) / 60), 5400..=129_599 => format!("{} h ago", (s + 1800) / 3600), _ => format!("{} days ago", (s + 43_200) / 86_400) } }
/// Settings, Network: what the always-on Boat hub costs (info `boathub.cost`, one JSON line from `scripts/boat-hub.sh cost --save`, docs/BOAT-HUB.md),
/// and a warning when the plan's hours will not last to the period's end at this rate: `warn` below twice what the box still needs (or a tenth
/// of the plan), `bad` below what it needs; a measurement over a day old says so. No line, no rows.
pub fn boat_hub_rows(raw: &str, now_s: u64) -> Vec<Value> {
    let Ok(c) = serde_json::from_str::<Value>(raw) else { return vec![] };
    let f = |k: &str| c[k].as_f64().unwrap_or(0.0);
    let (rate, left, plan, mult) = (f("rate_hour"), f("plan_hours_left"), f("plan_hours"), if f("multiplier") > 0.0 { f("multiplier") } else { 1.0 });
    let (end, measured) = (c["period_end_s"].as_u64().unwrap_or(0), c["measured"].as_u64().unwrap_or(0));
    let mut note = format!("${rate:.3} an hour, ${:.2} a month around the clock. {} h so far (${:.2}).", rate * 720.0, hours(f("used_hours")), f("used_dollars"));
    if plan > 0.0 { note.push_str(&format!(" Plan: {} of {} h left.", hours(left), hours(plan))); }
    if measured > 0 { note.push_str(&format!(" Measured {}.", ago(now_s.saturating_sub(measured)))); }
    let name = c["box"].as_str().unwrap_or("Boat");
    let kind = c["type"].as_str().unwrap_or("");
    let mut v = vec![row(&format!("Always-on hub: {name}{}", if kind.is_empty() { String::new() } else { format!(" ({kind})") }), &note, vec![])];
    // what the box still needs this period, in plan hours (a small box counts half)
    let need = if end > now_s { (end - now_s) as f64 / 3600.0 * mult } else { 0.0 };
    if plan > 0.0 {
        if left < need { v.push(row_tone("Boat plan hours run out", &format!("{} h left, the always-on hub needs {} h to the end of the period. Add hours or the hub stops when they run out.", hours(left), hours(need)), "bad")) }
        else if left < (need * 2.0).max(plan * 0.1) { v.push(row_tone("Boat plan hours running low", &format!("{} h left; the always-on hub needs {} h to the end of the period, and builds use the same hours.", hours(left), hours(need)), "warn")) }
    }
    if measured > 0 && now_s.saturating_sub(measured) > 26 * 3600 { v.push(row_tone("Boat cost not measured", &format!("Last measured {}: the desktop measures it (scripts/boat-hub.sh cost --save).", ago(now_s - measured)), "warn")) }
    v
}

/// Network's pairing row when the hub has no tailnet link: where links are made now.
pub const PAIR_ELSEWHERE: &str = "Devices, Add a device makes one: for your tailnet, the local network or this computer.";
/// What a device invitation says when every address it names is this computer's own (`localOnly`): a phone that scans it opens 127.0.0.1 on itself.
pub const LOCAL_DEVICE: &str = "This link names only this computer (127.0.0.1): a phone or another computer cannot reach it. Start the hub with Tailscale on, or on an address they can reach, then make a new link.";
/// The same for a machine invite (`bp1:<url>:<peer>:<secret>`) whose address is this computer's own.
pub const LOCAL_MACHINE: &str = "This invite names only this computer (127.0.0.1): another computer cannot reach it. Start the hub with Tailscale on, or set BACKPLANE_URL to an address the other machine can reach, then make a new invite.";
/// An http(s) origin on this computer only: localhost, 127.x, [::1].
pub fn loopback(url: &str) -> bool {
    let r = url.trim().strip_prefix("http://").or_else(|| url.trim().strip_prefix("https://")).unwrap_or("");
    r.starts_with("localhost") || r.starts_with("127.") || r.starts_with("[::1]")
}
/// A machine invite whose address is loopback: the url is what `bp1:` holds before its last two fields (peer, secret).
pub fn loopback_invite(invite: &str) -> bool {
    invite.strip_prefix("bp1:").and_then(|r| r.rsplitn(3, ':').nth(2)).map_or(false, loopback)
}
fn row_tone(label: &str, note: &str, tone: &str) -> Value { let mut r = row(label, note, vec![]); r["tone"] = json!(tone); r }

/// Settings → Devices: Add a device, the paired devices, the local network addresses the hub was let answer on, and the pairing
/// links (net/pairing.rs `Registry::list` with `lan` and `host`, put in `info.devices` by the presenter's caller: `routes::AuthRoutes::devices`).
/// An invitation made here (`device-invite`, kept in the draft `@invite`) carries every address it can be shown with (net/plan.rs): the
/// person picks Tailscale, Local network or This computer (`device-addr`, the draft `@pair.url`), and the page shows that address's own link,
/// its QR code and the short code beside the hub address. A local network address the hub does not answer on yet offers to let it answer
/// there (`device-lan-on`), and one it answers on because the person let it offers to stop (`device-lan-off`). The phones draw the same rows
/// (`devices.page`, net/devicespage.rs).
pub fn devices(st: &State, session: &Value, info: &Value) -> Vec<Value> {
    let inv: Value = serde_json::from_str(draft(session, "@invite")).unwrap_or(Value::Null);
    let list = &info["devices"];
    let now = info["now"].as_u64().unwrap_or_else(|| crate::hub::now_ms());
    let mut r = vec![];
    if let Some(link) = inv["link"].as_str() {
        let at = inv["expiresAt"].as_u64().unwrap_or(0);
        let until = crate::agents::rules::when(&zone_of(st).0, at / 1000, now / 1000);
        // the registry says when it was used or revoked meanwhile (by a device that scanned it, or from another screen)
        let rec = list["invitations"].as_array().into_iter().flatten().find(|i| !s(&inv, "id").is_empty() && s(i, "id") == s(&inv, "id"));
        let done = match rec.map(|i| s(i, "state")) {
            Some("used") => Some(format!("Used{}. Each link pairs one device.", rec.map(|i| s(i, "deviceName")).filter(|n| !n.is_empty()).map_or(String::new(), |n| format!(" by {n}")))),
            Some("revoked") => Some("Revoked.".to_string()),
            _ => None,
        };
        if let Some(d) = done { r.push(row("Pairing link", &d, vec![btn("Add a device", "device-invite", "", false)])) }
        else if at > now { r.extend(invitation(&inv, link, &until, draft(session, "@pair.url"), list)) }
        else { r.push(row("Pairing link", &format!("Expired at {until}. Each link works for ten minutes."), vec![btn("Add a device", "device-invite", "", false)])) }
    } else if let Some(e) = inv["error"].as_str() { r.push(row("Add a device", &format!("Could not make an invitation: {e}."), vec![btn("Try again", "device-invite", "", false)])) }
    else { r.push(row("Add a device", "Makes a link, a QR code and a short code for one new device. Each works once, for ten minutes.", vec![btn("Add a device", "device-invite", "", false)])) }
    let e = draft(session, "@lan.err");
    if !e.is_empty() { r.push(row_tone("Local network", e, "bad")) }
    // the paired devices (a revoked one is kept in the registry, and would only bury these)
    for d in list["devices"].as_array().into_iter().flatten() {
        let name = if s(d, "name").is_empty() { s(d, "id") } else { s(d, "name") };
        if d["revoked"].as_bool().unwrap_or(false) || s(d, "revoked") == "true" { continue }
        r.push(row(name, "Paired", vec![btn("Revoke", "device-revoke", s(d, "id"), false)]));
    }
    for x in list["lan"].as_array().into_iter().flatten() {
        let note = if x["listening"] == true { "The hub also answers here: plain HTTP on that network, not encrypted. Every device still has to pair." } else { "Kept, but this computer does not have this address now." };
        r.push(row(&format!("Local network · {}", s(x, "url")), note, vec![btn("Stop answering here", "device-lan-off", s(x, "ip"), false)]));
    }
    // the links that still mean something: open ones, and used ones whose device is still paired (revoking one revokes its device);
    // the registry keeps a hundred closed ones, which would only bury these
    for i in list["invitations"].as_array().into_iter().flatten() {
        let (state, kind) = (s(i, "state"), s(i, "kind"));
        if kind == "machine" { continue } // a machine invite is Bots' (linking a machine)
        let live = state == "open" || (state == "used" && i["deviceRevoked"] != true);
        if !live { continue }
        let who = if s(i, "deviceName").is_empty() { String::new() } else { format!(" · {}", s(i, "deviceName")) };
        r.push(row(&format!("Pairing link · {state}{who}"), "", vec![btn("Revoke", "device-uninvite", s(i, "id"), false)]));
    }
    r
}
/// What each kind of address is called on the choice.
fn kind_word(k: &str) -> &'static str { match k { "tailnet" => "Tailscale", "lan" => "Local network", _ => "This computer" } }
fn host_of(url: &str) -> &str {
    let r = url.strip_prefix("http://").or_else(|| url.strip_prefix("https://")).unwrap_or(url);
    let a = r.split(|c| c == '/' || c == '?' || c == '#').next().unwrap_or("");
    a.rsplit_once(':').map_or(a, |(h, _)| h)
}
/// The addresses of an invitation (an invitation made before addresses existed has its one link), each answering as the hub answers now:
/// a local network address answers when the main listener covers it or the person let the hub answer there (`info.devices.lan`, `.host`).
fn addresses(inv: &Value, list: &Value) -> Vec<Value> {
    let mut xs: Vec<Value> = inv["addresses"].as_array().cloned().unwrap_or_default();
    if xs.is_empty() { xs.push(json!({"kind": if inv["localOnly"] == true { "local" } else { "" }, "url": inv["routes"][0].as_str().unwrap_or(""), "link": inv["link"], "listening": true})) }
    if let Some(host) = list["host"].as_str() {
        for x in xs.iter_mut().filter(|x| x["kind"] == "lan") {
            let ip = host_of(s(x, "url")).to_string();
            let let_on = list["lan"].as_array().into_iter().flatten().any(|l| s(l, "ip") == ip && l["listening"] == true);
            x["listening"] = json!(host == "0.0.0.0" || host == ip || let_on);
        }
    }
    xs
}
/// The invitation's rows: the address choice, then the shown address's link, QR code and code (or the offer to answer there).
fn invitation(inv: &Value, default_link: &str, until: &str, want: &str, list: &Value) -> Vec<Value> {
    let xs = addresses(inv, list);
    let kinds: Vec<&str> = ["tailnet", "lan", "local"].into_iter().filter(|k| xs.iter().any(|x| x["kind"] == *k)).collect();
    let lans: Vec<&Value> = xs.iter().filter(|x| x["kind"] == "lan").collect();
    // the one asked for; else Tailscale, the local network (the first that answers) or this computer, in that order; else the default link's
    let shown = xs.iter().find(|x| !want.is_empty() && s(x, "url") == want)
        .or_else(|| match kinds.first() { Some(&"lan") => lans.iter().find(|x| x["listening"] == true).or(lans.first()).copied(), Some(k) => xs.iter().find(|x| x["kind"] == *k), None => None })
        .or_else(|| xs.iter().find(|x| s(x, "link") == default_link)).or(xs.first()).cloned().unwrap_or(Value::Null);
    let kind = s(&shown, "kind");
    let mut r = vec![];
    let note = match kind {
        "tailnet" => "Over your tailnet: the other device needs Tailscale, signed in to the same tailnet.",
        "lan" if shown["listening"] == true => "Plain HTTP on this network: the traffic is not encrypted, though every device still has to pair. Prefer Tailscale on a network you do not trust.",
        _ => "",
    };
    if kinds.len() > 1 {
        let pick = |k: &str| { let to = if k == "lan" { lans.iter().find(|x| x["listening"] == true).or(lans.first()).map_or("", |x| s(x, "url")) } else { xs.iter().find(|x| x["kind"] == k).map_or("", |x| s(x, "url")) }; btn(kind_word(k), "device-addr", to, k == kind) };
        r.push(row("Reach this hub by", note, kinds.iter().map(|k| pick(k)).collect()));
    } else if !note.is_empty() { r.push(row(kind_word(kind), note, vec![])) }
    if kind == "lan" && lans.len() > 1 { r.push(row("Local network address", "", lans.iter().map(|x| btn(s(x, "url"), "device-addr", s(x, "url"), s(x, "url") == s(&shown, "url"))).collect())) }
    let ip = host_of(s(&shown, "url")).to_string();
    if shown["listening"] == true {
        let link = s(&shown, "link");
        let code = s(inv, "code");
        let mut row_l = row("Pairing link", link, vec![btn("Copy link", "copy", link, false), btn("Copy code", "copy", code, false), btn("Add another", "device-invite", "", false)]);
        row_l["qr"] = json!(crate::net::qr::text(link).lines().map(str::to_string).collect::<Vec<_>>());
        let hub = s(&shown, "url").trim_start_matches("http://").trim_start_matches("https://");
        row_l["qrnote"] = json!(format!("Code {code} · hub {hub} · works once, until {until}"));
        r.push(row_l);
    } else {
        r.push(row("Local network", &format!("The hub does not answer at {} yet: it listens only on this computer. Letting it answer there opens plain HTTP on this network (not encrypted); every device still has to pair, and you can turn it off here or under Devices.", s(&shown, "url")),
            vec![btn("Let the hub answer on the local network", "device-lan-on", &ip, false)]));
    }
    if kind == "local" || (kind.is_empty() && inv["localOnly"] == true) {
        let other: Vec<&str> = kinds.iter().filter(|k| **k != "local").map(|k| kind_word(k)).collect();
        r.push(row_tone("Only this computer", &if other.is_empty() { LOCAL_DEVICE.to_string() } else { format!("This link works only on this computer. Choose {} to pair another device.", other.join(" or ")) }, "warn"));
    }
    r
}

/// Settings → Devices' presses (the desktop's view actions and the phones' `devices.page.act`): the session patch. The registry is the hub's
/// own, the one /auth/invitations uses; local network listening is `net::lan`'s. None for another action.
pub fn device_act(action: &str, value: &str) -> Option<Value> {
    let Some(routes) = crate::net::start::routes() else { return Some(json!({"drafts": {"@invite": json!({"error": "no_registry"}).to_string()}})) };
    Some(match action {
        "device-invite" => json!({"drafts": {"@invite": routes.invite().to_string(), "@pair.url": "", "@lan.err": ""}}),
        "device-addr" => json!({"drafts": {"@pair.url": value, "@lan.err": ""}}),
        "device-lan-on" | "device-lan-off" => {
            let j = routes.lan_set(value, action == "device-lan-on");
            let err = match j["error"].as_str() {
                None => String::new(),
                Some("lan_unknown") => format!("{value} is not one of this computer's local network addresses now."),
                Some("copy") => "A hub on a copied log does not listen on the network.".into(),
                Some(_) => format!("The hub could not listen on {value}: the port may be taken there."),
            };
            json!({"drafts": {"@lan.err": err}})
        }
        "device-revoke" => { crate::net::start::revoke("revoke", value); json!({}) }
        "device-uninvite" => { crate::net::start::revoke("uninvite", value); json!({}) }
        _ => return None,
    })
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
/// The records of a settings family (`rule.<id>`, `board.<id>`), oldest first: the agents' side reads the same ones (agents/rules.rs, bots/ledger.rs).
fn family(st: &State, pre: &str) -> Vec<Value> {
    let mut v: Vec<Value> = st.settings.iter().filter(|(k, x)| k.starts_with(pre) && !x.is_empty()).filter_map(|(k, x)| {
        let mut o: Value = serde_json::from_str(x).ok()?; if !o.is_object() { return None }
        if s(&o, "id").is_empty() { o["id"] = json!(&k[pre.len()..]); } Some(o) }).collect();
    v.sort_by(|a, b| (a["at"].as_u64().unwrap_or(0), s(a, "id")).cmp(&(b["at"].as_u64().unwrap_or(0), s(b, "id")))); v
}
/// Rules and task board: what the person wants always or never done (`rules.set`, `rules.drop`), and the shared list of what is to be done (`board.set`, `board.drop`).
fn rules(st: &State, session: &Value) -> Vec<Value> {
    let mut v = vec![row("Standing rules", "Every agent's turn starts with the rules that apply to it. An enforced rule with tools is also checked: those tools are refused unless your latest message says its word.", vec![])];
    for r in family(st, "rule.") {
        let on = r["on"].as_bool().unwrap_or(true); let (tool, word) = (s(&r, "tool"), s(&r, "word"));
        let mut note = vec![if on { "On" } else { "Off" }.to_string()];
        if !tool.is_empty() { note.push(if r["enforce"] == true { if word.is_empty() { format!("refuses {tool}") } else { format!("refuses {tool} unless you say \"{word}\"") } } else { format!("when using {tool}") }); }
        v.push(row(s(&r, "text"), &note.join(" · "), vec![btn(if on { "Turn off" } else { "Turn on" }, "rule-on", &format!("{}={}", s(&r, "id"), !on), false), btn("Remove", "rule-drop", s(&r, "id"), false)]));
    }
    v.push(row_f("New rule", "Its words, as you would say them to an agent.", vec![field("rtext", "e.g. Never push to main", draft(session, "@f.rtext"))], vec![btn("Add rule", "rule-add", "", true)]));
    v.push(row("Task board", "One shared list: agents in every thread read and update it.", vec![]));
    let mut items = family(st, "board.");
    let rank = |x: &str| match x { "doing" => 0, "blocked" => 1, "todo" => 2, "done" => 3, _ => 4 };
    items.sort_by_key(|i| rank(s(i, "status")));
    for i in items.iter().filter(|i| rank(s(i, "status")) < 4) {
        let st_ = s(i, "status"); let due = s(i, "due");
        let note = if due.is_empty() { st_.to_string() } else { format!("{st_} · due {due}") };
        let mut b = vec![]; if st_ != "done" { b.push(btn("Done", "board-done", s(i, "id"), false)); }
        b.push(btn("Remove", "board-drop", s(i, "id"), false));
        v.push(row(s(i, "title"), &note, b));
    }
    v.push(row_f("New item", "What is to be done.", vec![field("btitle", "e.g. Order the boards", draft(session, "@f.btitle"))], vec![btn("Add item", "board-add", "", true)]));
    v
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

/// The preferences the page shows, bound into the On button: `setting.set` cannot re-enable telemetry (settings/telemetry.rs `reserved`), only a `telemetry.choice` carrying this token can.
fn tm_token(st: &State) -> String {
    crate::settings::telemetry::of(st).token()
}

fn privacy(st: &State, info: &Value) -> Vec<Value> {
    let on = !crate::settings::telemetry::off(set(st, "telemetry.share", ""), set(st, "telemetry.usage.asked", ""), set(st, "onboard.usage", ""));
    let id = if on { info["telemetry.id"].as_str().unwrap_or("") } else { "" };
    let mut note = "All telemetry: installs, updates, daily activity, daily counts of threads, messages, model tokens, bot traffic, boards created and completed board orders; random install id, version, package channel, OS, architecture and UTC day. Used for product and aggregate investor metrics. Board/order data is integer counts only: no design paths, hashes, dimensions, layers, part numbers, BOMs, netlists or prices. No content, files, names or credentials. On unless you turn it off; Off disables every category. Nothing is sent before setup or the one-time notice has shown this. Local diagnostics stay on this device unless you choose to export them. Privacy policy: https://backplane.works/privacy".to_string();
    if !id.is_empty() { note.push_str(&format!(" Install id: {id}")); }
    let n = set(st, "telemetry.reset", "").parse::<u64>().unwrap_or(0) + 1;
    vec![row("Share anonymous usage", &note, vec![btn("On", "telemetry-on", &tm_token(st), on), btn("Off", "setting", "telemetry.share=off", !on), btn("Reset id", "setting", &format!("telemetry.reset={n}"), false),
        btn("Privacy", "open-url", "https://backplane.works/privacy", false), btn("Report an issue", "open-url", &bug_url(info), false)])]
}

/// Settings > Writing (TextGen): the small model that titles threads, names branches and writes commit messages: `text.provider` (claude, codex, grok or muse) and `text.model` ("" is the provider's light model).
fn writing(st: &State, info: &Value) -> Vec<Value> {
    const TEXT: &[&str] = &["claude", "codex", "grok", "muse"];
    let models: Value = info["models"].as_str().and_then(|s| serde_json::from_str(s).ok()).unwrap_or(Value::Null);
    let prov = { let p = set(st, "text.provider", "claude").trim(); if TEXT.contains(&p) { p } else { "claude" } };
    let model = set(st, "text.model", "").trim();
    let mut ids: Vec<&str> = vec![""];
    ids.extend(models[prov].as_array().into_iter().flatten().filter_map(Value::as_str).take(12));
    if !ids.contains(&model) { ids.push(model); }
    vec![
        row("Writing: provider", "A small model keeps this cheap. It names each thread from its first message.", chips(TEXT, prov, "text.provider")),
        row("Model for titles, branch names and commit messages", &format!("{}: the provider's light model unless one is chosen.", if model.is_empty() { "default" } else { model }), chips(&ids, model, "text.model")),
    ]
}

/// Settings > CAD's Install KiCad row (0.14's `kicad-install`), only while no KiCad is found: what it would install and from where, the consent
/// step (Install, Cancel), its progress and its failure, from info `kicad.install` (kicad/install.rs). None while KiCad is found or still looked for.
pub fn install_row(session: &Value, info: &Value, pkg: Option<&crate::files::kicad::install::Package>) -> Option<Value> {
    use crate::files::kicad::install::FROM;
    let i = |k: &str| info[k].as_str().unwrap_or("");
    if i("kicad.status") == "checking" { return None }
    let st: Value = serde_json::from_str(i("kicad.install")).unwrap_or_default();
    let (state, note) = (st["state"].as_str().unwrap_or(""), st["note"].as_str().unwrap_or(""));
    // a Backplane KiCad found: the row only while it is an older build of the published one (the update), or while that update runs
    if !i("kicad.cli").is_empty() {
        let Some(p) = pkg.filter(|p| i("kicad.status") == "ok" && crate::files::kicad::install::older(i("kicad.version"), p)) else { return None };
        return Some(match state {
            "running" => running_row(&format!("Updating to Backplane KiCad {}", p.version), &st, p.version, note),
            _ if draft(session, "@kinstall") == "ask" => row("Update Backplane KiCad?", &format!("Downloads Backplane KiCad {} (about 1 GB) from {FROM} and puts it beside {}, which stays. It brings the KiCad editor inside Backplane's viewer (Edit).", p.version, i("kicad.version")),
                vec![btn("Update", "kicad-install", "", true), btn("Cancel", "kicad-install-cancel", "", false)]),
            "failed" => row("Update Backplane KiCad", &format!("{note}."), vec![btn("Try again", "kicad-install-ask", "", true)]),
            _ => row("Update Backplane KiCad", &format!("Backplane KiCad {} is out; this computer has {}. It brings the KiCad editor inside Backplane's viewer (Edit).", p.version, i("kicad.version")), vec![btn("Update KiCad", "kicad-install-ask", "", true)]),
        });
    }
    let Some(p) = pkg else { return Some(row("Install KiCad", &format!("No Backplane KiCad is published for this computer ({}). Install KiCad from kicad.org, then set its path above.", crate::files::kicad::install::this_computer()), vec![])) };
    let home = i("home"); let into = if home.is_empty() { "Backplane's own folder".to_string() } else { format!("{home}/kicad") };
    Some(match state {
        "running" => running_row(&format!("Installing Backplane KiCad {}", p.version), &st, p.version, note),
        _ if draft(session, "@kinstall") == "ask" => row("Install Backplane KiCad?", &format!("Downloads Backplane KiCad {} (about 1 GB) from {FROM}, checks its checksum and unpacks it into {into}. It needs no administrator password and changes nothing else on this computer.", p.version),
            vec![btn("Install", "kicad-install", "", true), btn("Cancel", "kicad-install-cancel", "", false)]),
        "failed" => row("Install KiCad", &format!("{note}."), vec![btn("Try again", "kicad-install-ask", "", true)]),
        _ => row("Install KiCad", &format!("Backplane KiCad {} for this computer, from {FROM}. Agents edit boards with it, and it runs the checks and exports.", p.version), vec![btn("Install KiCad", "kicad-install-ask", "", true)]),
    })
}

/// The install row while it runs: setup's words and bar (`onboard::install_progress`).
fn running_row(label: &str, st: &Value, version: &str, note: &str) -> Value {
    let (words, bar) = super::onboard::install_progress(st, &format!(" {version}"), note);
    // the label names it already: the note is the step alone
    let step = words.split_once(&format!("KiCad {version}. ")).map(|(_, w)| w.to_string()).or_else(|| words.split_once(&format!("KiCad {version}: ")).map(|(_, w)| format!("Downloading: {w}"))).unwrap_or(words);
    let mut r = row(label, &step, vec![]); if let Some(b) = bar { r["progress"] = json!(b); } r
}
/// Settings > CAD: the KiCad the hub found (what it told: `kicad.*`), Install KiCad while none is, the path field, the IPC switch, and FreeCAD.
fn cad(st: &State, session: &Value, info: &Value) -> Vec<Value> { cad_with(st, session, info, crate::files::kicad::install::package()) }
fn cad_with(st: &State, session: &Value, info: &Value, pkg: Option<&crate::files::kicad::install::Package>) -> Vec<Value> {
    let i = |k: &str| info[k].as_str().unwrap_or("");
    let kicad = match (i("kicad.status"), i("kicad.version"), i("kicad.cli")) {
        (_, v, c) if !c.is_empty() => format!("{}{}", if v.is_empty() { "found".to_string() } else { format!("version {v}") }, format!(" · {c}")),
        ("checking", _, _) => "checking".to_string(),
        _ => "KiCad was not found. Install it below, or set the folder of one you have.".to_string(),
    };
    let done: Value = serde_json::from_str(i("kicad.install")).unwrap_or_default();
    let kicad = if done["state"] == "done" && !i("kicad.cli").is_empty() { format!("{kicad}. Installed by Backplane just now.") } else { kicad };
    // a path set below that holds no KiCad is passed over for the next one found (kicad/ipc.rs `runtime`): say so, as FreeCAD's row does
    let kset = set(st, "kicad.path", "").trim().trim_end_matches('/');
    let kicad = if !kset.is_empty() && !i("kicad.cli").is_empty() && !i("kicad.cli").starts_with(kset) { format!("{kicad}. Nothing runs at {kset}, so this one is used.") } else { kicad };
    let path = if session["drafts"]["@f.kpath"].is_string() { draft(session, "@f.kpath") } else { set(st, "kicad.path", "") };
    let free = if i("freecad").is_empty() { if set(st, "freecad.path", "").is_empty() { "FreeCAD was not found. Mechanical parts are made with it.".to_string() } else { "FreeCAD was not found at that path.".to_string() } } else { i("freecad").to_string() };
    let fpath = if session["drafts"]["@f.fpath"].is_string() { draft(session, "@f.fpath") } else { set(st, "freecad.path", "") };
    let mut v = vec![row_f("KiCad", &kicad, vec![field("kpath", "Path to KiCad's command line, empty for the one on PATH", path)], vec![btn("Set", "kicad-path", "", true)])];
    v.extend(install_row(session, info, pkg));
    v.extend([
        row_f("FreeCAD", &free, vec![field("fpath", "Path to FreeCAD's command line (freecadcmd), empty to look for it", fpath)], vec![btn("Set", "freecad-path", "", true)]),
        row("KiCad over its API", "Lets agents edit a board in a running KiCad. Off uses the files only.", chips(&["on", "off"], set(st, "kicad.ipc", "on"), "kicad.ipc")),
        row("Board 3D", "How a board's 3D model is made: natively, the board at once and its parts as they convert, or by kicad-cli, all at the end.", chips(&["native", "kicad-cli"], set(st, "viewer.board3d", "native"), "viewer.board3d")),
    ]);
    v
}

/// The API keys the hub keeps by name (never a value): a list with Forget (asks once), and a form: a name and the key, saved straight to the hub (the key is a vault field, never an action).
fn keys(st: &State, session: &Value, info: &Value) -> Vec<Value> {
    let _ = st;
    let home = info["home"].as_str().unwrap_or("");
    let text = if home.is_empty() { String::new() } else { crate::settings::keys::names_kept(std::path::Path::new(home)) };
    let names: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    let arming = draft(session, "@keys.rm");
    let mut v = vec![row("API keys", "Kept on this machine, in the hub's secrets folder. Agents are told their names and where to load them, never the values.", vec![])];
    if names.is_empty() { v.push(row("No keys yet", "", vec![])); }
    for n in &names {
        let ask = arming == *n;
        v.push(row(n, if ask { "Press again to forget it." } else { "" }, vec![btn(if ask { "Forget it" } else { "Forget" }, "key-del", n, ask)]));
    }
    let err = draft(session, "@key.err");
    let name = draft(session, "@f.kname");
    let mut form = row_f("New key", if err.is_empty() { "The name is the environment variable it is loaded as (OPENAI_API_KEY). The key is sent to the hub once and never shown again." } else { err },
        vec![field("kname", "Name", name), json!({"name": "kval", "hint": "Key", "secret": true, "vault": true, "value": ""})],
        vec![btn("Save", "secret-save", &format!("@apikey\u{1f}{name}"), true)]);
    if !err.is_empty() { form["tone"] = json!("bad"); }
    v.push(form);
    v
}

/// The failover order (Fallbackv.rows): one ordered list of providers (or provider/model) per hub, on unless turned off, Claude, Codex, Antigravity and Grok until the person
/// sets one. Edited with `setting fallback.chain=...` (Move up, Move down, Remove, Add, Default order).
fn fallback(st: &State, info: &Value) -> Vec<Value> {
    use crate::settings::fallback as fb;
    let caps: Value = info["models"].as_str().and_then(|s| serde_json::from_str(s).ok()).unwrap_or(Value::Null);
    let chain = fb::chain_of(st.settings.get("fallback.chain").map(String::as_str));
    let on = fb::chain_on(st.settings.get("fallback.on").map(String::as_str));
    let inv: Value = serde_json::from_str(set(st, "accounts.inventory", "")).unwrap_or(Value::Null);
    let accounts = |p: &str| inv["profiles"].as_array().map_or(0, |a| a.iter().filter(|x| x["provider"] == p && x["state"] == "connected").count());
    let mut v = vec![row("Failover order", "Applies to all threads, bots, routines and subthreads. When an account hits its usage limit, work moves to another account of the same provider that has usage left, then down this list. When every account is out, Backplane says when the first one resets and continues then. Work moves back up the list as accounts reset.",
        vec![btn("Enabled", "setting", "fallback.on=on", on), btn("Disabled", "setting", "fallback.on=off", !on)])];
    let down = |key: &str| { let mut r = chain.clone(); r.reverse(); let mut r = fb::up(&r, key); r.reverse(); r };
    for (i, t) in chain.iter().enumerate() {
        let key = t.key();
        let name = if t.model.is_empty() { fb::provider_name(&t.provider) } else { format!("{} {}", fb::provider_name(&t.provider), t.model) };
        let n = accounts(&t.provider);
        let note = if !fb::caps_valid(&caps, t) && !(t.model.is_empty() && n > 0) { "Not installed on this machine. Skipped.".to_string() }
            else if n == 0 { "Its own sign-in on this machine.".to_string() } else if n == 1 { "1 account.".to_string() } else { format!("{n} accounts, the one with the most usage left first.") };
        v.push(row(&format!("{}. {name}", i + 1), &note,
            vec![btn("Move up", "setting", &format!("fallback.chain={}", fb::show(&fb::up(&chain, &key))), false), btn("Move down", "setting", &format!("fallback.chain={}", fb::show(&down(&key))), false),
                 btn("Remove", "setting", &format!("fallback.chain={}", fb::show(&fb::remove(&chain, &key))), false)]));
    }
    if fb::show(&chain) != fb::DEFAULT_CHAIN { v.push(row("Default order", "Claude, Codex, Antigravity, Grok.", vec![btn("Use the default order", "setting", &format!("fallback.chain={}", fb::DEFAULT_CHAIN), false)])) }
    let providers: Vec<&str> = PROVIDERS.iter().copied().filter(|p| caps[p].as_array().map_or(false, |a| !a.is_empty()) || accounts(p) > 0).collect();
    let ap = { let p = set(st, "fallback.add.provider", "codex"); if providers.contains(&p) { p } else { providers.first().copied().unwrap_or("codex") } };
    let mut models: Vec<&str> = vec!["default"]; models.extend(caps[ap].as_array().into_iter().flatten().filter_map(Value::as_str).take(16));
    let am = { let m = set(st, "fallback.add.model", "default"); if models.contains(&m) { m } else { "default" } };
    if !providers.is_empty() {
        v.push(row("Add a provider", "Choose an installed provider, and a model or its default, then add it to the end.", chips(&providers, ap, "fallback.add.provider")));
        v.push(row("Add a model", "Only installed models can be saved. Duplicate entries are not allowed.", chips(&models, am, "fallback.add.model")));
        let target = fb::Target { provider: ap.to_string(), model: if am == "default" { String::new() } else { am.to_string() } };
        let mut next = chain.clone(); next.push(target.clone());
        let addable = (fb::caps_valid(&caps, &target) || (target.model.is_empty() && accounts(ap) > 0)) && !fb::has(&chain, &target.key());
        v.push(row(&target.label(), "", if addable { vec![btn("Add to the order", "setting", &format!("fallback.chain={}", fb::show(&next)), false)] } else { vec![] }));
    }
    v
}

/// The Privacy page's links: the privacy policy, and the issue form carrying the version, OS and architecture.
fn bug_url(info: &Value) -> String {
    let enc = |s: &str| -> String { s.bytes().map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") }).collect() };
    let v = VERSION.get().map(String::as_str).filter(|v| !v.is_empty()).unwrap_or("unknown");
    let known = |k: &str, ok: &[&str]| { let s = info[k].as_str().unwrap_or(""); if ok.contains(&s) { s.to_string() } else { "unknown".to_string() } };
    format!("https://github.com/BackplaneWorks/Backplane/issues/new?body={}", enc(&format!("Version: {v}\nOS: {} {}\n\nWhat happened:\n", known("os", &["linux", "darwin", "windows"]), known("arch", &["x64", "arm64"]))))
}

/// Settings > Voice: the same rows the phones get (voice/page.rs), with the Add a term field as typed here.
fn voice(st: &State, session: &Value, info: &Value) -> Vec<Value> {
    let p: crate::voice::page::Prefs = st.settings.iter().filter(|(k, _)| k.starts_with("voice.")).filter(|(_, v)| !v.is_empty()).map(|(k, v)| (k.clone(), v.clone())).collect();
    let mut rows = crate::voice::page::rows(&p, info, draft(session, "@f.vterm"), draft(session, "@key.err"));
    for r in rows.iter_mut().filter(|r| r["label"] == "Send by voice") { r["fields"][0]["value"] = json!(draft(session, "@f.vphrase")); }
    for r in rows.iter_mut().filter(|r| r["label"] == "End of turn: Jev (TypeSafe)") { let d = draft(session, "@f.vjevmodel"); if !d.is_empty() { r["fields"][1]["value"] = json!(d); } }
    rows
}

/// Settings > Usage: the thread totals the agents lane counts (`tok.<epoch>.t.<thread>`: "turns/in/cread/cwrite/out/reasoning/cost/costTurns/knownTurns"), added up per provider.
fn usage(st: &State) -> Vec<Value> {
    use super::tokstats::{num, usd};
    let ep = format!("{}{}", set(st, "tok.epoch", ""), set(st, "tok.recount", ""));
    let pre = if ep.is_empty() { "tok.".to_string() } else { format!("tok.{ep}.") };
    let mut per: std::collections::BTreeMap<String, [u64; 7]> = Default::default();
    for (id, t) in st.threads.iter().filter(|(_, t)| !t.deleted) {
        let Some(v) = st.settings.get(&format!("{pre}t.{id}")) else { continue };
        let n: Vec<u64> = v.split('/').map(|x| x.parse().unwrap_or(0)).collect();
        if n.first().copied().unwrap_or(0) == 0 { continue }
        let a = per.entry(t.provider.clone()).or_default();
        for (i, k) in [0usize, 1, 2, 3, 4, 6, 7].into_iter().enumerate() { a[i] += n.get(k).copied().unwrap_or(0); }
    }
    let line = |a: &[u64; 7]| format!("{} turn{} · input {} · cache read {} · cache write {} · output {} · {}", num(a[0]), if a[0] == 1 { "" } else { "s" }, num(a[1]), num(a[2]), num(a[3]), num(a[4]), if a[6] == 0 { "cost not reported".to_string() } else { usd(a[5]) });
    if per.is_empty() { return vec![row("Tokens and cost", "No turns counted yet.", vec![])] }
    let mut all = [0u64; 7]; for a in per.values() { for i in 0..7 { all[i] += a[i]; } }
    let mut v: Vec<Value> = per.iter().map(|(p, a)| row(if p.is_empty() { "Unknown provider" } else { p }, &line(a), vec![])).collect();
    v.push(row("All threads", &line(&all), vec![])); v
}

/// Settings > Ring (docs/ring.md): the rows the phones get too (voice/ringpage.rs), from the hub's `ring.setup` and `ring.notes`, the search field as typed here.
fn ring(st: &State, session: &Value, info: &Value) -> Vec<Value> {
    let parsed = |k: &str| info[k].as_str().and_then(|t| serde_json::from_str::<Value>(t).ok()).unwrap_or(Value::Null);
    let zone = super::thread::person_zone(st).unwrap_or_else(super::tz::Rule::utc);
    crate::voice::ringpage::rows(&parsed("ring.setup"), &parsed("ring.notes"), s(info, "ring.busy"), draft(session, "@f.ringq"), &|t| super::thread::clock(t, 0, &zone), false)
}

/// Settings > MCP (mcpx/page.rs): the hub's rows from `info["mcp"]`, the page's state in `@mcp.*` drafts and what is typed in `@f.m*`.
fn mcp_get(session: &Value) -> impl Fn(&str) -> Option<String> + Sync + '_ {
    move |k: &str| { let key = if crate::mcpx::page::UI_KEYS.contains(&k) { format!("@mcp.{k}") } else { format!("@f.{k}") }; session["drafts"][key.as_str()].as_str().map(str::to_string) }
}
fn mcp(session: &Value, info: &Value) -> Vec<Value> {
    let m: Value = info["mcp"].as_str().and_then(|t| serde_json::from_str(t).ok()).unwrap_or(Value::Null);
    if m.is_null() { return vec![row("MCP servers", "Not running on this hub.", vec![])] }
    let get = mcp_get(session);
    crate::mcpx::page::rows(&m, &crate::mcpx::page::Ui { get: &get }, crate::hub::now_ms() / 1000)
}
fn mcp_patch(p: serde_json::Map<String, Value>) -> Value {
    let mut d = serde_json::Map::new();
    for (k, v) in p { let key = if crate::mcpx::page::UI_KEYS.contains(&k.as_str()) { format!("@mcp.{k}") } else { format!("@f.{k}") }; d.insert(key, v); }
    json!({"drafts": d})
}

/// The rows shown while the dialog is open: the page menu (no page chosen) or one page, its first row the way back.
pub fn view(st: &State, session: &Value, info: &Value) -> Value {
    let p = page(session);
    let rows = if p.is_empty() {
        PAGES.iter().map(|(id, title)| row(title, "", vec![btn(&format!("{title} ›"), "settings-page", id, false)])).collect()
    } else {
        let title = PAGES.iter().find(|(id, _)| *id == p).map_or("", |(_, t)| *t);
        let mut r = vec![row(title, "", vec![btn("‹ Settings", "settings-page", "", false)])];
        r.extend(match p { "appearance" => appearance(st), "threads" => threads(st, session), "notifications" => notifications(st, session, info), "about" => about(st, info), "setup" => super::onboard::settings_rows(st, session, info, &super::onboard::Client::of(info), &super::onboard::home_dir()), "import" => super::onboard::import_rows(info), "agents" => { let mut a = agents(st, info); a.extend(fallback(st, info)); a }, "writing" => writing(st, info), "cad" => cad(st, session, info), "keys" => keys(st, session, info), "ordering" => ordering(st), "network" => network(st, info), "devices" => devices(st, session, info), "machines" => { let mut r = crate::net::machinespage::rows(&info["machines"], &|k| draft(session, &format!("@f.{k}")).to_string());
            r.extend(crate::net::homemove::rows(!set(st, "uber.bot", "").is_empty(), set(st, "home.at", ""), info["home.move"].as_str().unwrap_or(""), &info["machines"]["linked"]));
            r.extend(crate::net::alwayson::rows::machines_rows(info, set(st, crate::net::alwayson::other::STAYS_ON, "") == "on"));
            r.extend(super::signin_there::open_rows(info)); r }, "timezone" => timezone(st, session), "rules" => rules(st, session), "privacy" => privacy(st, info), "voice" => voice(st, session, info), "ring" => ring(st, session, info), "usage" => usage(st), "developer" => developer(st), "mcp" => mcp(session, info), _ => vec![] });
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
        // opening Settings shuts the viewer beside the thread (0.14's Settings.viewer.shut): Settings takes the room
        "flag" if value == "settings" => { let opening = !is_open(session); let mut p = json!({"open": {"settings": opening}});
            if opening { let (shut, c) = super::viewer::shut(hub, session); crate::session::patch::merge(&mut p, &shut); cmds.extend(c); if let Some(r) = p["rid"].as_u64() { rid = r; } } p }
        "settings-page" => {
            let p = if PAGES.iter().any(|(id, _)| *id == value) { value } else { "" };
            // opening Threads fills the webhook fields from the saved settings (Settings.seed): the drafts are dropped so the saved values show
            if p == "voice" { send(session, hub, "bots.voice", json!({"op": "status"}), &mut cmds, &mut rid); } // what is downloaded, the key's mask and the microphones are read afresh
            if p == "ring" { send(session, hub, "ring.search", json!({}), &mut cmds, &mut rid); } // the list and the setup as they stand now, the search kept
            json!({"drafts": {"@settings.page": p, "@f.whurl": Value::Null, "@f.qfrom": Value::Null, "@f.qto": Value::Null, "@quiet.err": "", "@f.whsec": Value::Null, "@hook.err": "", "@f.tzname": "", "@tz.err": "", "@f.kpath": Value::Null, "@f.fpath": Value::Null, "@kinstall": "", "@f.kname": "", "@keys.rm": "", "@key.err": ""}})
        }
        "dev-star" => { send(session, hub, "star.replay", json!({}), &mut cmds, &mut rid); json!({"open": {"settings": false}}) }
        "update-check" => { send(session, hub, "update.check", json!({}), &mut cmds, &mut rid); json!({}) }
        "update-now" => { send(session, hub, "update.now", json!({}), &mut cmds, &mut rid); json!({}) }
        "rule-add" | "board-add" => {
            let (f, m, body) = if action == "rule-add" { ("@f.rtext", "rules.set", "rule") } else { ("@f.btitle", "board.set", "item") };
            let text = draft(session, f).trim().to_string(); if text.is_empty() { return Some(Out { patch: json!({}), cmds }) }
            let key = if body == "rule" { "text" } else { "title" };
            send(session, hub, m, json!({body: {key: text}}), &mut cmds, &mut rid); json!({"drafts": {f: ""}})
        }
        "rule-on" => { let (id, on) = value.split_once('=')?; send(session, hub, "rules.set", json!({"rule": {"id": id, "on": on == "true"}}), &mut cmds, &mut rid); json!({}) }
        "rule-drop" => { send(session, hub, "rules.drop", json!({"id": value}), &mut cmds, &mut rid); json!({}) }
        "board-done" => { send(session, hub, "board.set", json!({"item": {"id": value, "status": "done"}}), &mut cmds, &mut rid); json!({}) }
        "board-drop" => { send(session, hub, "board.drop", json!({"id": value}), &mut cmds, &mut rid); json!({}) }
        // Devices: an invitation made here (the registry is the hub's own, the same one /auth/invitations uses), a device or a link revoked
        "device-invite" | "device-revoke" | "device-uninvite" | "device-addr" | "device-lan-on" | "device-lan-off" => return device_act(action, value).map(|patch| Out { patch, cmds }),
        "kicad-path" => {
            if !session["drafts"]["@f.kpath"].is_string() { return Some(Out { patch: json!({}), cmds }) }
            let raw = draft(session, "@f.kpath").trim().to_string();
            send(session, hub, "setting.set", json!({"key": "kicad.path", "value": raw}), &mut cmds, &mut rid);
            json!({"drafts": {"@f.kpath": raw}})
        }
        // Install KiCad: the first press asks (what, from where, into where); Install is the consent the hub needs; Cancel sends nothing
        "kicad-install-ask" => json!({"drafts": {"@kinstall": "ask"}}),
        "kicad-install-cancel" => json!({"drafts": {"@kinstall": ""}}),
        "kicad-install" => {
            if draft(session, "@kinstall") != "ask" { return Some(Out { patch: json!({"drafts": {"@kinstall": "ask"}}), cmds }) }
            send(session, hub, "kicad.install", json!({"consent": true}), &mut cmds, &mut rid); json!({"drafts": {"@kinstall": ""}})
        }
        "freecad-path" => {
            if !session["drafts"]["@f.fpath"].is_string() { return Some(Out { patch: json!({}), cmds }) }
            let raw = draft(session, "@f.fpath").trim().to_string();
            send(session, hub, "setting.set", json!({"key": "freecad.path", "value": raw}), &mut cmds, &mut rid);
            json!({"drafts": {"@f.fpath": raw}})
        }
        // an API key forgotten: the first press asks, the same again forgets it (ApiKeys.del)
        "key-del" => {
            if value.is_empty() { return None }
            if draft(session, "@keys.rm") == value { send(session, hub, "bots.voice", json!({"op": "api-del", "name": value}), &mut cmds, &mut rid); json!({"drafts": {"@keys.rm": ""}}) }
            else { json!({"drafts": {"@keys.rm": value}}) }
        }
        // the client's own answers to the key it saved straight to the hub
        "voice-op" => { send(session, hub, "bots.voice", json!({"op": "page-act", "action": "voice-op", "value": value}), &mut cmds, &mut rid); json!({}) }
        // Ring: setup, hear again, send again and a search go to the hub (voice/ringpage.rs `request`); Show all empties the field
        "ring-op" | "ring-search" => {
            let (m, p) = crate::voice::ringpage::request(action, value, draft(session, "@f.ringq"))?;
            send(session, hub, m, p, &mut cmds, &mut rid);
            if action == "ring-search" && value == "clear" { json!({"drafts": {"@f.ringq": ""}}) } else { json!({}) }
        }
        "voice-always-ignore" => { send(session, hub, "bots.voice", json!({"op": "page-act", "action": "voice-always-ignore", "value": draft(session, "@f.valwign")}), &mut cmds, &mut rid); json!({"drafts": {"@f.valwign": ""}}) }
        "voice-jev-model" => { send(session, hub, "setting.set", json!({"key": crate::voice::eot::MODEL, "value": draft(session, "@f.vjevmodel").trim()}), &mut cmds, &mut rid); json!({"drafts": {"@f.vjevmodel": ""}}) }
        "voice-phrases" => { let (k, v) = crate::voice::page::phrases_setting(draft(session, "@f.vphrase")); send(session, hub, "setting.set", json!({"key": k, "value": v}), &mut cmds, &mut rid); json!({"drafts": {"@f.vphrase": ""}}) }
        "voice-term" => { for (k, v) in crate::voice::page::term_settings(draft(session, "@f.vterm")) { send(session, hub, "setting.set", json!({"key": k, "value": v}), &mut cmds, &mut rid); } json!({"drafts": {"@f.vterm": ""}}) }
        // Machines: every press goes to the hub as machines.page.act, with the fields as typed
        a if a.starts_with("machine-") => {
            let fields = json!({"maddr": draft(session, "@f.maddr"), "mcode": draft(session, "@f.mcode"), "fcode": draft(session, "@f.fcode"), "rfrom": draft(session, "@f.rfrom"), "rname": draft(session, "@f.rname")});
            send(session, hub, "machines.page.act", json!({"action": a, "value": value, "fields": fields}), &mut cmds, &mut rid);
            if a == "machine-pair" { json!({"drafts": {"@f.maddr": "", "@f.mcode": "", "@f.fcode": ""}}) } else if a == "machine-rename" { json!({"drafts": {"@f.rfrom": "", "@f.rname": ""}}) } else { json!({}) }
        }
        // the always-on machine (net/alwayson): every press goes to the hub as alwayson.act, with its two fields as typed
        a if a.starts_with("alwayson-") => {
            let fields = json!({"addr": draft(session, "@f.aonaddr"), "code": draft(session, "@f.aoncode")});
            send(session, hub, "alwayson.act", json!({"action": a, "value": value, "fields": fields}), &mut cmds, &mut rid);
            if a == "alwayson-other-pair" { json!({"drafts": {"@f.aonaddr": "", "@f.aoncode": ""}}) } else { json!({}) }
        }
        // MCP: a press that only moves the page stays here; the rest go to the hub as mcp.page.act with the page state and what is typed (the phones send the same)
        a if a.starts_with("mcp-") => {
            let get = mcp_get(session); let ui = crate::mcpx::page::Ui { get: &get };
            if let Some(p) = crate::mcpx::page::step(&ui, a, value) {
                if a == "mcp-view" && value == "import" { send(session, hub, "mcp.page.act", json!({"action": "mcp-scan", "value": "", "desktop": true}), &mut cmds, &mut rid); }
                mcp_patch(p)
            } else {
                let (mut uiv, mut fields) = (serde_json::Map::new(), serde_json::Map::new());
                for k in crate::mcpx::page::UI_KEYS { if let Some(v) = get(k) { uiv.insert(k.into(), json!(v)); } }
                for k in crate::mcpx::page::FIELDS { if let Some(v) = get(k) { fields.insert(k.into(), json!(v)); } }
                send(session, hub, "mcp.page.act", json!({"action": a, "value": value, "ui": uiv, "fields": fields, "desktop": true}), &mut cmds, &mut rid);
                // where the page goes when the hub takes it (a refusal is said at the top of the page): back to the list, or the server's card
                let to = match a { "mcp-add" | "mcp-import" | "mcp-gallery" | "mcp-dup" => Some(String::new()), "mcp-remove" => Some(String::new()), "mcp-save" => Some(format!("open:{value}")), _ => None };
                match to { Some(v) => { let mut p = serde_json::Map::new(); p.insert("view".into(), json!(v)); p.insert("rm".into(), json!("")); p.insert("skip".into(), json!("")); p.insert("imp".into(), json!(""));
                    if a != "mcp-dup" { for f in crate::mcpx::page::FIELDS { p.insert(f.into(), Value::Null); } } mcp_patch(p) } None => json!({}) }
            }
        }
        // Notifications: every press goes to the hub as notify.page.act, with the times as typed (the phones send the same)
        a if a.starts_with("notify-") => {
            let fields = json!({"qfrom": draft(session, "@f.qfrom"), "qto": draft(session, "@f.qto")});
            send(session, hub, "notify.page.act", json!({"action": a, "value": value, "fields": fields}), &mut cmds, &mut rid);
            if a == "notify-quiet" && value != "save" { json!({"drafts": {"@f.qfrom": Value::Null, "@f.qto": Value::Null, "@quiet.err": ""}}) } else if a == "notify-quiet" {
                if quiet_spec(draft(session, "@f.qfrom"), draft(session, "@f.qto")).is_some() { json!({"drafts": {"@quiet.err": ""}}) } else { json!({"drafts": {"@quiet.err": "bad"}}) } } else { json!({}) }
        }
        "key-sent" => json!({"drafts": {"@f.kname": "", "@key.err": "", "@keys.rm": ""}}),
        "key-err" => json!({"drafts": {"@key.err": value}}),
        "bfield" => {
            let (name, text) = value.split_once('\u{1f}')?;
            if !matches!(name, "whurl" | "whsec" | "tzname" | "kpath" | "fpath" | "kname" | "vterm" | "rtext" | "btitle" | "maddr" | "mcode" | "fcode" | "rfrom" | "rname" | "qfrom" | "qto" | "aonaddr" | "aoncode" | "mpaste" | "mname" | "mcmd" | "menv" | "mcwd" | "murl" | "mhdr" | "mgarg") { return None }
            json!({"drafts": {format!("@f.{name}"): text}})
        }
        "telemetry-on" => { send(session, hub, "telemetry.choice", json!({"value": "on", "expected": value, "decision": "", "popup": false}), &mut cmds, &mut rid); json!({}) }
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

    /// Matrix S09: a Voice chip chosen in Settings is lit when the page is drawn again. The page is drawn from the lean copy of the state,
    /// which once dropped every `voice.` setting, so the engine, the model, a dictionary entry and a learned fix all looked unchosen.
    #[test] fn the_ring_page_draws_the_hubs_rows_and_sends_its_presses() {
        let s = State::new();
        let info = json!({"ring.setup": json!({"bot": "b1", "hook": "h1", "url": "https://desk.ts.net:8451/hook/h1", "token": "t0k", "test": 1_790_000_000u64}).to_string(),
            "ring.notes": json!({"q": "", "total": 1, "found": 1, "notes": [{"id": "n1", "said": 1_790_000_000u64, "gesture": "Hold & talk", "words": "Call Jeff", "model": "parakeet-v2", "conf": 91, "state": "sent", "thread": "t9", "play": "https://x/ring/audio/n1?k=c", "heard": 1}]}).to_string()});
        let r = ring(&s, &json!({"drafts": {"@f.ringq": "jeff"}}), &info);
        assert_eq!(r[1]["note"], "Arrived Mon 21 Sep 14:13. Backplane hears this webhook."); assert_eq!(r[2]["fields"][0]["value"], "jeff"); assert_eq!(r[3]["label"], "Mon 21 Sep 14:13 · Hold & talk");
        assert!(!r[0]["buttons"].as_array().unwrap().iter().any(|b| b["action"] == "share-text"), "the window has no share sheet");
        let sent = |a: &str, v: &str, session: Value| { let o = act("h", &session, a, v).unwrap(); o.cmds.iter().map(|c| serde_json::from_str::<Value>(c["text"].as_str().unwrap()).unwrap()).map(|m| (m["m"].as_str().unwrap().to_string(), m["p"].clone())).collect::<Vec<_>>() };
        assert_eq!(sent("ring-op", "hear:n1:openai", json!({})), [("ring.hear".to_string(), json!({"note": "n1", "engine": "openai"}))]);
        assert_eq!(sent("ring-search", "", json!({"drafts": {"@f.ringq": " jeff "}})), [("ring.search".to_string(), json!({"q": "jeff"}))]);
        assert_eq!(sent("settings-page", "ring", json!({})).iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["ring.search"]);
        assert!(act("h", &json!({}), "ring-op", "rm:n1").is_none());
    }
    #[test] fn voice_choices_are_lit_from_the_lean_state() {
        let mut s = State::new(); s.lean = true;
        for (k, v) in [("voice.engine", "local"), ("voice.model", "parakeet-v3"), ("voice.device", "mic2"), ("voice.e.kicad", "KiCad = key cad"), ("voice.l.jason", "JSON\t2\t0")] {
            assert!(s.apply(&json!({"$": "SettingSet", "key": k, "value": v, "at": 1})), "{k}");
        }
        let info = json!({"voice.devices": json!({"default": "mic1", "devices": [{"name": "mic1", "label": "Desk"}, {"name": "mic2", "label": "Headset"}]}).to_string()});
        let rows = voice(&s, &json!({"drafts": {}}), &info);
        let lit = |label: &str| -> Vec<String> { rows.iter().filter(|r| r["label"] == label).flat_map(|r| r["buttons"].as_array().unwrap().iter().filter(|b| b["on"] == true).map(|b| b["value"].as_str().unwrap().to_string())).collect() };
        assert_eq!(lit("Speech to text"), ["voice.engine=local"]);
        assert_eq!(lit("Local model"), ["voice.model=parakeet-v3"]);
        assert_eq!(lit("Microphone"), ["voice.device=mic2"]);
        let r = rows.iter().find(|r| r["label"] == "Dictionary").unwrap(); assert!(r["buttons"].to_string().contains("KiCad = key cad"), "{r}");
        assert_eq!(lit("Learned fixes").len(), 1, "a fix made twice and never undone applies by itself, so its chip is lit");
        // and choosing another drops the old one, as the walk does: cloud after local
        s.apply(&json!({"$": "SettingSet", "key": "voice.engine", "value": "cloud", "at": 2}));
        let rows = voice(&s, &json!({"drafts": {}}), &info);
        assert_eq!(rows[0]["buttons"].as_array().unwrap().iter().filter(|b| b["on"] == true).map(|b| b["value"].clone()).collect::<Vec<_>>(), [json!("voice.engine=cloud")]);
    }

    #[test] fn the_menu_lists_the_ported_pages_each_a_settings_page_button() {
        let v = view(&st(&[]), &json!({"open": {"settings": true}, "drafts": {}}), &json!({}));
        assert_eq!(labels(&v), ["Appearance", "Threads", "Notifications", "Agents", "MCP servers", "Writing", "CAD", "Ordering", "Network and updates", "Devices", "Machines", "Voice", "Ring", "API keys", "Time zone", "Rules and task board", "Usage", "Setup", "Import from 0.14", "About", "Privacy", "Developer"]);
        assert_eq!(v["rows"][1]["buttons"][0]["action"], "settings-page"); assert_eq!(v["rows"][1]["buttons"][0]["value"], "threads");
    }
    #[test] fn about_says_what_this_windows_browser_tab_is() {
        let about = |client: Value| { let v = view(&st(&[]), &json!({"open": {"settings": true}, "drafts": {"@settings.page": "about"}}), &json!({"client": client})); labels(&v) };
        let notes = |client: Value| { let v = view(&st(&[]), &json!({"open": {"settings": true}, "drafts": {"@settings.page": "about"}}), &json!({"client": client})); v["rows"].as_array().unwrap().iter().map(|r| r["note"].as_str().unwrap_or("").to_string()).collect::<Vec<_>>() };
        assert!(about(json!({"kind": "desktop", "browser": "frames"})).contains(&"Browser".to_string()));
        assert!(notes(json!({"kind": "desktop", "browser": "frames"})).iter().any(|n| n.starts_with("Frame mode")));
        assert!(notes(json!({"kind": "desktop", "browser": "cef"})).iter().any(|n| n.contains("Embedded Chromium")));
        assert!(!about(json!({"kind": "phone"})).contains(&"Browser".to_string()), "a phone says nothing of a browser");
    }
    #[test] fn about_says_where_the_open_source_notices_are() {
        let rows = |client: Value| { let v = view(&st(&[]), &json!({"open": {"settings": true}, "drafts": {"@settings.page": "about"}}), &json!({"client": client})); v["rows"].as_array().unwrap().clone() };
        let r = rows(json!({"kind": "desktop", "browser": "frames", "licenses": "/opt/bp/builds/v0.15.0/licenses"}));
        let n = r.iter().find(|r| r["label"] == "Open-source notices").expect("a notices row");
        assert!(n["note"].as_str().unwrap().contains("/opt/bp/builds/v0.15.0/licenses"));
        assert_eq!(n["buttons"][0]["action"], "open-licenses"); assert_eq!(n["buttons"][0]["value"], "", "the client opens its own folder, never a path the hub sends");
        // a client without one (a phone, a dev build): only the hub's own folder, named and never opened, and none at all in a test binary
        assert!(rows(json!({"kind": "phone"})).iter().filter(|r| r["label"] == "Open-source notices").all(|r| r["buttons"].as_array().unwrap().is_empty()));
    }
    #[test] fn developer_replays_setup_and_the_star_suggestion() {
        let sess = json!({"open": {"settings": true}, "drafts": {"@settings.page": "developer"}, "rid": 5});
        let v = view(&st(&[("star.nudge.v1", r#"{"status":"dismissed","ids":["a","b","c","d","e"],"token":"e"}"#)]), &sess, &json!({}));
        assert_eq!(labels(&v), ["Developer", "Replay first run", "Star suggestion"]);
        assert_eq!(v["rows"][1]["buttons"][0]["action"], "ob-replay"); assert_eq!(v["rows"][2]["note"], "Dismissed"); assert_eq!(v["rows"][2]["buttons"][0]["action"], "dev-star");
        let o = act("h", &sess, "dev-star", "").unwrap();
        assert_eq!(o.cmds.len(), 1); assert!(o.cmds[0]["text"].as_str().unwrap().contains(r#""m":"star.replay""#)); assert_eq!(o.patch["open"]["settings"], false, "closed, so the strip shows");
    }
    #[test] fn devices_lists_what_the_registry_says_and_offers_revoke_on_the_live_ones() {
        let info = json!({"devices": {"devices": [{"id": "d1", "name": "Phone", "revoked": false}, {"id": "d2", "name": "Old", "revoked": true}],
            "invitations": [{"id": "i1", "kind": "device", "state": "open"}, {"id": "i2", "kind": "machine", "state": "open"}, {"id": "i3", "kind": "device", "state": "used", "deviceName": "Phone", "deviceRevoked": true}]}});
        let r = pg_rows("devices", &[], json!({}), info);
        assert_eq!(r[1]["label"], "Add a device"); assert_eq!(r[1]["buttons"][0]["action"], "device-invite");
        let by = |l: &str| r.iter().find(|x| x["label"] == l).unwrap_or_else(|| panic!("{l}")).clone();
        assert_eq!((by("Phone")["buttons"][0]["action"].as_str(), by("Phone")["buttons"][0]["value"].as_str()), (Some("device-revoke"), Some("d1")));
        assert!(r.iter().all(|x| x["label"] != "Old"), "a revoked device is history, not listed");
        assert_eq!(by("Pairing link · open")["buttons"][0]["action"], "device-uninvite");
        assert!(r.iter().all(|x| x["label"] != "Pairing link · used · Phone"), "a used link whose device was revoked is history, not listed");
        assert_eq!(r.len(), 1 + 1 + 1 + 1, "a machine invite is Bots' and is not listed");
    }
    #[test] fn a_device_link_that_names_only_this_computer_says_so() {
        let inv = |local: bool| json!({"link": "http://127.0.0.1:3797/pair#invite=x", "code": "ABCD2345", "expiresAt": FAR, "routes": [if local { "http://127.0.0.1:3797" } else { "http://100.64.1.2:3797" }], "localOnly": local}).to_string();
        let r = pg_rows("devices", &[], json!({}), json!({}));
        assert!(r.iter().all(|x| x["label"] != "Only this computer"), "no invitation, no note");
        let v = view(&st(&[]), &json!({"open": {"settings": true}, "drafts": {"@settings.page": "devices", "@invite": inv(true)}}), &json!({}));
        let w = v["rows"].as_array().unwrap().iter().find(|x| x["label"] == "Only this computer").cloned().expect("the loopback note");
        assert_eq!((w["note"].as_str(), w["tone"].as_str()), (Some(LOCAL_DEVICE), Some("warn")));
        let v = view(&st(&[]), &json!({"open": {"settings": true}, "drafts": {"@settings.page": "devices", "@invite": inv(false)}}), &json!({}));
        assert!(v["rows"].as_array().unwrap().iter().all(|x| x["label"] != "Only this computer"), "a reachable link has no note");
    }
    const FAR: u64 = 4_000_000_000_000;
    /// An invitation as `routes::AuthRoutes::invite` makes it: one code, an address of each kind, each its own link.
    fn invite3(lan_listening: bool) -> String {
        json!({"id": "i9", "code": "GZCYJVVP", "expiresAt": FAR, "link": "http://100.101.102.103:3871/pair#invite=t&routes=a", "routes": ["http://100.101.102.103:3871"], "localOnly": false, "addresses": [
            {"kind": "tailnet", "url": "http://100.101.102.103:3871", "link": "http://100.101.102.103:3871/pair#invite=t&routes=a", "listening": true},
            {"kind": "lan", "url": "http://192.168.77.2:3871", "link": "http://192.168.77.2:3871/pair#invite=t&routes=b", "listening": lan_listening},
            {"kind": "local", "url": "http://127.0.0.1:3871", "link": "http://127.0.0.1:3871/pair#invite=t&routes=c", "listening": true}]}).to_string()
    }
    fn dev_rows(drafts: Value, devices: Value) -> Vec<Value> {
        let mut d = json!({"@settings.page": "devices"}); for (k, v) in drafts.as_object().unwrap() { d[k] = v.clone() }
        view(&st(&[]), &json!({"open": {"settings": true}, "drafts": d}), &json!({"devices": devices, "now": 1_000_000_000_000u64}))["rows"].as_array().unwrap().clone()
    }
    fn by<'a>(r: &'a [Value], l: &str) -> &'a Value { r.iter().find(|x| x["label"] == l).unwrap_or_else(|| panic!("no row {l}: {r:?}")) }
    /// Add a device: Tailscale first, its own link and QR code, the code beside the hub address and the expiry; the link and code copy.
    #[test] fn devices_an_invitation_shows_the_chosen_address_link_qr_code_and_expiry() {
        let r = dev_rows(json!({"@invite": invite3(false)}), json!({"devices": [], "invitations": [], "lan": [], "host": "127.0.0.1"}));
        let pick = by(&r, "Reach this hub by");
        let chips: Vec<(&str, &str, bool)> = pick["buttons"].as_array().unwrap().iter().map(|b| (b["label"].as_str().unwrap(), b["value"].as_str().unwrap(), b["on"] == true)).collect();
        assert_eq!(chips, [("Tailscale", "http://100.101.102.103:3871", true), ("Local network", "http://192.168.77.2:3871", false), ("This computer", "http://127.0.0.1:3871", false)]);
        assert!(pick["note"].as_str().unwrap().contains("Tailscale"));
        let l = by(&r, "Pairing link");
        assert_eq!(l["note"], "http://100.101.102.103:3871/pair#invite=t&routes=a");
        assert_eq!(l["qr"], json!(crate::net::qr::text("http://100.101.102.103:3871/pair#invite=t&routes=a").lines().collect::<Vec<_>>()), "the QR code of that address's own link");
        let qn = l["qrnote"].as_str().unwrap();
        assert!(qn.starts_with("Code GZCYJVVP · hub 100.101.102.103:3871 · works once, until "), "{qn}");
        let acts: Vec<(&str, &str)> = l["buttons"].as_array().unwrap().iter().map(|b| (b["action"].as_str().unwrap(), b["value"].as_str().unwrap())).collect();
        assert_eq!(acts, [("copy", "http://100.101.102.103:3871/pair#invite=t&routes=a"), ("copy", "GZCYJVVP"), ("device-invite", "")]);
        assert!(r.iter().all(|x| x["label"] != "Only this computer"));
    }
    /// Local network on a loopback hub: no link to hand out, the offer to answer there (plain HTTP said); once it answers, the link, and Stop.
    #[test] fn devices_the_local_network_is_offered_then_shown_then_stopped() {
        let want = json!({"@invite": invite3(false), "@pair.url": "http://192.168.77.2:3871"});
        let r = dev_rows(want.clone(), json!({"devices": [], "invitations": [], "lan": [], "host": "127.0.0.1"}));
        assert!(r.iter().all(|x| x["label"] != "Pairing link"), "no link for an address the hub does not answer on");
        let offer = by(&r, "Local network");
        assert!(offer["note"].as_str().unwrap().contains("plain HTTP"));
        assert_eq!((offer["buttons"][0]["action"].as_str(), offer["buttons"][0]["value"].as_str()), (Some("device-lan-on"), Some("192.168.77.2")));
        assert_eq!(by(&r, "Reach this hub by")["buttons"][1]["on"], true);
        // the person let it: the same invitation's local network link (its listening flag from the hub now, not from when it was made)
        let lan = json!([{"ip": "192.168.77.2", "url": "http://192.168.77.2:3871", "listening": true}]);
        let r = dev_rows(want.clone(), json!({"devices": [], "invitations": [], "lan": lan, "host": "127.0.0.1"}));
        assert_eq!(by(&r, "Pairing link")["note"], "http://192.168.77.2:3871/pair#invite=t&routes=b");
        assert!(by(&r, "Pairing link")["qrnote"].as_str().unwrap().contains("hub 192.168.77.2:3871"));
        assert!(by(&r, "Reach this hub by")["note"].as_str().unwrap().contains("not encrypted"));
        assert_eq!(by(&r, "Local network · http://192.168.77.2:3871")["buttons"][0]["action"], "device-lan-off", "stopped under Devices");
        // a hub on every interface answers there anyway: nothing to stop
        let r = dev_rows(want, json!({"devices": [], "invitations": [], "lan": [], "host": "0.0.0.0"}));
        assert!(r.iter().any(|x| x["label"] == "Pairing link") && r.iter().all(|x| x["label"] != "Local network"));
    }
    #[test] fn devices_this_computer_warns_and_an_expired_link_says_so() {
        let r = dev_rows(json!({"@invite": invite3(true), "@pair.url": "http://127.0.0.1:3871"}), json!({"devices": [], "invitations": [], "lan": [], "host": "127.0.0.1"}));
        assert_eq!(by(&r, "Only this computer")["note"], "This link works only on this computer. Choose Tailscale or Local network to pair another device.");
        let mut old: Value = serde_json::from_str(&invite3(true)).unwrap(); old["expiresAt"] = json!(999_999_000_000u64);
        let r = dev_rows(json!({"@invite": old.to_string()}), json!({}));
        let x = by(&r, "Pairing link"); assert!(x["note"].as_str().unwrap().starts_with("Expired at ") && x.get("qr").is_none(), "{x}");
        assert_eq!(x["buttons"][0]["action"], "device-invite");
        // used meanwhile (a phone scanned it): no QR code to scan again
        let r = dev_rows(json!({"@invite": invite3(true)}), json!({"invitations": [{"id": "i9", "kind": "device", "state": "used", "deviceName": "Pixel"}]}));
        let x = by(&r, "Pairing link"); assert_eq!((x["note"].as_str(), x.get("qr").is_none()), (Some("Used by Pixel. Each link pairs one device."), true));
        // a lan choice that failed says why
        let r = dev_rows(json!({"@lan.err": "The hub could not listen on 192.168.77.2: the port may be taken there."}), json!({}));
        assert_eq!(by(&r, "Local network")["tone"], "bad");
    }
    #[test] fn loopback_addresses_and_invites() {
        for u in ["http://127.0.0.1:3797", "https://localhost", "http://[::1]:1", "http://127.1.2.3:9"] { assert!(loopback(u), "{u}") }
        for u in ["http://100.64.84.41:3787", "https://desktop.tail.ts.net:8451", "http://192.168.1.5:3787", ""] { assert!(!loopback(u), "{u}") }
        let sec = "e4af8fb618176ac9cd1bf9fe7d62e765f20914b4d3d3aeae60ae3bf5ac91571a";
        assert!(loopback_invite(&format!("bp1:http://127.0.0.1:3811:pd0302ba9e290b6e4:{sec}")));
        assert!(!loopback_invite(&format!("bp1:http://desktop.tail.ts.net:3787:pd0302ba9e290b6e4:{sec}")));
        assert!(!loopback_invite("nonsense") && !loopback_invite(""));
    }
    fn pg_rows(page: &str, kv: &[(&str, &str)], drafts: Value, info: Value) -> Vec<Value> {
        let v = view(&st(kv), &json!({"open": {"settings": true}, "drafts": {"@settings.page": page, "x": drafts}}), &info); v["rows"].as_array().unwrap().clone()
    }
    #[test] fn writing_offers_its_providers_and_the_providers_models() {
        let info = json!({"models": r#"{"claude":["haiku","sonnet"],"codex":["gpt-6"]}"#});
        let r = pg_rows("writing", &[("text.provider", "claude")], json!({}), info);
        assert_eq!(r[1]["buttons"].as_array().unwrap().iter().map(|b| b["label"].as_str().unwrap()).collect::<Vec<_>>(), ["claude", "codex", "grok", "muse"]);
        assert_eq!(r[1]["buttons"][0]["on"], true);
        assert_eq!(r[2]["buttons"].as_array().unwrap().iter().map(|b| b["value"].as_str().unwrap()).collect::<Vec<_>>(), ["text.model=", "text.model=haiku", "text.model=sonnet"]);
    }
    #[test] fn the_privacy_page_has_its_two_link_buttons_and_the_issue_form_names_the_version_os_and_arch() {
        let r = pg_rows("privacy", &[], json!({}), json!({"os": "linux", "arch": "x64"}));
        let b = r[1]["buttons"].as_array().unwrap();
        let open: Vec<&Value> = b.iter().filter(|x| x["action"] == "open-url").collect();
        assert_eq!(open.len(), 2); assert_eq!(open[0]["value"], "https://backplane.works/privacy");
        let u = open[1]["value"].as_str().unwrap(); assert!(u.starts_with("https://github.com/BackplaneWorks/Backplane/issues/new?body=Version%3A") && u.contains("OS%3A%20linux%20x64"), "{u}");
        assert!(bug_url(&json!({"os": "plan9"})).contains("OS%3A%20unknown%20unknown"));
    }
    #[test] fn api_keys_list_names_only_and_forgetting_asks_once() {
        let dir = crate::plat::tmp::uncreated("keys-page");
        crate::settings::keys::add(&dir, "OPENAI_API_KEY", "sk-secret-123").unwrap();
        let info = json!({"home": dir.to_str().unwrap()});
        let r = pg_rows("keys", &[], json!({}), info.clone());
        let all = serde_json::to_string(&r).unwrap(); assert!(all.contains("OPENAI_API_KEY") && !all.contains("sk-secret-123"), "a key's value is never in a row");
        assert!(r.iter().any(|x| x["fields"].as_array().map_or(false, |f| f.iter().any(|f| f["vault"] == true && f["secret"] == true && f["value"] == ""))), "the key field is sealed");
        // press once: armed; the same again: the request, and nothing in between goes to the hub
        let s0 = json!({"rid": 1, "drafts": {}}); let o = act("h", &s0, "key-del", "OPENAI_API_KEY").unwrap(); assert!(o.cmds.is_empty()); assert_eq!(o.patch["drafts"]["@keys.rm"], "OPENAI_API_KEY");
        let s1 = json!({"rid": 1, "drafts": {"@keys.rm": "OPENAI_API_KEY"}}); let o = act("h", &s1, "key-del", "OPENAI_API_KEY").unwrap(); assert_eq!(o.cmds.len(), 1);
        assert!(o.cmds[0]["text"].as_str().unwrap().contains("api-del"));
        assert!(act("h", &s0, "key-err", "Name the key first.").unwrap().patch["drafts"]["@key.err"] == "Name the key first.");
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test] fn the_fallback_hierarchy_edits_its_chain_by_setting_and_adds_only_installed_models() {
        let info = json!({"models": r#"{"claude":["opus"],"codex":["gpt-6"]}"#});
        let r = pg_rows("agents", &[("fallback.chain", "claude/opus,codex/gpt-6"), ("fallback.on", "on")], json!({}), info.clone());
        let labels: Vec<&str> = r.iter().map(|x| x["label"].as_str().unwrap()).collect();
        assert!(labels.contains(&"1. Claude opus") && labels.contains(&"2. Codex gpt-6"), "{labels:?}");
        let second = r.iter().find(|x| x["label"] == "2. Codex gpt-6").unwrap();
        assert_eq!(second["buttons"][0]["value"], "fallback.chain=codex/gpt-6,claude/opus", "Move up");
        assert_eq!(second["buttons"][2]["value"], "fallback.chain=claude/opus", "Remove");
        let enabled = r.iter().find(|x| x["label"] == "Failover order").unwrap(); assert_eq!(enabled["buttons"][0]["on"], true);
        assert_eq!(r.iter().find(|x| x["label"] == "Default order").unwrap()["buttons"][0]["value"], "fallback.chain=claude,codex,antigravity,grok");
        // nothing set: on, in the default order, each provider on its default model
        let r = pg_rows("agents", &[], json!({}), info);
        let labels: Vec<&str> = r.iter().map(|x| x["label"].as_str().unwrap()).collect();
        assert!(labels.contains(&"1. Claude") && labels.contains(&"2. Codex") && labels.contains(&"3. Antigravity") && labels.contains(&"4. Grok") && !labels.contains(&"Default order"), "{labels:?}");
        assert_eq!(r.iter().find(|x| x["label"] == "Failover order").unwrap()["buttons"][0]["on"], true, "on until turned off");
        assert_eq!(r.iter().find(|x| x["label"] == "2. Codex").unwrap()["buttons"][0]["value"], "fallback.chain=codex,claude,antigravity,grok");
    }
    #[test] fn cad_keeps_the_typed_path_and_sets_it_on_a_press() {
        let r = pg_rows("cad", &[], json!({}), json!({"kicad.cli": "/usr/bin/kicad-cli", "kicad.version": "10.0.7"}));
        assert!(r[1]["note"].as_str().unwrap().contains("10.0.7")); assert_eq!(r[1]["fields"][0]["name"], "kpath");
        let s = json!({"rid": 3, "drafts": {"@f.kpath": " /opt/kicad/bin "}}); let o = act("h", &s, "kicad-path", "").unwrap();
        assert!(o.cmds[0]["text"].as_str().unwrap().contains(r#""key":"kicad.path","value":"/opt/kicad/bin""#));
    }
    fn cad_rows(info: Value, drafts: Value, pkg: Option<&crate::files::kicad::install::Package>) -> Vec<Value> {
        let r = cad_with(&st(&[]), &json!({"drafts": drafts}), &info, pkg); r
    }
    fn btns(r: &Value) -> Vec<(String, String)> { r["buttons"].as_array().unwrap().iter().map(|b| (b["label"].as_str().unwrap().to_string(), b["action"].as_str().unwrap().to_string())).collect() }
    #[test] fn install_kicad_shows_only_while_kicad_is_not_found() {
        use crate::files::kicad::install::LINUX;
        let found = cad_rows(json!({"kicad.cli": "/usr/bin/kicad-cli", "kicad.version": "10.0.7"}), json!({}), Some(&LINUX));
        assert!(!found.iter().any(|r| r["label"].as_str().unwrap().contains("Install")), "{found:?}");
        assert!(!cad_rows(json!({"kicad.status": "checking"}), json!({}), Some(&LINUX)).iter().any(|r| r["label"].as_str().unwrap().contains("Install")));
        let r = cad_rows(json!({"kicad.status": "missing", "home": "/h"}), json!({}), Some(&LINUX));
        assert_eq!(r[0]["note"], "KiCad was not found. Install it below, or set the folder of one you have.");
        assert_eq!(r[1]["label"], "Install KiCad"); assert_eq!(btns(&r[1]), [("Install KiCad".to_string(), "kicad-install-ask".to_string())]);
        assert!(r[1]["note"].as_str().unwrap().contains("10.99.0-bp.10"));
        // no package for this computer: said, nothing to press
        let r = cad_rows(json!({"kicad.status": "missing"}), json!({}), None);
        assert!(r[1]["note"].as_str().unwrap().starts_with("No Backplane KiCad is published for this computer") && btns(&r[1]).is_empty());
    }
    #[test] fn install_kicad_asks_first_then_shows_progress_failure_and_the_result() {
        use crate::files::kicad::install::LINUX;
        let ask = cad_rows(json!({"kicad.status": "missing", "home": "/h"}), json!({"@kinstall": "ask"}), Some(&LINUX));
        assert_eq!(ask[1]["label"], "Install Backplane KiCad?");
        let n = ask[1]["note"].as_str().unwrap(); assert!(n.contains("github.com/i2cjak/Backplane_KiCad") && n.contains("/h/kicad") && n.contains("no administrator password"), "{n}");
        assert_eq!(btns(&ask[1]), [("Install".to_string(), "kicad-install".to_string()), ("Cancel".to_string(), "kicad-install-cancel".to_string())]);
        let run = cad_rows(json!({"kicad.status": "missing", "kicad.install": r#"{"state":"running","version":"10.99.0-bp.10","note":"Unpacking"}"#}), json!({}), Some(&LINUX));
        assert_eq!((run[1]["label"].as_str().unwrap(), run[1]["note"].as_str().unwrap()), ("Installing Backplane KiCad 10.99.0-bp.10", "Unpacking. This takes about a minute.")); assert!(btns(&run[1]).is_empty());
        let bad = cad_rows(json!({"kicad.status": "missing", "kicad.install": r#"{"state":"failed","note":"KiCad install failed: KiCad archive checksum mismatch"}"#}), json!({}), Some(&LINUX));
        assert_eq!(bad[1]["note"], "KiCad install failed: KiCad archive checksum mismatch."); assert_eq!(btns(&bad[1])[0].1, "kicad-install-ask");
        let done = cad_rows(json!({"kicad.cli": "/h/kicad/10.99.0-bp.10/bin/kicad-cli", "kicad.version": "10.99.0-bp.10", "kicad.install": r#"{"state":"done","note":"Installed Backplane KiCad 10.99.0-bp.10 in /h/kicad/10.99.0-bp.10"}"#}), json!({}), Some(&LINUX));
        assert_eq!(done[0]["note"], "version 10.99.0-bp.10 · /h/kicad/10.99.0-bp.10/bin/kicad-cli. Installed by Backplane just now.");
        assert_eq!(done[1]["label"], "FreeCAD");
    }
    /// An older Backplane KiCad (bp.4 when bp.8 is published) gets the Update row, through the same ask; the current one gets none.
    #[test] fn an_older_backplane_kicad_is_offered_the_update_row() {
        use crate::files::kicad::install::LINUX;
        let old = json!({"kicad.cli": "/h/kicad/10.99.0-bp.4/bin/kicad-cli", "kicad.version": "10.99.0-bp.4", "kicad.status": "ok"});
        let r = cad_rows(old.clone(), json!({}), Some(&LINUX));
        assert_eq!(r[1]["label"], "Update Backplane KiCad"); assert_eq!(btns(&r[1])[0].1, "kicad-install-ask");
        let ask = cad_rows(old, json!({"@kinstall": "ask"}), Some(&LINUX)); assert_eq!(btns(&ask[1])[0], ("Update".to_string(), "kicad-install".to_string()));
        let cur = cad_rows(json!({"kicad.cli": "/h/kicad/10.99.0-bp.10/bin/kicad-cli", "kicad.version": "10.99.0-bp.10", "kicad.status": "ok"}), json!({}), Some(&LINUX));
        assert_eq!(cur[1]["label"], "FreeCAD", "no update row for the current one");
    }
    #[test] fn install_is_sent_only_after_the_ask_and_cancel_sends_nothing() {
        let o = act("h", &json!({"rid": 4, "drafts": {}}), "kicad-install-ask", "").unwrap(); assert!(o.cmds.is_empty()); assert_eq!(o.patch["drafts"]["@kinstall"], "ask");
        let o = act("h", &json!({"rid": 4, "drafts": {"@kinstall": "ask"}}), "kicad-install-cancel", "").unwrap(); assert!(o.cmds.is_empty()); assert_eq!(o.patch["drafts"]["@kinstall"], "");
        let o = act("h", &json!({"rid": 4, "drafts": {}}), "kicad-install", "").unwrap(); assert!(o.cmds.is_empty(), "no consent without the ask");
        let o = act("h", &json!({"rid": 4, "drafts": {"@kinstall": "ask"}}), "kicad-install", "").unwrap();
        assert_eq!(o.cmds[0]["text"].as_str().unwrap(), r#"{"id":4,"m":"kicad.install","p":{"consent":true}}"#); assert_eq!(o.patch["drafts"]["@kinstall"], "");
    }
    #[test] fn a_kicad_path_that_holds_no_kicad_is_said_to_be_passed_over() {
        let used = json!({"kicad.cli": "/usr/bin/kicad-cli", "kicad.version": "10.0.6"});
        let bad = cad_with(&st(&[("kicad.path", "/nonexistent/kicad-cli")]), &json!({"drafts": {}}), &used, None);
        assert_eq!(bad[0]["note"], "version 10.0.6 · /usr/bin/kicad-cli. Nothing runs at /nonexistent/kicad-cli, so this one is used.");
        let folder = cad_with(&st(&[("kicad.path", "/opt/kicad/")]), &json!({"drafts": {}}), &json!({"kicad.cli": "/opt/kicad/bin/kicad-cli", "kicad.version": "9.0.1"}), None);
        assert_eq!(folder[0]["note"], "version 9.0.1 · /opt/kicad/bin/kicad-cli", "a folder that holds the one used is fine");
        assert_eq!(cad_with(&st(&[]), &json!({"drafts": {}}), &used, None)[0]["note"], "version 10.0.6 · /usr/bin/kicad-cli");
    }
    #[test] fn cad_sets_the_freecad_path_on_a_press() {
        let r = pg_rows("cad", &[], json!({}), json!({"kicad.cli": "/usr/bin/kicad-cli"}));
        assert_eq!(r[2]["fields"][0]["name"], "fpath"); assert_eq!(r[2]["buttons"][0]["action"], "freecad-path");
        let s = json!({"rid": 3, "drafts": {"@f.fpath": " /opt/fc/bin/freecadcmd "}}); let o = act("h", &s, "freecad-path", "").unwrap();
        assert!(o.cmds[0]["text"].as_str().unwrap().contains(r#""key":"freecad.path","value":"/opt/fc/bin/freecadcmd""#));
    }
    #[test] fn usage_adds_the_thread_totals_per_provider() {
        let mut s = st(&[("tok.t.a", "2/100/50/0/30/0/2000000000/2/2"), ("tok.t.b", "1/10/0/0/5/0/0/0/1"), ("tok.t.c", "3/1/1/1/1/0/0/0/3")]);
        for (id, p) in [("a", "claude"), ("b", "claude"), ("c", "codex"), ("d", "codex")] { s.apply(&json!({"$":"ThreadCreated","id":id,"project":"p","title":id,"at":1,"env":"local","branch":"","worktree":"","provider":p,"model":"m"})); }
        let v = view(&s, &json!({"drafts": {"@settings.page": "usage"}}), &json!({}));
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows[1..].iter().map(|r| r["label"].as_str().unwrap()).collect::<Vec<_>>(), ["claude", "codex", "All threads"]);
        let note = |l: &str| rows.iter().find(|r| r["label"] == l).unwrap()["note"].as_str().unwrap().to_string();
        assert_eq!(note("claude"), "3 turns · input 110 · cache read 50 · cache write 0 · output 35 · $2.00");
        assert!(note("codex").starts_with("3 turns") && note("codex").ends_with("cost not reported"));
        assert!(note("All threads").starts_with("6 turns · input 111"), "{}", note("All threads"));
        assert_eq!(note_of_empty(), "No turns counted yet.");
    }
    fn note_of_empty() -> String { view(&st(&[]), &json!({"drafts": {"@settings.page": "usage"}}), &json!({}))["rows"][1]["note"].as_str().unwrap().to_string() }
    #[test] fn a_page_leads_with_the_way_back_and_marks_the_current_chip() {
        let v = view(&st(&[("settle.idle.minutes", "30"), ("restart.continue", "off")]), &json!({"drafts": {"@settings.page": "threads"}}), &json!({}));
        assert_eq!(v["rows"][0]["buttons"][0]["value"], ""); assert_eq!(v["rows"][0]["buttons"][0]["action"], "settings-page");
        let grace = &v["rows"][1]; assert_eq!(grace["label"], "Worker grace (minutes)");
        let on: Vec<&str> = grace["buttons"].as_array().unwrap().iter().filter(|b| b["on"] == true).map(|b| b["label"].as_str().unwrap()).collect();
        assert_eq!(on, ["30"]); assert!(grace["note"].as_str().unwrap().contains("after 30 idle minutes"));
        let restart = v["rows"].as_array().unwrap().iter().find(|r| r["label"] == "Continue after a restart").unwrap();
        assert_eq!(restart["buttons"][1]["on"], true, "off is lit");
        let env = v["rows"].as_array().unwrap().iter().find(|r| r["label"] == "New threads work in").unwrap();
        assert_eq!((env["buttons"][0]["value"].as_str(), env["buttons"][0]["on"].as_bool()), (Some("thread.env=auto"), Some(true)), "auto by default");
        let v = view(&st(&[("thread.env", "local")]), &json!({"drafts": {"@settings.page": "threads"}}), &json!({}));
        assert_eq!(v["rows"].as_array().unwrap().iter().find(|r| r["label"] == "New threads work in").unwrap()["buttons"][1]["on"], true, "local is lit");
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
        assert_eq!(page(&json!({"drafts": {"@settings.page": "usage"}})), "usage"); assert_eq!(page(&json!({"drafts": {"@settings.page": "nope"}})), "", "an unknown page is the menu");
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
        assert_eq!(r[2]["note"], PAIR_ELSEWHERE); assert_eq!((r[2]["buttons"][0]["action"].as_str(), r[2]["buttons"][0]["value"].as_str()), (Some("settings-page"), Some("devices")), "it opens Devices");
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
        assert_eq!((b[0]["on"].as_bool(), b[1]["on"].as_bool()), (Some(false), Some(true)));
        // On goes through telemetry.choice with the token of what the page showed (setting.set cannot re-enable)
        assert_eq!(b[0]["action"], "telemetry-on");
        let o = act("h", &json!({"rid": 7}), "telemetry-on", b[0]["value"].as_str().unwrap()).unwrap();
        let sent: Value = serde_json::from_str(o.cmds[0]["text"].as_str().unwrap()).unwrap();
        assert_eq!((sent["m"].as_str(), sent["p"]["value"].as_str(), sent["p"]["expected"].as_str()), (Some("telemetry.choice"), Some("on"), b[0]["value"].as_str())); assert_eq!(b[2]["value"], "telemetry.reset=4");
        assert!(!r[1]["note"].as_str().unwrap().contains("Install id"), "no id while off");
    }
    #[test] fn the_update_line_says_what_the_hub_is_doing() {
        let r = |s: &str| update_row("0.15.0", s);
        assert_eq!(r("")["note"], "Updated by its package."); assert_eq!(r(r#"{"state":"package"}"#)["buttons"], json!([]), "nothing to press");
        assert_eq!(r(r#"{"state":"idle"}"#)["note"], "Up to date, checked hourly."); assert_eq!(r(r#"{"state":"idle"}"#)["buttons"][0]["action"], "update-check");
        assert_eq!(r(r#"{"state":"waiting","available":"v9"}"#)["buttons"][0]["action"], "update-now", "waiting on turns: Update now goes ahead"); assert!(r(r#"{"state":"idle"}"#)["buttons"].as_array().unwrap().iter().all(|b| b["action"] != "update-now"));
        assert_eq!(r(r#"{"state":"idle","note":"Updated to v0.16.0."}"#)["note"], "Updated to v0.16.0.");
        assert_eq!(r(r#"{"state":"waiting","available":"v0.16.0"}"#)["note"], "v0.16.0 installs when running turns finish.");
        assert_eq!(r(r#"{"state":"failed","note":"v0.16.0 failed its health check, so this build stayed."}"#)["buttons"][0]["label"], "Check now");
        assert_eq!(r(r#"{"state":"installing","available":"v0.16.0"}"#)["buttons"], json!([]), "nothing to press while it goes"); assert_eq!(r("not json")["note"], "Updated by its package.");
    }
    #[test] fn a_stored_nightly_lights_beta() { assert_eq!((update_channel("nightly"), update_channel("beta"), update_channel("stable"), update_channel("x")), ("beta", "beta", "stable", "stable")); }
    #[test] fn rules_and_the_board_are_listed_and_edited_from_settings() {
        let mut st = State::new();
        for e in [json!({"$":"SettingSet","key":"rule.r1","value":r#"{"id":"r1","text":"Never push to main","tool":"Bash","enforce":true,"word":"push","on":true,"at":2}"#,"at":2}),
            json!({"$":"SettingSet","key":"board.b1","value":r#"{"id":"b1","title":"Order the boards","status":"todo","due":"2026-12-01","at":3}"#,"at":3}),
            json!({"$":"SettingSet","key":"board.b2","value":r#"{"id":"b2","title":"Old","status":"dropped","at":1}"#,"at":3})] { assert!(st.apply(&e)); }
        let v = view(&st, &json!({"drafts": {"@settings.page": "rules"}}), &json!({}));
        let rows = v["rows"].as_array().unwrap(); let labels: Vec<&str> = rows.iter().map(|r| r["label"].as_str().unwrap()).collect();
        assert_eq!(labels, ["Rules and task board", "Standing rules", "Never push to main", "New rule", "Task board", "Order the boards", "New item"], "a dropped item is not listed");
        assert_eq!(rows[2]["note"], "On · refuses Bash unless you say \"push\""); assert_eq!(rows[5]["note"], "todo · due 2026-12-01");
        let sent = |o: Out| o.cmds[0]["text"].as_str().unwrap().to_string();
        assert_eq!(sent(act("h", &json!({"rid": 3, "drafts": {"@f.rtext": " Ask first "}}), "rule-add", "").unwrap()), r#"{"id":3,"m":"rules.set","p":{"rule":{"text":"Ask first"}}}"#);
        assert!(act("h", &json!({"rid": 3, "drafts": {"@f.rtext": "  "}}), "rule-add", "").unwrap().cmds.is_empty(), "nothing typed, nothing sent");
        assert_eq!(sent(act("h", &json!({"rid": 4}), "rule-on", "r1=false").unwrap()), r#"{"id":4,"m":"rules.set","p":{"rule":{"id":"r1","on":false}}}"#);
        assert_eq!(sent(act("h", &json!({"rid": 5}), "board-done", "b1").unwrap()), r#"{"id":5,"m":"board.set","p":{"item":{"id":"b1","status":"done"}}}"#);
        assert_eq!(sent(act("h", &json!({"rid": 6}), "board-drop", "b1").unwrap()), r#"{"id":6,"m":"board.drop","p":{"id":"b1"}}"#);
    }
    #[test] fn check_now_asks_the_hub() { assert_eq!(act("h", &json!({"rid": 3}), "update-check", "").unwrap().cmds[0]["text"], json!(r#"{"id":3,"m":"update.check","p":{}}"#)); }
    #[test] fn agents_offers_the_hubs_models_with_default_first_and_the_current_lit() {
        let info = json!({"models": json!({"claude": ["fable", "opus"], "about": {"claude": {"cli": "", "labels": {"default": "Claude Code's own default", "opus": "Opus 5.5"}}}}).to_string()});
        let r = page_rows(&st(&[("agent.model", "opus")]), "agents", info);
        let m = r[3]["buttons"].as_array().unwrap().iter().map(|b| (b["label"].as_str().unwrap().to_string(), b["on"].as_bool().unwrap())).collect::<Vec<_>>();
        assert_eq!(m, [("default".to_string(), false), ("fable".to_string(), false), ("opus".to_string(), true)]);
        assert_eq!(r[3]["note"], "opus: Opus 5.5"); assert_eq!(r[2]["buttons"][0]["on"], true, "claude is the default provider");
        let r = page_rows(&st(&[("agent.provider", "codex")]), "agents", json!({}));
        assert_eq!(r[2]["note"], "codex is not installed");
    }

    #[test] fn the_boat_hub_rows_show_its_cost_and_warn_before_the_plan_runs_out() {
        use super::boat_hub_rows as rows;
        assert!(rows("", 0).is_empty()); assert!(rows("not json", 0).is_empty());
        let now = 1_794_350_000u64; let day = 86_400u64;
        let c = |left: f64, end: u64, measured: u64| serde_json::json!({"box":"bp-hub","type":"small","rate_hour":0.018,"multiplier":0.5,"used_hours":2.0,"used_dollars":0.04,
            "plan_hours":2777.8,"plan_hours_left":left,"period_end_s":end,"measured":measured}).to_string();
        let r = rows(&c(2418.0, now + 23 * day, now - 300), now);
        assert_eq!(r.len(), 1, "{r:?}"); assert_eq!(r[0]["label"], "Always-on hub: bp-hub (small)");
        let n = r[0]["note"].as_str().unwrap(); assert!(n.contains("$0.018 an hour") && n.contains("$12.96 a month") && n.contains("2,418 of 2,778 h left") && n.contains("5 min ago"), "{n}");
        // 23 days at half rate needs 276 h: 400 left is under twice that, 200 is under it
        let w = rows(&c(400.0, now + 23 * day, now), now); assert_eq!(w[1]["tone"], "warn", "{w:?}");
        let b = rows(&c(200.0, now + 23 * day, now), now); assert_eq!((b[1]["label"].as_str(), b[1]["tone"].as_str()), (Some("Boat plan hours run out"), Some("bad")));
        // a tenth of the plan warns even near the end of the period
        assert_eq!(rows(&c(250.0, now + day, now), now)[1]["tone"], "warn");
        let s = rows(&c(2418.0, now + 23 * day, now - 2 * day), now); assert_eq!(s.last().unwrap()["label"], "Boat cost not measured");
    }
}

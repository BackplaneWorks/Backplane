//! The always-on machine (docs/ALWAYS-ON.md sections 1 step 4 and 2): the person's own Boat account (D1: their key in the vault, `boat.api_key`,
//! by Boat's browser sign-in or pasted), the sizes with their monthly cost and the memory the hub needs (D8), making the Boat (`make.rs`), and
//! Settings, Machines' always-on row: status, build, cost this month, plan hours left, Resize, Restart, Delete. Or another machine of the person's
//! (`other.rs`). This hub checks the Boat every 5 minutes and alerts once when it stays down.
//!
//! Kept: the setting `alwayson.boat` = `{id, name, size, created, network, url, link, made}` (the person's bp-hub is adopted into it once:
//! `adopt`). Shown: the info `alwayson` (one JSON line: account, sign-in, sizes, the run's steps, the Boat's status), which `rows.rs` draws for
//! Settings and the setup's step 4 alike. Presses: the RPC `alwayson.act {action, value, fields}`.
pub mod boat;
pub mod make;
pub mod other;
pub mod public;
pub mod rows;
pub mod sizes;
pub mod tunnel;

use crate::{api::{Caller, Fut, RpcHandler, Ticker}, hub::Hub};
use boat::Boat;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

pub const SETTING: &str = "alwayson.boat";
/// Setup's step 4 choice (here, boat, machine): present/onboard_alwayson.rs reads it.
pub const CHOICE_KEY: &str = "alwayson.choice";
/// The vault name of the Tailscale auth key the Boat joins the tailnet with.
pub const TS_KEY: &str = "tailscale.boat";
/// The person's own always-on Boat, made by hand before this existed (docs/BOAT-HUB.md): adopted, never touched beyond reading it.
pub const BP_HUB: (&str, &str) = ("bx_479dqxby", "bp-hub");

#[derive(Clone, Debug)]
pub struct Opts { pub size: String, pub dictation: bool, pub network: String, pub name: String, pub host: String }
impl Default for Opts { fn default() -> Opts { Opts { size: String::new(), dictation: true, network: String::new(), name: "Backplane always-on".into(), host: "bp-always-on".into() } } }

#[derive(Default)]
struct St {
    account: Value, account_err: String, account_at: u64,
    login: Value, login_err: String,
    opts: Opts, steps: Vec<Value>, running: bool,
    saved: Value,
    status: Value, down_since: u64, alerted: bool, cost: Value, cost_at: u64, busy: String, note: String, move_err: String,
}
/// What the run, the RPC and the ticker share.
#[derive(Default)]
pub struct Shared { st: Mutex<St> }
impl Shared {
    pub fn opts(&self) -> Opts { self.st.lock().unwrap().opts.clone() }
    pub fn saved(&self) -> Value { self.st.lock().unwrap().saved.clone() }
    pub fn box_id(&self) -> String { self.st.lock().unwrap().saved["id"].as_str().unwrap_or("").to_string() }
    pub fn step(&self, id: &str, state: &str, note: &str) {
        let mut g = self.st.lock().unwrap();
        if g.steps.is_empty() { g.steps = make::STEPS.iter().map(|(i, l)| json!({"id": i, "label": l, "state": "todo", "note": ""})).collect() }
        if let Some(s) = g.steps.iter_mut().find(|s| s["id"] == id) { s["state"] = json!(state); s["note"] = json!(note); }
    }
    pub fn end(&self) { self.st.lock().unwrap().running = false; }
    /// The info line every client draws from.
    pub fn view(&self, hub_info: &Value) -> Value {
        let g = self.st.lock().unwrap();
        let types: Vec<String> = g.account["types"].as_array().into_iter().flatten().filter_map(|t| t.as_str().map(str::to_string)).collect();
        let rate = g.account["rate_hour"].as_f64().unwrap_or(sizes::DEFAULT_RATE);
        let has_key = crate::settings::vault::person_get(boat::KEY).map_or(false, |k| !k.trim().is_empty());
        let tailnet = on_tailnet(hub_info);
        let network = if g.opts.network.is_empty() { if tailnet { "tailnet" } else { "public" } } else { g.opts.network.as_str() };
        json!({
            "key": has_key, "account": g.account, "account_err": g.account_err, "login": g.login, "login_err": g.login_err,
            "sizes": sizes::list(rate, g.opts.dictation, 2, &types, &g.opts.size), "need": sizes::need_line(g.opts.dictation, 2),
            "dictation": g.opts.dictation, "network": network, "tailnet": tailnet,
            "ts_key": crate::settings::vault::person_get(TS_KEY).map_or(false, |k| !k.trim().is_empty()),
            "steps": g.steps, "running": g.running, "boat": g.saved, "status": g.status, "down_since": g.down_since, "cost": g.cost,
            "busy": g.busy, "note": g.note, "move": serde_json::from_str::<Value>(hub_info["home.move"].as_str().unwrap_or("")).unwrap_or(Value::Null),
            "move_err": g.move_err,
        })
    }
    pub async fn publish(&self, hub: &Arc<Hub>) { let v = self.view(&hub.info_now()); hub.set_info("alwayson", &v.to_string()).await; }
}

/// This machine is on Tailscale (its tailnet listener reached the tailnet).
pub fn on_tailnet(info: &Value) -> bool { matches!(info["tailscale"].as_str(), Some("reachable")) || matches!(info["tailnet"].as_str(), Some("https")) }

/// Change `alwayson.boat` (read, `f`, written back as one SettingSet) and the copy the run reads.
pub async fn save(hub: &Arc<Hub>, sh: &Shared, f: impl FnOnce(&mut Value)) {
    let mut v = sh.saved(); if !v.is_object() { v = json!({}) }
    f(&mut v);
    let body = if v.as_object().map_or(true, |o| o.is_empty()) { String::new() } else { v.to_string() };
    let _ = hub.commit(&json!({"$":"SettingSet","key":SETTING,"value":body,"at":crate::hub::now_ms() / 1000})).await;
    sh.st.lock().unwrap().saved = if body.is_empty() { Value::Null } else { v };
}

static SHARED: std::sync::OnceLock<Arc<Shared>> = std::sync::OnceLock::new();
pub fn shared() -> Arc<Shared> { SHARED.get_or_init(Default::default).clone() }

/// The always-on row and the setup's step 4 read `alwayson`; the RPC; the ticker; the public listener when this hub is that Boat's.
pub fn install(hub: &Arc<Hub>) {
    let sh = shared();
    hub.register_rpc(Arc::new(Rpc(sh.clone())));
    hub.register_ticker(Arc::new(Keep { sh: sh.clone() }));
    public::start_from_env(hub);
    let (h, s) = (hub.clone(), sh);
    tokio::spawn(async move {
        let saved = h.setting(SETTING).await.and_then(|t| serde_json::from_str::<Value>(&t).ok()).filter(Value::is_object);
        s.st.lock().unwrap().saved = saved.clone().unwrap_or(Value::Null);
        if saved.is_none() { adopt(&h, &s).await }
        other::load(&h).await;
        refresh_account(&h, &s).await;
        s.publish(&h).await;
    });
}

/// The person's bp-hub, made by hand (docs/BOAT-HUB.md), adopted as their always-on Boat: only on the hub whose Boat it is (the desktop that made it
/// keeps `~/.config/backplane/boat-hub/id`, or boat-hub itself), once, a setting only. The running box and its service are not touched.
pub async fn adopt(hub: &Arc<Hub>, sh: &Shared) {
    if cfg!(test) || std::env::var("BACKPLANE_ALWAYSON_ADOPT").map_or(false, |v| v == "0") { return }
    let conf = std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config/backplane/boat-hub/id"));
    let id = conf.and_then(|p| std::fs::read_to_string(p).ok()).map(|s| s.trim().to_string()).unwrap_or_default();
    if id != BP_HUB.0 { return }
    let url = "https://boat-hub.tail9e0cf2.ts.net";
    save(hub, sh, |k| { *k = json!({"id": BP_HUB.0, "name": BP_HUB.1, "size": "small", "created": "", "network": "tailnet", "url": url, "made": true, "adopted": true}); }).await;
}

/// The account, read again (on connect, and at most every 10 minutes for the row).
async fn refresh_account(hub: &Arc<Hub>, sh: &Shared) {
    let Some(b) = Boat::from_vault(&hub.cfg.home) else { let mut g = sh.st.lock().unwrap(); g.account = Value::Null; g.account_err.clear(); return };
    let r = b.account().await;
    let mut g = sh.st.lock().unwrap();
    g.account_at = crate::hub::now_ms() / 1000;
    match r { Ok(a) => { g.account = a; g.account_err.clear(); } Err(e) => { g.account_err = e; } }
}

/// The start of this month, as Boat's usage API takes it.
pub fn month_start(now_s: u64) -> String {
    let days = now_s / 86_400; let (y, m, _) = civil(days as i64);
    format!("{y:04}-{m:02}-01T00:00:00Z")
}
fn civil(z: i64) -> (i64, u32, u32) { let z = z + 719_468; let era = z.div_euclid(146_097); let doe = z - era * 146_097; let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; let y = yoe + era * 400; let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); let mp = (5 * doy + 2) / 153; let d = (doy - (153 * mp + 2) / 5 + 1) as u32; let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; (if m <= 2 { y + 1 } else { y }, m, d) }

/// Every 5 minutes: the Boat's state and its hub (through the link, else its address), its cost this month and the plan hours left.
/// Down for 15 minutes: one alert, and the row says so; up again clears it.
struct Keep { sh: Arc<Shared> }
impl Ticker for Keep {
    fn every_s(&self) -> u64 { 10 }
    fn tick<'a>(&'a self, hub: &'a Arc<Hub>, now_s: u64) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let sh = &self.sh;
            // a key just pasted into the masked box: the account is read within a few seconds
            let stale = { let g = sh.st.lock().unwrap(); g.account.is_null() && g.account_err.is_empty() && now_s >= g.account_at + 5 };
            if stale && crate::settings::vault::person_get(boat::KEY).is_some() { refresh_account(hub, sh).await; sh.publish(hub).await; }
            let (id, last) = { let g = sh.st.lock().unwrap(); (g.saved["id"].as_str().unwrap_or("").to_string(), g.cost_at) };
            if id.is_empty() || (last != 0 && now_s < last + 300) || sh.st.lock().unwrap().running { return vec![] }
            sh.st.lock().unwrap().cost_at = now_s;
            check(hub, sh, &id, now_s).await;
            vec![]
        })
    }
}
pub async fn check(hub: &Arc<Hub>, sh: &Arc<Shared>, id: &str, now_s: u64) {
    let url = sh.saved()["url"].as_str().unwrap_or("").to_string();
    let up = !url.is_empty() && boat::send(&hub.cfg.home, boat::Call::new("GET", format!("{url}/healthz"), "").secs(15)).await.code == "200";
    let mut status = json!({"hub": if up { "up" } else { "down" }, "at": now_s});
    if let Some(b) = Boat::from_vault(&hub.cfg.home) {
        if let Ok(s) = b.sandbox(id).await { status["state"] = s["state"].clone(); status["size"] = s["type"].clone(); }
        if let Ok(u) = b.usage(id, &month_start(now_s)).await {
            let mult = u["billingMultiplier"].as_f64().unwrap_or(1.0).max(0.01);
            sh.st.lock().unwrap().cost = json!({"dollars": u["dollars"], "hours": u["seconds"].as_f64().unwrap_or(0.0) / 3600.0 / mult, "multiplier": mult, "measured": now_s});
        }
        refresh_account(hub, sh).await;
    }
    // a main-build home's Boat takes this hub's build (make::follow), once the hub there answers
    let saved = sh.saved();
    if up && saved["made"] == true && saved["adopted"] != true {
        if let Some(b) = Boat::from_vault(&hub.cfg.home) {
            match make::follow(hub, &b, id, saved["build"].as_str().unwrap_or(""), saved["network"].as_str().unwrap_or("")).await {
                Ok(Some(n)) => { save(hub, sh, |k| { k["build"] = json!(n); }).await; }
                Ok(None) => {}
                Err(e) => { sh.st.lock().unwrap().note = format!("The Boat could not take this computer's build: {e}"); }
            }
        }
    }
    let alert = {
        let mut g = sh.st.lock().unwrap();
        g.status = status;
        if up { g.down_since = 0; g.alerted = false; None }
        else { if g.down_since == 0 { g.down_since = now_s } if !g.alerted && now_s >= g.down_since + 900 { g.alerted = true; Some(g.saved["name"].as_str().unwrap_or("The always-on Boat").to_string()) } else { None } }
    };
    if let Some(name) = alert { tell_down(hub, &name, now_s).await }
    sh.publish(hub).await;
}
/// One alert to the person's phones (the push gate's `alwayson` source), beside the row's red line.
async fn tell_down(hub: &Arc<Hub>, name: &str, at: u64) {
    let text = format!("{name} has not answered for 15 minutes. Your uberbot, reminders and bots there are stopped until it is back. Settings, Machines can restart it.");
    let _ = hub.commit(&crate::push::Source::alwayson_down().order(&format!("alwayson-down-{at}"), "needs_you", "", "", &text, at)).await;
}

struct Rpc(Arc<Shared>);
impl RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["alwayson.act", "alwayson.view"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _w: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if method == "alwayson.view" { return vec![json!({"t":"reply","id":id,"ok":true,"view": self.0.view(&hub.info_now())})] }
            let s = |k: &str| p[k].as_str().unwrap_or("").to_string();
            let r = act(hub, &self.0, &s("action"), &s("value"), &p["fields"]).await;
            self.0.publish(hub).await;
            match r { Ok(x) => { let mut o = json!({"t":"reply","id":id,"ok":true}); if let (Some(m), Some(e)) = (o.as_object_mut(), x.as_object()) { for (k, v) in e { m.insert(k.clone(), v.clone()); } } vec![o] }
                Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":e})] }
        })
    }
}

/// What a press does (the setup's step 4 and Settings, Machines send the same actions).
pub async fn act(hub: &Arc<Hub>, sh: &Arc<Shared>, action: &str, value: &str, fields: &Value) -> Result<Value, String> {
    let home = hub.cfg.home.clone();
    match action {
        // the account: Boat's browser sign-in (best-effort), or the key pasted into the masked box (the client saves it to the vault itself)
        "alwayson-login" => {
            let provider = if value == "google" { "google" } else { "github" };
            match boat::login_start(&home, provider).await {
                Ok(l) => { { let mut g = sh.st.lock().unwrap(); g.login = l.clone(); g.login_err.clear(); }
                    let (h, s) = (hub.clone(), sh.clone()); tokio::spawn(async move { login_wait(h, s).await });
                    Ok(json!({"url": l["url"], "code": l["code"]})) }
                Err(e) => { sh.st.lock().unwrap().login_err = e.clone(); Err(e) }
            }
        }
        "alwayson-login-cancel" => { let mut g = sh.st.lock().unwrap(); g.login = Value::Null; g.login_err.clear(); Ok(json!({})) }
        "alwayson-account" => { refresh_account(hub, sh).await; let g = sh.st.lock().unwrap(); if g.account_err.is_empty() { Ok(json!({"account": g.account})) } else { Err(g.account_err.clone()) } }
        "alwayson-forget" => { crate::settings::vault::del(boat::KEY); let mut g = sh.st.lock().unwrap(); g.account = Value::Null; g.account_err.clear(); Ok(json!({})) }
        "alwayson-size" => { if sizes::of(value).is_none() { return Err("no such size".into()) } sh.st.lock().unwrap().opts.size = value.into(); Ok(json!({})) }
        "alwayson-dictation" => { sh.st.lock().unwrap().opts.dictation = value != "off"; Ok(json!({})) }
        "alwayson-network" => { if !matches!(value, "tailnet" | "public") { return Err("tailnet or public".into()) } sh.st.lock().unwrap().opts.network = value.into(); Ok(json!({})) }
        // Make it, and Try again from the step that failed
        "alwayson-make" | "alwayson-retry" => {
            let b = Boat::from_vault(&home).ok_or("connect your Boat account first")?;
            {
                let mut g = sh.st.lock().unwrap();
                if g.running { return Err("it is being made already".into()) }
                let lacks: Vec<String> = g.account["missing"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect();
                if !lacks.is_empty() && action == "alwayson-make" { return Err(format!("this key lacks {}: make one with {}", lacks.join(", "), boat::key_command())) }
                if g.saved["made"] == true && action == "alwayson-make" { return Err("this computer has an always-on Boat already".into()) }
                let info = hub.info_now();
                if g.opts.size.is_empty() { let types: Vec<String> = g.account["types"].as_array().into_iter().flatten().filter_map(|t| t.as_str().map(str::to_string)).collect(); g.opts.size = sizes::pick(g.opts.dictation, 2, &types).into(); }
                if g.opts.network.is_empty() { g.opts.network = if on_tailnet(&info) { "tailnet".into() } else { "public".into() }; }
                g.running = true;
            }
            let from = if action == "alwayson-retry" { let g = sh.st.lock().unwrap(); g.steps.iter().find(|s| s["state"] == "fail").and_then(|s| s["id"].as_str()).unwrap_or("").to_string() } else { sh.st.lock().unwrap().steps.clear(); String::new() };
            let (h, s) = (hub.clone(), sh.clone());
            tokio::spawn(async move { make::run(h, s, b, from).await });
            Ok(json!({}))
        }
        // Delete: two presses (the first arms it and says what goes); the Boat and all its data, the link, the setting
        "alwayson-delete" => {
            let id = sh.box_id(); if id.is_empty() { return Err("no always-on Boat".into()) }
            if sh.saved()["adopted"] == true && id == BP_HUB.0 { return Err("bp-hub was made by hand: delete it with the boat CLI (docs/BOAT-HUB.md)".into()) }
            if sh.st.lock().unwrap().busy != "delete" { sh.st.lock().unwrap().busy = "delete".into(); return Ok(json!({"armed": true})) }
            let b = Boat::from_vault(&home).ok_or("connect your Boat account first")?;
            if sh.st.lock().unwrap().running { return Err("wait for the step that runs to end".into()) }
            // off the person's tailnet first (best-effort: a Boat that does not answer is deleted all the same)
            if sh.saved()["network"] == "tailnet" { let _ = b.sh(&id, "sudo -n tailscale logout", 60).await; }
            b.delete(&id).await.or_else(|e| if e.contains("404") { Ok(()) } else if e.contains("api_key_action_forbidden") || e.contains("403") { Err("This key may delete only the Boats it made, and another key made this one. Delete it on boat.dev, or paste a key with sandbox.delete.".to_string()) } else { Err(e) })?;
            let link = sh.saved()["link"].as_str().unwrap_or("").to_string();
            if let (false, Some(p)) = (link.is_empty(), crate::net::start::peering()) {
                let name = p.table.with(|t| t.peer(&link).map(|r| r.name.clone()).unwrap_or_default());
                if hub.commit(&json!({"$":"PeerSet","id":link,"name":"","url":"","revoked":true,"at":crate::hub::now_ms() / 1000})).await.is_ok() { p.unpeered(&link); crate::net::farsync::forget_link(hub, &link, &name).await; }
            }
            save(hub, sh, |k| { *k = json!({}); }).await;
            let mut g = sh.st.lock().unwrap(); g.busy.clear(); g.steps.clear(); g.status = Value::Null; g.cost = Value::Null; g.note = format!("{id} deleted.");
            Ok(json!({"deleted": id}))
        }
        "alwayson-cancel" => { sh.st.lock().unwrap().busy.clear(); Ok(json!({})) }
        // Restart: the hub's service there (the Boat itself is power-cycled only when it does not answer)
        "alwayson-restart" | "alwayson-resize" if sh.saved()["adopted"] == true => Err("bp-hub was made by hand: restart or resize it with scripts/boat-hub.sh (docs/BOAT-HUB.md)".into()),
        "alwayson-restart" => {
            let id = sh.box_id(); let b = Boat::from_vault(&home).ok_or("connect your Boat account first")?;
            match b.sh(&id, "sudo -n systemctl restart bp-hub", 90).await {
                Ok(_) => { sh.st.lock().unwrap().note = "Restarting the hub there.".into(); Ok(json!({})) }
                Err(_) => { b.stop(&id).await.ok(); b.resume(&id, "").await?; sh.st.lock().unwrap().note = "The Boat did not answer: it was stopped and started again.".into(); Ok(json!({})) }
            }
        }
        // Resize: stop, resume on the new size (Boat keeps the disk), no auto-stop again
        "alwayson-resize" => {
            let id = sh.box_id(); if sizes::of(value).is_none() { return Err("no such size".into()) }
            let b = Boat::from_vault(&home).ok_or("connect your Boat account first")?;
            sh.st.lock().unwrap().busy = format!("resize:{value}");
            let (h, s, v) = (hub.clone(), sh.clone(), value.to_string());
            tokio::spawn(async move {
                let r = async { b.stop(&id).await?; for _ in 0..60 { if b.sandbox(&id).await.map_or(false, |x| x["state"] == "archived") { break } tokio::time::sleep(std::time::Duration::from_secs(3)).await; }
                    b.resume(&id, &v).await?; b.keep_on(&id).await.ok(); Ok::<(), String>(()) }.await;
                { let mut g = s.st.lock().unwrap(); g.busy.clear(); g.note = match &r { Ok(()) => format!("Now {v}."), Err(e) => format!("Resize failed: {e}") }; }
                if r.is_ok() { save(&h, &s, |k| { k["size"] = json!(v); }).await; }
                s.publish(&h).await;
            });
            Ok(json!({}))
        }
        // Move your uberbot there: a fresh setup has no uberbot yet at this step; it is made here first (as setup makes Bub on the way to Done),
        // then moved by the home move like any other. Any refusal is kept for the row.
        "alwayson-move" => {
            sh.st.lock().unwrap().move_err.clear();
            let r = move_there(hub, value).await;
            if let Err(e) = &r { sh.st.lock().unwrap().move_err = e.clone(); }
            r
        }
        "alwayson-check" => { let id = sh.box_id(); if !id.is_empty() { check(hub, sh, &id, crate::hub::now_ms() / 1000).await } Ok(json!({})) }
        // another machine of the person's as the always-on one
        a if a.starts_with("alwayson-other") => other::act(hub, a, value, fields).await,
        _ => Err("unknown action".into()),
    }
}

/// Make the uberbot here when there is none (the `onboard.uber` request setup sends on the way to Done), wait for it, then start the home move to `link`.
pub async fn move_there(hub: &Arc<Hub>, link: &str) -> Result<Value, String> {
    if link.is_empty() { return Err("which machine?".into()) }
    if crate::net::homemove::uber(hub).await.is_empty() {
        if crate::bots::api::of(hub).is_none() { return Err("bots are not running on this computer, so there is no uberbot to move".into()) }
        let h = hub.rpc_for("onboard.uber").ok_or("this hub cannot make an uberbot")?;
        let _ = h.call(hub, &Caller { gen: String::new(), client: hub.fresh_client() }, &json!(0), "onboard.uber", &json!({})).await;
        let mut made = false;
        for _ in 0..100 { if !crate::net::homemove::uber(hub).await.is_empty() { made = true; break } tokio::time::sleep(std::time::Duration::from_millis(100)).await }
        if !made { return Err("the uberbot could not be made here, so there was nothing to move".into()) }
    }
    crate::net::homemove::start(hub, link).await
}

/// The browser sign-in, polled every 2 s for ten minutes; the token goes to the vault and the account is read.
async fn login_wait(hub: Arc<Hub>, sh: Arc<Shared>) {
    let poll = sh.st.lock().unwrap().login["poll"].as_str().unwrap_or("").to_string();
    for _ in 0..300 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        if sh.st.lock().unwrap().login["poll"].as_str() != Some(poll.as_str()) { return } // cancelled or started again
        match boat::login_poll(&hub.cfg.home, &poll).await {
            Ok(None) => continue,
            Ok(Some(tok)) => {
                // the sign-in may do everything: it makes a key of only what the Boat needs, keeps that, and ends the sign-in's session
                let session = Boat::with_key(&hub.cfg.home, &tok);
                let name = format!("backplane-always-on-{}", crate::net::botnet::name().await.replace(' ', "-"));
                let (keep, note) = match session.make_key(&name).await {
                    Ok(k) => { let _ = session.logout().await; (k, String::new()) }
                    Err(e) => (tok.clone(), format!("Signed in, but Boat would not make a narrower key: {e}. This sign-in is kept as it is, and it can do everything on your account.")),
                };
                let r = crate::settings::vault::put(boat::KEY, &keep, "Boat account for the always-on machine", "hub");
                { let mut g = sh.st.lock().unwrap(); g.login = Value::Null; g.login_err = if !r.starts_with("ok") { format!("signed in, but the key could not be kept: {r}") } else { note }; }
                refresh_account(&hub, &sh).await; sh.publish(&hub).await; return
            }
            Err(e) => { { let mut g = sh.st.lock().unwrap(); g.login = Value::Null; g.login_err = e; } sh.publish(&hub).await; return }
        }
    }
    { let mut g = sh.st.lock().unwrap(); g.login = Value::Null; g.login_err = "The sign-in link expired. Try again, or paste an API key.".into(); }
    sh.publish(&hub).await;
}

/// Called after the masked box saved `boat.api_key` (the client's `secret-sent`): the account is read at once.
pub async fn key_saved(hub: &Arc<Hub>) { let sh = shared(); refresh_account(hub, &sh).await; sh.publish(hub).await; }

#[cfg(test)]
mod tests;

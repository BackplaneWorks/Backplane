//! Moving the person's home between their linked machines (docs/BOAT-HUB.md): the uberbot, the other bots, rooms, hooks, mail, memories,
//! routines, the vault, APNs and the paired devices go from this hub to another one, with their ids, and this hub keeps its projects. The same
//! job moves it back (the receiver skips the lines it already holds). One owner at a time: the sender retires what it sent the moment the
//! receiver holds it all, and a move whose check fails is undone at once.
//!
//! Requests (this hub's clients): `home.move {to}` (a linked machine's link id or name; "back" asks the machine that holds the home now to move
//! it here), `home.status`, `hub.offer.take {id}`, `hub.offer.done {id, ok}`. Far requests (an owner's machine, signed): `home.state`,
//! `home.adopt`, `home.secret`, `home.file`, `home.import`, `home.finish`, `home.retire`, `home.mint`, `home.pull`.
//! Settings: `home.at` (JSON `{link, name, url, at}`: the machine that holds the home now; "" when this one does), `home.here` (on the holder:
//! `{from, at}`). Info: `home.move` (the job's state), `hub.offer` (docs/PHONE-DATA.md: a client adds the holder to its hubs by itself).
use crate::{api::{Caller, Fut, RpcHandler}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}, sync::{Arc, Mutex}};

fn now_s() -> u64 { now_ms() / 1000 }
/// The uberbot of this hub, as the bots lane folds it (UberSet is not a SettingSet: `Hub::setting` does not have it); "" for none.
pub async fn uber(hub: &Arc<Hub>) -> String { match crate::bots::api::of(hub) { Some(b) => b.world().await.uber_bot(), None => String::new() } }
fn gs<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
/// A far request's JSON body is at most 256 KiB (net/rpc.rs `body_max`): what one carries.
const CHUNK: usize = 150 << 10;

// ---- what moves (pure) ----

/// What a move carries, worked out from the sender's whole log.
#[derive(Debug, Default)]
pub struct Carry {
    /// the lines, as they are in the log
    pub lines: Vec<String>,
    pub bots: Vec<String>, pub rooms: Vec<String>, pub hooks: Vec<String>,
    /// the uberbot's id ("" none)
    pub uber: String,
    /// committed on the receiver after the lines: each live bot (its thread as its last BotSet that named one), room and hook, and the uberbot, at `now`
    pub revive: Vec<Value>,
    /// committed on the sender once the receiver holds it all
    pub retire: Vec<Value>,
    /// the 64-hex names the lines mention (message images are kept under them)
    pub hashes: HashSet<String>,
}
const SKIP_T: &[&str] = &["UberOp", "LockChanged", "SessionSet", "WorktreeSet", "PeerSet", "ProjectCreated", "ProjectRemoved", "TaskDelegated", "TaskDelivered", "TaskStateSet", "PushCleared"];
const DEVICE_T: &[&str] = &["DeviceRegistered", "DevicePrefs", "DeviceDropped"];
/// Settings of this machine, or naming it, or a project thread's: they stay.
const SKIP_K: &[&str] = &["machine.", "peer.", "uber.", "projects.", "probe:", "lock.", "locks.", "accounts.", "account.", "allowance.", "limit.", "session.", "tz.checked",
    "restart.", "liveness.", "hub.", "update.", "sandbox.", "far.", "home.", "push.apns.", "ring.front"];

fn subject(e: &Value) -> &str {
    for k in ["thread", "room", "bot", "hook"] { let v = gs(e, k); if !v.is_empty() { return v } }
    gs(e, "id")
}
fn hex64(text: &str, out: &mut HashSet<String>) {
    let b = text.as_bytes(); let mut i = 0;
    while i + 64 <= b.len() {
        if b[i].is_ascii_hexdigit() && (i == 0 || !b[i - 1].is_ascii_hexdigit()) {
            let mut j = i; while j < b.len() && b[j].is_ascii_hexdigit() { j += 1 }
            if j - i == 64 { out.insert(text[i..j].to_ascii_lowercase()); }
            i = j;
        } else { i += 1 }
    }
}
pub fn carry(lines: &[String], now: u64) -> Carry {
    let ev: Vec<Option<Value>> = lines.iter().map(|l| serde_json::from_str::<Value>(l).ok()).collect();
    let (mut desk, mut bots, mut rooms, mut hooks): (HashSet<String>, HashMap<String, Value>, HashMap<String, Value>, HashMap<String, Value>) = Default::default();
    let mut thread_of: HashMap<String, String> = HashMap::new(); let mut uber: Option<Value> = None;
    let (mut bot_order, mut room_order, mut hook_order) = (vec![], vec![], vec![]);
    for e in ev.iter().flatten() {
        let id = gs(e, "id").to_string();
        match gs(e, "$") {
            "ThreadCreated" | "ThreadForked" if !gs(e, "project").is_empty() => { desk.insert(id); }
            "ProjectCreated" => { desk.insert(id); }
            "BotSet" => { if !gs(e, "thread").is_empty() { thread_of.insert(id.clone(), gs(e, "thread").into()); } if !bots.contains_key(&id) { bot_order.push(id.clone()) } bots.insert(id, e.clone()); }
            "BotDeleted" => { bots.remove(&id); }
            "RoomSet" => { if !rooms.contains_key(&id) { room_order.push(id.clone()) } rooms.insert(id, e.clone()); }
            "RoomDeleted" => { rooms.remove(&id); }
            "HookSet" => { if e["revoked"] == true { hooks.remove(&id); } else { if !hooks.contains_key(&id) { hook_order.push(id.clone()) } hooks.insert(id, e.clone()); } }
            "UberSet" => { uber = if gs(e, "bot").is_empty() { None } else { Some(e.clone()) }; }
            _ => {}
        }
    }
    let mut c = Carry::default();
    for (l, e) in lines.iter().zip(ev.iter()) {
        let Some(e) = e else { continue };
        let t = gs(e, "$");
        if SKIP_T.contains(&t) { continue }
        if t == "SettingSet" {
            let k = gs(e, "key");
            if SKIP_K.iter().any(|p| k.starts_with(p)) || k.replace(':', ".").split('.').any(|p| desk.contains(p)) { continue }
        } else if !DEVICE_T.contains(&t) {
            let s = subject(e);
            if s.is_empty() || desk.contains(s) { continue }
            if ["parent", "by", "source"].iter().any(|f| desk.contains(gs(e, f))) { continue }
        }
        hex64(l, &mut c.hashes);
        c.lines.push(l.clone());
    }
    let ub = uber.as_ref().map(|u| gs(u, "bot").to_string()).unwrap_or_default();
    for id in bot_order.iter().filter(|b| bots.contains_key(*b)) {
        let mut b = bots[id].clone(); if gs(&b, "thread").is_empty() { b["thread"] = json!(thread_of.get(id).cloned().unwrap_or_default()) } b["at"] = json!(now);
        c.revive.push(b); c.retire.push(json!({"$":"BotDeleted","id":id,"at":now})); c.bots.push(id.clone());
    }
    for id in room_order.iter().filter(|r| rooms.contains_key(*r)) { let mut r = rooms[id].clone(); r["at"] = json!(now); c.revive.push(r); c.retire.push(json!({"$":"RoomDeleted","id":id,"at":now})); c.rooms.push(id.clone()); }
    for id in hook_order.iter().filter(|h| hooks.contains_key(*h)) { let mut h = hooks[id].clone(); h["at"] = json!(now); c.revive.push(h); c.retire.push(json!({"$":"HookSet","id":id,"bot":"","name":"","revoked":true,"at":now})); c.hooks.push(id.clone()); }
    if let Some(mut u) = uber { u["at"] = json!(now); c.revive.push(u); c.retire.insert(0, json!({"$":"UberSet","bot":"","uid":"","at":now})); }
    c.uber = ub;
    c
}
/// The retire lines a sender wrote are not news for the receiver: they leave the lines written after the carry was taken.
pub fn not_retire(line: &str, retire: &[Value]) -> bool {
    let Ok(e) = serde_json::from_str::<Value>(line) else { return false };
    !retire.iter().any(|r| r["$"] == e["$"] && r["id"] == e["id"] && r["bot"] == e["bot"] && r["at"] == e["at"])
}
pub fn line_hash(l: &str) -> [u8; 32] { Sha256::digest(l.as_bytes()).into() }

/// A file path a move may write: relative, inside one of the roots it knows, nothing that climbs out.
pub fn rel_ok(root: &str, rel: &str) -> bool {
    if rel.is_empty() || rel.starts_with('/') || rel.contains('\\') || rel.split('/').any(|p| p.is_empty() || p == "." || p == "..") { return false }
    match root {
        "bots" | "apns" => true,
        "home" => ["mail/", "ring/", "message-images/", "secrets/hooks/"].iter().any(|p| rel.starts_with(p)) || rel == "secrets/ring.key",
        _ => false,
    }
}
fn root_dir(hub: &Hub, root: &str) -> Option<PathBuf> {
    match root {
        "bots" => Some(crate::bots::relocate::root(&hub.cfg.home)),
        "home" => Some(hub.cfg.home.clone()),
        "apns" => std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/backplane/apns")),
        _ => None,
    }
}

// ---- the receiving side (far requests from an owner's machine) ----

static SEEN: Mutex<Option<HashSet<[u8; 32]>>> = Mutex::new(None);

pub async fn serve(hub: &Arc<Hub>, link: &str, m: &str, p: &Value) -> Value {
    let ok = |v: Value| { let mut v = v; v["ok"] = json!(true); v };
    let no = |e: &str| json!({"ok": false, "error": e});
    match m {
        "home.state" => {
            let uber = uber(hub).await;
            ok(json!({"uber": uber, "version": hub.cfg.version, "lines": hub.store.len(), "here": hub.setting("home.here").await.unwrap_or_default(), "at": hub.setting("home.at").await.unwrap_or_default()}))
        }
        "home.adopt" => {
            let ds = p["devices"].as_array().cloned().unwrap_or_default();
            match crate::net::start::registry(&hub.cfg.home) { Some(r) => ok(json!({"added": r.adopt(&ds).unwrap_or(0)})), None => no("no device registry") }
        }
        "home.secret" => {
            let (name, value, purpose) = (gs(p, "name"), gs(p, "value"), gs(p, "purpose"));
            use crate::settings::vault as v;
            let r = if v::reserved(name) { if v::own_get(name).is_some() { "kept".to_string() } else { v::own_put(name, value, purpose) } }
                    else if v::names().iter().any(|n| n == name) { "kept".to_string() } else { v::put(name, value, purpose, "home move") };
            if r.starts_with("fail") { no(&r) } else { ok(json!({})) }
        }
        "home.file" => {
            let (root, rel) = (gs(p, "root"), gs(p, "rel"));
            if !rel_ok(root, rel) { return no("not a path a move writes") }
            let Some(dir) = root_dir(hub, root) else { return no("no such root") };
            use base64::Engine as _;
            let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(gs(p, "b64")) else { return no("not base64") };
            let (path, first, secret) = (dir.join(rel), p["off"].as_u64().unwrap_or(0) == 0, root == "apns" || rel.starts_with("secrets/"));
            let rel = rel.to_string(); let root = root.to_string();
            let r = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
                use std::io::Write; use crate::plat::fs::OpenOptionsExt;
                if let Some(d) = path.parent() { std::fs::create_dir_all(d)?; }
                // a file this hub has already (a move back): replaced, whatever its mode (git keeps its objects read-only)
                if first && std::fs::symlink_metadata(&path).map_or(false, |m| m.is_file()) { std::fs::remove_file(&path)?; }
                let mut o = std::fs::OpenOptions::new(); o.create(true).write(true); if first { o.truncate(true); } else { o.append(true); } if secret { o.mode(0o600); }
                let mut f = o.open(&path)?; f.write_all(&bytes)?; f.sync_all()
            }).await;
            match r { Ok(Ok(())) => ok(json!({})), Ok(Err(e)) => no(&format!("could not write {root}/{rel}: {e}")), Err(_) => no("could not write it") }
        }
        "home.import" => {
            let lines: Vec<String> = p["lines"].as_array().map(|a| a.iter().filter_map(|l| l.as_str().map(str::to_string)).collect()).unwrap_or_default();
            if lines.iter().any(|l| l.contains('\n') || serde_json::from_str::<Value>(l).is_err()) { return no("a line that is not one JSON event") }
            // the lines this hub holds already (a move back carries what came from here): taken once
            let fresh: Vec<String> = {
                let store = hub.store.clone();
                let mut g = SEEN.lock().unwrap();
                if g.is_none() { *g = Some(store.all().iter().map(|l| line_hash(l)).collect()); }
                let seen = g.as_mut().unwrap();
                lines.into_iter().filter(|l| seen.insert(line_hash(l))).collect()
            };
            if fresh.is_empty() { return ok(json!({"took": 0})) }
            let parsed: Vec<Value> = fresh.iter().filter_map(|l| serde_json::from_str(l).ok()).collect();
            let n = fresh.len();
            match hub.import_lines(fresh).await {
                Ok(end) => { crate::statehost::fold_quiet(hub, end - n as u64 + 1, parsed); ok(json!({"took": n})) }
                Err(e) => no(&format!("{e:#}")),
            }
        }
        "home.finish" => {
            *SEEN.lock().unwrap() = None;
            let mut ev: Vec<Value> = p["revive"].as_array().cloned().unwrap_or_default().into_iter().filter(|e| matches!(gs(e, "$"), "BotSet" | "RoomSet" | "HookSet" | "UberSet")).collect();
            let now = now_s();
            ev.push(json!({"$":"SettingSet","key":"home.here","value":json!({"from":link,"at":now}).to_string(),"at":now}));
            ev.push(json!({"$":"SettingSet","key":"home.at","value":"","at":now}));
            // APNs: the sender's key, written into apns/ above, named here
            let a = &p["apns"];
            if !gs(a, "file").is_empty() { if let Some(d) = root_dir(hub, "apns") {
                for (k, v) in [("push.apns.key", d.join(gs(a, "file")).to_string_lossy().into_owned()), ("push.apns.keyid", gs(a, "keyid").to_string()), ("push.apns.team", gs(a, "team").to_string())] {
                    if !v.is_empty() { ev.push(json!({"$":"SettingSet","key":k,"value":v,"at":now})) } } } }
            if let Err(e) = hub.commit_many(&ev).await { return no(&format!("{e:#}")) }
            // the ring's front door (bots/front.rs): this hub owns its hooks now. When it is the front, it takes them itself; otherwise it asks the front to pass them here
            let front = hub.setting("ring.front").await.unwrap_or_default();
            let me = crate::bots::ring::base(hub).await;
            for h in p["hooks"].as_array().cloned().unwrap_or_default().iter().filter_map(|h| h.as_str()) {
                if front.is_empty() || front.trim_end_matches('/') == me.trim_end_matches('/') { let _ = hub.commit(&json!({"$":"SettingSet","key":format!("ring.front.to.{h}"),"value":"","at":now})).await; }
                else if let Some(fl) = hub.far_machines().into_iter().find(|(_, _, u)| front.starts_with(u.trim_end_matches('/')) || same_host(u, &front)).map(|(id, _, _)| id) {
                    let _ = hub.far_raw(&fl, "ring.front", &json!({"hook": h, "on": true})).await; }
            }
            ok(json!({}))
        }
        "home.retire" => {
            // a move that did not hold: what came here is switched off again (its lines stay)
            let ev: Vec<Value> = p["retire"].as_array().cloned().unwrap_or_default();
            let now = now_s();
            let mut ev: Vec<Value> = ev.into_iter().filter(|e| matches!(gs(e, "$"), "BotDeleted" | "RoomDeleted" | "HookSet" | "UberSet")).collect();
            ev.push(json!({"$":"SettingSet","key":"home.here","value":"","at":now}));
            match hub.commit_many(&ev).await { Ok(_) => ok(json!({})), Err(e) => no(&format!("{e:#}")) }
        }
        // the ring's record on the machine that holds the home now, for the Ring page of the one that asks
        "home.ring" => match crate::bots::api::of(hub) { Some(b) => ok(json!({"setup": b.ring_setup_info().await})), None => no("bots are not running") },
        "home.mint" => match crate::net::start::routes() {
            Some(rt) => { let j = rt.invite(); if j["error"].is_string() { no(gs(&j, "error")) } else { ok(json!({"link": gs(&j, "link")})) } }
            None => no("no registry"),
        },
        "home.pull" => {
            // the machine that asks wants the home back: this hub moves it there
            if !hub.setting("home.here").await.unwrap_or_default().contains(link) && uber(hub).await.is_empty() { return no("the home is not here") }
            let (h, l) = (hub.clone(), link.to_string());
            tokio::spawn(async move { let _ = move_to(&h, &l).await; });
            ok(json!({"started": true}))
        }
        _ => no("not something this machine does"),
    }
}
fn same_host(a: &str, b: &str) -> bool {
    let h = |u: &str| u.split("://").nth(1).unwrap_or(u).split(['/', ':']).next().unwrap_or("").to_ascii_lowercase();
    !h(a).is_empty() && h(a) == h(b)
}

// ---- the sending side ----

static STATE: Mutex<Option<Value>> = Mutex::new(None);
async fn tell(hub: &Arc<Hub>, state: &str, step: &str, note: &str) {
    let v = json!({"state": state, "step": step, "note": note, "at": now_s()});
    *STATE.lock().unwrap() = Some(v.clone());
    hub.set_info("home.move", &v.to_string()).await;
    eprintln!("backplane: home move: {state} {step} {note}");
}
async fn far(hub: &Arc<Hub>, link: &str, m: &str, p: &Value) -> Result<Value, String> {
    let (code, text) = hub.far_raw(link, m, p).await;
    if code == 0 { return Err("that machine is away".into()) }
    if code != 200 { return Err(format!("{m}: {code} {}", text.chars().take(160).collect::<String>())) }
    let v: Value = serde_json::from_str(&text).map_err(|_| format!("{m}: not JSON"))?;
    if v["ok"] == true { Ok(v) } else { Err(format!("{m}: {}", gs(&v, "error"))) }
}
/// Every file under `dir`, relative to it (no links followed).
fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir.join(rel)) else { return };
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        let name = e.file_name().to_string_lossy().into_owned();
        let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        if ft.is_dir() { walk(dir, &r, out) } else if ft.is_file() { out.push(r) }
    }
}
async fn send_file(hub: &Arc<Hub>, link: &str, root: &str, rel: &str, path: &Path) -> Result<(), String> {
    use base64::Engine as _;
    let bytes = std::fs::read(path).map_err(|e| format!("{rel}: {e}"))?;
    let mut off = 0usize;
    loop {
        let end = (off + CHUNK).min(bytes.len());
        far(hub, link, "home.file", &json!({"root": root, "rel": rel, "off": off, "b64": base64::engine::general_purpose::STANDARD.encode(&bytes[off..end])})).await?;
        off = end; if off >= bytes.len() { return Ok(()) }
    }
}
async fn send_lines(hub: &Arc<Hub>, link: &str, lines: &[String]) -> Result<usize, String> {
    let (mut i, mut took) = (0usize, 0usize);
    while i < lines.len() {
        let (mut j, mut size) = (i, 0usize);
        while j < lines.len() && (j == i || size + lines[j].len() < CHUNK) { size += lines[j].len() + 8; j += 1 }
        let v = far(hub, link, "home.import", &json!({"lines": &lines[i..j]})).await?;
        took += v["took"].as_u64().unwrap_or(0) as usize; i = j;
    }
    Ok(took)
}

/// The whole move of the home from this hub to `link`. Err with what went wrong; a failure after the sender retired is undone.
pub async fn move_to(hub: &Arc<Hub>, link: &str) -> Result<Value, String> {
    let Some((_, name, url)) = hub.far_machines().into_iter().find(|(id, _, _)| id == link) else { return Err("no such machine is linked".into()) };
    tell(hub, "moving", "checking", &name).await;
    let st = far(hub, link, "home.state", &json!({})).await.map_err(|e| if e.contains("not something") || e.contains("403") { format!("{name} runs a Backplane without home moves: update it first") } else { e })?;
    let uber_here = uber(hub).await;
    if uber_here.is_empty() { tell(hub, "failed", "checking", "the home is not on this machine").await; return Err("the home is not on this machine".into()) }
    if !gs(&st, "uber").is_empty() { tell(hub, "failed", "checking", &format!("{name} has an uberbot of its own")).await; return Err(format!("{name} has an uberbot of its own")) }
    let home = hub.cfg.home.clone();
    // the log as it is now
    let n0 = hub.store.len();
    let lines = hub.store.range(0, n0).map_err(|e| format!("{e:#}"))?;
    let c = { let l = lines.clone(); tokio::task::spawn_blocking(move || carry(&l, now_s())).await.map_err(|_| "carry".to_string())? };
    drop(lines);
    let step = |s: &'static str| s;
    let r: Result<(), String> = async {
        tell(hub, "moving", step("devices"), "").await;
        let ds: Vec<Value> = std::fs::read_to_string(home.join("devices-v1.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v["devices"].as_array().cloned()).unwrap_or_default();
        far(hub, link, "home.adopt", &json!({"devices": ds})).await?;
        tell(hub, "moving", step("vault"), "").await;
        if let Ok(items) = crate::settings::vault::read_other(&home) { for (nm, purpose, value) in items { far(hub, link, "home.secret", &json!({"name": nm, "purpose": purpose, "value": value})).await?; } }
        // the sync accounts arrive whole: their registry entries with their tokens (the vault entries above are only the tokens; a renewal made since is in this one)
        tell(hub, "moving", step("accounts"), "").await;
        crate::settings::accounts_sync::push_to(hub, link).await?;
        tell(hub, "moving", step("files"), "").await;
        let bots_dir = crate::bots::relocate::root(&home);
        for b in &c.bots { let mut fs = vec![]; walk(&bots_dir, b, &mut fs); for f in fs { send_file(hub, link, "bots", &f, &bots_dir.join(&f)).await?; } }
        let mut fs = vec![]; for d in ["mail", "ring"] { walk(&home, d, &mut fs); }
        for h in &c.hooks { for f in [format!("secrets/hooks/{h}"), format!("secrets/hooks/{h}.token")] { if home.join(&f).is_file() { fs.push(f) } } }
        if home.join("secrets/ring.key").is_file() { fs.push("secrets/ring.key".into()) }
        let mut imgs = vec![]; walk(&home, "message-images", &mut imgs);
        fs.extend(imgs.into_iter().filter(|f| f.rsplit('/').next().and_then(|n| n.split('.').next()).map_or(false, |s| c.hashes.contains(&s.to_ascii_lowercase()))));
        for f in fs { send_file(hub, link, "home", &f, &home.join(&f)).await?; }
        tell(hub, "moving", step("log"), &format!("{} events", c.lines.len())).await;
        send_lines(hub, link, &c.lines).await?;
        Ok(())
    }.await;
    if let Err(e) = r { tell(hub, "failed", "copying", &e).await; return Err(e) }
    // one owner: the receiver holds it all; this hub lets go now, and what was written meanwhile follows
    tell(hub, "moving", "switching", "").await;
    hub.commit_many(&c.retire).await.map_err(|e| format!("{e:#}"))?;
    for h in &c.hooks { crate::bots::hooks::forget(&home, h); }
    let n1 = hub.store.len();
    let later = hub.store.range(n0, n1).unwrap_or_default();
    let late = carry(&later, now_s());
    let extra: Vec<String> = late.lines.into_iter().filter(|l| not_retire(l, &c.retire)).collect();
    let apns = apns_of(hub).await;
    let fin: Result<(), String> = async {
        if let Some((file, path)) = apns.as_ref().map(|a| (gs(a, "file").to_string(), PathBuf::from(gs(a, "path")))) { if path.is_file() { send_file(hub, link, "apns", &file, &path).await?; } }
        send_lines(hub, link, &extra).await?;
        far(hub, link, "home.finish", &json!({"revive": c.revive, "hooks": c.hooks, "apns": apns.clone().unwrap_or(Value::Null)})).await?;
        Ok(())
    }.await;
    let at = json!({"link": link, "name": name, "url": url, "at": now_s()}).to_string();
    let verified = match fin { Ok(()) => verify(hub, link, &c).await, Err(e) => Err(e) };
    if let Err(e) = verified {
        // undo: this hub takes the home back as it was, the receiver lets go
        tell(hub, "moving", "undoing", &e).await;
        let _ = hub.commit_many(&c.revive).await;
        let _ = far(hub, link, "home.retire", &json!({"retire": c.retire})).await;
        tell(hub, "failed", "undone", &format!("{e}; the home stayed here")).await;
        return Err(e)
    }
    let _ = hub.commit_many(&[json!({"$":"SettingSet","key":"home.at","value":at,"at":now_s()}), json!({"$":"SettingSet","key":"home.here","value":"","at":now_s()})]).await;
    publish_offer(hub).await;
    tell(hub, "done", "moved", &name).await;
    Ok(json!({"to": name, "events": c.lines.len() + extra.len(), "bots": c.bots.len(), "rooms": c.rooms.len(), "hooks": c.hooks.len()}))
}
/// APNs of this hub, to carry: `{file, path, keyid, team}`, None when it has none.
async fn apns_of(hub: &Arc<Hub>) -> Option<Value> {
    let (k, id, team) = (hub.setting("push.apns.key").await.unwrap_or_default(), hub.setting("push.apns.keyid").await.unwrap_or_default(), hub.setting("push.apns.team").await.unwrap_or_default());
    let (k, id, team) = if k.is_empty() || id.is_empty() {
        let e = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/backplane/apns.env")).and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        let g = |n: &str| e.lines().find_map(|l| l.trim().trim_start_matches("export ").strip_prefix(&format!("{n}="))).unwrap_or("").trim_matches('"').to_string();
        let id = if id.is_empty() { g("BACKPLANE_APNS_KEY_ID") } else { id };
        (if k.is_empty() { g("BACKPLANE_APNS_PRIVATE_KEY_PATH") } else { k }, id, if team.is_empty() { g("BACKPLANE_APNS_TEAM_ID") } else { team })
    } else { (k, id, team) };
    let k = if k.is_empty() && !id.is_empty() { std::env::var_os("HOME").map(|h| PathBuf::from(h).join(format!(".config/backplane/apns/AuthKey_{id}.p8")).to_string_lossy().into_owned()).unwrap_or_default() } else { k };
    if k.is_empty() || id.is_empty() || team.is_empty() { return None }
    let file = Path::new(&k).file_name()?.to_string_lossy().into_owned();
    Some(json!({"file": file, "path": k, "keyid": id, "team": team}))
}
/// The move holds when the receiver has the uberbot, and this hub sees it there (its mirror has the uberbot's bot).
async fn verify(hub: &Arc<Hub>, link: &str, c: &Carry) -> Result<(), String> {
    for _ in 0..60 {
        if let Ok(st) = far(hub, link, "home.state", &json!({})).await {
            if gs(&st, "uber") == c.uber && (c.uber.is_empty() || hub.mirrors().has_bot(link, &c.uber)) { return Ok(()) }
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    Err("the other machine did not show the uberbot within two minutes".into())
}

/// The ring's record from the machine that holds the home (`home.at`), marked as being there; None when the home is here or that machine is away.
pub async fn ring_elsewhere(hub: &Arc<Hub>) -> Option<Value> {
    let at: Value = serde_json::from_str(&hub.setting("home.at").await.unwrap_or_default()).ok()?;
    let link = gs(&at, "link"); if link.is_empty() { return None }
    let v = far(hub, link, "home.ring", &json!({})).await.ok()?;
    let mut s = v["setup"].clone();
    if gs(&s, "hook").is_empty() || gs(&s, "url").is_empty() { return None }
    s["home"] = json!({"url": gs(&s, "url"), "machine": gs(&at, "name")});
    Some(s)
}
/// The offer the clients take (docs/PHONE-DATA.md `hub.offer`): the holder of the home, when it is another machine.
pub fn offer_of(at: &str) -> String {
    let Ok(v) = serde_json::from_str::<Value>(at) else { return String::new() };
    if gs(&v, "url").is_empty() { return String::new() }
    json!({"id": format!("home-{}", v["at"].as_u64().unwrap_or(0)), "name": gs(&v, "name"), "url": public_url(gs(&v, "url")), "why": "home", "credential": "same"}).to_string()
}
/// A phone reaches a hub served by `tailscale serve` at its https address; the link table may name its plain port.
pub fn public_url(u: &str) -> String {
    let u = u.trim_end_matches('/');
    if let Some(rest) = u.strip_prefix("http://") { if let Some((host, port)) = rest.split_once(':') { if host.ends_with(".ts.net") && port.chars().all(|c| c.is_ascii_digit()) { return format!("https://{host}") } } }
    u.to_string()
}
pub async fn publish_offer(hub: &Arc<Hub>) { let at = hub.setting("home.at").await.unwrap_or_default(); hub.set_info("hub.offer", &offer_of(&at)).await; }

/// One press: `to` a linked machine (its link id or name) moves the home there; "back" asks the machine that holds it to move it here.
pub async fn start(hub: &Arc<Hub>, to: &str) -> Result<Value, String> {
    if STATE.lock().unwrap().as_ref().map_or(false, |s| s["state"] == "moving") { return Err("a move is running".into()) }
    if to == "back" {
        let at: Value = serde_json::from_str(&hub.setting("home.at").await.unwrap_or_default()).unwrap_or(Value::Null);
        let l = gs(&at, "link").to_string(); if l.is_empty() { return Err("the home is here".into()) }
        far(hub, &l, "home.pull", &json!({})).await?;
        tell(hub, "moving", "back", gs(&at, "name")).await;
        return Ok(json!({"started": true}))
    }
    let Some(l) = hub.far_machines().into_iter().find(|(lid, n, _)| lid == to || n.eq_ignore_ascii_case(to)).map(|(lid, _, _)| lid) else { return Err("no machine of yours by that name is linked".into()) };
    let h = hub.clone(); tokio::spawn(async move { let _ = move_to(&h, &l).await; });
    Ok(json!({"started": true}))
}
/// Settings, Machines: where the home is and the one press that moves it (the phones' Machines page draws the same rows).
/// `uber`: this hub has the uberbot; `at`: the setting `home.at`; `mv`: the info `home.move`; `linked`: the page's linked machines.
pub fn rows(uber: bool, at: &str, mv: &str, linked: &Value) -> Vec<Value> {
    let row = |label: &str, note: &str, b: Vec<Value>| json!({"label": label, "note": note, "buttons": b});
    let btn = |label: &str, v: &str| json!({"label": label, "action": "machine-home", "value": v, "on": false});
    let mut r = vec![];
    let at: Value = serde_json::from_str(at).unwrap_or(Value::Null);
    let mv: Value = serde_json::from_str(mv).unwrap_or(Value::Null);
    let moving = gs(&mv, "state") == "moving";
    if !gs(&at, "name").is_empty() {
        r.push(row("Home", &format!("Bub, the bots, rooms, push and the ring run on {}, which stays on when this machine sleeps.", gs(&at, "name")), if moving { vec![] } else { vec![btn("Move home back here", "back")] }));
    } else if uber {
        let b: Vec<Value> = if moving { vec![] } else { linked.as_array().into_iter().flatten().filter(|l| gs(l, "state") == "paired").map(|l| btn(&format!("Move home to {}", gs(l, "name")), gs(l, "id"))).collect() };
        r.push(row("Home", "Bub, the bots, rooms, push and the ring run on this machine.", b));
    }
    if !gs(&mv, "state").is_empty() && gs(&mv, "state") != "idle" {
        let note = match gs(&mv, "state") { "moving" => format!("Moving: {} {}", gs(&mv, "step"), gs(&mv, "note")), "done" => format!("Moved to {}.", gs(&mv, "note")), _ => format!("The move did not finish: {}", gs(&mv, "note")) };
        let mut x = row("Home move", note.trim(), vec![]); if gs(&mv, "state") == "failed" { x["tone"] = json!("bad") } r.push(x);
    }
    r
}
pub struct HomeRpc;
impl RpcHandler for HomeRpc {
    fn methods(&self) -> &'static [&'static str] { &["home.move", "home.status", "hub.offer.take", "hub.offer.done", "ring.front.use"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let reply = |v: Value| vec![json!({"t":"reply","id":id,"ok":true,"result":v})];
            let fail = |e: &str| vec![json!({"t":"reply","id":id,"ok":false,"error":e})];
            match m {
                "home.status" => reply(STATE.lock().unwrap().clone().unwrap_or(json!({"state":"idle"}))),
                "home.move" => match start(hub, gs(p, "to")).await { Ok(v) => reply(v), Err(e) => fail(&e) },
                "hub.offer.take" => {
                    let at: Value = serde_json::from_str(&hub.setting("home.at").await.unwrap_or_default()).unwrap_or(Value::Null);
                    let l = gs(&at, "link"); if l.is_empty() { return fail("no hub is offered") }
                    match far(hub, l, "home.mint", &json!({})).await { Ok(v) => reply(json!({"link": gs(&v, "link")})), Err(e) => fail(&e) }
                }
                // the ring's front door (bots/front.rs): `link` (the always-on machine) takes every delivery for this hub's hooks and passes them here while they are here
                "ring.front.use" => {
                    let Some(l) = hub.far_machines().into_iter().find(|(lid, n, _)| lid == gs(p, "to") || n.eq_ignore_ascii_case(gs(p, "to"))).map(|(lid, _, _)| lid) else { return fail("no machine of yours by that name is linked") };
                    let hooks: Vec<String> = { let w = hub.store.all(); carry(&w, now_s()).hooks };
                    let mut base = String::new();
                    for h in &hooks { match far(hub, &l, "ring.front", &json!({"hook": h, "on": true})).await { Ok(v) => base = gs(&v, "base").to_string(), Err(e) => return fail(&e) } }
                    if base.is_empty() { return fail("no webhook here to front") }
                    let _ = hub.commit(&json!({"$":"SettingSet","key":"ring.front","value":base,"at":now_s()})).await;
                    reply(json!({"base": base, "hooks": hooks}))
                }
                "hub.offer.done" => { eprintln!("backplane: hub offer {} taken: {}", gs(p, "id"), p["ok"]); reply(json!({})) }
                _ => fail("unknown"),
            }
        })
    }
}
pub fn install(hub: &Arc<Hub>) {
    hub.register_rpc(Arc::new(HomeRpc));
    let h = hub.clone(); tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await; publish_offer(&h).await;
        let me = crate::bots::ring::base(&h).await; if me.starts_with("https://") { crate::push::set_hub_url(&me) }
        // while the home is elsewhere, this hub's Ring page follows that machine's ring (its record is asked for each minute)
        loop {
            if !h.setting("home.at").await.unwrap_or_default().is_empty() { if let Some(b) = crate::bots::api::of(&h) { b.ring_publish().await; } }
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn l(v: Value) -> String { v.to_string() }
    #[test] fn a_move_carries_the_home_and_leaves_the_projects() {
        let img = "ab".repeat(32);
        let log = vec![
            l(json!({"$":"ProjectCreated","id":"p1","title":"Board","at":1})),
            l(json!({"$":"ThreadCreated","id":"t1","project":"p1","title":"layout","at":1})),
            l(json!({"$":"MessagePosted","thread":"t1","msg":"m1","text":"route it","at":2})),
            l(json!({"$":"ThreadCreated","id":"tb","project":"","title":"Bub","at":1})),
            l(json!({"$":"BotSet","id":"b1","name":"Bub","thread":"tb","at":1})),
            l(json!({"$":"BotSet","id":"b1","name":"Bub","thread":"","at":3})),
            l(json!({"$":"UberSet","bot":"b1","uid":"u1","at":1})),
            l(json!({"$":"MessagePosted","thread":"tb","msg":"m2","text":"hi","images":[img.clone()],"at":4})),
            l(json!({"$":"RoomSet","id":"r1","name":"staff","members":"b1","at":5})),
            l(json!({"$":"HookSet","id":"h1","bot":"b1","name":"Pebble Index","revoked":false,"at":5})),
            l(json!({"$":"SettingSet","key":"machine.id","value":"desk","at":1})),
            l(json!({"$":"SettingSet","key":"alert.t1","value":"x","at":1})),
            l(json!({"$":"SettingSet","key":"voice.engine","value":"local","at":1})),
            l(json!({"$":"SettingSet","key":"peer.own.L","value":"approved","at":1})),
            l(json!({"$":"UberOp","uid":"u1","op":"{}","at":1})),
            l(json!({"$":"DeviceRegistered","token":"ab12","platform":"ios","at":1})),
            l(json!({"$":"BotSet","id":"b2","name":"Gone","thread":"tg","at":1})),
            l(json!({"$":"BotDeleted","id":"b2","at":2})),
        ];
        let c = carry(&log, 100);
        let texts: Vec<&str> = c.lines.iter().map(String::as_str).collect();
        assert!(!texts.iter().any(|t| t.contains("\"t1\"") || t.contains("\"p1\"") || t.contains("machine.id") || t.contains("peer.own") || t.contains("UberOp")), "{texts:#?}");
        assert!(texts.iter().any(|t| t.contains("\"m2\"")) && texts.iter().any(|t| t.contains("voice.engine")) && texts.iter().any(|t| t.contains("DeviceRegistered")));
        assert_eq!((c.bots.clone(), c.rooms.clone(), c.hooks.clone(), c.uber.as_str()), (vec!["b1".to_string()], vec!["r1".to_string()], vec!["h1".to_string()], "b1"));
        let bub = c.revive.iter().find(|e| e["$"] == "BotSet").unwrap(); assert_eq!((bub["thread"].as_str(), bub["at"].as_u64()), (Some("tb"), Some(100)), "the thread from its last BotSet that named one");
        assert_eq!(c.retire[0], json!({"$":"UberSet","bot":"","uid":"","at":100}));
        assert!(c.retire.iter().any(|e| e["$"] == "BotDeleted" && e["id"] == "b1") && !c.retire.iter().any(|e| e["id"] == "b2"));
        assert!(c.hashes.contains(&img));
        assert!(!not_retire(&l(json!({"$":"BotDeleted","id":"b1","at":100})), &c.retire) && not_retire(&l(json!({"$":"BotDeleted","id":"b1","at":99})), &c.retire));
    }
    #[test] fn a_move_writes_only_inside_its_roots() {
        assert!(rel_ok("bots", "b1/notes.md") && rel_ok("home", "mail/x.jsonl") && rel_ok("home", "secrets/hooks/h1.token") && rel_ok("home", "secrets/ring.key") && rel_ok("apns", "AuthKey_X.p8"));
        for (r, p) in [("home", "events.jsonl"), ("home", "secrets/vault.key"), ("home", "mail/../token"), ("bots", "/etc/passwd"), ("bots", "a//b"), ("x", "a"), ("home", "ring/./a")] { assert!(!rel_ok(r, p), "{r} {p}") }
    }
    #[test] fn the_machines_page_offers_one_press_each_way() {
        let linked = json!([{"id":"L1","name":"boat-hub","state":"paired"},{"id":"L2","name":"old","state":"unpaired"}]);
        let r = rows(true, "", "", &linked); assert_eq!(r.len(), 1); assert_eq!(r[0]["buttons"].as_array().unwrap().len(), 1, "only a paired machine"); assert_eq!((r[0]["buttons"][0]["action"].as_str(), r[0]["buttons"][0]["value"].as_str()), (Some("machine-home"), Some("L1")));
        let at = json!({"link":"L1","name":"boat-hub","url":"u","at":1}).to_string();
        let r = rows(false, &at, "", &linked); assert!(r[0]["note"].as_str().unwrap().contains("boat-hub")); assert_eq!(r[0]["buttons"][0]["value"], "back");
        assert!(rows(false, "", "", &linked).is_empty(), "no home here and none elsewhere: nothing to move");
        let mv = json!({"state":"moving","step":"log","note":"94000 events"}).to_string(); let r = rows(true, "", &mv, &linked); assert!(r[0]["buttons"].as_array().unwrap().is_empty() && r[1]["note"].as_str().unwrap().starts_with("Moving: log"));
        let mv = json!({"state":"failed","note":"x; the home stayed here"}).to_string(); assert_eq!(rows(true, "", &mv, &linked)[1]["tone"], "bad");
    }
    #[test] fn the_offer_names_the_holder_at_its_https_address() {
        let at = json!({"link":"L","name":"boat-hub","url":"http://boat-hub.tail9e0cf2.ts.net:3797","at":7}).to_string();
        let o: Value = serde_json::from_str(&offer_of(&at)).unwrap();
        assert_eq!(o, json!({"id":"home-7","name":"boat-hub","url":"https://boat-hub.tail9e0cf2.ts.net","why":"home","credential":"same"}));
        assert_eq!(offer_of(""), ""); assert_eq!(public_url("https://boat-hub.x.ts.net/"), "https://boat-hub.x.ts.net");
        assert!(same_host("https://boat-hub.x.ts.net", "http://boat-hub.x.ts.net:3797/") && !same_host("https://a.ts.net", "https://b.ts.net"));
    }
}

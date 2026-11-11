//! The project registry (docs/ALWAYS-ON.md section 5, D7): the uberbot's home keeps every machine's projects and threads, so a machine that is
//! off still shows them and the uberbot still knows where a project lives.
//!
//! Each hub, every minute (`tick`, from `mirror::watch_origins`, which also runs when a link comes up), works out its own rows (`rows_of`:
//! project key, name, origin, id, folder; each thread's id, title, status word, activity) and sends them to the home when they changed or ten
//! minutes went by (`registry.put` over the signed far link; the home is `home.at`'s link when another machine holds
//! it, else this hub keeps the registry itself). It reads the merged list back (`registry.get`) into `cached`, which `present::merged` folds in.
//!
//! On the home a machine's rows are one SettingSet `registry.m.<machine>` (JSON), listed in `registry.index`: events in the home's log, so the
//! registry moves with `home.move`. Nothing is rewritten: keys are computed from each machine's origin when it sends its rows.
use crate::hub::Hub;
use crate::present::merged::{RegMachine, RegProject, RegThread};
use crate::state::{State, Turn};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Arc, Mutex, OnceLock, RwLock}};

fn slot() -> &'static RwLock<Vec<RegProject>> { static S: OnceLock<RwLock<Vec<RegProject>>> = OnceLock::new(); S.get_or_init(Default::default) }
/// The merged registry as the home last sent it (empty before the first sync).
pub fn cached() -> Vec<RegProject> { slot().read().unwrap().clone() }
/// Replace the cached copy (after a sync with the home).
pub fn set_cached(v: Vec<RegProject>) { *slot().write().unwrap() = v; }

/// At most this many threads a machine sends (the newest, and every one that needs the person): a row of the sidebar, not an archive.
const MAX_THREADS: usize = 300;
/// A machine's rows are sent again after this long even when nothing changed (s): the home's `seen` says the machine was up.
const AGAIN: u64 = 600;

/// A thread's status word, as the sidebar sections classify it (lane J's `present::sections::status`; this is its stand-in until that lands:
/// the same words). An open ask or a waiting permission is "needs".
pub fn status(st: &State, id: &str) -> &'static str {
    let Some(t) = st.threads.get(id) else { return "idle" };
    if t.pending > 0 || st.asks.values().any(|(th, open)| *open && th == id) { return "needs" }
    match t.turn { Turn::Running => "running", Turn::Failed => "failed", Turn::Interrupted => "interrupted", _ => if crate::shelf::shelf_of(t) == crate::shelf::Shelf::Settled { "done" } else { "idle" } }
}

/// This machine's rows: `{machine, seen, projects:[{key,name,origin,id,folder}], threads:[{key,id,title,status,activity,pinned}]}`. Pure.
pub fn rows_of(st: &State, machine: &str, now: u64, origin: &dyn Fn(&str) -> String) -> Value {
    let mut projects = vec![]; let mut keys: HashMap<&str, String> = HashMap::new();
    let mut ps: Vec<_> = st.projects.iter().filter(|(id, p)| !p.removed && id.as_str() != crate::sidebar::SCRATCH).collect();
    ps.sort_by(|a, b| a.0.cmp(b.0));
    for (id, p) in ps {
        let o = origin(&p.root);
        let key = crate::present::merged::key_of(&o, st.settings.get(&format!("project.link.{id}")).map(String::as_str).unwrap_or(""), "", machine, id);
        projects.push(json!({"key":key,"name":p.title,"origin":o,"id":id,"folder":p.root}));
        keys.insert(id.as_str(), key);
    }
    let mut ts: Vec<(&String, &crate::state::Thread)> = st.threads.iter().filter(|(_, t)| !t.deleted && keys.contains_key(t.project.as_str())).collect();
    ts.sort_by(|a, b| (status(st, b.0) == "needs").cmp(&(status(st, a.0) == "needs")).then(b.1.activity.max(b.1.created).cmp(&a.1.activity.max(a.1.created))).then(a.0.cmp(b.0)));
    let threads: Vec<Value> = ts.into_iter().filter(|(id, t)| !t.archived || status(st, id) == "needs").take(MAX_THREADS).map(|(id, t)| {
        let title: String = t.title.chars().take(80).collect();
        json!({"key":keys[t.project.as_str()],"id":id,"title":title,"status":status(st, id),"activity":t.activity.max(t.created),"pinned":t.pinned})
    }).collect();
    json!({"machine":machine,"seen":now,"projects":projects,"threads":threads})
}

/// The merged list from every machine's rows (the home's answer to `registry.get`): one `RegProject` per key, its machines and their threads.
/// A thread's id is `<machine>~<id>` (shown, never opened: a live machine's own come from its mirror). Pure.
pub fn merge(machines: &[Value]) -> Vec<RegProject> {
    let mut out: Vec<RegProject> = vec![]; let mut at: HashMap<String, usize> = HashMap::new();
    for m in machines {
        let name = m["machine"].as_str().unwrap_or(""); if name.is_empty() { continue }
        let seen = m["seen"].as_u64().unwrap_or(0);
        for p in m["projects"].as_array().into_iter().flatten() {
            let key = p["key"].as_str().unwrap_or("").to_string(); if key.is_empty() { continue }
            let i = *at.entry(key.clone()).or_insert_with(|| { out.push(RegProject { key: key.clone(), name: p["name"].as_str().unwrap_or("").into(), origin: p["origin"].as_str().unwrap_or("").into(), ..Default::default() }); out.len() - 1 });
            out[i].machines.push(RegMachine { machine: name.into(), id: p["id"].as_str().unwrap_or("").into(), folder: p["folder"].as_str().unwrap_or("").into(), seen });
        }
        for t in m["threads"].as_array().into_iter().flatten() {
            let Some(&i) = at.get(t["key"].as_str().unwrap_or("")) else { continue };
            out[i].threads.push(RegThread { machine: name.into(), id: format!("{name}~{}", t["id"].as_str().unwrap_or("")), title: t["title"].as_str().unwrap_or("").into(),
                status: t["status"].as_str().unwrap_or("idle").into(), activity: t["activity"].as_u64().unwrap_or(0), pinned: t["pinned"] == true });
        }
    }
    out
}

// ---- the home's store -----------------------------------------------------------------------------------------------------------------
fn key_of(machine: &str) -> String { format!("registry.m.{machine}") }
async fn index(hub: &Hub) -> Vec<String> { hub.setting("registry.index").await.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default() }
/// Keep a machine's rows (on the home). Unchanged rows (but `seen`) are not written again unless ten minutes went by.
pub async fn put(hub: &Hub, rows: &Value) -> Result<(), String> {
    // one at a time: two machines' rows arriving together must not each write an index without the other
    static ONE: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new(); let _one = ONE.get_or_init(Default::default).lock().await;
    let machine = rows["machine"].as_str().unwrap_or("").trim().to_string();
    if machine.is_empty() || machine.len() > 200 || machine.contains(['\n', '"']) { return Err("machine".into()) }
    let now = crate::hub::now_ms() / 1000;
    let old: Option<Value> = hub.setting(&key_of(&machine)).await.and_then(|v| serde_json::from_str(&v).ok());
    let same = old.as_ref().map_or(false, |o| o["projects"] == rows["projects"] && o["threads"] == rows["threads"] && rows["seen"].as_u64().unwrap_or(0) < o["seen"].as_u64().unwrap_or(0) + AGAIN);
    if same { return Ok(()) }
    let mut ev = vec![json!({"$":"SettingSet","key":key_of(&machine),"value":rows.to_string(),"at":now})];
    let mut ix = index(hub).await;
    if !ix.contains(&machine) { ix.push(machine.clone()); ev.push(json!({"$":"SettingSet","key":"registry.index","value":json!(ix).to_string(),"at":now})); }
    hub.commit_many(&ev).await.map(|_| ()).map_err(|e| e.to_string())
}
/// A machine that is no longer linked: the rows it sent are forgotten here, and the home (this hub, when it keeps the registry) drops its entry,
/// so it is never drawn as a machine that is off. A name a live link still has is kept (two links, one name: the other is still linked).
pub async fn forget(hub: &Arc<Hub>, machine: &str) {
    let machine = machine.trim();
    let live = crate::net::start::peering().map_or(false, |p| p.table.with(|t| t.live().iter().any(|r| r.name == machine)));
    if machine.is_empty() || live { return }
    received().lock().unwrap().remove(machine);
    let mut ix = index(hub).await;
    if ix.iter().any(|m| m == machine) {
        ix.retain(|m| m != machine);
        let now = crate::hub::now_ms() / 1000;
        let _ = hub.commit_many(&[json!({"$":"SettingSet","key":key_of(machine),"value":"","at":now}), json!({"$":"SettingSet","key":"registry.index","value":json!(ix).to_string(),"at":now})]).await;
    }
    set_cached(merge(&with_received(all(hub).await)));
}
/// Every machine's rows the home holds.
pub async fn all(hub: &Hub) -> Vec<Value> {
    let mut v = vec![];
    for m in index(hub).await { if let Some(r) = hub.setting(&key_of(&m)).await.and_then(|s| serde_json::from_str::<Value>(&s).ok()) { v.push(r) } }
    v
}
/// Rows a linked machine sent this hub (every hub sends its rows to each linked machine too, not only to the home): its origins and keys
/// reach this hub even when this hub cannot call it (one-direction pairing, a machine behind a tunnel). Kept in memory, by machine.
fn received() -> &'static Mutex<HashMap<String, Value>> { static R: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new(); R.get_or_init(Default::default) }
/// Every machine's rows this hub knows: `base` (the home's list, or its own store), then what linked machines sent (newer `seen` wins).
fn with_received(mut base: Vec<Value>) -> Vec<Value> {
    for (m, r) in received().lock().unwrap().iter() {
        match base.iter_mut().find(|b| b["machine"].as_str() == Some(m.as_str())) {
            Some(b) => { if r["seen"].as_u64().unwrap_or(0) > b["seen"].as_u64().unwrap_or(0) { *b = r.clone() } }
            None => base.push(r.clone()),
        }
    }
    base
}
/// The far requests (net/rpc.rs; the owner's own machines only): `registry.put {rows}` and `registry.get`. `link` is the sender.
pub async fn serve(hub: &Arc<Hub>, link: &str, m: &str, p: &Value) -> Value {
    match m {
        // its rows say its projects' origins (the sidebar keys its copies at once); the home keeps them in its log, any other hub in memory
        "registry.put" => {
            let rows = &p["rows"]; let machine = rows["machine"].as_str().unwrap_or("").to_string();
            if machine.is_empty() { return json!({"ok":false,"error":"machine"}) }
            hub.mirrors().origins_from_rows(link, rows);
            received().lock().unwrap().insert(machine, rows.clone());
            // (kept in the log only where the uberbot lives: it moves with home.move; elsewhere it is a copy, refreshed as it comes)
            let r = if !crate::net::homemove::uber(hub).await.is_empty() { put(hub, rows).await } else { Ok(()) };
            set_cached(merge(&with_received(all(hub).await))); hub.mirrors().bump(); hub.mirrors().publish(hub).await;
            match r { Ok(()) => json!({"ok":true}), Err(e) => json!({"ok":false,"error":e}) }
        }
        "registry.get" => json!({"ok":true,"machines":all(hub).await}),
        _ => json!({"ok":false,"error":"unknown"}),
    }
}

// ---- each hub ---------------------------------------------------------------------------------------------------------------------------

/// Where the registry is: the home's link when another machine holds it, else None (this hub holds it, or none is known: it keeps its own).
async fn home_link(hub: &Arc<Hub>) -> Option<String> {
    // `home.at` names the machine that holds the home (net/homemove.rs); "" or unset: this one, or none known (it keeps its own)
    let at: Value = hub.setting("home.at").await.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or(Value::Null);
    at["link"].as_str().filter(|l| !l.is_empty()).map(str::to_string)
}
/// What was last sent, where: (the home's link, "" this hub; the rows' digest; when).
fn last() -> &'static Mutex<HashMap<String, (String, u64)>> { static L: OnceLock<Mutex<HashMap<String, (String, u64)>>> = OnceLock::new(); L.get_or_init(Default::default) }
fn due(dest: &str, digest: &str, now: u64) -> bool { last().lock().unwrap().get(dest).map_or(true, |(d, at)| d != digest || now >= at + AGAIN) }
fn sent(dest: &str, digest: &str, now: u64) { last().lock().unwrap().insert(dest.to_string(), (digest.to_string(), now)); }
/// Send this hub's rows to the home (when they changed, or ten minutes went by) and read the merged list back. Best effort: a home that does
/// not answer leaves the last list in place.
pub async fn tick(hub: &Arc<Hub>, machine: &str) {
    if machine.is_empty() { return }
    let now = crate::hub::now_ms() / 1000;
    let Some(rows) = crate::statehost::policies().and_then(|p| p.with_state(|st| rows_of(st, machine, now, &|r| crate::mirror::origin_of_root(r)))) else { return };
    let digest = { let mut r = rows.clone(); r["seen"] = json!(0); r.to_string() };
    let home = home_link(hub).await;
    let base = match &home {
        None => { if due("", &digest, now) && put(hub, &rows).await.is_ok() { sent("", &digest, now) } all(hub).await }
        Some(link) => {
            if due(link, &digest, now) {
                let (code, text) = hub.far_raw(link, "registry.put", &json!({"rows":rows})).await;
                if code == 200 && serde_json::from_str::<Value>(&text).map_or(false, |v| v["ok"] == true) { sent(link, &digest, now) }
            }
            let (code, text) = hub.far_raw(link, "registry.get", &json!({})).await;
            let mut v: Vec<Value> = if code == 200 { serde_json::from_str::<Value>(&text).ok().filter(|v| v["ok"] == true).and_then(|v| v["machines"].as_array().cloned()).unwrap_or_default() } else { vec![] };
            if !v.iter().any(|r| r["machine"].as_str() == Some(machine)) { v.push(rows.clone()) }
            v
        }
    };
    // every other machine of the owner's hears this hub's rows too: its sidebar keys this hub's copies by them even when it cannot call this one.
    // Only the owner's own verified machines (`far_machines`, and `far_raw` refuses any other link): project folders, thread titles and states never
    // go to a machine linked for sharing or to another person's hub; and a hub takes `registry.put` only from its owner's own machines (net/rpc.rs)
    for (link, _, _) in hub.far_machines() {
        if Some(&link) == home.as_ref() || !due(&link, &digest, now) { continue }
        let (code, text) = hub.far_raw(&link, "registry.put", &json!({"rows":rows})).await;
        if code == 200 && serde_json::from_str::<Value>(&text).map_or(false, |v| v["ok"] == true) { sent(&link, &digest, now) }
    }
    set_cached(merge(&with_received(base)));
}

/// Whether a Boat account is connected, so the picker's "A Boat (fresh clone)" can run (lane F's always-on Boat fills this in; until then off).
pub fn boat_ready() -> bool { BOAT.get().map_or(false, |f| f()) }
/// Lane F's entry points: whether a Boat account is connected, and the launch of a thread on a fresh Boat with a clone of `origin`.
pub static BOAT: OnceLock<Box<dyn Fn() -> bool + Send + Sync>> = OnceLock::new();

/// Where a clone of `origin` (`host/owner/repo`) goes on this machine: `~/Documents/<repo>`, or `<repo>-2`... when that folder holds something else.
pub fn clone_dir(home: &std::path::Path, origin: &str, exists: &dyn Fn(&std::path::Path) -> Option<String>) -> std::path::PathBuf {
    let name = origin.rsplit('/').next().filter(|n| !n.is_empty() && !n.starts_with('.')).unwrap_or("project");
    let base = home.join("Documents");
    for i in 1.. { let d = if i == 1 { base.join(name) } else { base.join(format!("{name}-{i}")) };
        match exists(&d) { None => return d, Some(o) if o == origin => return d, _ => {} } }
    unreachable!()
}
/// `project.clone {origin}`: clone the project's remote here (ssh first, then https) and add it as a project ("Clone it here").
/// `path` is the folder the person confirmed (the picker's; empty: `clone_dir`'s). A folder that is there is taken only when it is that repository.
async fn clone_here(hub: &Arc<Hub>, origin: &str, path: &str) -> Result<Value, String> {
    if origin.is_empty() || origin.contains("..") || origin.starts_with('-') || !origin.contains('/') || origin.chars().any(|c| c.is_whitespace()) { return Err("origin".into()) }
    let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let dir = if path.trim().is_empty() { clone_dir(&home, origin, &|d| d.exists().then(|| crate::mirror::origin_of_root(&d.to_string_lossy()))) } else {
        let p = path.trim(); let p = if let Some(r) = p.strip_prefix("~/") { home.join(r) } else { std::path::PathBuf::from(p) };
        if !p.is_absolute() { return Err("the folder must be a full path".into()) }
        p };
    if dir.exists() && crate::mirror::origin_of_root(&dir.to_string_lossy()) != origin { return Err(format!("{} is there and holds something else", dir.display())) }
    if !dir.exists() {
        let (host, path) = origin.split_once('/').unwrap_or((origin, ""));
        let mut done = false; let mut why = String::new();
        for url in [format!("git@{host}:{path}.git"), format!("https://{origin}.git")] {
            let o = crate::proc::tokio_cmd(crate::proc::Kind::Net, "git").args(["clone", "--", &url]).arg(&dir).env("GIT_TERMINAL_PROMPT", "0").env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes").output().await.map_err(|e| e.to_string())?;
            if o.status.success() { done = true; break }
            why = String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("").to_string(); let _ = std::fs::remove_dir_all(&dir);
        }
        if !done { return Err(format!("could not clone {origin}: {why}")) }
    }
    let mut c = crate::hub::Conn { sent: vec![], client: 0, gen: String::new(), view_id: String::new(), target: Value::Null, cbor: false, lastrev: 0 };
    let r = hub.request_here(&mut c, &json!({"id":0,"m":"project.add","p":{"path":dir.to_string_lossy()}})).await.into_iter().find(|v| v["t"] == "reply").unwrap_or(Value::Null);
    if r["ok"] == true { Ok(json!({"path":dir.to_string_lossy()})) } else { Err(r["error"].as_str().unwrap_or("the project was not added").to_string()) }
}

/// Whether `word` names a merged row: its key, its remote in any form (`mirror::normalize_origin`), or its name (any case).
pub fn names(p: &crate::present::merged::MergedProject, word: &str) -> bool {
    let w = word.trim(); if w.is_empty() { return false }
    let o = crate::mirror::normalize_origin(w);
    p.key == w || (!o.is_empty() && (p.key == o || p.origin == o)) || p.name.eq_ignore_ascii_case(w)
}
/// Where a launch named by project (key, remote or name) goes, for Bub's thread_launch with no machine (D6): this machine's copy, else the most
/// recently active live machine's (an away one too: the launch is held for it). `(link, id there)`; link "" is this machine. None: no such project.
pub fn resolve(hub: &Hub, word: &str) -> Option<(String, String)> {
    let m = hub.mirrors();
    crate::statehost::policies()?.with_state(|st| crate::present::merged::merged_of(st, m, |list| {
        let p = list.iter().find(|p| p.key == word.trim()).or_else(|| list.iter().find(|p| names(p, word)))?;
        let (machine, id) = p.ids.iter().find(|(mach, _)| mach.is_empty()).or_else(|| p.ids.iter().find(|(_, id)| id.contains('~')))?;
        Some(if machine.is_empty() { (String::new(), id.clone()) } else { let (l, i) = id.split_once('~')?; (l.to_string(), i.to_string()) })
    })).flatten()
}
/// This machine's project a launch names by key or remote (the local half of `resolve`, for agents/launch.rs).
pub fn local_id(word: &str) -> Option<String> {
    crate::statehost::policies()?.with_state(|st| crate::present::merged::merged_projects(st, &[], &|_| false, &|_| String::new(), "", &[]).into_iter().find(|p| names(p, word)).and_then(|p| p.here)).flatten()
}

/// `registry.list` (this hub's clients, tests): the merged registry as this hub holds it now. `project.clone {origin}`: Clone it here.
pub struct RegistryRpc;
impl crate::api::RpcHandler for RegistryRpc {
    fn methods(&self) -> &'static [&'static str] { &["registry.list", "project.clone"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a crate::api::Caller, id: &'a Value, m: &'a str, p: &'a Value) -> crate::api::Fut<'a, Vec<Value>> {
        Box::pin(async move { match m {
            "project.clone" => match clone_here(hub, p["origin"].as_str().unwrap_or(""), p["path"].as_str().unwrap_or("")).await { Ok(v) => vec![json!({"t":"reply","id":id,"ok":true,"result":v})], Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":e})] },
            _ => vec![json!({"t":"reply","id":id,"ok":true,"result":{"projects":cached()}})],
        } })
    }
}
pub fn install(hub: &Arc<Hub>) { hub.register_rpc(Arc::new(RegistryRpc)); }

#[cfg(test)]
mod tests {
    use super::*;
    fn st(evs: &[Value]) -> State { let mut s = State::new(); s.lean = true; for e in evs { s.apply(e); } s }
    #[test] fn a_machines_rows_carry_keys_status_and_the_newest_threads_and_merge_by_key() {
        let a = st(&[json!({"$":"ProjectCreated","id":"p1","title":"Backplane","root":"/a/bp","at":1}), json!({"$":"ProjectCreated","id":"p2","title":"Notes","root":"/a/n","at":1}),
            json!({"$":"ThreadCreated","id":"t1","project":"p1","title":"one","at":5}), json!({"$":"ThreadCreated","id":"t2","project":"p1","title":"two","at":9}),
            json!({"$":"TurnChanged","thread":"t2","state":"running","at":10}), json!({"$":"AskOpened","id":"a1","thread":"t1","at":11})]);
        let origin = |r: &str| if r == "/a/bp" { "github.com/o/bp".to_string() } else { String::new() };
        let r = rows_of(&a, "desktop", 100, &origin);
        assert_eq!(r["projects"].as_array().unwrap().iter().map(|p| p["key"].as_str().unwrap()).collect::<Vec<_>>(), ["github.com/o/bp", "desktop/p2"]);
        assert_eq!(r["threads"].as_array().unwrap().iter().map(|t| (t["id"].as_str().unwrap(), t["status"].as_str().unwrap())).collect::<Vec<_>>(), [("t1", "needs"), ("t2", "running")], "what needs the person first");
        let b = json!({"machine":"framework","seen":90,"projects":[{"key":"github.com/o/bp","name":"bp","origin":"github.com/o/bp","id":"p7","folder":"/f/bp"}],"threads":[{"key":"github.com/o/bp","id":"t7","title":"seven","status":"idle","activity":50,"pinned":true}]});
        let m = merge(&[r, b]);
        assert_eq!(m.len(), 2); let bp = &m[0];
        assert_eq!((bp.key.as_str(), bp.name.as_str()), ("github.com/o/bp", "Backplane"));
        assert_eq!(bp.machines.iter().map(|x| (x.machine.as_str(), x.id.as_str(), x.seen)).collect::<Vec<_>>(), [("desktop", "p1", 100), ("framework", "p7", 90)]);
        assert_eq!(bp.threads.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["desktop~t1", "desktop~t2", "framework~t7"]); assert!(bp.threads[2].pinned);
    }
    #[test] fn a_clone_goes_to_documents_beside_another_folder_of_that_name() {
        let h = std::path::Path::new("/h");
        assert_eq!(clone_dir(h, "github.com/o/Backplane", &|_| None), std::path::PathBuf::from("/h/Documents/Backplane"));
        assert_eq!(clone_dir(h, "github.com/o/Backplane", &|d| (d.ends_with("Backplane")).then(|| "github.com/x/Backplane".to_string())), std::path::PathBuf::from("/h/Documents/Backplane-2"));
        assert_eq!(clone_dir(h, "github.com/o/Backplane", &|d| (d.ends_with("Backplane")).then(|| "github.com/o/Backplane".to_string())), std::path::PathBuf::from("/h/Documents/Backplane"), "the same repository: added as it is");
    }
}

//! Bring a Backplane 0.14 home across into this one (docs/IMPORT-0.14.md). 0.14 kept everything in `<user folder>/.backplane-bend` on Linux,
//! macOS and Windows alike (its desktop app's `os.homedir()`); `BACKPLANE_IMPORT_FROM` names another (the 0.14 → 0.15 update passes the home
//! 0.14 ran with). Finding it only looks: nothing is read until the person asks what is there, and nothing here is written until they say import.
//! The old home is only ever read: no file in it is changed, moved or deleted.
//!
//! What comes across is decided in `plan` (the log) and `files` (attachments, bot folders, account profiles, the vault, paired devices). Keys and
//! sign-ins come only when the person chose "with keys". The lines go in through `Hub::import_lines`, which no policy, push or feed sees, with the
//! policies folded again around it. Ways in: the RPC `import.v014` {op: "scan"|"run", secrets} (setup and Settings), and for a script with the
//! home's token, GET `/import/0.14` (what is there) and GET `/import/0.14/run[?secrets=1]` (import, answers when done).
//! Progress and the result are the hub info key `import.v014` (JSON text), which setup and Settings draw; the last result is also the setting
//! `import.v014.done`, so it outlives a restart.
pub mod files;
pub mod plan;
pub mod progress;
#[cfg(test)] mod tests;

use crate::{api::{Caller, Fut, HttpHandler, HttpReply, RpcHandler}, auth::Req, hub::Hub};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::Arc};

pub const INFO: &str = "import.v014";
pub const DONE: &str = "import.v014.done";
/// The person saw the summary (the time of the result they saw): the desktop's overlay and the phones' banner stop showing it, and its notice leaves the push book.
pub const ACK: &str = "import.v014.ack";
/// What runs now, as everyone may see it (`/healthz`, `/hello`): None when no import runs.
static NOW: std::sync::Mutex<Option<Arc<progress::Progress>>> = std::sync::Mutex::new(None);
pub fn migrating() -> Option<Value> { NOW.lock().unwrap().as_ref().map(|p| p.public()) }
/// The same with the names, for the person's own clients (data/1 hello).
pub fn migrating_full() -> Option<Value> { NOW.lock().unwrap().as_ref().map(|p| p.full(crate::hub::now_ms())) }
/// Lines per write: each is one append and fsync and one fold, and a client's write waits behind at most one. The size follows how long the last one held
/// the writer and the policies (`next_chunk`): about `HOLD_MS` at most, so a slow or loaded machine writes smaller chunks rather than stall a request.
const CHUNK_MIN: usize = 100; const CHUNK_MAX: usize = 2000; const HOLD_MS: u64 = 100;
/// The next chunk's size after one of `n` lines held for `ms`: halved when it took over 1.5 times the target, doubled when under half of it.
pub fn next_chunk(n: usize, ms: u64) -> usize { if ms > HOLD_MS * 3 / 2 { (n / 2).max(CHUNK_MIN) } else if ms < HOLD_MS / 2 { (n * 2).min(CHUNK_MAX) } else { n } }

fn user_folder() -> Option<PathBuf> { std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).filter(|v| !v.is_empty()).map(PathBuf::from) }
fn same(a: &Path, b: &Path) -> bool { crate::plat::fs::canonicalize(a).unwrap_or_else(|_| a.into()) == crate::plat::fs::canonicalize(b).unwrap_or_else(|_| b.into()) }

/// A folder that holds a 0.14 home and is not this home nor inside it (or it inside the old one).
pub fn is_old_home(old: &Path, home: &Path) -> bool { old.join("events.jsonl").is_file() && !same(old, home) && !home.starts_with(old) && !old.starts_with(home) }

/// The folders 0.14 may have used as its home on this system, from its own launchers: everywhere `<user folder>/.backplane-bend` (the desktop app's
/// `os.homedir()`); on Windows also the Cygwin user folder 0.14's hub-only launcher (Run-Backplane.ps1: `bash --login`, HOME unset) gave it,
/// `<cygwin root>\home\<user>\.backplane-bend`, the root being BACKPLANE_CYGWIN_ROOT, `%LOCALAPPDATA%\Backplane\cygwin64` or `C:\cygwin64`.
pub fn candidates(windows: bool, user: Option<PathBuf>, local_app_data: Option<PathBuf>, user_name: Option<String>, cygwin_root: Option<PathBuf>) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = user.into_iter().map(|u| u.join(".backplane-bend")).collect();
    if windows {
        let roots: Vec<PathBuf> = cygwin_root.into_iter().chain(local_app_data.map(|l| l.join("Backplane").join("cygwin64"))).chain([PathBuf::from(r"C:\cygwin64")]).collect();
        if let Some(n) = user_name.filter(|n| !n.is_empty()) { for r in roots { v.push(r.join("home").join(&n).join(".backplane-bend")) } }
    }
    v
}
/// Where this hub imports from: `from` (BACKPLANE_IMPORT_FROM) when given, else the candidate that is a 0.14 home and was written last (a machine that ran
/// 0.14 both ways keeps its newest data). A home under the temp folder (tests) looks only at `from`.
pub fn source_in(home: &Path, from: Option<PathBuf>, candidates: Vec<PathBuf>) -> Option<PathBuf> {
    if let Some(f) = from { return is_old_home(&f, home).then_some(f) }
    if home.starts_with(std::env::temp_dir()) { return None }
    let when = |p: &PathBuf| std::fs::metadata(p.join("events.jsonl")).and_then(|m| m.modified()).ok();
    candidates.into_iter().filter(|c| is_old_home(c, home)).max_by_key(when)
}
pub fn source(home: &Path) -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty());
    source_in(home, env("BACKPLANE_IMPORT_FROM").map(PathBuf::from),
        candidates(cfg!(windows), user_folder(), env("LOCALAPPDATA").map(PathBuf::from), env("USERNAME").and_then(|n| n.into_string().ok()), env("BACKPLANE_CYGWIN_ROOT").map(PathBuf::from)))
}

/// Whether 0.14's hub holds its home now (its flock on hub.lock); then what it writes after the read comes in a later import.
fn running(old: &Path) -> bool {
    let Ok(f) = std::fs::File::open(old.join("hub.lock")) else { return false };
    !crate::plat::hub::lock(&f) // (taken for an instant when free, and let go as the file closes)
}

/// What this home already has: every line's hash, its settings, the ids it made.
pub fn have_of(store: &crate::store::Store) -> plan::Have {
    let mut h = plan::Have::default(); let n = store.len(); let mut a = 0;
    while a < n { let b = (a + 8192).min(n); if let Ok(ls) = store.range(a, b) { for l in &ls { h.add(l) } } a = b; }
    h
}

pub struct Importer { pub from: Option<PathBuf>, busy: tokio::sync::Mutex<()>, me: std::sync::Weak<Importer>,
    /// ms slept after each chunk written: a slow import on purpose, for tests (BACKPLANE_IMPORT_THROTTLE_MS)
    pub throttle_ms: std::sync::atomic::AtomicU64 }

/// Finds the old home and says so in the hub info; registers the RPC and the routes. Reads nothing more until asked. A copy of a real log imports nothing.
/// BACKPLANE_IMPORT_AUTO=1 (set by the 0.14 to 0.15 update's shim: the person pressed Update in 0.14): the import begins by itself once the hub serves, on every start until
/// it has finished, without keys (those still need the person's yes). Without it a fresh install waits for the person's Import.
pub fn start(hub: &Arc<Hub>) -> Arc<Importer> {
    let from = if crate::copy::on() { None } else { source(&hub.cfg.home) };
    start_auto(hub, from, std::env::var("BACKPLANE_IMPORT_AUTO").as_deref() == Ok("1"))
}
pub fn start_from(hub: &Arc<Hub>, from: Option<PathBuf>) -> Arc<Importer> { start_auto(hub, from, false) }
pub fn start_auto(hub: &Arc<Hub>, from: Option<PathBuf>, auto: bool) -> Arc<Importer> {
    let me = Arc::new_cyclic(|w| Importer { from, busy: Default::default(), me: w.clone(), throttle_ms: std::sync::atomic::AtomicU64::new(std::env::var("BACKPLANE_IMPORT_THROTTLE_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(0)) });
    hub.register_rpc(me.clone()); hub.register_http(me.clone());
    let (h, m) = (hub.clone(), me.clone());
    tokio::spawn(async move {
        let done = h.setting(DONE).await.unwrap_or_default();
        let v = match (&m.from, done.is_empty()) {
            (Some(f), true) => json!({"state":"found","from":f.to_string_lossy()}),
            (Some(f), false) => { let mut d: Value = serde_json::from_str(&done).unwrap_or(json!({})); d["state"] = json!("done"); d["from"] = json!(f.to_string_lossy()); d }
            (None, false) => { let mut d: Value = serde_json::from_str(&done).unwrap_or(json!({})); d["state"] = json!("done"); d }
            (None, true) => return,
        };
        publish(&h, &v).await;
        if auto && done.is_empty() && m.from.is_some() {
            while !(crate::guard::is_ready() || cfg!(test)) || !crate::statehost::booted(&h) { tokio::time::sleep(std::time::Duration::from_millis(200)).await; }
            eprintln!("import: bringing the 0.14 home across (BACKPLANE_IMPORT_AUTO)");
            m.run(&h, false).await;
        }
    });
    me
}

/// The import's state for every client: the info key, and a redraw of every open view (setup and Settings read it).
/// The last state published, for every view (the card is drawn over any page, and a view carries the hub's info only on a few).
static LAST: std::sync::Mutex<Vec<(PathBuf, Value)>> = std::sync::Mutex::new(Vec::new());
pub fn last_published(home: &Path) -> Option<Value> { LAST.lock().unwrap().iter().find(|(h, _)| h == home).map(|(_, v)| v.clone()) }
/// An import ran in this start: the uberbot's restart wake stays asleep (it would act on 0.14's history, maybe while 0.14 still runs).
static RAN: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());
pub fn ran_this_start(home: &Path) -> bool { RAN.lock().unwrap().iter().any(|h| h == home) }
async fn publish(hub: &Arc<Hub>, v: &Value) {
    { let mut l = LAST.lock().unwrap(); l.retain(|(h, _)| h != &hub.cfg.home); l.push((hub.cfg.home.clone(), v.clone())); }
    hub.set_info(INFO, &v.to_string()).await; hub.bump_views();
    let mut f = v.clone(); f["t"] = json!("migrating"); f["card"] = crate::present::onboard::import_card(v, false, false).unwrap_or(Value::Null); hub.push_all(&f); // the phones (data/1 `migrating`)
}

/// What a phone shows on connect (data/1 hello `migrating`): the import running, or its result until the person has seen it (ACK); null otherwise.
pub async fn phone_state(hub: &Arc<Hub>) -> Value {
    if let Some(mut m) = migrating_full() { m["card"] = crate::present::onboard::import_card(&m, false, false).unwrap_or(Value::Null); return m }
    let done = hub.setting(DONE).await.unwrap_or_default(); if done.is_empty() { return Value::Null }
    let mut v: Value = serde_json::from_str(&done).unwrap_or(Value::Null);
    let seen = hub.setting(ACK).await.unwrap_or_default();
    if !v.is_object() || seen == v["at"].to_string() { return Value::Null }
    v["state"] = json!("done"); v["card"] = crate::present::onboard::import_card(&v, false, false).unwrap_or(Value::Null); v
}

/// The notes on what stays behind, as codes the words are chosen for (present/onboard.rs `import_note`).
fn notes(p: &plan::Plan, old: &Path, secrets: bool, extra: Vec<Value>) -> Vec<Value> {
    let mut n = vec![];
    if p.counts.runs > 0 { n.push(json!({"k":"runs","n":p.counts.runs})) }
    if p.routines_on > 0 { n.push(json!({"k":"routines-off","n":p.routines_on})) }
    if p.peers > 0 { n.push(json!({"k":"machines"})) }
    if !secrets { n.push(json!({"k":"keys-stay"})) }
    if old.join("secrets/google.json").is_file() { n.push(json!({"k":"google"})) }
    if p.counts.clashes > 0 { n.push(json!({"k":"clashes","n":p.counts.clashes})) }
    n.push(json!({"k":"fresh"}));
    if running(old) { n.push(json!({"k":"running"})) }
    n.extend(extra);
    n
}

impl Importer {
    /// What would come across now: counts and notes, nothing written.
    pub async fn scan(&self, hub: &Arc<Hub>, secrets: bool) -> Value {
        let Some(old) = self.from.clone() else { return json!({"state":"none"}) };
        publish(hub, &json!({"state":"scanning","from":old.to_string_lossy()})).await;
        let (store, o) = (hub.store.clone(), old.clone());
        let r = tokio::task::spawn_blocking(move || { let have = have_of(&store); plan::plan(&o.join("events.jsonl"), &have, plan::Opts { secrets, keep: false }) }).await;
        let v = match r {
            Ok(Ok(mut p)) => { p.counts.devices = files::live_devices(&old); json!({"state":"ready","from":old.to_string_lossy(),"counts":p.counts,"bots":p.bots.iter().map(|(_, n)| n).collect::<Vec<_>>(),"notes":notes(&p, &old, secrets, vec![])}) },
            Ok(Err(e)) => json!({"state":"failed","from":old.to_string_lossy(),"why":format!("The 0.14 log could not be read: {e}.")}),
            Err(e) => json!({"state":"failed","from":old.to_string_lossy(),"why":format!("{e}")}),
        };
        publish(hub, &v).await;
        v
    }
    /// Import: the lines, then the files. One at a time; answers the result (also the info and the setting `import.v014.done`).
    pub async fn run(&self, hub: &Arc<Hub>, secrets: bool) -> Value {
        let Some(old) = self.from.clone() else { return json!({"state":"none"}) };
        let Ok(_one) = self.busy.try_lock() else { return json!({"state":"running","from":old.to_string_lossy(),"why":"An import is running already."}) };
        let from = old.to_string_lossy().into_owned();
        RAN.lock().unwrap().push(hub.cfg.home.clone());
        let now = crate::hub::now_ms;
        // what runs, for /healthz, /hello and the clients, and the guard's word that this start is not proven until it is over
        let prog = Arc::new(progress::Progress::default()); prog.begin(&from, secrets, now());
        *NOW.lock().unwrap() = Some(prog.clone()); let _folding = crate::guard::Folding::begin();
        struct Clear; impl Drop for Clear { fn drop(&mut self) { *NOW.lock().unwrap() = None; } }
        let _clear = Clear;
        let tick = { let (h, p) = (hub.clone(), prog.clone()); tokio::spawn(async move { loop { publish(&h, &p.full(now())).await; tokio::time::sleep(std::time::Duration::from_secs(1)).await; } }) };
        struct Stop(tokio::task::JoinHandle<()>); impl Drop for Stop { fn drop(&mut self) { self.0.abort() } }
        let _tick = Stop(tick);
        let (store, o, p2) = (hub.store.clone(), old.clone(), prog.clone());
        let p = match tokio::task::spawn_blocking(move || { let have = have_of(&store); p2.step("reading", 0, "bytes", now());
            plan::plan_with(&o.join("events.jsonl"), &have, plan::Opts { secrets, keep: true }, &|b, of| { if b == 0 { return } p2.step_total(of); p2.at(b, now()) }) }).await {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => return self.failed(hub, &from, &format!("The 0.14 log could not be read: {e}")).await,
            Err(e) => return self.failed(hub, &from, &format!("{e}")).await,
        };
        prog.totals(p.counts.projects as u64, p.counts.threads as u64);
        // files first: the lines name them (bots' folders, accounts), and a cut-off import copies again without harm
        prog.step("copying", 0, "files", now());
        let (h2, o2, bots) = (hub.cfg.home.clone(), old.clone(), p.bots.clone());
        let hooks = p.hooks.clone();
        let have_profiles = crate::settings::accounts_hub::live(&hub.cfg.home).map(|m| m.ids()).unwrap_or_default();
        let p3 = prog.clone();
        let fr = tokio::task::spawn_blocking(move || files::copy_counted(&o2, &h2, &bots, &hooks, secrets, &have_profiles, Box::new(move |n| p3.at(n, now())))).await.unwrap_or_default();
        let mut extra = fr.notes.clone();
        let accounts = files::adopt_accounts(&hub.cfg.home, secrets, &fr);
        if let Some(n) = &accounts.note { extra.push(n.clone()) }
        // the quotes and orders of the threads coming across, and of those an earlier import brought (one made before records came across gets them on Import again);
        // followed from the next start, as at any start
        let threads = p.threads.clone();
        let (h3, o3) = (hub.cfg.home.clone(), old.clone()); let _ = tokio::task::spawn_blocking(move || files::copy_orders(&o3, &h3, &threads)).await;
        let devices = files::adopt_devices(&hub.cfg.home, &old);
        let keys = if secrets { files::adopt_vault(&old) } else { files::Keys::default() };
        if let Some(n) = &keys.note { extra.push(n.clone()) }
        // 0.14's Google sign-ins (secrets/google.json): its client and its accounts' refresh tokens into the vault; no bot is allowed one until the person says so
        if secrets { let g = crate::google::store::adopt_014(&old); if g > 0 { extra.push(json!({"k":"google","n":g})) } }
        // the lines: each chunk joins the policies' state as it is written (the views keep answering, nothing is derived); a client's request waits for at most one chunk
        let total = p.lines.len(); let mut done = 0usize; let mut err = String::new();
        prog.step("writing", total as u64, "events", now());
        let throttle = Some(self.throttle_ms.load(std::sync::atomic::Ordering::Relaxed)).filter(|ms| *ms > 0); // tests only: a slow import on purpose
        let mut titles: std::collections::HashMap<String, String> = Default::default();
        let (mut at, mut size) = (0usize, 500usize);
        while at < total {
            let ch = &p.lines[at..(at + size).min(total)]; at += ch.len();
            let (mut np, mut nt, mut cur) = (0, 0, None);
            for l in ch { match crate::session::event_type(l) {
                Some("ProjectCreated") => { np += 1; if let Ok(v) = serde_json::from_str::<Value>(l) { titles.insert(v["id"].as_str().unwrap_or("").into(), v["title"].as_str().unwrap_or("").into()); } }
                Some("ThreadCreated") => { nt += 1; if let Ok(v) = serde_json::from_str::<Value>(l) { let pt = titles.get(v["project"].as_str().unwrap_or("")).cloned().unwrap_or_default();
                    let t = v["title"].as_str().unwrap_or("").chars().take(80).collect::<String>(); cur = Some(if pt.is_empty() { t } else { format!("{pt} \u{b7} {t}") }); } }
                _ => {} } }
            let parsed: Vec<Value> = ch.iter().filter_map(|l| serde_json::from_str::<Value>(l).ok()).collect(); // (before the writer: a request waits for none of this)
            let tw = std::time::Instant::now();
            match hub.import_lines(ch.to_vec()).await {
                Ok(end) => { let w = tw.elapsed().as_millis() as u64; let tf = std::time::Instant::now();
                    let f = crate::statehost::fold_quiet(hub, end - ch.len() as u64 + 1, parsed); // (the longest single hold of the policies' lock)
                    prog.held(w, f); size = next_chunk(ch.len(), w.max(tf.elapsed().as_millis() as u64)); }
                Err(e) => { err = format!("{e:#}"); break }
            }
            done += ch.len(); prog.made(np, nt, cur); prog.at(done as u64, now());
            // tests only: a slow import on purpose, the same per 2,000 lines whatever the chunk
            if let Some(ms) = throttle { tokio::time::sleep(std::time::Duration::from_millis(ms * ch.len() as u64 / 2000)).await; }
        }
        prog.step("finishing", 0, "", now());
        if !err.is_empty() { return self.failed(hub, &from, &format!("Writing into this hub stopped after {done} of {total} events: {err}")).await }
        let mut c = p.counts.clone(); c.devices = devices;
        let r = json!({"state":"done","from":from,"counts":c,"bots":p.bots.iter().map(|(_, n)| n).collect::<Vec<_>>(),"devices":devices,"accounts":accounts.added,"signin":accounts.signin,"keys":keys.added,
            "files":fr.copied,"secrets":secrets,"at":now() / 1000,"elapsed_s":prog.full(now())["elapsed_s"],"holds":prog.holds(),"notes":notes(&p, &old, secrets, extra)});
        let _ = hub.commit(&json!({"$":"SettingSet","key":DONE,"value":r.to_string(),"at":now() / 1000})).await;
        // one notice for the phones that were away (the push book's rules: quiet hours, mutes)
        if c.lines > 0 { let text = format!("Your 0.14 work is here: {}", crate::present::onboard::import_counted(&json!(c)));
            let _ = hub.commit(&crate::push::Source::import_done().order(&format!("import-{}", now()), "import", "", "", &text, now() / 1000)).await; }
        drop(_tick); *NOW.lock().unwrap() = None;
        publish(hub, &r).await;
        r
    }
    async fn failed(&self, hub: &Arc<Hub>, from: &str, why: &str) -> Value {
        *NOW.lock().unwrap() = None;
        let v = json!({"state":"failed","from":from,"why":why.trim_end_matches('.')}); publish(hub, &v).await; v
    }
}

impl RpcHandler for Importer {
    fn methods(&self) -> &'static [&'static str] { &["import.v014"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let (op, secrets) = (p["op"].as_str().unwrap_or("scan").to_string(), p["secrets"] == true);
            if self.from.is_none() { return vec![json!({"t":"reply","id":id,"ok":false,"error":"No Backplane 0.14 data was found on this computer."})] }
            let (h, me) = (hub.clone(), self.me.upgrade());
            tokio::spawn(async move { let Some(me) = me else { return }; if op == "run" { me.run(&h, secrets).await; } else { me.scan(&h, secrets).await; } });
            vec![json!({"t":"reply","id":id,"ok":true})]
        })
    }
}
impl HttpHandler for Importer {
    fn routes(&self) -> &'static [&'static str] { &["/import/0.14"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            let (path, q) = req.target.split_once('?').unwrap_or((req.target.as_str(), ""));
            let secrets = q.split('&').any(|kv| kv == "secrets=1");
            let v = match path.trim_end_matches('/') {
                "/import/0.14" => match hub.info_now()[INFO].as_str().and_then(|t| serde_json::from_str::<Value>(t).ok()) {
                    Some(d) if d["state"] == "done" || d["state"] == "running" => d,
                    _ => self.scan(hub, secrets).await,
                },
                "/import/0.14/run" => self.run(hub, secrets).await,
                _ => return HttpReply::text("404 Not Found", "no such route"),
            };
            HttpReply::ok("application/json", v.to_string().into_bytes())
        })
    }
}

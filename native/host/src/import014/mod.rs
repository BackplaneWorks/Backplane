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
#[cfg(test)] mod tests;

use crate::{api::{Caller, Fut, HttpHandler, HttpReply, RpcHandler}, auth::Req, hub::Hub};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::Arc};

pub const INFO: &str = "import.v014";
pub const DONE: &str = "import.v014.done";
/// Lines per write: each is one append and fsync, one fold, one progress note.
const CHUNK: usize = 4000;

fn user_folder() -> Option<PathBuf> { std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).filter(|v| !v.is_empty()).map(PathBuf::from) }
fn same(a: &Path, b: &Path) -> bool { crate::plat::fs::canonicalize(a).unwrap_or_else(|_| a.into()) == crate::plat::fs::canonicalize(b).unwrap_or_else(|_| b.into()) }

/// A folder that holds a 0.14 home and is not this home nor inside it (or it inside the old one).
pub fn is_old_home(old: &Path, home: &Path) -> bool { old.join("events.jsonl").is_file() && !same(old, home) && !home.starts_with(old) && !old.starts_with(home) }

/// Where this hub looks: `from` (BACKPLANE_IMPORT_FROM), else `<user>/.backplane-bend`. A home under the temp folder (tests) looks only at `from`.
pub fn source_in(home: &Path, from: Option<PathBuf>, user: Option<PathBuf>) -> Option<PathBuf> {
    let c = from.or_else(|| if home.starts_with(std::env::temp_dir()) { None } else { user.map(|u| u.join(".backplane-bend")) })?;
    is_old_home(&c, home).then_some(c)
}
pub fn source(home: &Path) -> Option<PathBuf> { source_in(home, std::env::var_os("BACKPLANE_IMPORT_FROM").filter(|v| !v.is_empty()).map(PathBuf::from), user_folder()) }

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

pub struct Importer { pub from: Option<PathBuf>, busy: tokio::sync::Mutex<()>, me: std::sync::Weak<Importer> }

/// Finds the old home and says so in the hub info; registers the RPC and the routes. Reads nothing more until asked. A copy of a real log imports nothing.
pub fn start(hub: &Arc<Hub>) -> Arc<Importer> { start_from(hub, if crate::copy::on() { None } else { source(&hub.cfg.home) }) }
pub fn start_from(hub: &Arc<Hub>, from: Option<PathBuf>) -> Arc<Importer> {
    let me = Arc::new_cyclic(|w| Importer { from, busy: Default::default(), me: w.clone() });
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
        h.set_info(INFO, &v.to_string()).await;
    });
    me
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
        hub.set_info(INFO, &json!({"state":"scanning","from":old.to_string_lossy()}).to_string()).await;
        let (store, o) = (hub.store.clone(), old.clone());
        let r = tokio::task::spawn_blocking(move || { let have = have_of(&store); plan::plan(&o.join("events.jsonl"), &have, plan::Opts { secrets, keep: false }) }).await;
        let v = match r {
            Ok(Ok(mut p)) => { p.counts.devices = files::live_devices(&old); json!({"state":"ready","from":old.to_string_lossy(),"counts":p.counts,"bots":p.bots.iter().map(|(_, n)| n).collect::<Vec<_>>(),"notes":notes(&p, &old, secrets, vec![])}) },
            Ok(Err(e)) => json!({"state":"failed","from":old.to_string_lossy(),"why":format!("The 0.14 log could not be read: {e}.")}),
            Err(e) => json!({"state":"failed","from":old.to_string_lossy(),"why":format!("{e}")}),
        };
        hub.set_info(INFO, &v.to_string()).await;
        v
    }
    /// Import: the lines, then the files. One at a time; answers the result (also the info and the setting `import.v014.done`).
    pub async fn run(&self, hub: &Arc<Hub>, secrets: bool) -> Value {
        let Some(old) = self.from.clone() else { return json!({"state":"none"}) };
        let Ok(_one) = self.busy.try_lock() else { return json!({"state":"running","from":old.to_string_lossy(),"why":"An import is running already."}) };
        let from = old.to_string_lossy().into_owned();
        hub.set_info(INFO, &json!({"state":"running","from":from,"done":0,"of":0}).to_string()).await;
        let (store, o) = (hub.store.clone(), old.clone());
        let p = match tokio::task::spawn_blocking(move || { let have = have_of(&store); plan::plan(&o.join("events.jsonl"), &have, plan::Opts { secrets, keep: true }) }).await {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => return self.failed(hub, &from, &format!("The 0.14 log could not be read: {e}.")).await,
            Err(e) => return self.failed(hub, &from, &format!("{e}")).await,
        };
        // files first: the lines name them (bots' folders, accounts), and a cut-off import copies again without harm
        let (h2, o2, bots) = (hub.cfg.home.clone(), old.clone(), p.bots.clone());
        let hooks = p.hooks.clone();
        let have_profiles = crate::settings::accounts_hub::live(&hub.cfg.home).map(|m| m.ids()).unwrap_or_default();
        let fr = tokio::task::spawn_blocking(move || files::copy(&o2, &h2, &bots, &hooks, secrets, &have_profiles)).await.unwrap_or_default();
        let mut extra = fr.notes.clone();
        let accounts = files::adopt_accounts(&hub.cfg.home, secrets, &fr);
        if let Some(n) = &accounts.note { extra.push(n.clone()) }
        let devices = files::adopt_devices(&hub.cfg.home, &old);
        let keys = if secrets { files::adopt_vault(&old) } else { files::Keys::default() };
        if let Some(n) = &keys.note { extra.push(n.clone()) }
        // the lines, with the policies holding their batches until the log is folded again
        let owned = crate::statehost::refold_begin(hub);
        let total = p.lines.len(); let mut done = 0usize; let mut err = String::new();
        for ch in p.lines.chunks(CHUNK) {
            if let Err(e) = hub.import_lines(ch.to_vec()).await { err = format!("{e:#}"); break }
            done += ch.len();
            hub.set_info(INFO, &json!({"state":"running","from":from,"done":done,"of":total}).to_string()).await;
        }
        crate::statehost::refold_end(hub, owned);
        if !err.is_empty() { return self.failed(hub, &from, &format!("Writing to this home failed after {done} of {total} events: {err}. Import again to finish: what is here already is not written twice.")).await }
        let mut c = p.counts.clone(); c.devices = devices;
        let r = json!({"state":"done","from":from,"counts":c,"bots":p.bots.iter().map(|(_, n)| n).collect::<Vec<_>>(),"devices":devices,"accounts":accounts.added,"signin":accounts.signin,"keys":keys.added,
            "files":fr.copied,"secrets":secrets,"at":crate::hub::now_ms() / 1000,"notes":notes(&p, &old, secrets, extra)});
        let _ = hub.commit(&json!({"$":"SettingSet","key":DONE,"value":r.to_string(),"at":crate::hub::now_ms() / 1000})).await;
        hub.set_info(INFO, &r.to_string()).await;
        r
    }
    async fn failed(&self, hub: &Arc<Hub>, from: &str, why: &str) -> Value {
        let v = json!({"state":"failed","from":from,"why":why}); hub.set_info(INFO, &v.to_string()).await; v
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

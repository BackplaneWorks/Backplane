//! The hub core for native clients: connection identity, view requests, reply assembly,
//! segmentation, change notices. State is what the event log folds to; the picture comes from
//! the presenter. (window/overlay for phones arrive with M2's windowed history.)
use crate::{api::{AuthRoute, Caller, FarView, Transform, HttpHandler, Observer, Pairing, RpcHandler, Ticker}, auth::Auth, model::Model, presenter::Presenter, session::ViewMethod, store::Store};
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::path::PathBuf;
use std::sync::{atomic::{AtomicU64, Ordering::Relaxed}, Arc};
use tokio::sync::{broadcast, Mutex};


pub struct Cfg { pub home: PathBuf, pub version: String, pub commit: String, pub listen: String, pub os: String, pub arch: String }

/// What `Hub::rebuild_presenter` did: ran the rebuild (and whether the restart took), or left the request to the rebuild already running.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Rebuilt { Ran(bool), Merged }

pub struct Hub {
    pub cfg: Cfg, pub auth: Auth, pub epoch: String, pub presenter: Arc<dyn Presenter>,
    pub store: Store, pub model: Mutex<Model>, gen: AtomicU64, pub changed: broadcast::Sender<u64>, pub frames: broadcast::Sender<Value>, pub native_key: String,
    commits: tokio::sync::mpsc::UnboundedSender<Commit>,
    rpc: std::sync::RwLock<Vec<(&'static str, Arc<dyn RpcHandler>)>>, http: std::sync::RwLock<Vec<(&'static str, Arc<dyn HttpHandler>)>>,
    views: std::sync::RwLock<Vec<(&'static str, Arc<dyn ViewMethod>)>>, observers: std::sync::RwLock<Vec<Arc<dyn Observer>>>, transforms: std::sync::RwLock<Vec<Arc<dyn Transform>>>, far: std::sync::OnceLock<Arc<dyn FarView>>, far_router: std::sync::OnceLock<Arc<dyn crate::api::FarRouter>>, far_mcp: std::sync::OnceLock<Arc<dyn crate::api::FarMcp>>, mirror_feed: std::sync::OnceLock<Arc<dyn crate::api::MirrorFeed>>, mirror_gate: tokio::sync::Mutex<()>, mirrors: crate::mirror::Mirrors, rebuild_run: tokio::sync::Mutex<()>, rebuild_again: std::sync::atomic::AtomicBool, pairing: std::sync::OnceLock<Arc<dyn Pairing>>, auth_route: std::sync::OnceLock<Arc<dyn AuthRoute>>,
    live: std::sync::Mutex<std::collections::HashMap<String, String>>,
    pub window_bin: std::sync::OnceLock<PathBuf>,
    /// The hub's one account manager (settings/accounts_hub.rs `manager`): opened by whichever needs it first, the agents' recovery or settings::start, so neither waits on the other.
    pub accounts: std::sync::OnceLock<Arc<crate::settings::accounts_io::Accounts>>,
    me: std::sync::OnceLock<std::sync::Weak<Hub>>,
    outboxes: std::sync::Mutex<std::collections::HashMap<u64, Slot>>, outbox_max: std::sync::atomic::AtomicUsize, signal_seq: AtomicU64, id_alloc: AtomicU64, view_bump: AtomicU64, need_resync: std::sync::atomic::AtomicBool, extra_info: std::sync::Mutex<std::collections::BTreeMap<String, String>>,
}

/// One item of a connection's outbox: a value the hub encodes (CBOR with the dictionaries), or bytes that go out as
/// one binary frame exactly as given (plot frames carry byte strings and plain text the dictionaries would change).
pub enum Out { Value(Value), Raw(Vec<u8>), /// end the connection (its device was revoked)
    Close }

/// The most bytes a client's outbox may hold unread. A client that stops reading is dropped (it reconnects and resumes
/// from its cursor) rather than growing the hub without limit; droppable frames (terminal output, plots) are skipped once
/// the outbox is half full, so frames that must arrive keep their room.
pub const OUTBOX_MAX: usize = 8 << 20;
struct Slot { tx: tokio::sync::mpsc::UnboundedSender<(Out, usize)>, pending: Arc<std::sync::atomic::AtomicUsize> }
/// What a connection loop reads: the frames pushed to it. Reading one gives its bytes back to the budget.
pub struct Outbox { rx: tokio::sync::mpsc::UnboundedReceiver<(Out, usize)>, pending: Arc<std::sync::atomic::AtomicUsize> }
impl Outbox {
    pub async fn recv(&mut self) -> Option<Out> { let (o, n) = self.rx.recv().await?; self.pending.fetch_sub(n, Relaxed); Some(o) }
    /// Without waiting (tests, drains): Err(Empty) when nothing is queued, Err(Disconnected) when the client was dropped and the queue is empty.
    pub fn try_recv(&mut self) -> Result<Out, tokio::sync::mpsc::error::TryRecvError> { let (o, n) = self.rx.try_recv()?; self.pending.fetch_sub(n, Relaxed); Ok(o) }
}
/// SignalRevision.affects: only activity lines and assistant/system messages leave the signal inputs alone; every other event,
/// including one this hub does not know, counts.
/// Setting.wire_blank: what clients get blank on the wire.
pub fn wire_blank(key: &str) -> bool { key == crate::push::BOOK || key.starts_with("launch.input.") || key.starts_with("fallback.input.") || key.starts_with("ride.") }

pub fn signal_affects(e: &Value) -> bool {
    match e["$"].as_str() { Some("ActivityLogged") => false, Some("MessagePosted") => !matches!(e["role"].as_str(), Some("assistant") | Some("system")), _ => true }
}

/// A cheap size of a frame (its strings, numbers and structure), without encoding it.
pub fn frame_bytes(v: &Value) -> usize {
    match v { Value::Null | Value::Bool(_) => 4, Value::Number(_) => 8, Value::String(s) => s.len() + 2,
        Value::Array(a) => 2 + a.iter().map(|x| frame_bytes(x) + 1).sum::<usize>(), Value::Object(o) => 2 + o.iter().map(|(k, x)| k.len() + 3 + frame_bytes(x)).sum::<usize>() }
}

/// `raw`: lines brought in whole from another log (the 0.14 import, `import_lines`): written byte for byte, folded and announced, never seen by a transform or an observer.
struct Commit { events: Vec<Value>, raw: Vec<String>, done: tokio::sync::oneshot::Sender<Result<u64>> }

/// One WebSocket connection's identity and its single open view.
/// Client numbers are unique in the process, not per hub: what is keyed by the number alone (the account grant, session::forget, a detach) must never reach
/// another hub's client. One hub runs in a real process; the tests run many at once, and each counting from 1 let one test's disconnect revoke another's
/// account grant. Tests start above the numbers they attach by hand (`hub.attach(2)`).
static NEXT_CLIENT: AtomicU64 = AtomicU64::new(if cfg!(test) { 1 << 20 } else { 1 });
pub struct Conn { pub sent: Vec<(String, Value)>, pub client: u64, pub gen: String, pub view_id: String, pub target: Value, pub cbor: bool, pub lastrev: u64 }

pub fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0) }
/// (zone name, POSIX rule): TZ or /etc/timezone or /etc/localtime, the rule from the zone file's footer.
fn machine_zone() -> (String, String) {
    #[cfg(windows)] {
        // Windows: no /etc/localtime or zoneinfo: TZ, else the system's zone key mapped to IANA, its rule from the built-in table
        let name = std::env::var("TZ").ok().filter(|z| !z.is_empty()).or_else(windows_zone).unwrap_or_else(|| "UTC".into());
        let rule = crate::settings::zone::rule(&name);
        return (name, if rule.is_empty() { "UTC0".into() } else { rule })
    }
    #[allow(unreachable_code)]
    let name = std::env::var("TZ").ok().filter(|z| !z.is_empty()).or_else(|| std::fs::read_to_string("/etc/timezone").ok().map(|s| s.trim().to_string()))
        .or_else(|| std::fs::read_link("/etc/localtime").ok().and_then(|p| p.to_string_lossy().split("zoneinfo/").nth(1).map(str::to_string))).unwrap_or_else(|| "UTC".into());
    let rule = std::fs::read(format!("/usr/share/zoneinfo/{name}")).ok().and_then(|b| { let s = String::from_utf8_lossy(&b).into_owned(); let t = s.trim_end_matches('\n'); t.rsplit('\n').next().map(str::to_string) })
        .filter(|r| !r.is_empty() && r.len() < 80 && r.is_ascii()).unwrap_or_else(|| "UTC0".into());
    (name, rule)
}
#[cfg(windows)]
fn windows_zone() -> Option<String> {
    use windows_sys::Win32::System::Time::{GetDynamicTimeZoneInformation, DYNAMIC_TIME_ZONE_INFORMATION};
    let mut z: DYNAMIC_TIME_ZONE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetDynamicTimeZoneInformation(&mut z) } == u32::MAX { return None }
    let n = z.TimeZoneKeyName.iter().position(|c| *c == 0).unwrap_or(z.TimeZoneKeyName.len());
    crate::settings::zone::from_windows(&String::from_utf16_lossy(&z.TimeZoneKeyName[..n])).map(str::to_string)
}
pub fn limits() -> Value { json!({"replyBytes":262144,"entries":32,"recent":24,"searchResults":25,"searchCandidates":128,"chunkBytes":65536}) }
pub fn capabilities() -> Value { json!({"version":1,"views":{"version":1,"websocketQuery":"view=1","renderers":["mobile"],"compatibility":"explicit-only"},"data":{"proto":"data/1","features":crate::data::FEATURES}}) }
pub fn error(id: &Value, code: &str) -> Value { json!({"t":"reply","id":id,"ok":false,"error":{"code":code,"retryable":false,"compatibilityRequired":false}}) }


impl Hub {
    pub async fn start(cfg: Cfg, presenter: Arc<dyn Presenter>) -> Result<Arc<Hub>> {
        let store = Store::open(&cfg.home.join("events.jsonl"))?;
        crate::startup::mark("store indexed");
        { let s = store.clone(); crate::ckpt::preload("agents", &store, move || crate::agents::world::World::load(&s)); }
        { let s = store.clone(); crate::ckpt::preload("bots", &store, move || crate::bots::world::World::load(&s)); }
        crate::statehost::preload(&store);
        let auth = Auth::load(&cfg.home);
        let epoch = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs().to_string();
        let (ctx, crx) = tokio::sync::mpsc::unbounded_channel::<Commit>();
        let (tx, _) = broadcast::channel(64);
        let (ftx, _) = broadcast::channel(1024);
        let model = Model::open(&store)?;
        crate::startup::mark("model built");
        let hub = Arc::new(Hub { cfg, auth, epoch, presenter, store: store.clone(), commits: ctx, model: Mutex::new(model), gen: AtomicU64::new(1), changed: tx, frames: ftx, native_key: { let mut b = [0u8; 16]; let _ = crate::plat::fs::random(&mut b); hex::encode(b) },
            rpc: Default::default(), mirrors: Default::default(), http: Default::default(), views: Default::default(), observers: Default::default(), transforms: Default::default(), far: Default::default(), far_router: Default::default(), far_mcp: Default::default(), mirror_feed: Default::default(), mirror_gate: Default::default(), rebuild_run: Default::default(), rebuild_again: Default::default(), pairing: Default::default(), auth_route: Default::default(), outbox_max: std::sync::atomic::AtomicUsize::new(OUTBOX_MAX), signal_seq: AtomicU64::new(store.len() as u64), id_alloc: AtomicU64::new(0), view_bump: AtomicU64::new(0), need_resync: Default::default(), extra_info: Default::default(), live: Default::default(), window_bin: Default::default(), accounts: Default::default(), me: Default::default(), outboxes: Default::default() });
        let _ = hub.me.set(Arc::downgrade(&hub));
        // housekeeping that cannot be written (a full disk) must not stop a hub that can still serve what it has
        if let Err(e) = hub.bootstrap() { crate::guard::degraded("log writes", &format!("{e:#}")); }
        hub.signal_seq.store(hub.store.len() as u64, Relaxed); // start-up housekeeping counts: a restart is a new signal
        crate::startup::mark("bootstrapped");
        hub.fold_log().await?;
        crate::startup::mark("presenter folded");
        tokio::spawn(Hub::writer(hub.clone(), crx));
        // the presenter's process died (kill -9, a crash): restart it and fold the log into it again from the writer task
        { let hub = hub.clone(); tokio::spawn(async move { loop {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            if hub.presenter.is_down() {
                if let Some(why) = hub.presenter.why_down() { eprintln!("{why}; restarting it") }
                match hub.rebuild_presenter().await { Rebuilt::Ran(true) => eprintln!("presenter restarted; folded the log and the mirrors again"), Rebuilt::Ran(false) => tokio::time::sleep(std::time::Duration::from_secs(2)).await, Rebuilt::Merged => {} }
            }
        } }); }
        Ok(hub)
    }

    /// hub's start-up housekeeping events, written once when the log lacks them (a hub that already ran
    /// here has them): the machine's id, the time zone and its rule, empty account and allowance state.
    fn bootstrap(&self) -> Result<()> {
        let have = |k: &str| self.model.try_lock().map_or(true, |m| m.setting_seq(k).is_some());
        let at = now_ms() / 1000;
        let zone = machine_zone();
        let mut evs: Vec<Value> = vec![];
        let mut set = |k: &str, v: String| evs.push(json!({"$":"SettingSet","key":k,"value":v,"at":at}));
        if !have("machine.id") { let mut b = [0u8; 24]; let _ = crate::plat::fs::random(&mut b); set("machine.id", format!("m{}", hex::encode(b))) }
        if !have("accounts.inventory") { set("accounts.inventory", "{\"version\":1,\"revision\":0,\"profiles\":[],\"error\":\"\"}".into()) }
        if !have("allowance.codex") { set("allowance.codex", format!("{{\"provider\":\"codex\",\"at\":0,\"attempted_at\":{at},\"state\":\"error\",\"auth\":\"unknown\",\"windows\":[],\"source\":\"codex-app-server\",\"phase\":\"complete\"}}")) }
        if !have("tz.machine") { set("tz.machine", zone.0.clone()); set(&format!("tz.rule.{}", zone.0), zone.1.clone()); set("tz.checked", at.to_string()) }
        // projects whose root folder is gone: " <id> <id> "
        let missing = { let m = self.model.try_lock().map_err(|_| anyhow::anyhow!("model busy at start"))?;
            let mut ids: Vec<&String> = m.projects.iter().filter(|(_, p)| !p.removed && !p.root.is_empty() && !std::path::Path::new(&p.root).exists()).map(|(i, _)| i).collect(); ids.sort();
            if ids.is_empty() { String::new() } else { format!(" {} ", ids.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ")) } };
        let current = match self.model.try_lock() { Ok(m) => m.setting_seq("projects.missing").and_then(|s| self.store.get(s as usize - 1).ok()).and_then(|l| serde_json::from_str::<Value>(&l).ok()).and_then(|v| v["value"].as_str().map(str::to_string)).unwrap_or_default(), Err(_) => missing.clone() };
        if missing != current { evs.push(json!({"$":"SettingSet","key":"projects.missing","value":missing,"at":at})); }
        if evs.is_empty() { return Ok(()) }
        let lines: Vec<String> = evs.iter().map(|e| e.to_string()).collect();
        let first = self.store.append_many(&lines.iter().map(String::as_str).collect::<Vec<_>>())?;
        let mut m = self.model.try_lock().map_err(|_| anyhow::anyhow!("model busy at start"))?;
        for (i, l) in lines.iter().enumerate() { m.apply_line((first + i + 1) as u32, l); }
        Ok(())
    }

    /// The hub's info map as a joining client gets it (the Settings pages read it).
    pub fn info_now(&self) -> Value { self.info() }
    fn info(&self) -> Value {
        let c = &self.cfg;
        json!({"cad.install":"","accounts.live":"{\"version\":1,\"revision\":0,\"default_allowance\":null,\"error\":\"\",\"profiles\":[],\"attempts\":[]}",
            "telemetry.channel":"release","telemetry.id":"","telemetry.live":"","os":c.os,"arch":c.arch,"home":c.home.to_string_lossy(),
            "listen":c.listen,"models":crate::settings::models::info(),"models.all":crate::settings::models::all(),"machines":"","bots.remote":"[]","tailnet":"off","update":"package","tailscale":"off",
            "freecad":"","kicad.cli":"","kicad.fork":"","kicad.commit":"","kicad.status":"checking","kicad.source":"path","kicad.version":"",
            "kicad.api":"0","kicad.caps":"","kicad.why":"","kicad.min":"10.99.0-bp.4","kicad.can_install":"","kicad.ipc.servers":"",
            "browser.engine.live":"","browser.engine.err":""})
        .as_object().map(|o| { let mut o = o.clone(); crate::wire::side_by_side_info(&mut o); for (k, v) in self.extra_info.lock().unwrap().iter() { o.insert(k.clone(), json!(v)); } Value::Object(o) }).unwrap_or(Value::Null)
    }

    /// Hub info a module publishes (a terminal's state, a browser engine's...): joining clients get it through `info()`, every
    /// client now connected gets {"t":"info","info":{key:value}}; an empty value removes the key (the frame carries ""). The map is
    /// bounded (256 keys, key 128 bytes, value 64 KiB): a key past the bound is refused (false), one that exists can still change.
    pub async fn set_info(&self, key: &str, value: &str) -> bool {
        if key.is_empty() || key.len() > 128 || value.len() > 64 * 1024 { return false }
        {
            let mut m = self.extra_info.lock().unwrap();
            if value.is_empty() { if m.remove(key).is_none() { return true } }
            else {
                if m.get(key).map(String::as_str) == Some(value) { return true }
                if !m.contains_key(key) && m.len() >= 256 { return false }
                m.insert(key.to_string(), value.to_string());
            }
        }
        let frame = json!({"t":"info","info":{key:value}});
        self.push_all(&frame);
        let _ = self.presenter.fold(&json!({"t":"info","info":{key:value},"now":now_ms()})).await;
        true
    }

    /// The signal revision: the log position of the last batch that held something besides
    /// activity lines and assistant/system messages. The native sidebar keeps a saved page only while it is unchanged.
    /// A fresh number for ids built from the log position (`p<at>-<n>`, `t<at>-<n+1>`, `g<at>-<n+3>`): the log length, or past a number
    /// already handed out, so concurrent requests never share one (the log length alone is read before the commit lands). Reserves `width`.
    pub fn alloc_n(&self, width: u64) -> u64 {
        loop {
            let cur = self.id_alloc.load(Relaxed); let base = cur.max(self.store.len() as u64);
            if self.id_alloc.compare_exchange(cur, base + width, Relaxed, Relaxed).is_ok() { return base }
        }
    }
    pub fn signal_seq(&self) -> u64 { self.signal_seq.load(Relaxed) }

    /// The whole log, as the frames a client's join receives, into the presenter.
    async fn fold_log(&self) -> Result<()> {
        if !self.presenter.wants_log() { return Ok(()) }
        let total = self.store.len().min(std::env::var("BACKPLANE_FOLD_LIMIT").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX)); // dev: fold only the first N events
        let chunk = |a: usize| async move { self.store.range(a, a + 200).map(|ls| ls.iter().filter_map(|l| serde_json::from_str::<Value>(l).ok()).collect::<Vec<Value>>()) };
        let first: Vec<Value> = chunk(0).await?;
        // version "": the JS hub's mobile pictures carry no hub version (compare.mjs, shown.version)
        self.presenter.fold(&json!({"t":"log","since":0,"origin":crate::wire::origin(self),"version":"","info":self.info(),"items":first,"now":now_ms()})).await?;
        let mut at = 200;
        while at < total { let c = chunk(at).await?; self.presenter.fold(&json!({"t":"changes","items":c,"now":now_ms()})).await?; at += 200; }
        self.presenter.fold(&json!({"t":"live","items":[],"now":now_ms()})).await?;
        self.presenter.fold(&json!({"t":"info","info":{"pair":"","pair.qr":""},"now":now_ms()})).await?;
        Ok(())
    }

    /// Commit one event: durable, then folded, then announced. Callers acknowledge after this returns.
    pub async fn commit(&self, event: &Value) -> Result<u64> { self.commit_many(std::slice::from_ref(event)).await }

    /// Events of one request. Requests queue to one writer task that group-commits everything waiting: one
    /// write and one fsync for the batch, the model and the presenter in log order, then every caller is told.
    /// Fold the whole log into the presenter again, in the writer task (so no commit interleaves with it): the next batch, even an empty one,
    /// is replaced by `fold_log`. Views answer PRESENTER_UNAVAILABLE meanwhile.
    pub async fn resync_presenter(&self) { let _g = self.mirror_gate.lock().await; self.resync_held().await }
    /// (the caller holds the mirror gate)
    async fn resync_held(&self) {
        if self.mirror_feed.get().is_some() { self.mirrors.clear(); } // (given again by the feed below)
        self.need_resync.store(true, Relaxed);
        let (done, rx) = tokio::sync::oneshot::channel();
        if self.commits.send(Commit { events: vec![], raw: vec![], done }).is_ok() { let _ = rx.await; }
        // the mirrors live only in the presenter (net lane): give them back
        if let Some(f) = self.mirror_feed.get().cloned() { f.replay(&self.arc()).await }
    }
    /// Installs what re-feeds the mirrors after the presenter lost them (net lane).
    pub fn set_mirror_feed(&self, f: Arc<dyn crate::api::MirrorFeed>) { let _ = self.mirror_feed.set(f); }
    /// One mirror fold, store-and-fold or snapshot-and-fold, at a time: the net lane holds this gate around "store a batch, fold it" and the
    /// replay holds it around "snapshot a mirror, fold it", so a fold can neither overtake nor repeat what a replay gives (and a presenter
    /// rebuild cannot start between a batch's storing and its folding).
    pub async fn mirror_gate(&self) -> tokio::sync::MutexGuard<'_, ()> { self.mirror_gate.lock().await }
    /// A mirror's history was replaced, or the presenter died: the presenter is restarted, the log folded again, and the registered
    /// `MirrorFeed` gives it every mirror. One rebuild at a time: a request that arrives while one runs makes a following pass (what it
    /// changed may be missing from the running one's snapshot) and returns `Merged` at once. `Ran(false)`: the presenter would not restart (said).
    pub async fn rebuild_presenter(&self) -> Rebuilt {
        self.rebuild_again.store(true, Relaxed);
        let mut ran = None;
        loop {
            let Ok(run) = self.rebuild_run.try_lock() else { return ran.map_or(Rebuilt::Merged, Rebuilt::Ran) };
            let mut ok = ran.unwrap_or(true);
            while self.rebuild_again.swap(false, Relaxed) {
                let _g = self.mirror_gate.lock().await;
                match self.presenter.restart().await {
                    Ok(()) => self.resync_held().await,
                    Err(e) => { eprintln!("presenter restart failed: {e:#}"); ok = false; break }
                }
            }
            ran = Some(ok);
            drop(run);
            // A request that came after the last `swap` but before `run` was released found the lock held, left the flag set and returned:
            // nobody would act on it. Look again once the lock is free (the staging: far_many_rebuild_requests_leave_none_unserved).
            if !self.rebuild_again.load(Relaxed) { return Rebuilt::Ran(ok) }
        }
    }
    /// `rebuild_presenter` as a bool: false only when the presenter would not restart (a merged request counts as served).
    pub async fn forget_mirror(&self) -> bool { !matches!(self.rebuild_presenter().await, Rebuilt::Ran(false)) }
    #[cfg(test)]
    pub fn rebuild_pending(&self) -> bool { self.rebuild_again.load(Relaxed) }
    pub async fn commit_many(&self, events: &[Value]) -> Result<u64> {
        let (done, rx) = tokio::sync::oneshot::channel();
        // settings lane: a stored secret never reaches the log, the model, the presenter or a client (Hub.step.go, Secret.changes)
        let mut events = events.to_vec(); crate::settings::mask::events(&mut events);
        self.commits.send(Commit { events, raw: vec![], done }).map_err(|_| anyhow::anyhow!("hub is stopping"))?;
        rx.await?
    }
    /// History brought in from another log (the 0.14 import): each line as it was, durable, then folded into the model and the presenter and
    /// announced to the log's followers, in the writer's order. No transform and no observer sees them: they are past events, not news (no push,
    /// no alert, no feed to a linked machine, no policy reaction). The caller refolds the policies around the import (`state::host::refold_*`).
    /// Every line must be one whole JSON event without a newline.
    pub async fn import_lines(&self, lines: Vec<String>) -> Result<u64> {
        if lines.iter().any(|l| l.contains('\n')) { anyhow::bail!("an imported line holds a newline") }
        let (done, rx) = tokio::sync::oneshot::channel();
        self.commits.send(Commit { events: vec![], raw: lines, done }).map_err(|_| anyhow::anyhow!("hub is stopping"))?;
        rx.await?
    }
    async fn write_raw(hub: &Arc<Hub>, c: Commit) {
        let (lines, done) = (c.raw, c.done);
        let store = hub.store.clone(); let owned = lines.clone();
        let wrote = tokio::task::spawn_blocking(move || store.append_many(&owned.iter().map(String::as_str).collect::<Vec<_>>())).await;
        match wrote {
            Ok(Ok(first_idx)) => {
                { let mut m = hub.model.lock().await; for (i, l) in lines.iter().enumerate() { m.apply_line((first_idx + i + 1) as u32, l); } }
                let all: Vec<Value> = lines.iter().filter_map(|l| serde_json::from_str::<Value>(&crate::store::clean(l)).ok()).collect();
                if hub.need_resync.swap(false, Relaxed) { if let Err(e) = hub.fold_log().await { eprintln!("presenter resync failed: {e:#}"); hub.need_resync.store(true, Relaxed); } }
                else { for ch in all.chunks(200) { let _ = hub.presenter.fold(&json!({"t":"changes","items":ch,"now":now_ms()})).await; } }
                let n = hub.store.len() as u64; hub.signal_seq.store(n, Relaxed); let _ = hub.changed.send(n);
                let _ = done.send(Ok((first_idx + lines.len()) as u64));
            }
            Ok(Err(e)) => { let _ = done.send(Err(anyhow::anyhow!("{e:#}"))); }
            Err(e) => { let _ = done.send(Err(anyhow::anyhow!("{e}"))); }
        }
    }

    async fn writer(hub: Arc<Hub>, mut rx: tokio::sync::mpsc::UnboundedReceiver<Commit>) {
        let mut held: Option<Commit> = None;
        loop {
            let first = match held.take() { Some(c) => c, None => match rx.recv().await { Some(c) => c, None => break } };
            // an import's lines go alone, in their place in the queue
            if !first.raw.is_empty() { Hub::write_raw(&hub, first).await; continue }
            let mut batch = vec![first];
            while let Ok(c) = rx.try_recv() { if !c.raw.is_empty() { held = Some(c); break } batch.push(c); }
            // the policies get the batch before it is written: their events join the same write and fsync (Shelve.auto.fix)
            let transforms: Vec<Arc<dyn Transform>> = hub.transforms.read().unwrap().clone();
            let base = hub.store.len() as u64 + 1; // the 1-based line of the batch's first event (only this task appends)
            if !transforms.is_empty() {
                let mut seq = base;
                for c in batch.iter_mut() {
                    for t in &transforms {
                        let evs = std::mem::take(&mut c.events);
                        let kept = evs.clone();
                        c.events = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| t.before_commit(seq, evs))).unwrap_or(kept);
                    }
                    seq += c.events.len() as u64;
                }
            }
            // `_end` is a policy-side end marker (agents lane): it never reaches the log or a client, whether a transform ran, panicked or none is registered
            // shift_remove keeps the field order of the log line
            for c in batch.iter_mut() { for e in c.events.iter_mut() { if let Some(o) = e.as_object_mut() { o.shift_remove("_end"); } } }
            let lines: Vec<String> = batch.iter().flat_map(|c| c.events.iter().map(|e| serde_json::to_string(e).unwrap_or_default())).collect();
            let store = hub.store.clone(); let owned = lines.clone();
            let wrote = tokio::task::spawn_blocking(move || store.append_many(&owned.iter().map(String::as_str).collect::<Vec<_>>())).await;
            match wrote {
                Ok(Ok(first_idx)) => {
                    { let mut m = hub.model.lock().await; for (i, l) in lines.iter().enumerate() { m.apply_line((first_idx + i + 1) as u32, l); } }
                    let all: Vec<Value> = batch.iter().flat_map(|c| c.events.iter().cloned()).collect();
                    { let obs: Vec<Arc<dyn Observer>> = hub.observers.read().unwrap().clone(); for o in obs { let first = (first_idx + 1) as u32; let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| o.committed(&hub, &all, first))); } }
                    // after a presenter restart the whole log (this batch included) is folded instead of the batch
                    if hub.need_resync.swap(false, Relaxed) { if let Err(e) = hub.fold_log().await { eprintln!("presenter resync failed: {e:#}"); hub.need_resync.store(true, Relaxed); } }
                    else { let _ = hub.presenter.fold(&json!({"t":"changes","items":all,"now":now_ms()})).await; }
                    let n = hub.store.len() as u64; if all.iter().any(signal_affects) { hub.signal_seq.store(n, Relaxed); } let _ = hub.changed.send(n);
                    let mut upto = first_idx;
                    for c in batch { upto += c.events.len(); let _ = c.done.send(Ok(upto as u64)); }
                }
                Ok(Err(e)) => { for t in &transforms { t.commit_failed(base) } let m = format!("{e:#}"); for c in batch { let _ = c.done.send(Err(anyhow::anyhow!(m.clone()))); } }
                Err(e) => { for t in &transforms { t.commit_failed(base) } let m = format!("{e}"); for c in batch { let _ = c.done.send(Err(anyhow::anyhow!(m.clone()))); } }
            }
        }
    }

    // ---- the published interface (see api.rs, docs/HUB-API.md) ----
    /// Later registrations win over earlier ones for the same method.
    pub fn register_rpc(&self, h: Arc<dyn RpcHandler>) { let mut v = self.rpc.write().unwrap(); for m in h.methods() { v.retain(|(n, _)| n != m); v.push((m, h.clone())) } }
    /// Runs before every batch is written (api.rs `Transform`), after the secret mask, in registration order. Once this returns, no batch
    /// is written without passing it: read `store.len()` AFTER registering to know where your replay ends.
    pub fn register_transform(&self, t: Arc<dyn Transform>) { self.transforms.write().unwrap().push(t) }
    /// Sees every committed batch (api.rs `Observer`).
    pub fn register_observer(&self, o: Arc<dyn Observer>) { self.observers.write().unwrap().push(o) }
    /// Runs `t` every `every_s` seconds from now; what it returns is committed (api.rs `Ticker`). Call inside the runtime.
    pub fn register_ticker(&self, t: Arc<dyn Ticker>) {
        let hub = self.arc();
        tokio::spawn(async move {
            let mut iv = tokio::time::interval(std::time::Duration::from_secs(t.every_s().max(1)));
            iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            iv.tick().await;
            loop {
                iv.tick().await;
                // a panicking tick must not end the job: it is skipped, the next one runs
                let evs = futures_util::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(t.tick(&hub, now_ms() / 1000))).await.unwrap_or_default();
                if !evs.is_empty() { let _ = hub.commit_many(&evs).await; }
            }
        });
    }
    /// Installs the mirrored threads' page source (net lane). Pages of a `<link>~<id>` thread come from it.
    pub fn set_far_router(&self, r: Arc<dyn crate::api::FarRouter>) { let _ = self.far_router.set(r); }
    pub fn set_far_mcp(&self, m: Arc<dyn crate::api::FarMcp>) { let _ = self.far_mcp.set(m); }
    pub fn far_mcp(&self) -> Option<Arc<dyn crate::api::FarMcp>> { self.far_mcp.get().cloned() }
    /// A tool call for the thread `from` on the machine `link` (net lane): its answer, or why it could not be asked.
    pub async fn far_call(&self, link: &str, from: &str, tool: &str, args: &Value) -> Result<(bool, Value), String> {
        let Some(r) = self.far_router.get().cloned() else { return Err("no machine is linked".into()) };
        r.mcp(&self.arc(), link, from, tool, args).await
    }
    /// The machines this hub may act on: (link id, name, address)
    /// A thing every other machine of the owner's should hear (see `FarRouter::announce`).
    pub async fn far_announce(self: &Arc<Self>, m: &str, p: &Value) { if let Some(r) = self.far_router.get().cloned() { r.announce(self, m, p).await } }
    pub fn far_machines(&self) -> Vec<(String, String, String)> { self.far_router.get().map(|r| r.machines()).unwrap_or_default() }
    /// The display-only states of the linked machines' mirrors (the sidebar); never the policy state.
    pub fn mirrors(&self) -> &crate::mirror::Mirrors { &self.mirrors }
    pub fn set_far(&self, f: Arc<dyn FarView>) { let _ = self.far.set(f); }
    pub fn far(&self) -> Option<Arc<dyn FarView>> { self.far.get().cloned() }
    /// Mirrored events (already renamed to `<link>~<id>` ids) to the presenter's state only: never the log, never `Model`, never a client's
    /// log stream. That state lives only in the presenter: a presenter restart loses the mirrors, so the caller re-feeds them. The presenter shows them in the sidebar and thread views as it shows local ones.
    pub async fn fold_mirror(&self, link: &str, events: &[Value]) { let _g = self.mirror_gate.lock().await; self.fold_mirror_held(link, events).await }
    /// (the caller holds `mirror_gate`)
    pub async fn fold_mirror_held(&self, link: &str, events: &[Value]) { self.mirrors.fold(link, events); let _ = self.presenter.fold(&json!({"t":"changes","items":events,"now":now_ms()})).await; }
    pub fn register_view(&self, h: Arc<dyn ViewMethod>) { let mut v = self.views.write().unwrap(); for m in h.methods() { v.push((m, h.clone())) } }
    pub fn view_for(&self, method: &str) -> Option<Arc<dyn ViewMethod>> { self.views.read().unwrap().iter().find(|(n, _)| *n == method).map(|(_, h)| h.clone()) }
    pub fn register_http(&self, h: Arc<dyn HttpHandler>) { let mut v = self.http.write().unwrap(); for r in h.routes() { v.retain(|(n, _)| n != r); v.push((r, h.clone())) } }
    pub fn rpc_for(&self, method: &str) -> Option<Arc<dyn RpcHandler>> { self.rpc.read().unwrap().iter().find(|(n, _)| *n == method).map(|(_, h)| h.clone()) }
    pub fn http_for(&self, route: &str) -> Option<Arc<dyn HttpHandler>> { self.http.read().unwrap().iter().filter(|(p, _)| route == *p || route.starts_with(&format!("{p}/"))).max_by_key(|(p, _)| p.len()).map(|(_, h)| h.clone()) }
    pub fn arc(&self) -> Arc<Hub> { self.me.get().and_then(|w| w.upgrade()).expect("hub is alive") }
    /// Send an unsolicited frame (a plot, a notice) to one connection; false when it has gone.
    pub fn push(&self, client: u64, frame: Value) -> bool { let n = frame_bytes(&frame); self.enqueue(client, Out::Value(frame), n, false) }
    /// A frame that may be skipped: false (and nothing queued) when the client's outbox is more than half full or it has gone. `bytes` is its size.
    pub fn push_budgeted(&self, client: u64, frame: Value, bytes: usize) -> bool { self.enqueue(client, Out::Value(frame), bytes, true) }
    /// A droppable frame to every connection that has room (terminal output...): skipped, never dropping a client. Returns how many took it.
    pub fn push_all_budgeted(&self, frame: &Value, bytes: usize) -> usize { let ids = self.clients(); ids.into_iter().filter(|id| self.enqueue(*id, Out::Value(frame.clone()), bytes, true)).count() }
    /// The ids of the connections with an outbox now.
    pub fn clients(&self) -> Vec<u64> { self.outboxes.lock().unwrap().keys().copied().collect() }
    /// Bytes queued for the client and not yet read by its connection (0 when it has gone).
    pub fn outbox_pending(&self, client: u64) -> usize { self.outboxes.lock().unwrap().get(&client).map_or(0, |s| s.pending.load(Relaxed)) }
    /// Tests: the byte bound.
    pub fn set_outbox_max(&self, n: usize) { self.outbox_max.store(n, Relaxed) }
    fn enqueue(&self, client: u64, out: Out, bytes: usize, droppable: bool) -> bool {
        let max = self.outbox_max.load(Relaxed); let cap = if droppable { max / 2 } else { max };
        let mut m = self.outboxes.lock().unwrap();
        let Some(s) = m.get(&client) else { return false };
        // one frame larger than the cap still goes to an empty outbox (a whole model on open): memory stays at the cap plus one frame
        let held = s.pending.load(Relaxed);
        if held > 0 && held + bytes > cap {
            // a frame that must arrive cannot: the client is too slow. Drop it; it reconnects and resumes from its cursor.
            if !droppable { m.remove(&client); }
            return false
        }
        s.pending.fetch_add(bytes, Relaxed);
        s.tx.send((out, bytes)).is_ok()
    }
    /// Bytes as one binary WebSocket frame, not re-encoded (plot frames). False when the connection has gone.
    pub fn push_raw(&self, client: u64, bytes: Vec<u8>) -> bool { let n = bytes.len(); self.enqueue(client, Out::Raw(bytes), n, false) }
    /// ... to every connection.
    pub fn push_all(&self, frame: &Value) { let ids: Vec<u64> = self.outboxes.lock().unwrap().keys().copied().collect(); let n = frame_bytes(frame); for id in ids { self.enqueue(id, Out::Value(frame.clone()), n, false); } }
    pub fn attach(&self, client: u64) -> Outbox { let (tx, rx) = tokio::sync::mpsc::unbounded_channel(); let pending = Arc::new(std::sync::atomic::AtomicUsize::new(0)); self.outboxes.lock().unwrap().insert(client, Slot { tx, pending: pending.clone() }); Outbox { rx, pending } }
    /// True while the connection's outbox exists (it has not gone away).
    pub fn is_attached(&self, client: u64) -> bool { self.outboxes.lock().unwrap().contains_key(&client) }
    pub fn detach(&self, client: u64) { crate::settings::accounts_hub::revoke_client(client); self.outboxes.lock().unwrap().remove(&client); if let Some(p) = self.pairing.get() { p.forget(client) } crate::session::forget(client); }
    /// Installs the pairing registry and the /auth routes (net lane). Call it BEFORE the listener accepts: until then, devices-v1.json read once at start
    /// decides and no /auth route exists. Afterwards the registry decides every request.
    pub fn set_pairing(&self, p: Arc<dyn Pairing>, r: Arc<dyn AuthRoute>) { self.auth.set_pairing(p.clone()); let _ = self.pairing.set(p); let _ = self.auth_route.set(r); }
    pub fn auth_route(&self) -> Option<Arc<dyn AuthRoute>> { self.auth_route.get().cloned() }
    /// A connection id for an HTTP request (the /auth routes remember it).
    pub fn fresh_client(&self) -> u64 { NEXT_CLIENT.fetch_add(1, Relaxed) }
    /// A /ws connection presenting `token` may proceed: its credential resolves and its device is not revoked.
    /// `admit` off the async threads (the registry fsyncs now and then).
    pub async fn admit_blocking(&self, client: u64, token: String) -> bool {
        match self.pairing.get().cloned() { None => true, Some(p) => tokio::task::spawn_blocking(move || !token.is_empty() && p.resolve(&token, client, "/ws") == token && p.active(client)).await.unwrap_or(false) }
    }
    pub fn admit(&self, client: u64, token: &str) -> bool {
        match self.pairing.get() { None => true, // the snapshot path decided in `serve` (Auth::allowed)
            // pairing.js resolve answers the credential that holds for this connection (the hub token for an empty one): it holds only if it is the one presented
            Some(p) => !token.is_empty() && p.resolve(token, client, "/ws") == token && p.active(client) }
    }
    /// Ends the connections a revoke named.
    pub fn close_clients(&self, ids: &[u64]) { let o = self.outboxes.lock().unwrap(); for id in ids { if let Some(t) = o.get(id) { let _ = t.tx.send((Out::Close, 0)); } } }
    /// Read access to the model: threads, projects, entry indexes, pages.
    pub async fn model(&self) -> tokio::sync::MutexGuard<'_, Model> { self.model.lock().await }
    /// A setting's current value (None when it has none or was removed); read from the log line the model points at.
    pub async fn setting(&self, key: &str) -> Option<String> {
        let seq = self.model.lock().await.setting_seq(key)?;
        let v: Value = serde_json::from_str(&self.store.get(seq as usize - 1).ok()?).ok()?;
        v["value"].as_str().map(str::to_string)
    }
    /// A receiver that hears the log's new length after every commit.
    pub fn subscribe(&self) -> broadcast::Receiver<u64> { self.changed.subscribe() }
    /// Streamed text of a running turn, not yet an event: shown to views as the thread's live reply.
    pub async fn live_delta(&self, thread: &str, text: &str) -> Result<()> {
        let masked = crate::settings::mask::mask_cow(text); let text: &str = &masked; // settings lane: Hub.tagged (a secret split across two chunks is a known limit, settings/mask.rs)
        self.live.lock().unwrap().entry(thread.to_string()).or_default().push_str(text);
        let _ = self.frames.send(json!({"t":"delta","thread":thread,"text":text}));
        self.presenter.fold(&json!({"t":"delta","thread":thread,"text":text,"now":now_ms()})).await?;
        let _ = self.changed.send(self.store.len() as u64);
        Ok(())
    }
    /// The stream for `thread` ended (its MessagePosted follows or has been committed).
    pub async fn live_end(&self, thread: &str) -> Result<()> {
        self.live.lock().unwrap().remove(thread);
        let _ = self.frames.send(json!({"t":"delta_end","thread":thread}));
        self.presenter.fold(&json!({"t":"delta_end","thread":thread,"now":now_ms()})).await?;
        let _ = self.changed.send(self.store.len() as u64);
        Ok(())
    }
    /// The live text of every streaming thread (for a client that joins mid-turn).
    pub fn live_snapshot(&self) -> Vec<(String, String)> { self.live.lock().unwrap().iter().map(|(a, b)| (a.clone(), b.clone())).collect() }
    /// The `live` frame a joining client gets: every streaming thread's text so far.
    pub fn live_frame(&self) -> Value {
        let mut v = self.live_snapshot(); v.sort();
        json!({"t":"live","items":v.into_iter().map(|(t, x)| json!({"thread":t,"text":x,"trunc":false,"gen":0})).collect::<Vec<_>>()})
    }
    /// A stored log line as ANY client receives it, join or live: a device's registration and drop
    /// travel as `{"$","at"}` only (the push token, platform and bundle never leave the hub: laws device_token_never_sent,
    /// device_drop_never_sent), and a turn's input and riding mail (`launch.input.`, `fallback.input.`, `ride.`) travel blank
    /// (Setting.wire_blank). The log and the presenter keep the whole line.
    pub fn wire_item(line: &str) -> Option<Value> {
        let v: Value = serde_json::from_str(line).ok()?;
        let at = v["at"].as_u64().unwrap_or(0);
        match v["$"].as_str() {
            Some(t @ ("DeviceRegistered" | "DeviceDropped" | "DevicePrefs")) => Some(json!({"$": t, "at": at})),
            Some("SettingSet") if v["key"].as_str().map_or(false, wire_blank) => Some(json!({"$":"SettingSet","key":v["key"],"value":"","at":at})),
            _ => Some(v),
        }
    }
    pub fn info_frame(&self) -> Value { let mut i = serde_json::Map::new(); i.insert("pair".into(), json!("")); i.insert("pair.qr".into(), json!("")); for (k, v) in self.extra_info.lock().unwrap().iter() { i.insert(k.clone(), json!(v)); } json!({"t":"info","info":Value::Object(i)}) }
    pub fn log_info(&self) -> Value { self.info() }

    /// The revision a view notice carries: the log's length, plus one for each change the log does not hold (a watched folder changed on disk).
    pub async fn revision(&self) -> u64 { self.store.len() as u64 + 1 + self.view_bump.load(Relaxed) }
    /// Something a view shows changed outside the log: every connection with a view is told (`view.changed`) and asks again.
    pub fn bump_views(&self) { self.view_bump.fetch_add(1, Relaxed); let _ = self.changed.send(self.store.len() as u64); }

    pub fn connect(&self, cbor: bool) -> (Conn, Value) {
        let gen = self.gen.fetch_add(1, Relaxed).to_string();
        let g = json!({"t":"view","version":1,"renderer":"mobile","generation":gen,"epoch":self.epoch,"limits":limits()});
        (Conn { sent: vec![], client: NEXT_CLIENT.fetch_add(1, Relaxed), gen, view_id: String::new(), target: Value::Null, cbor, lastrev: 0 }, g)
    }

    pub fn notice(&self, c: &Conn, rev: u64) -> Value {
        json!({"t":"view.changed","version":1,"generation":c.gen,"epoch":self.epoch,"viewId":c.view_id,"target":c.target,"viewRevision":rev.to_string()})
    }

    /// One inbound request -> the frames to send back, in order.
    pub async fn request(&self, c: &mut Conn, msg: &Value) -> Vec<Value> {
        // a view session that names the accounts wire version may manage accounts (settings/accounts_hub.rs `note_request`)
        crate::settings::accounts_hub::note_request(&c.gen, c.client, !c.view_id.is_empty() || msg.get("m").and_then(Value::as_str).map_or(false, |m| m.starts_with("view.")), msg);
        let out = self.request_one(c, msg).await;
        // the Files tab and its kin are reads of the hub for a hub-drawn view: the policy applies the reply (Fm.reply, Pdf.got, Mech.after ...) on the view's next request
        let method = msg.get("m").and_then(Value::as_str).unwrap_or("");
        // the intent written: the reply brings the composer back (pages::apply_reads), on the very next view request
        if method == "fab.intent" && !c.view_id.is_empty() { if let Some(r) = out.first().filter(|r| r["t"] == "reply") { crate::session::record_reply_bound(c.client, method, r, &c.view_id, &c.target); } }
        if matches!(method, "files.dir" | "pdf.page" | "mech.list" | "mech.render" | "boards.list" | "history.refs") && !c.view_id.is_empty() {
            if let Some(r) = out.first().filter(|r| r["t"] == "reply") { crate::session::record_reply(c.client, method, r); }
        }
        // Diff and Review are the same kind of read (U.Ui.recv takes git.diff's reply by its `diff` field and a review page's by its @rp.req id), but the policy
        // helper's list of read kinds predates them: it drops any other kind, so the reply is handed over under one it takes.
        // Without this the Diff panel never opened and Review sat on "Working…".
        // (git.act and git.revert too: a Diff on show is asked again after a commit or a revert; `method` names what the reply answers, for the Rust presenter)
        if matches!(method, "git.diff" | "git.status" | "git.act" | "git.revert" | "review.bom" | "review.checks" | "review.tick" | "review.release" | "bots.routine") && !c.view_id.is_empty() {
            if let Some(r) = out.first().filter(|r| r["t"] == "reply") { crate::session::record_reply_bound_m(c.client, "history.refs", method, r, &c.view_id, &c.target); }
        }
        // the Settings pages' status reads (secret box, API keys/voice): the policy applies the hub's own answer to the view's Ui (settings::read_of)
        if !c.view_id.is_empty() { if let (Some(m), Some(id), Some(f)) = (msg.get("m").and_then(Value::as_str), msg.get("id").and_then(Value::as_u64), out.first()) { if let Some((kind, ok, body)) = crate::settings::read_of(m, f) { crate::session::record_read(c.client, &c.view_id, kind, id, ok, body); } } }
        // the view actor folds the answer of a request its own commands sent (session/actor.rs: @rp.req, @telemetry.req)
        if !msg.get("m").and_then(Value::as_str).unwrap_or("").starts_with("view.") { if let Some(id) = msg.get("id").and_then(Value::as_u64) { let m = msg.get("m").and_then(Value::as_str).unwrap_or(""); if crate::session::is_job(m) { crate::session::replied_late(c.client, id) } else { crate::session::replied(c.client, id) } } }
        out
    }
    async fn request_one(&self, c: &mut Conn, msg: &Value) -> Vec<Value> { self.request_one_r(c, msg, true).await }
    /// A request run on this hub whatever its params name: the owner's side of /far/rpc runs a linked machine's request here, and must never route it onward
    ///.
    pub async fn request_here(&self, c: &mut Conn, msg: &Value) -> Vec<Value> { self.request_one_r(c, msg, false).await }
    async fn request_one_r(&self, c: &mut Conn, msg: &Value, routed: bool) -> Vec<Value> {
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let m = msg.get("m").and_then(Value::as_str).unwrap_or("");
        // `ping` and `load.ping` are not methods of hub: it answers them "unknown method" (recorded, parity/recA)
        // a copy of a log: git that writes and acting on the machine are told, not run (copy.rs)
        if crate::copy::rpc_refused(m) { return vec![json!({"t":"reply","id":id,"ok":false,"error":crate::copy::GIT_TOLD})] }
        // a request naming a thing on a linked machine goes there (net lane: Far.rpc.go)
        if routed { if let Some(r) = self.far_router.get().cloned() { let p = msg.get("p").unwrap_or(&Value::Null); if r.wants_for(c.client, m, p) { return r.call(&self.arc(), c.client, &id, m, p).await } } }
        if m == "fab.look" {
            let reply = crate::settings::fab::reply(&self.arc(), &id, "fab.look", msg.get("p").unwrap_or(&Value::Null)).await; // settings/fab.rs: the project card
            // the hub's own read, applied to this view's held state by the policy on its next V (Fab.reply)
            // bound to the view that asked, so the one view update the window asks for after this answer carries it (Buy it in the header)
            if !c.view_id.is_empty() { crate::session::record_read_bound(c.client, &c.view_id, &c.target, "fab.look", id.as_u64().unwrap_or(0), reply["ok"].as_bool().unwrap_or(true), reply["result"].clone()); }
            return vec![reply];
        }
        if let Some(handler) = self.rpc_for(m) {
            let me = self.arc();
            return handler.call(&me, &Caller { gen: c.gen.clone(), client: c.client }, &id, m, msg.get("p").unwrap_or(&Value::Null)).await;
        }
        if m == "turn.start" { return self.turn_start(&id, msg.get("p").unwrap_or(&Value::Null)).await }
        if !m.starts_with("view.") {
            return vec![json!({"t":"reply","id":id,"ok":false,"error":"unknown method"})];
        }
        self.view_request(c, id, m, msg).await
    }

    /// fab.look: the card for a thread with no goal, intent or fab files.
    fn fab_look(&self, id: &Value, p: &Value) -> Value {
        json!({"t":"reply","id":id,"ok":true,"result":{"thread":p.get("thread").cloned().unwrap_or(json!("")),"intent":"","goal":"","parts":[],"boards":0,"label":"","estimate_problems":"","error":"","estimates":true,"board_files":[],"board_default":"","order":""}})
    }


    /// turn.start: the person's message is committed (durable) and queued, then acknowledged. The
    /// events are the JS hub's first two for it (MessagePosted, TurnQueued); the agent turn is M5.
    async fn turn_start(&self, id: &Value, p: &Value) -> Vec<Value> {
        let (thread, text) = (p.get("thread").and_then(Value::as_str).unwrap_or(""), p.get("text").and_then(Value::as_str).unwrap_or(""));
        if thread.is_empty() || !self.model.lock().await.threads.contains_key(thread) { return vec![json!({"t":"reply","id":id,"ok":false,"error":"unknown thread"})] }
        let at = now_ms() / 1000;
        let msg = p.get("msg").and_then(Value::as_str).filter(|m| !m.is_empty()).map(str::to_string).unwrap_or_else(|| format!("m{at}-{}", id.as_u64().unwrap_or(0)));
        // `agentText`: what the agent is told when it differs from what the person sees (the phone data API adds the paths of attached images); `attachments`: their metadata, kept on the message
        let agent_text = p.get("agentText").and_then(Value::as_str).filter(|t| !t.is_empty()).unwrap_or(text);
        let mut posted = json!({"$":"MessagePosted","thread":thread,"msg":msg,"role":"user","text":text,"at":at});
        if let Some(a) = p.get("attachments").filter(|a| a.as_array().map_or(false, |x| !x.is_empty())) { posted["attachments"] = a.clone(); }
        let events = [posted, json!({"$":"TurnQueued","thread":thread,"msg":msg,"text":agent_text,"at":at})];
        if self.commit_many(&events).await.is_err() { return vec![json!({"t":"reply","id":id,"ok":false,"error":"store write failed"})] }
        vec![json!({"t":"reply","id":id,"ok":true})]
    }


}

#[cfg(test)]
mod tests {
    use super::*; use crate::presenter::Canned; use crate::session::*;
    fn cfg(home: &std::path::Path) -> Cfg { Cfg { home: home.to_path_buf(), version: "0.14.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() } }
    fn home(name: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-hub-{}-{}", std::process::id(), name)); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    fn req(c: &Conn, hub: &Hub, m: &str, id: u64, extra: Value) -> Value {
        let mut p = json!({"version":1,"hub":"http://x","generation":c.gen,"epoch":hub.epoch,"requestRevision":1,"target":{"kind":"recent","id":""},"viewId":c.view_id,"session":{}});
        for (k, v) in extra.as_object().into_iter().flatten() { p[k] = v.clone() }
        json!({"id":id,"m":m,"p":p})
    }
    // what the presenter is told of the hub: the native sidebar's key, the new view's id, the request's revision (policy: Ns.prepare)
    #[tokio::test] async fn the_presenter_is_given_the_native_key_the_view_id_and_the_request_revision() {
        struct Spy(std::sync::Mutex<Vec<Value>>);
        impl Presenter for Spy {
            fn name(&self) -> &'static str { "spy" }
            fn wants_log(&self) -> bool { false }
            fn fold<'a>(&'a self, _f: &'a Value) -> crate::presenter::Fut<'a, ()> { Box::pin(async { Ok(()) }) }
            fn view<'a>(&'a self, p: &'a Value) -> crate::presenter::Fut<'a, Value> { self.0.lock().unwrap().push(p.clone()); Box::pin(async { Ok(json!({"remoteReady":true,"shown":{},"session":{},"cmds":[]})) }) }
            fn ping(&self) -> crate::presenter::Fut<'_, ()> { Box::pin(async { Ok(()) }) }
        }
        let spy = Arc::new(Spy(Default::default()));
        let h = Hub::start(cfg(&home("spy")), spy.clone()).await.unwrap();
        let (mut c, _g) = h.connect(true);
        let r0 = req(&c, &h, "view.open", (1 << 47) + 5, json!({"nativeSidebar":1})); let _ = h.request(&mut c, &r0).await;
        let r1 = req(&c, &h, "view.update", (1 << 47) + 6, json!({"nativeSidebar":1, "viewId": format!("{}:{}", c.gen, (1u64 << 47) + 5)})); let _ = h.request(&mut c, &r1).await;
        // activity lines and assistant/system text leave the revision alone (SignalRevision.affects); a user message, a setting or an unknown event move it
        let vid = format!("{}:{}", c.gen, (1u64 << 47) + 5);
        h.commit_many(&[json!({"$":"ActivityLogged","thread":"t1","act":"a1","tone":"tool","kind":"Bash","summary":"ls","at":5}),
            json!({"$":"MessagePosted","thread":"t1","msg":"m9","role":"assistant","text":"streamed","at":6}),
            json!({"$":"MessagePosted","thread":"t1","msg":"m10","role":"system","text":"note","at":6})]).await.unwrap();
        let r2 = req(&c, &h, "view.update", (1 << 47) + 7, json!({"nativeSidebar":1, "viewId": vid.clone()})); let _ = h.request(&mut c, &r2).await;
        h.commit_many(&[json!({"$":"MessagePosted","thread":"t1","msg":"m11","role":"user","text":"hi","at":7})]).await.unwrap();
        let r3 = req(&c, &h, "view.update", (1 << 47) + 8, json!({"nativeSidebar":1, "viewId": vid.clone()})); let _ = h.request(&mut c, &r3).await;
        h.commit_many(&[json!({"$":"SomethingNew","at":8})]).await.unwrap();
        let r4 = req(&c, &h, "view.update", (1 << 47) + 9, json!({"nativeSidebar":1, "viewId": vid})); let _ = h.request(&mut c, &r4).await;
        let all = spy.0.lock().unwrap().clone(); assert_eq!(all.len(), 5);
        assert_eq!(all[2]["revision"], all[0]["revision"], "activity and assistant/system text do not move the revision");
        assert!(all[3]["revision"].as_u64() > all[2]["revision"].as_u64(), "a user message moves it");
        assert!(all[4]["revision"].as_u64() > all[3]["revision"].as_u64(), "an unknown event moves it");
        let seen = all[..2].to_vec(); assert_eq!(seen.len(), 2);
        for p in &seen {
            let k = p["nativeKey"].as_str().unwrap_or(""); assert!(k.len() == 32 && k.bytes().all(|b| b.is_ascii_hexdigit()), "nativeKey: {k}");
            assert_eq!(p["nativeKey"], seen[0]["nativeKey"], "one key for the hub's run");
            assert!(p["revision"].as_u64().unwrap_or(0) > 0, "the hub revision"); assert_eq!(p["revision"], seen[0]["revision"], "the same while the log is unchanged: a page position survives later requests");
            assert_eq!(p["viewId"], format!("{}:{}", c.gen, (1u64 << 47) + 5), "the open view's id");
            assert_eq!(p["nativeSidebar"], 1, "the client's offer passes through");
        }
    }

    // Alerts: a desk not watching the thread is told about a need, and once the need is answered it is told to close that notification
    #[tokio::test] async fn an_answered_need_clears_the_desktop_notification_it_raised() {
        let h = Hub::start(cfg(&home("needclear")), Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let desks = crate::settings::alerts::start(&h);
        let mut o = h.attach(3); desks.say(3, "t-other", false, now_ms() / 1000);
        async fn frame(o: &mut Outbox) -> Value { loop { match tokio::time::timeout(std::time::Duration::from_secs(5), o.recv()).await { Ok(Some(Out::Value(v))) if v["t"].as_str().map_or(false, |t| t.starts_with("notify")) => return v, Ok(Some(_)) => continue, _ => return Value::Null } } }
        h.commit(&json!({"$":"AskOpened","id":"q1","thread":"t1","kind":"approval","tool":"order_quote","detail":"Send 1 file?","options":"","at":1})).await.unwrap();
        h.commit(&json!({"$":"SettingSet","key":"alert.t1","value":"ask\task-t1\t\t1.1\tSend 1 file?","at":1})).await.unwrap();
        let shown = frame(&mut o).await; assert_eq!((shown["t"].as_str(), shown["key"].as_str()), (Some("notify"), Some("ask-t1")), "{shown}");
        h.commit(&json!({"$":"AskClosed","id":"q1","answer":"accept","at":2})).await.unwrap();
        let cleared = frame(&mut o).await; assert_eq!(cleared, json!({"t":"notify.clear","thread":"t1","key":"ask-t1"}), "the answered need's notification goes");
    }

    // Hub::set_info: stored for joiners, pushed to every client, empty removes, bounded
    #[tokio::test] async fn set_info_is_stored_pushed_removed_and_bounded() {
        let h = Hub::start(cfg(&home("setinfo")), Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let mut o = h.attach(1);
        assert!(h.set_info("term.demo", "state-1").await);
        assert!(matches!(o.recv().await, Some(Out::Value(v)) if v["t"] == "info" && v["info"]["term.demo"] == "state-1"));
        assert_eq!(h.log_info()["term.demo"], "state-1", "joiners get it");
        assert_eq!(h.info_frame()["info"]["term.demo"], "state-1");
        assert!(h.set_info("term.demo", "state-1").await); assert!(o.try_recv().is_err(), "no change, no frame");
        assert!(h.set_info("term.demo", "").await);
        assert!(matches!(o.recv().await, Some(Out::Value(v)) if v["info"]["term.demo"] == ""), "removal is told as an empty value");
        assert!(h.log_info().get("term.demo").is_none());
        for i in 0..256 { assert!(h.set_info(&format!("k{i}"), "v").await) }
        assert!(!h.set_info("one-too-many", "v").await, "the map is bounded");
        assert!(h.set_info("k0", "changed").await, "an existing key can still change");
        assert!(!h.set_info("big", &"x".repeat(70_000)).await && !h.set_info(&"k".repeat(200), "v").await);
    }

    const TOKEN: &str = "apns-token-SECRET-0123456789abcdef";
    #[test] fn device_prefs_never_send_the_token_and_the_push_book_travels_blank() {
        let w = Hub::wire_item(&format!("{{\"$\":\"DevicePrefs\",\"token\":\"{TOKEN}\",\"mute\":true,\"at\":42}}")).unwrap();
        assert_eq!(w, json!({"$":"DevicePrefs","at":42}));
        assert_eq!(Hub::wire_item(r#"{"$":"SettingSet","key":"push.book","value":"{\"open\":[]}","at":1}"#).unwrap()["value"], "");
    }
    // laws device_token_never_sent, device_drop_never_sent, input_wire_blank, ride_wire_blank
    #[test] fn device_token_never_sent() {
        let line = format!("{{\"$\":\"DeviceRegistered\",\"id\":\"d1\",\"token\":\"{TOKEN}\",\"platform\":\"ios\",\"bundle\":\"dev.backplane.mobile\",\"name\":\"Phone\",\"at\":42}}");
        let w = Hub::wire_item(&line).unwrap();
        assert_eq!(w, json!({"$":"DeviceRegistered","at":42}));
        assert!(!w.to_string().contains(TOKEN) && !w.to_string().contains("ios") && !w.to_string().contains("dev.backplane"));
    }
    #[test] fn device_drop_never_sent() {
        let w = Hub::wire_item(&format!("{{\"$\":\"DeviceDropped\",\"id\":\"d1\",\"token\":\"{TOKEN}\",\"at\":43}}")).unwrap();
        assert_eq!(w, json!({"$":"DeviceDropped","at":43}));
        assert!(!w.to_string().contains(TOKEN));
    }
    #[test] fn input_wire_blank() {
        for k in ["launch.input.t1", "fallback.input.t1"] {
            let w = Hub::wire_item(&format!("{{\"$\":\"SettingSet\",\"key\":\"{k}\",\"value\":\"a long prompt\",\"at\":5}}")).unwrap();
            assert_eq!(w, json!({"$":"SettingSet","key":k,"value":"","at":5}));
        }
    }
    #[test] fn ride_wire_blank() {
        let w = Hub::wire_item("{\"$\":\"SettingSet\",\"key\":\"ride.t1\",\"value\":\"mail text\",\"at\":6}").unwrap();
        assert_eq!(w["value"], "");
        // everything else is untouched, key order included
        let l = "{\"$\":\"SettingSet\",\"key\":\"theme\",\"value\":\"dark\",\"at\":7}";
        assert_eq!(Hub::wire_item(l).unwrap().to_string(), l);
        assert_eq!(Hub::wire_item("not json"), None);
    }

    // Transform: policy events join the same write, in order, with the right first_seq; a panic leaves the batch; a failed write is told
    #[tokio::test] async fn a_transform_adds_events_to_the_same_batch_and_is_told_of_a_failed_write() {
        struct T(std::sync::Mutex<Vec<u64>>, std::sync::atomic::AtomicBool);
        impl Transform for T {
            fn before_commit(&self, first_seq: u64, events: Vec<Value>) -> Vec<Value> {
                self.0.lock().unwrap().push(first_seq);
                if events.iter().any(|e| e["$"] == "Boom") { panic!("a bad policy") }
                let mut out = events.clone(); if events.iter().any(|e| e["$"] == "MessagePosted") { out.push(json!({"$":"SettingSet","key":"quiet.x","value":"1","at":1})); } out
            }
            fn commit_failed(&self, _s: u64) { self.1.store(true, Relaxed) }
        }
        let dir = home("transform"); let h = Hub::start(cfg(&dir), Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let t = Arc::new(T(Default::default(), Default::default())); h.register_transform(t.clone());
        let n0 = h.store.len() as u64;
        let upto = h.commit_many(&[json!({"$":"MessagePosted","thread":"t1","msg":"m1","role":"user","text":"hi","at":2})]).await.unwrap();
        assert_eq!(upto, n0 + 2, "the message and the policy's event were written together");
        assert_eq!(h.store.get((n0 + 1) as usize).map(|l| l.contains("quiet.x")).unwrap_or(false), true, "in order, right after the message");
        assert_eq!(t.0.lock().unwrap().as_slice(), &[n0 + 1], "first_seq is the 1-based line of the batch's first event");
        let u2 = h.commit_many(&[json!({"$":"Boom"})]).await.unwrap();
        assert_eq!(u2, n0 + 3, "a panicking transform leaves the batch as it came");
        assert!(!t.1.load(Relaxed));
    }

    // the `_end` marker never reaches the log: not with a transform that panics on the batch, not with none registered
    #[tokio::test] async fn the_end_marker_never_reaches_the_log() {
        struct P; impl Transform for P { fn before_commit(&self, _f: u64, _e: Vec<Value>) -> Vec<Value> { panic!("a bad policy") } fn commit_failed(&self, _s: u64) {} }
        let ev = || json!({"$":"TurnChanged","thread":"t","state":"completed","_end":true});
        for with in [false, true] {
            let dir = home(if with { "endmark1" } else { "endmark0" }); let h = Hub::start(cfg(&dir), Arc::new(Canned { reply: json!({}) })).await.unwrap();
            if with { h.register_transform(Arc::new(P)); }
            let n0 = h.store.len();
            h.commit_many(&[ev()]).await.unwrap();
            let line = h.store.get(n0).expect("written");
            assert!(!line.contains("_end"), "transform={with}: {line}");
            assert!(line.contains("TurnChanged"), "{line}");
        }
    }

    // fab.look is a read the hub makes for a view: it is recorded for that view's next presenter call (session::record_read)
    #[tokio::test] async fn fab_look_is_recorded_as_a_read_of_its_view() {
        let h = Hub::start(cfg(&home("fabread")), Arc::new(Canned { reply: json!({"remoteReady":true,"shown":{},"session":{},"cmds":[]}) })).await.unwrap();
        let (mut c, _g) = h.connect(true);
        let r0 = req(&c, &h, "view.open", (1 << 47) + 5, json!({})); let _ = h.request(&mut c, &r0).await;
        let _ = crate::session::take_reads(c.client, &c.view_id, &c.target); // whatever the open itself recorded
        let out = h.request(&mut c, &json!({"id": 77, "m": "fab.look", "p": {"thread": "t1"}})).await;
        assert_eq!(out[0]["ok"], true);
        // bound to the view that asked: the very next view request carries it (the window asks for one update after the answer)
        let reads = crate::session::take_reads(c.client, &c.view_id, &c.target);
        assert_eq!(reads.len(), 1, "{reads:?}");
        assert_eq!(reads[0]["kind"], "fab.look"); assert_eq!(reads[0]["result"]["thread"], "t1");
        assert!(crate::session::take_reads(c.client, &c.view_id, &c.target).is_empty(), "carried once");
    }

    // soak finding: ids built from store.len() before the commit collided under concurrent project.add: id numbers are handed out past any given
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn concurrent_id_allocations_never_overlap() {
        let h = Hub::start(cfg(&home("pids")), Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let mut tasks = vec![];
        for _ in 0..8 { let h = h.clone(); tasks.push(tokio::spawn(async move { let mut v = vec![]; for _ in 0..200 { v.push(h.alloc_n(4)); if v.len() % 50 == 0 { h.commit_many(&[json!({"$":"SettingSet","key":"k","value":"v","at":1})]).await.unwrap(); } } v })); }
        let mut all: Vec<u64> = vec![]; for t in tasks { all.extend(t.await.unwrap()); }
        all.sort(); let n = all.len();
        for w in all.windows(2) { assert!(w[1] >= w[0] + 4, "ranges overlap: {} then {}", w[0], w[1]); }
        assert_eq!(n, 1600);
    }

    #[tokio::test] async fn open_assembles_identity_in_js_order() {
        let h = Hub::start(cfg(&home("open")), Arc::new(Canned { reply: json!({"remoteReady":true,"shown":{"a":1},"session":{"sel":""},"cmds":[]}) })).await.unwrap();
        let (mut c, g) = h.connect(true); assert_eq!(g["renderer"], "mobile");
        let r0 = req(&c, &h, "view.open", (1 << 47) + 5, json!({})); let out = h.request(&mut c, &r0).await;
        let r = &out[0]["result"]; let keys: Vec<&String> = r.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["remoteReady","viewRevision","renderer","viewId","target","requestRevision","epoch","generation","hub","version","mailDraftContext","window","cmds","session","shown"]);
        assert_eq!(r["viewId"], format!("{}:{}", c.gen, (1u64 << 47) + 5));
    }
    #[tokio::test] async fn stale_and_invalid_requests_are_refused() {
        let h = Hub::start(cfg(&home("stale")), Arc::new(Canned { reply: json!({"remoteReady":true,"shown":{},"session":{},"cmds":[]}) })).await.unwrap();
        let (mut c, _) = h.connect(true);
        let mut bad = req(&c, &h, "view.open", (1 << 47) + 1, json!({})); bad["p"]["generation"] = json!("999");
        assert_eq!(h.request(&mut c, &bad).await[0]["error"]["code"], "STALE_CONNECTION");
        let low = req(&c, &h, "view.open", 5, json!({})); assert_eq!(h.request(&mut c, &low).await[0]["error"]["code"], "STALE_CONNECTION");
        let upd = req(&c, &h, "view.update", (1 << 47) + 2, json!({"viewId":"nope"})); assert_eq!(h.request(&mut c, &upd).await[0]["error"]["code"], "STALE_VIEW");
        assert_eq!(h.request(&mut c, &json!({"id":3,"m":"x.y","p":{}})).await[0]["error"], "unknown method");
    }
    #[tokio::test] async fn big_replies_are_segmented_when_asked() {
        let big = "x".repeat(300_000);
        let h = Hub::start(cfg(&home("seg")), Arc::new(Canned { reply: json!({"remoteReady":true,"shown":{"s":1},"session":{"big":big},"cmds":[]}) })).await.unwrap();
        let (mut c, _) = h.connect(true);
        let r0 = req(&c, &h, "view.open", (1 << 47) + 9, json!({"viewControl":1})); let out = h.request(&mut c, &r0).await;
        assert!(out.len() > 2); assert_eq!(out.last().unwrap()["result"]["controlId"], (1u64 << 47) + 9);
        let text: String = out[..out.len() - 1].iter().map(|f| f["text"].as_str().unwrap().to_string()).collect();
        let values: Value = serde_json::from_str(&text).unwrap(); assert!(values.get("shown").is_none()); assert_eq!(values["session"]["big"].as_str().unwrap().len(), 300_000);
        assert_eq!(out[out.len() - 2]["last"], true);
        for f in &out { assert!(serde_json::to_string(f).unwrap().len() <= REPLY_BYTES) }
    }
    #[tokio::test] async fn commit_is_durable_and_counts_revisions() {
        let dir = home("commit"); let h = Hub::start(cfg(&dir), Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let (r0, n0) = (h.revision().await, h.store.len()); // the start-up housekeeping events are already in
        assert!(n0 >= 5, "machine id, zone, accounts... are written once at start");
        h.commit(&json!({"$":"SettingSet","key":"k","value":"v","at":1})).await.unwrap();
        assert_eq!(h.revision().await, r0 + 1); assert_eq!(Store::open(&dir.join("events.jsonl")).unwrap().len(), n0 + 1);
        assert_eq!(h.setting("k").await.as_deref(), Some("v"));
        drop(h); let again = Hub::start(cfg(&dir), Arc::new(Canned { reply: json!({}) })).await.unwrap(); assert_eq!(again.store.len(), n0 + 1, "a restart writes no second set of housekeeping events");
    }
}

#[cfg(test)]
mod write_tests {
    use super::*; use crate::presenter::Canned; use crate::session::*;
    #[tokio::test] async fn turn_start_commits_a_message_and_acks_after_it_is_durable() {
        let dir = std::env::temp_dir().join(format!("bp-hub-ts-{}", std::process::id())); let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("events.jsonl"), "{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"p\",\"root\":\"/x\",\"at\":1}\n{\"$\":\"ThreadCreated\",\"id\":\"t1\",\"project\":\"p1\",\"title\":\"T\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":1}\n").unwrap();
        let cfg = Cfg { home: dir.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() };
        let h = Hub::start(cfg, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let n0 = h.store.len();
        let (mut c, _) = h.connect(true);
        let out = h.request(&mut c, &json!({"id":9,"m":"turn.start","p":{"thread":"t1","text":"hello","msg":"m1"}})).await;
        assert_eq!(out[0]["ok"], true);
        let disk = Store::open(&dir.join("events.jsonl")).unwrap(); assert_eq!(disk.len(), n0 + 2); assert!(disk.get(n0).unwrap().contains("\"MessagePosted\""));
        assert_eq!(h.model.lock().await.entries(), 1);
        let bad = h.request(&mut c, &json!({"id":10,"m":"turn.start","p":{"thread":"nope","text":"x"}})).await; assert_eq!(bad[0]["ok"], false);
    }

    // a client that never reads never grows the hub: past the bound it is dropped (plain push), droppable frames are skipped sooner
    #[tokio::test] async fn a_client_that_never_reads_cannot_grow_the_outbox_past_its_bound() {
        let dir = std::env::temp_dir().join(format!("bp-outbox-{}", std::process::id())); let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
        let hub = Hub::start(Cfg { home: dir.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        hub.set_outbox_max(100_000);
        let _never = hub.attach(1); let _reads = hub.attach(2);
        let frame = json!({"t":"term","data":"x".repeat(1000)}); let n = frame_bytes(&frame);
        // droppable frames stop at half the bound, and the client stays
        let mut budgeted = 0; while hub.push_budgeted(1, frame.clone(), n) { budgeted += 1; assert!(budgeted < 1000) }
        assert!(hub.outbox_pending(1) <= 50_000 && hub.outbox_pending(1) > 40_000, "pending {}", hub.outbox_pending(1));
        assert!(hub.is_attached(1), "skipping a droppable frame does not drop the client");
        // plain pushes go on to the bound, then the client is dropped and nothing more is queued
        let mut pushed = 0; while hub.push(1, frame.clone()) { pushed += 1; assert!(pushed < 1000) }
        assert!(!hub.is_attached(1), "past the bound the client is dropped");
        assert_eq!(hub.outbox_pending(1), 0);
        assert!(!hub.push(1, frame.clone()) && !hub.push_budgeted(1, frame.clone(), n), "nothing is queued for a dropped client");
        // push_all: a reading client keeps working while the stalled one goes
        let mut r = hub.attach(3); for _ in 0..200 { hub.push_all(&frame); if let Ok(Some(_)) = tokio::time::timeout(std::time::Duration::from_millis(1), r.recv()).await {} }
        assert!(hub.outbox_pending(3) <= 100_000, "bounded for push_all too: {}", hub.outbox_pending(3));
        // reading frees the budget
        let mut o = hub.attach(4); assert!(hub.push(4, frame.clone())); let before = hub.outbox_pending(4); assert!(before > 0);
        o.recv().await.unwrap(); assert_eq!(hub.outbox_pending(4), 0);
    }

    // one frame over the cap goes through an empty outbox (a whole model on open), and still bounds memory at cap + one frame
    #[tokio::test] async fn a_frame_larger_than_the_cap_goes_to_an_empty_outbox_and_only_then() {
        let dir = std::env::temp_dir().join(format!("bp-outbox-big-{}", std::process::id())); let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
        let hub = Hub::start(Cfg { home: dir.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        hub.set_outbox_max(10_000);
        let mut o = hub.attach(1);
        assert!(hub.push_raw(1, vec![0u8; 50_000]), "a frame bigger than the cap reaches an empty outbox");
        assert!(hub.is_attached(1) && hub.outbox_pending(1) == 50_000);
        assert!(!hub.push_raw(1, vec![0u8; 10]) && !hub.is_attached(1), "with the big one still unread, any further frame drops the client: memory is cap + one frame");
        assert!(matches!(o.recv().await, Some(Out::Raw(b)) if b.len() == 50_000), "the frame that was queued is still delivered");
        // droppable frames: skipped for the slow one, taken by the healthy one
        let _slow = hub.attach(2); let mut ok = hub.attach(3);
        let f = json!({"t":"term"}); assert!(hub.push_budgeted(2, f.clone(), 4000));
        assert!(!hub.push_budgeted(2, f.clone(), 4000), "over half: skipped");
        assert_eq!(hub.push_all_budgeted(&f, 4000), 1, "client 3 takes it, client 2 is skipped, nobody is dropped");
        assert!(hub.is_attached(2) && hub.clients().len() == 2);
        ok.recv().await.unwrap();
    }
}

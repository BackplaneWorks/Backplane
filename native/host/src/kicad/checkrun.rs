//! The live DRC/ERC state of each project a client looks at (the desktop's checks panel and chip, the phones beside their viewers).
//! Per project and check: state (none / running / fresh / stale / failed), counts by severity, when it ran, how long, the design
//! revision it ran on, and its findings (kicad/findings.rs). Published as info `kicad.checks.<digest>` (the summary, small) and served
//! by RPC (the list):
//! - `kicad.checks.watch {project}`: the project's checks are kept live while some client asks at least every WATCH_MS; answers the summary.
//! - `kicad.checks.run {project, source?: "drc"|"erc", full?}`: run now (the Run button); `full` with the library checks too, which
//!   the background runs leave out (findings::library_rule), under the same memory guard with its own measured peak.
//! - `kicad.checks.list {project, source?, severity?, rule?, q?, offset?, limit?, rows?}`: {summary, total, findings: [...]}, errors first
//!   (`rows`: the panel's small rows, findings::row_json).
//! - The management panel "Checks" (kicad/mgmt.rs): info `mgmt.checks.<digest>`, the summary with `badge`, `total` and `groups`.
//! - `kicad.checks.focus {project, id}`: cross-probe the finding into the open KiCad (its items selected and zoomed, in the board for
//!   DRC, the schematic for ERC: `editor.kicad.select` with `show`) and light it in the viewers.
//! - `kicad.checks.fix {project, id}`: {text}: the message "Fix with agent" sends.
//! - `kicad.checks.set {project, source, findings, meta}`: another source's findings (`set_findings`, which the hub's own sources call).
//! Source: `kicad-cli pcb drc` / `sch erc` (JSON) on the saved files, DRC and ERC side by side when memory allows (findings::admit),
//! at low priority, cancelled when a newer save arrives. When it runs is findings::next (saves debounced, held while the
//! person works in KiCad or an agent's turn holds the document). Results are kept in `<home>/cache/checks/` (never beside the design),
//! so a restart shows the last result at once, fresh or stale. When this machine cannot take a run (memory) or is busy, and the setting
//! `checks.remote` names another machine of the person's with the same KiCad build, the run goes there (`Remote`); never to a Boat.
use super::{cad, embed, findings::{self as fd, Counts, Do, Finding, Sched}, ipc, kserve as ks, project};
use crate::{api::{Caller, Fut, RpcHandler}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::{collections::{BTreeMap, HashMap}, path::{Path, PathBuf}, sync::{Arc, Mutex, OnceLock, Weak}, time::Duration};

/// A project stays live this long after a client last asked (clients ask again while their panel or chip shows).
const WATCH_MS: u64 = 10 * 60_000;
const TICK_MS: u64 = 1000;
const RUN_SECS: u64 = 600;
pub const SOURCES: [&str; 2] = ["erc", "drc"];

// Other sources -------------------------------------------------------------------------------------------------------------------

/// The sources besides DRC and ERC (id, label), in the order they first put findings (or were registered).
static EXT_SOURCES: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
/// Name a source and its label ahead of its first findings (optional: `set_findings` registers a new id itself, its label from
/// `meta.label`).
pub fn register_source(id: &str, label: &str) {
    let mut v = EXT_SOURCES.lock().unwrap();
    match v.iter_mut().find(|x| x.0 == id) { Some(x) => { if !label.is_empty() { x.1 = label.to_string() } } None => v.push((id.to_string(), if label.is_empty() { id.to_string() } else { label.to_string() })) }
}
fn ext_known(id: &str) -> bool { EXT_SOURCES.lock().unwrap().iter().any(|x| x.0 == id) }
fn known_source(id: &str) -> bool { SOURCES.contains(&id) || ext_known(id) }
/// A source's findings for a project (`pro`: any path of it), replacing the ones it set before (the design review's "fab", "bom",
/// "outputs", "assembly", "mech", ...). `meta` is kept and published as given; the panel reads `label` (the source's name, "Fab rules"),
/// `judged` (true: an agent's judgement, labelled so in every row) and `confidence`. Each finding's `source` becomes `source`; an empty
/// `id` is made from what it is about (findings::assign_ids), so give stable anchors (KIIDs, names) for ids that last across runs. The
/// set is of the design as it is now: it reads stale once the design on disk changes, as DRC's does.
pub async fn set_findings(hub: &Arc<Hub>, pro: &str, source: &str, mut findings: Vec<Finding>, meta: Value) -> Result<(), String> {
    if source.is_empty() || SOURCES.contains(&source) || source.len() > 32 { return Err(format!("{source:?} cannot be set from outside")) }
    register_source(source, meta["label"].as_str().unwrap_or(""));
    let pro = named(hub, &json!({"project": pro})).await?;
    let given: Vec<String> = findings.iter().map(|f| f.id.clone()).collect();
    for f in findings.iter_mut() { f.source = source.to_string(); }
    fd::assign_ids(&mut findings);
    for (f, g) in findings.iter_mut().zip(given) { if !g.is_empty() { f.id = g } }
    let files = with(&pro, |p| p.files.clone()).unwrap_or_default();
    let rev = tokio::task::spawn_blocking(move || disk_revs(&files)).await.map(|(e, d)| format!("{e}{d}")).unwrap_or_default();
    {
        let mut m = state().lock().unwrap();
        let Some(p) = m.get_mut(&pro) else { return Err("the project is not live".into()) };
        p.ext.insert(source.to_string(), Ext { at: now_ms(), counts: fd::counts(&findings), findings, rev, meta });
        save(hub, &pro, p);
    }
    publish(hub, &pro).await;
    Ok(())
}
/// Every finding of a project: ERC, DRC, then the other sources in their order.
fn all_findings(p: &Proj, only: &str) -> Vec<Finding> {
    let mut v: Vec<Finding> = vec![];
    if only.is_empty() || only == "erc" { v.extend(p.erc.findings.iter().cloned()) }
    if only.is_empty() || only == "drc" { v.extend(p.drc.findings.iter().cloned()) }
    for (id, _) in EXT_SOURCES.lock().unwrap().iter() { if only.is_empty() || only == id { if let Some(e) = p.ext.get(id) { v.extend(e.findings.iter().cloned()) } } }
    v
}

#[derive(Default)]
struct Src {
    sched: Sched,
    state: String,
    reason: String,
    findings: Vec<Finding>,
    counts: Counts,
    /// the last result: when (ms), how long it took, of what revision
    at: u64,
    took: u64,
    had: bool,
    cancel: Option<Arc<tokio::sync::Notify>>,
    /// why a run that may go waits for the machine ("" none), and the last run's peak memory (MB; 0 unknown)
    deferred: String,
    peak_mb: u64,
    /// a full run (the library checks too) was asked for; the last result was one; its measured peak
    full_next: bool,
    full: bool,
    peak_full_mb: u64,
    /// the machine the last result came from ("" this one)
    ran_on: String,
}
/// Findings another part of the hub puts in this list (the design review's sources: the fab's rules, the BOM, the fab outputs, the
/// datasheets...): their state and set, replaced whole each time (`put_findings`).
#[derive(Clone, Default)]
struct Ext { at: u64, findings: Vec<Finding>, counts: Counts, rev: String, meta: Value }
#[derive(Default)]
struct Proj { files: project::Files, drc: Src, erc: Src, asked: u64, loaded: bool, ext: BTreeMap<String, Ext> }
impl Proj {
    fn src(&mut self, s: &str) -> &mut Src { if s == "drc" { &mut self.drc } else { &mut self.erc } }
    fn has(&self, s: &str) -> bool { if s == "drc" { !self.files.pcb.is_empty() } else { !self.files.sch.is_empty() } }
}

static STATE: OnceLock<Mutex<HashMap<String, Proj>>> = OnceLock::new();
fn state() -> &'static Mutex<HashMap<String, Proj>> { STATE.get_or_init(Default::default) }
/// The person's last action in each project's KiCad (ms).
static PERSON: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
static HUB: Mutex<Option<Weak<Hub>>> = Mutex::new(None);
static WAKE: OnceLock<tokio::sync::Notify> = OnceLock::new();
fn wake() { if let Some(n) = WAKE.get() { n.notify_one() } }
/// The runs going in the whole hub: (project|check, expected peak MB); fd::admit decides whether one more may start.
static RUNNING: Mutex<Vec<(String, u64)>> = Mutex::new(Vec::new());

pub fn start(hub: &Arc<Hub>) {
    *HUB.lock().unwrap() = Some(Arc::downgrade(hub));
    let _ = WAKE.set(tokio::sync::Notify::new());
    hub.register_rpc(Arc::new(ChecksRpc));
    super::mgmt::register(Arc::new(ChecksPanel));
    let h = Arc::downgrade(hub);
    tokio::spawn(async move {
        loop {
            let Some(hub) = h.upgrade() else { return };
            let wait = tick(&hub).await;
            drop(hub);
            let _ = tokio::time::timeout(Duration::from_millis(wait), WAKE.get().unwrap().notified()).await;
        }
    });
}

/// From the fork's events loop (embed.rs): the person acted in KiCad (a selection, an edit, a modified mark): runs wait QUIET_MS.
pub fn on_event(pro: &str, ev: &Value) {
    let ty = ev["type"].as_str().unwrap_or(""); let n = ev["type"].as_i64().unwrap_or(0);
    if matches!(ty, "ET_SELECTION_CHANGED" | "ET_DOCUMENT_EDITED" | "ET_MODIFIED_CHANGED" | "ET_FOCUS_IN") || matches!(n, 14 | 20 | 6 | 7) {
        PERSON.get_or_init(Default::default).lock().unwrap().insert(pro.to_string(), now_ms());
    }
}

// Revisions ----------------------------------------------------------------------------------------------------------------------

/// The schematic files of a project: every .kicad_sch in its folder and the folders below (3 deep), copies and hidden folders left out.
/// (A listing, not a read: it runs every second.)
fn sheet_files(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path(); let name = e.file_name().to_string_lossy().to_lowercase();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() { if depth > 0 && !name.starts_with('.') && !project::copies(&name) { sheet_files(&p, depth - 1, out) } }
        else if name.ends_with(".kicad_sch") { out.push(p) }
        if out.len() > 256 { return }
    }
}
fn stamps(paths: &[PathBuf]) -> String {
    let mut v: Vec<String> = paths.iter().map(|p| { let s = p.to_string_lossy(); format!("{s}={:?}", super::watch::stamp(&s)) }).collect();
    v.sort();
    format!("{:016x}", v.join("\n").bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3)))
}
/// The revisions on disk now: (ERC's: the project file and every sheet; DRC's: the board, its rules, and the sheets too, for the
/// schematic parity check).
fn disk_revs(f: &project::Files) -> (String, String) {
    let pro = PathBuf::from(&f.pro);
    let dir = pro.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut sch = vec![]; if !f.sch.is_empty() { sheet_files(&dir, 3, &mut sch) }
    let mut e = sch.clone(); e.push(pro.clone());
    let mut d = sch; d.push(pro.clone());
    if !f.pcb.is_empty() { d.push(PathBuf::from(&f.pcb)); d.push(Path::new(&f.pcb).with_extension("kicad_dru")) }
    (if f.sch.is_empty() { String::new() } else { stamps(&e) }, if f.pcb.is_empty() { String::new() } else { stamps(&d) })
}

// Cache --------------------------------------------------------------------------------------------------------------------------

fn cache_dir(hub: &Hub) -> PathBuf { hub.cfg.home.join("cache").join("checks") }
fn cache_file(hub: &Hub, pro: &str) -> PathBuf { cache_dir(hub).join(format!("{}.json", ks::digest(pro))) }
fn save(hub: &Hub, pro: &str, p: &Proj) {
    let one = |s: &Src| json!({"rev": s.sched.ran_rev, "at": s.at, "took": s.took, "had": s.had, "peak_mb": s.peak_mb, "peak_full_mb": s.peak_full_mb, "full": s.full, "failed": if s.state == "failed" { s.reason.clone() } else { String::new() }, "findings": s.findings.iter().map(fd::to_json).collect::<Vec<_>>()});
    let ext: serde_json::Map<String, Value> = p.ext.iter().map(|(k, e)| (k.clone(), json!({"rev": e.rev, "meta": e.meta, "at": e.at, "findings": e.findings.iter().map(fd::to_json).collect::<Vec<_>>()}))).collect();
    let v = json!({"pro": pro, "drc": one(&p.drc), "erc": one(&p.erc), "ext": ext});
    let f = cache_file(hub, pro);
    let _ = std::fs::create_dir_all(cache_dir(hub));
    let tmp = f.with_extension("tmp");
    if std::fs::write(&tmp, v.to_string()).is_ok() { let _ = std::fs::rename(&tmp, &f); }
}
fn load(hub: &Hub, pro: &str, p: &mut Proj) {
    let Some(v) = std::fs::read_to_string(cache_file(hub, pro)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { return };
    if v["pro"] != pro { return }
    for s in SOURCES {
        let o = &v[s]; let src = p.src(s);
        src.peak_mb = o["peak_mb"].as_u64().unwrap_or(0); src.peak_full_mb = o["peak_full_mb"].as_u64().unwrap_or(0); src.full = o["full"].as_bool().unwrap_or(false);
        if !o["had"].as_bool().unwrap_or(false) { continue }
        src.findings = o["findings"].as_array().into_iter().flatten().map(fd::from_json).collect();
        src.counts = fd::counts(&src.findings);
        src.sched.ran_rev = o["rev"].as_str().unwrap_or("").into();
        src.at = o["at"].as_u64().unwrap_or(0); src.took = o["took"].as_u64().unwrap_or(0); src.had = true;
        let failed = o["failed"].as_str().unwrap_or("");
        if !failed.is_empty() { src.state = "failed".into(); src.reason = failed.into(); }
    }
    for (k, o) in v["ext"].as_object().into_iter().flatten() {
        let findings: Vec<Finding> = o["findings"].as_array().into_iter().flatten().map(fd::from_json).collect();
        register_source(k, o["meta"]["label"].as_str().unwrap_or(""));
        p.ext.insert(k.clone(), Ext { rev: o["rev"].as_str().unwrap_or("").into(), meta: o["meta"].clone(), at: o["at"].as_u64().unwrap_or(0), counts: fd::counts(&findings), findings });
    }
}

// Summary ------------------------------------------------------------------------------------------------------------------------

fn src_json(p: &Proj, s: &str, now: u64) -> Value {
    let x = if s == "drc" { &p.drc } else { &p.erc };
    if !p.has(s) { return Value::Null }
    let st = state_word(x);
    let running = x.sched.running_since > 0;
    let reason = if st == "deferred" { x.deferred.trim_start_matches("deferred: ").to_string() } else { x.reason.clone() };
    json!({"state": st, "reason": reason, "running": running, "fraction": if running { fd::fraction(now.saturating_sub(x.sched.running_since), x.took) } else { -1.0 },
        "counts": x.counts.to_json(), "at": x.at, "took_ms": x.took, "rev": x.sched.ran_rev, "has_result": x.had, "peak_mb": x.peak_mb, "full": x.full, "full_next": x.full_next, "ran_on": x.ran_on, "note": if x.full { "" } else { fd::LIBRARY_NOTE }})
}
/// none (no result yet), running, deferred (a run waits for memory or another heavy KiCad job), fresh (the result is of the design on
/// disk), stale (the design changed since), failed.
fn state_word(x: &Src) -> &'static str {
    if x.sched.running_since > 0 { "running" } else if !x.deferred.is_empty() { "deferred" } else if x.state == "failed" { "failed" } else if !x.had { "none" } else if x.sched.ran_rev != x.sched.disk_rev && !x.sched.disk_rev.is_empty() { "stale" } else { "fresh" }
}
fn summary(pro: &str, p: &Proj, now: u64) -> Value {
    let pick = |s: &str| { let x = if s == "drc" { &p.drc } else { &p.erc }; p.has(s).then(|| (if x.state == "failed" && x.sched.running_since == 0 { "failed" } else { "ok" }, x.counts, x.had)) };
    // the other sources, by id, in their order (`order`): stale once the design changed since their set
    let now_rev = format!("{}{}", p.erc.sched.disk_rev, p.drc.sched.disk_rev);
    let mut sources = serde_json::Map::new(); let mut order = vec![];
    for (id, label) in EXT_SOURCES.lock().unwrap().iter() {
        let Some(e) = p.ext.get(id) else { continue };
        let stale = !e.rev.is_empty() && !p.erc.sched.disk_rev.is_empty() && e.rev != now_rev;
        sources.insert(id.clone(), json!({"label": label, "state": if stale { "stale" } else { "fresh" }, "counts": e.counts.to_json(), "at": e.at, "rev": e.rev, "meta": e.meta, "judged": e.meta["judged"] == true}));
        order.push(id.clone());
    }
    json!({"project": pro, "pcb": p.files.pcb, "sch": p.files.sch, "drc": src_json(p, "drc", now), "erc": src_json(p, "erc", now), "sources": sources, "order": order, "chip": fd::chip(pick("drc"), pick("erc"))})
}
async fn publish(hub: &Arc<Hub>, pro: &str) {
    let (v, panel) = { let m = state().lock().unwrap(); match m.get(pro) { Some(p) => { let v = summary(pro, p, now_ms()); (v.clone(), panel_json(p, v)) } None => return } };
    hub.set_info(&info_key(pro), &v.to_string()).await;
    super::mgmt::publish(hub, "checks", pro, panel).await;
}
/// The Checks panel's data (info `mgmt.checks.<digest>`): the summary, the tab's badge (errors red, else warnings amber) and the findings
/// grouped by rule, as many rows as fit (`kicad.checks.list` has the rest).
fn panel_json(p: &Proj, mut v: Value) -> Value {
    let all = all_findings(p, "");
    let c = fd::counts(&all);
    v["badge"] = json!({"n": c.error + c.warning, "tone": if c.error > 0 { "bad" } else { "warn" }});
    v["total"] = json!(all.len());
    v["groups"] = Value::Array(fd::groups_within(&all, 48_000));
    v
}
pub fn info_key(pro: &str) -> String { format!("kicad.checks.{}", ks::digest(pro)) }

// The loop -----------------------------------------------------------------------------------------------------------------------

/// Whether an agent's turn holds the lock of one of the project's documents, since when (ms; 0 none).
async fn agent_since(hub: &Arc<Hub>, f: &project::Files, held: &mut HashMap<String, u64>, pro: &str) -> u64 {
    let mut any = false;
    for d in [&f.pcb, &f.sch] {
        if d.is_empty() { continue }
        let l = crate::agents::lockrec::Lock::read(&hub.setting(&format!("lock.{}", super::canonical::doc_lock(d))).await.unwrap_or_default());
        if l.holder.is_some() { any = true }
    }
    if !any { held.remove(pro); return 0 }
    *held.entry(pro.to_string()).or_insert_with(now_ms)
}
static HELD: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();

/// One look at every live project: revisions, then each check's next step. Returns how long to sleep.
async fn tick(hub: &Arc<Hub>) -> u64 {
    let now = now_ms();
    let pros: Vec<(String, project::Files)> = {
        let mut m = state().lock().unwrap();
        // live while a client asked lately, its panel is open, its KiCad is open, or a run goes
        m.retain(|k, p| now.saturating_sub(p.asked) < WATCH_MS || super::mgmt::is_open(k, "checks") || embed::gui_route(k).is_some() || p.drc.sched.running_since > 0 || p.erc.sched.running_since > 0);
        m.iter().map(|(k, p)| (k.clone(), p.files.clone())).collect()
    };
    if pros.is_empty() { return 60_000 }
    let mut sleep = TICK_MS * 5;
    let (free, heavy, busy) = tokio::task::spawn_blocking(|| (free_mb(), heavy_job(), busy_now())).await.unwrap_or_default();
    let remote = remote_ready(hub).await;
    for (pro, files) in pros {
        let f2 = files.clone();
        let (erc_rev, drc_rev) = tokio::task::spawn_blocking(move || disk_revs(&f2)).await.unwrap_or_default();
        let agent = { let mut held = HELD.get_or_init(Default::default).lock().unwrap().clone(); let a = agent_since(hub, &files, &mut held, &pro).await; *HELD.get().unwrap().lock().unwrap() = held; a };
        let person = PERSON.get_or_init(Default::default).lock().unwrap().get(&pro).copied().unwrap_or(0);
        let mut acts = vec![]; let mut changed = false;
        {
            let mut m = state().lock().unwrap();
            let Some(p) = m.get_mut(&pro) else { continue };
            for s in SOURCES {
                if !p.has(s) { continue }
                let rev = if s == "drc" { &drc_rev } else { &erc_rev };
                let x = p.src(s);
                if x.sched.disk_rev != *rev { if !x.sched.disk_rev.is_empty() { x.sched.changed_at = now } x.sched.disk_rev = rev.clone(); changed = true }
                x.sched.agent_since = agent; x.sched.person_at = person; x.sched.free_mb = free; x.sched.heavy = heavy.clone();
                x.sched.need_mb = if x.full_next { fd::need_full_mb(s, doc_bytes(&files, s), x.peak_full_mb) } else { fd::need_mb(s, doc_bytes(&files, s), x.peak_mb) };
                let was = std::mem::take(&mut x.deferred);
                // where a run that may go runs: here, on the person's other machine set up for checks, or not yet
                let start = |x: &mut Src, host: Option<Remote>, acts: &mut Vec<_>| {
                    if host.is_none() { RUNNING.lock().unwrap().push((format!("{pro}|{s}"), x.sched.need_mb)); }
                    x.sched.demand = false; let full = std::mem::take(&mut x.full_next); x.sched.running_since = now; x.sched.running_rev = rev.clone();
                    let c = Arc::new(tokio::sync::Notify::new()); x.cancel = Some(c.clone()); acts.push((s, rev.clone(), c, full, host));
                };
                match fd::next_ignoring_machine(now, &x.sched) {
                    Do::Run => match fd::place(&fd::machine_guard(&x.sched), remote.is_some(), busy) {
                        fd::Place::Here if !fd::admit(free, &RUNNING.lock().unwrap().iter().map(|r| r.1).collect::<Vec<_>>(), x.sched.need_mb) => sleep = sleep.min(300),
                        fd::Place::Here => { start(x, None, &mut acts); changed = true }
                        fd::Place::There => { start(x, remote.clone(), &mut acts); changed = true }
                        fd::Place::Defer(why) => { x.deferred = why; sleep = sleep.min(fd::DEFER_MS) }
                    },
                    Do::Defer(why, ms) => { x.deferred = why; sleep = sleep.min(ms) }
                    Do::Cancel => { if let Some(c) = &x.cancel { c.notify_one() } }
                    Do::Wait(ms) => sleep = sleep.min(ms.max(100)),
                    Do::Nothing => {}
                }
                if x.deferred != was { changed = true }
                if x.sched.running_since > 0 { sleep = sleep.min(TICK_MS) }
            }
        }
        if changed { publish(hub, &pro).await }
        for (s, rev, c, full, host) in acts { let (h, p, f) = (hub.clone(), pro.clone(), files.clone()); tokio::spawn(async move { run(&h, &p, &f, s, &rev, c, full, host).await }); }
    }
    sleep
}

/// What one run found: its findings, or why it failed (None: cancelled).
type Outcome = Option<Result<Vec<Finding>, String>>;

async fn run(hub: &Arc<Hub>, pro: &str, f: &project::Files, s: &'static str, rev: &str, cancel: Arc<tokio::sync::Notify>, full: bool, host: Option<Remote>) {
    let t0 = now_ms();
    let (o, peak) = match &host { Some(r) => (run_remote(hub, r, pro, f, s, &cancel, full).await, 0), None => run_cli(hub, pro, f, s, &cancel, full).await };
    let took = now_ms() - t0;
    if let Some(p) = state().lock().unwrap().get_mut(pro) {
        let x = p.src(s);
        if peak > 0 { if full { x.peak_full_mb = peak } else { x.peak_mb = peak } }
        if matches!(o, Some(Ok(_))) { x.full = full; x.ran_on = host.as_ref().map(|r| r.host.clone()).unwrap_or_default() }
    }
    finish(hub, pro, s, rev, o, took).await
}

/// One kicad-cli run: its outcome and its peak memory (MB; 0 when this platform cannot say).
async fn run_cli(hub: &Arc<Hub>, pro: &str, f: &project::Files, s: &str, cancel: &tokio::sync::Notify, full: bool) -> (Outcome, u64) {
    let rt = ipc::runtime(hub).await;
    if rt.cli.is_empty() { return (Some(Err("KiCad is not installed on this machine (no kicad-cli)".into())), 0) }
    let dir = cache_dir(hub).join(ks::digest(pro));
    let _ = std::fs::create_dir_all(&dir);
    let Some(c) = cad::CHECKS.iter().find(|c| c.id == s) else { return (None, 0) };
    let argv = cad::fill(c.argv, &[("cli", &rt.cli), ("pcb", &f.pcb), ("sch", &f.sch), ("out", &dir.to_string_lossy())]);
    let report = dir.join(c.report);
    let _ = std::fs::remove_file(&report);
    let mut cmd = low_priority(&argv);
    if !full { cmd.env("KICAD_CONFIG_HOME", lib_free_config(hub, &rt.version)); }
    cmd.kill_on_drop(true).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let child = match cmd.spawn() { Ok(c) => c, Err(e) => return (Some(Err(format!("could not start kicad-cli: {e}"))), 0) };
    let pid = child.id().unwrap_or(0);
    OURS.get_or_init(Default::default).lock().unwrap().push(pid);
    struct Unlist(u32);
    impl Drop for Unlist { fn drop(&mut self) { if let Some(o) = OURS.get() { o.lock().unwrap().retain(|p| *p != self.0) } } }
    let _unlist = Unlist(pid);
    let mut peak = 0u64;
    let wait = tokio::time::timeout(Duration::from_secs(RUN_SECS), child.wait_with_output());
    tokio::pin!(wait);
    let mut every = tokio::time::interval(Duration::from_millis(200));
    let res = loop {
        tokio::select! {
            r = &mut wait => break r,
            _ = cancel.notified() => return (None, peak), // the child is killed as it is dropped
            _ = every.tick() => { peak = peak.max(rss_peak_mb(pid)) }
        }
    };
    let o = match res { Ok(Ok(o)) => o, Ok(Err(e)) => return (Some(Err(format!("kicad-cli: {e}"))), peak), Err(_) => return (Some(Err(format!("{} took longer than {RUN_SECS} s", s.to_uppercase()))), peak) };
    let text = std::fs::read_to_string(&report).unwrap_or_default();
    let parsed = if s == "drc" { fd::drc(&text) } else { fd::erc(&text) };
    let out = match parsed {
        Some(mut fs) => { if !full { fs.retain(|x| !fd::library_rule(s, &x.rule)) } Some(Ok(fs)) }
        None if String::from_utf8_lossy(&o.stderr).contains("transient scope") && !NO_SCOPE.swap(true, std::sync::atomic::Ordering::Relaxed) => {
            eprintln!("checks: systemd made no scope ({}); runs go without one", String::from_utf8_lossy(&o.stderr).trim());
            return Box::pin(run_cli(hub, pro, f, s, cancel, full)).await
        }
        None if killed(&o.status) => Some(Err("stopped at its memory limit (the machine needed the memory)".into())),
        None => {
            let why = String::from_utf8_lossy(&o.stderr).trim().lines().last().unwrap_or("").to_string();
            let why = if why.is_empty() { String::from_utf8_lossy(&o.stdout).trim().lines().last().unwrap_or("").to_string() } else { why };
            Some(Err(format!("kicad-cli {} failed (exit {}){}", s, o.status.code().unwrap_or(-1), if why.is_empty() { String::new() } else { format!(": {why}") })))
        }
    };
    (out, peak)
}
#[cfg(unix)] fn killed(st: &std::process::ExitStatus) -> bool { use std::os::unix::process::ExitStatusExt; st.signal() == Some(9) }
#[cfg(not(unix))] fn killed(_: &std::process::ExitStatus) -> bool { false }

/// A KiCad settings folder whose global footprint and symbol tables are empty (`KICAD_CONFIG_HOME`, every platform): kicad-cli then
/// loads only the project's own libraries (fd::library_rule). KiCad reads `<KICAD_CONFIG_HOME>/<major.minor>/`.
fn lib_free_config(hub: &Hub, version: &str) -> PathBuf {
    let root = cache_dir(hub).join("kicad-config");
    let mm: String = { let mut it = version.split(['.', '-']); match (it.next(), it.next()) { (Some(a), Some(b)) if !a.is_empty() => format!("{a}.{b}"), _ => String::new() } };
    for v in ["10.99", "10.0", "9.0"].into_iter().map(String::from).chain((!mm.is_empty()).then_some(mm)) {
        let d = root.join(&v);
        if d.join("fp-lib-table").is_file() { continue }
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join("fp-lib-table"), "(fp_lib_table\n  (version 7)\n)\n");
        let _ = std::fs::write(d.join("sym-lib-table"), "(sym_lib_table\n  (version 7)\n)\n");
    }
    root
}

/// kicad-cli at a low priority, in a scope of its own outside the hub's where systemd can make one (Linux: `systemd-run --user --scope`
/// in bpkicad.slice, MemoryMax 50%, so a runaway run is stopped alone and the hub's scope is never the one picked), then `nice -n 15`;
/// below-normal priority on Windows. Its folder is the root: every path is absolute.
fn low_priority(argv: &[String]) -> tokio::process::Command {
    #[cfg(unix)] let mut c = {
        let scope = cfg!(target_os = "linux") && std::env::var_os("BACKPLANE_CHECKS_NO_SCOPE").is_none() && !NO_SCOPE.load(std::sync::atomic::Ordering::Relaxed) && on_path("systemd-run")
            && std::env::var_os("XDG_RUNTIME_DIR").map_or(false, |d| Path::new(&d).join("bus").exists());
        let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, if scope { "systemd-run" } else { "nice" });
        if scope { c.args(["--user", "--scope", "-q", "--collect", "--slice=bpkicad.slice", "-p", "MemoryMax=50%", "-p", "ManagedOOMPreference=none", "--", "nice"]); }
        c.arg("-n").arg("15").args(argv); c
    };
    #[cfg(windows)] let mut c = { const BELOW_NORMAL: u32 = 0x0000_4000; let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, &argv[0]); c.args(&argv[1..]); c.creation_flags(crate::plat::proc::NO_WINDOW | BELOW_NORMAL); c };
    c.current_dir(std::path::MAIN_SEPARATOR_STR);
    c
}

// Another machine ----------------------------------------------------------------------------------------------------------------

/// The person's other machine checks may run on: the setting `checks.remote` (an ssh destination, "h@backplane"; empty, the default:
/// never), and its kicad-cli, the same build as this machine's (its `--version` the same): `checks.remote.cli`, else the runtime of
/// the same name in `~/.cache/bp-kembed-remote/kicad/` or `~/.backplane-gpui-fresh/home/kicad/` there. Only the project's design files
/// go there (its .kicad_pro, board, rules, sheets and project library tables), into `~/.cache/backplane-checks/<digest>/`. A Boat is
/// never used for this: design files leave the person's machines only where they say so.
#[derive(Clone, Debug, PartialEq)]
pub struct Remote { pub host: String, pub cli: String }
/// (setting, local version) -> the probe's answer, so a host is asked once
static PROBED: Mutex<Option<((String, String, String), Option<Remote>)>> = Mutex::new(None);
async fn remote_ready(hub: &Arc<Hub>) -> Option<Remote> {
    let host = hub.setting("checks.remote").await.unwrap_or_default();
    if host.is_empty() || crate::copy::on() { return None }
    let cli_set = hub.setting("checks.remote.cli").await.unwrap_or_default();
    let rt = ipc::runtime(hub).await;
    if rt.cli.is_empty() { return None }
    let key = (host.clone(), cli_set.clone(), rt.cli.clone());
    if let Some((k, v)) = PROBED.lock().unwrap().clone() { if k == key { return v } }
    let name = Path::new(&rt.cli).parent().and_then(Path::parent).and_then(|d| d.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let here = crate::proc::tokio_cmd(crate::proc::Kind::Local, &rt.cli).arg("--version").output().await.ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let cands = if cli_set.is_empty() { format!("$HOME/.cache/bp-kembed-remote/kicad/{name}/bin/kicad-cli $HOME/.backplane-gpui-fresh/home/kicad/{name}/bin/kicad-cli") } else { cli_set.clone() };
    let script = format!("for c in {cands}; do if [ -x \"$c\" ]; then printf '%s\\t' \"$c\"; \"$c\" --version; exit 0; fi; done; exit 1");
    let out = ssh(&host, &script, 20).await;
    let found = out.ok().and_then(|o| { let (c, v) = o.trim().split_once('\t')?; (v.trim() == here && !here.is_empty()).then(|| Remote { host: host.clone(), cli: c.to_string() }) });
    if found.is_none() { eprintln!("checks: {host} has no kicad-cli of this build ({here}); checks stay on this machine"); }
    *PROBED.lock().unwrap() = Some((key, found.clone()));
    found
}
/// A shell script run on the other machine (ssh, no prompts), its output; Err on failure or after `secs`.
async fn ssh(host: &str, script: &str, secs: u64) -> Result<String, String> {
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Net, "ssh");
    c.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5", host, "sh", "-c", &shell_quote(script)]).stdin(std::process::Stdio::null()).kill_on_drop(true);
    match tokio::time::timeout(Duration::from_secs(secs), c.output()).await {
        Ok(Ok(o)) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).into_owned()),
        Ok(Ok(o)) => Err(String::from_utf8_lossy(&o.stderr).trim().lines().last().unwrap_or("ssh failed").to_string()),
        Ok(Err(e)) => Err(e.to_string()), Err(_) => Err(format!("{host} did not answer in {secs} s")),
    }
}
/// A word for a remote shell: single-quoted.
pub fn shell_quote(s: &str) -> String { format!("'{}'", s.replace('\'', "'\\''")) }
/// The project's design files the checks read, relative to its folder: the project file, board, rules, every sheet, project library tables.
pub fn design_files(dir: &Path, f: &project::Files) -> Vec<String> {
    let rel = |p: &Path| p.strip_prefix(dir).ok().map(|r| r.to_string_lossy().replace('\\', "/"));
    let mut v: Vec<String> = vec![];
    let mut add = |p: PathBuf| if p.is_file() { if let Some(r) = rel(&p) { if !v.contains(&r) { v.push(r) } } };
    add(PathBuf::from(&f.pro));
    if !f.pcb.is_empty() { add(PathBuf::from(&f.pcb)); add(Path::new(&f.pcb).with_extension("kicad_dru")); }
    let mut sch = vec![]; if !f.sch.is_empty() { sheet_files(dir, 3, &mut sch) }
    for p in sch { add(p) }
    add(dir.join("fp-lib-table")); add(dir.join("sym-lib-table"));
    v
}
/// One check on the other machine: the design files copied there, the same command as here, its report read back.
async fn run_remote(hub: &Arc<Hub>, r: &Remote, pro: &str, f: &project::Files, s: &str, cancel: &tokio::sync::Notify, full: bool) -> Outcome {
    let Some(c) = cad::CHECKS.iter().find(|c| c.id == s) else { return None };
    let dir = Path::new(&f.pro).parent().map(Path::to_path_buf).unwrap_or_default();
    let list = { let (d, f) = (dir.clone(), f.clone()); tokio::task::spawn_blocking(move || design_files(&d, &f)).await.unwrap_or_default() };
    let rdir = format!(".cache/backplane-checks/{}", ks::digest(pro));
    let tmp = cache_dir(hub).join(format!("{}.files", ks::digest(pro)));
    let _ = std::fs::create_dir_all(cache_dir(hub));
    if let Err(e) = std::fs::write(&tmp, list.join("\n")) { return Some(Err(format!("the list of design files: {e}"))) }
    if let Err(e) = ssh(&r.host, &format!("mkdir -p $HOME/{rdir}/proj $HOME/{rdir}/out"), 20).await { return Some(Err(format!("{} cannot be reached: {e}", r.host))) }
    let mut rs = crate::proc::tokio_cmd(crate::proc::Kind::Net, "rsync");
    rs.args(["-a", "--files-from"]).arg(&tmp).arg("-e").arg("ssh -o BatchMode=yes -o ConnectTimeout=5").arg(format!("{}/", dir.display())).arg(format!("{}:{rdir}/proj/", r.host)).kill_on_drop(true);
    match tokio::select! { o = rs.output() => o, _ = cancel.notified() => return None } {
        Ok(o) if o.status.success() => {}
        Ok(o) => return Some(Err(format!("copying the design to {} failed: {}", r.host, String::from_utf8_lossy(&o.stderr).trim().lines().last().unwrap_or("")))),
        Err(e) => return Some(Err(format!("rsync: {e}"))),
    }
    let rel = |p: &str| Path::new(p).strip_prefix(&dir).map(|x| format!("$HOME/{rdir}/proj/{}", x.to_string_lossy())).unwrap_or_default();
    let argv = cad::fill(c.argv, &[("cli", &r.cli), ("pcb", &rel(&f.pcb)), ("sch", &rel(&f.sch)), ("out", &format!("$HOME/{rdir}/out"))]);
    let cmd = argv.iter().map(|a| if a.contains("$HOME") { format!("\"{a}\"") } else { shell_quote(a) }).collect::<Vec<_>>().join(" ");
    let cfg = if full { String::new() } else { format!("K=$HOME/{rdir}/../kicad-config; for v in 10.99 10.0 9.0; do mkdir -p $K/$v; [ -f $K/$v/fp-lib-table ] || printf '(fp_lib_table\\n  (version 7)\\n)\\n' > $K/$v/fp-lib-table; [ -f $K/$v/sym-lib-table ] || printf '(sym_lib_table\\n  (version 7)\\n)\\n' > $K/$v/sym-lib-table; done; export KICAD_CONFIG_HOME=$K; ") };
    let script = format!("{cfg}mkdir -p $HOME/{rdir}/out && rm -f $HOME/{rdir}/out/{rep} && nice -n 10 {cmd} >/dev/null 2>&1; cat $HOME/{rdir}/out/{rep}", rep = c.report);
    let text = tokio::select! { o = ssh(&r.host, &script, RUN_SECS) => o, _ = cancel.notified() => return None };
    let text = match text { Ok(t) => t, Err(e) => return Some(Err(format!("{s} on {} failed: {e}", r.host))) };
    match if s == "drc" { fd::drc(&text) } else { fd::erc(&text) } {
        Some(mut fs) => { if !full { fs.retain(|x| !fd::library_rule(s, &x.rule)) } Some(Ok(fs)) }
        None => Some(Err(format!("{s} on {} gave no report", r.host))),
    }
}

// The machine ----------------------------------------------------------------------------------------------------------------------

/// Whether this machine is busy now: its load at or past its cores (Linux; never elsewhere).
fn busy_now() -> bool {
    let load = std::fs::read_to_string("/proc/loadavg").ok().and_then(|t| t.split_whitespace().next()?.parse::<f64>().ok()).unwrap_or(0.0);
    load > 0.0 && load >= std::thread::available_parallelism().map_or(1, |n| n.get()) as f64
}
/// Memory available now (MB; 0 when this platform cannot say: the guard then does not hold runs back).
fn free_mb() -> u64 {
    // a test hub says how much is free (embed_checks_e2e's low-memory case)
    if let Some(n) = std::env::var("BACKPLANE_CHECKS_FREE_MB").ok().and_then(|v| v.parse().ok()) { return n }
    std::fs::read_to_string("/proc/meminfo").ok().and_then(|m| m.lines().find(|l| l.starts_with("MemAvailable:"))?.split_whitespace().nth(1)?.parse::<u64>().ok()).map_or(0, |kb| kb / 1024)
}
/// A process's peak resident memory so far (MB; Linux, 0 elsewhere).
fn rss_peak_mb(pid: u32) -> u64 {
    if pid == 0 { return 0 }
    std::fs::read_to_string(format!("/proc/{pid}/status")).ok().and_then(|t| t.lines().find(|l| l.starts_with("VmHWM:"))?.split_whitespace().nth(1)?.parse::<u64>().ok()).map_or(0, |kb| kb / 1024)
}
/// Another heavy KiCad job on this machine, in words ("" none): a long job in an open KiCad (a DRC, a zone fill, a load), or a
/// kicad-cli this hub did not start (an agent's export, a release's checks). Linux reads /proc; elsewhere only the open KiCad's jobs.
fn heavy_job() -> String {
    if let Some(t) = super::progress::running().into_iter().next() { return if t.is_empty() { "a job in KiCad".into() } else { format!("{t} in KiCad") } }
    let mine: Vec<u32> = OURS.get_or_init(Default::default).lock().unwrap().clone();
    let Ok(rd) = std::fs::read_dir("/proc") else { return String::new() };
    for e in rd.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
        if mine.contains(&pid) { continue }
        if std::fs::read_to_string(format!("/proc/{pid}/comm")).map_or(false, |c| c.trim() == "kicad-cli") { return "another kicad-cli".into() }
    }
    String::new()
}
/// systemd would not make a scope here once: every run after goes without one.
static NO_SCOPE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn on_path(c: &str) -> bool { std::env::var_os("PATH").map_or(false, |p| std::env::split_paths(&p).any(|d| d.join(c).is_file())) }
/// The kicad-cli runs this hub has going (their PIDs).
static OURS: OnceLock<Mutex<Vec<u32>>> = OnceLock::new();
fn doc_bytes(f: &project::Files, s: &str) -> u64 { std::fs::metadata(if s == "drc" { &f.pcb } else { &f.sch }).map_or(0, |m| m.len()) }

async fn finish(hub: &Arc<Hub>, pro: &str, s: &str, rev: &str, o: Outcome, took: u64) {
    { let k = format!("{pro}|{s}"); RUNNING.lock().unwrap().retain(|r| r.0 != k); }
    {
        let mut m = state().lock().unwrap();
        let Some(p) = m.get_mut(pro) else { return };
        let x = p.src(s);
        x.sched.running_since = 0; x.sched.running_rev.clear(); x.cancel = None;
        match o {
            None => {} // cancelled: the newer revision runs next
            Some(Ok(fs)) => { x.counts = fd::counts(&fs); x.findings = fs; x.sched.ran_rev = rev.into(); x.at = now_ms(); x.took = took; x.had = true; x.state.clear(); x.reason.clear(); }
            Some(Err(why)) => { x.state = "failed".into(); x.reason = why; x.sched.ran_rev = rev.into(); x.at = now_ms(); }
        }
        save(hub, pro, p);
    }
    publish(hub, pro).await;
    wake();
}

// Requests -----------------------------------------------------------------------------------------------------------------------

/// The project a request names, known to this hub; it is kept live from now on.
async fn named(hub: &Arc<Hub>, p: &Value) -> Result<String, String> {
    let raw = p["project"].as_str().or_else(|| p["path"].as_str()).unwrap_or("");
    if !crate::plat::fs::is_abs(raw) { return Err("project must be an absolute path (.kicad_pro, .kicad_pcb, .kicad_sch, or project folder)".into()) }
    let files = super::xprobe::files_of(raw).ok_or_else(|| format!("no KiCad project (.kicad_pro) found for {raw}"))?;
    let root = Path::new(&files.pro).parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
    if !super::known_root(hub, &root, "").await { return Err(format!("{} is not in a project this hub knows", files.pro)) }
    let pro = files.pro.clone();
    let fresh = {
        let mut m = state().lock().unwrap();
        let e = m.entry(pro.clone()).or_default();
        e.files = files; e.asked = now_ms();
        let first = !e.loaded; e.loaded = true; first
    };
    if fresh {
        let mut tmp = Proj::default();
        load(hub, &pro, &mut tmp);
        let mut m = state().lock().unwrap();
        if let Some(e) = m.get_mut(&pro) { for s in SOURCES { let (a, b) = (e.src(s), tmp.src(s)); if !a.had && b.had { std::mem::swap(a, b) } } for (k, x) in tmp.ext { e.ext.entry(k).or_insert(x); } }
        drop(m);
        wake();
    }
    Ok(pro)
}
fn with<T>(pro: &str, f: impl FnOnce(&Proj) -> T) -> Option<T> { state().lock().unwrap().get(pro).map(f) }
fn find(pro: &str, id: &str) -> Option<Finding> {
    with(pro, |p| all_findings(p, "").into_iter().find(|f| f.id == id)).flatten()
}

pub async fn request(hub: &Arc<Hub>, method: &str, p: &Value) -> Result<Value, String> {
    let pro = named(hub, p).await?;
    let src = p["source"].as_str().unwrap_or("");
    if !src.is_empty() && !known_source(src) && method != "kicad.checks.set" { return Err(format!("unknown source {src}")) }
    match method {
        "kicad.checks.watch" => Ok(with(&pro, |x| summary(&pro, x, now_ms())).unwrap_or(Value::Null)),
        "kicad.checks.run" => {
            { let mut m = state().lock().unwrap(); if let Some(x) = m.get_mut(&pro) { let full = p["full"] == true; for s in SOURCES { if (src.is_empty() || s == src) && x.has(s) { let y = x.src(s); y.sched.demand = true; y.full_next = full } } } }
            wake();
            Ok(json!({"project": pro, "queued": true}))
        }
        "kicad.checks.list" => {
            let (sev, q, rule) = (p["severity"].as_str().unwrap_or(""), p["q"].as_str().unwrap_or(""), p["rule"].as_str().unwrap_or(""));
            let offset = p["offset"].as_u64().unwrap_or(0) as usize; let limit = p["limit"].as_u64().unwrap_or(500).min(2000) as usize;
            Ok(with(&pro, |x| {
                let all = all_findings(x, src);
                let mut l = fd::filter(&all, sev, q);
                if !rule.is_empty() { l.retain(|f| f.rule == rule) }
                let rows = p["rows"] == true;
                json!({"summary": summary(&pro, x, now_ms()), "total": l.len(), "offset": offset, "findings": l.iter().skip(offset).take(limit).map(|f| if rows { fd::row_json(f) } else { fd::to_json(f) }).collect::<Vec<_>>()})
            }).unwrap_or(Value::Null))
        }
        "kicad.checks.set" => {
            let fs: Vec<Finding> = p["findings"].as_array().into_iter().flatten().map(fd::from_json).collect();
            let n = fs.len();
            set_findings(hub, &pro, src, fs, p["meta"].clone()).await?;
            Ok(json!({"project": pro, "source": src, "findings": n}))
        }
        "kicad.checks.focus" => {
            let f = find(&pro, p["id"].as_str().unwrap_or("")).ok_or("no such finding (the checks ran again: list them anew)")?;
            let mut sel = fd::select_params(&f);
            sel["project"] = json!(pro);
            let empty = ["references", "pins", "nets"].iter().all(|k| sel[*k].as_array().map_or(true, |a| a.is_empty()));
            if empty {
                // nothing with a name (a board edge, a drawing): show the editor
                let shown = embed::show_for(&pro, sel["show"].as_str().unwrap_or("PCB"), "").await.is_ok();
                return Ok(json!({"project": pro, "id": f.id, "kicad": shown, "select": sel}))
            }
            let mut out = super::xprobe::select(hub, &sel).await?;
            out["id"] = json!(f.id); out["select"] = sel;
            Ok(out)
        }
        "kicad.checks.fix" => {
            let f = find(&pro, p["id"].as_str().unwrap_or("")).ok_or("no such finding (the checks ran again: list them anew)")?;
            Ok(json!({"project": pro, "id": f.id, "text": fd::fix_message(&f, &pro)}))
        }
        _ => Err("unknown method".into()),
    }
}

/// The Checks tab of the management panels (kicad/mgmt.rs): a client shows the project, so its checks are kept live and published.
pub struct ChecksPanel;
impl super::mgmt::Panel for ChecksPanel {
    fn id(&self) -> &'static str { "checks" }
    fn label(&self) -> &'static str { "Checks" }
    fn watch(&self, hub: &Arc<Hub>, project: &str) {
        let (hub, p) = (hub.clone(), json!({"project": project}));
        tokio::spawn(async move { if let Ok(pro) = named(&hub, &p).await { publish(&hub, &pro).await } });
    }
}

pub struct ChecksRpc;
impl RpcHandler for ChecksRpc {
    fn methods(&self) -> &'static [&'static str] { &["kicad.checks.watch", "kicad.checks.run", "kicad.checks.list", "kicad.checks.focus", "kicad.checks.fix", "kicad.checks.set"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if method == "kicad.checks.focus" && crate::copy::on() { return vec![json!({"t":"reply","id":id,"ok":false,"error":"a hub on a copied log (BACKPLANE_COPY=1) does not control GUI KiCad"})] }
            match request(hub, method, p).await {
                Ok(v) => vec![json!({"t":"reply","id":id,"ok":true,"result":v})],
                Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":e})],
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn revisions_follow_saves_of_any_sheet_and_ignore_copies() {
        let d = crate::plat::tmp::dir("checkrun");
        std::fs::create_dir_all(d.join("sheets")).unwrap(); std::fs::create_dir_all(d.join("backups")).unwrap();
        for (f, t) in [("a.kicad_pro", "{}"), ("a.kicad_pcb", "(kicad_pcb)"), ("a.kicad_sch", "(kicad_sch)"), ("sheets/io.kicad_sch", "(kicad_sch)"), ("backups/old.kicad_sch", "x")] { std::fs::write(d.join(f), t).unwrap() }
        let s = |p: &str| d.join(p).to_string_lossy().to_string();
        let files = project::Files { pro: s("a.kicad_pro"), pcb: s("a.kicad_pcb"), sch: s("a.kicad_sch"), all: vec![] };
        let (e0, d0) = disk_revs(&files);
        assert_eq!(disk_revs(&files), (e0.clone(), d0.clone()));
        std::fs::write(d.join("backups/old.kicad_sch"), "changed").unwrap();
        assert_eq!(disk_revs(&files), (e0.clone(), d0.clone()), "a backup copy is no part of the design");
        std::fs::write(d.join("sheets/io.kicad_sch"), "(kicad_sch (x))").unwrap();
        let (e1, d1) = disk_revs(&files); assert_ne!(e1, e0); assert_ne!(d1, d0, "the schematic is in DRC's parity check");
        std::fs::write(d.join("a.kicad_pcb"), "(kicad_pcb (x))").unwrap();
        let (e2, d2) = disk_revs(&files); assert_eq!(e2, e1, "a board save leaves ERC fresh"); assert_ne!(d2, d1);
        let no_board = project::Files { pcb: String::new(), ..files.clone() };
        assert_eq!(disk_revs(&no_board).1, "");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn another_source_joins_the_one_list() {
        register_source("fabtest", "Fab rules");
        let mut p = Proj::default();
        p.erc.findings = vec![Finding { id: "erc:1".into(), source: "erc".into(), severity: "error".into(), ..Default::default() }];
        let f = Finding { id: "fabtest:1".into(), source: "fabtest".into(), severity: "warning".into(), rule: "min_drill".into(), title: "Drill 0.15 mm under the fab's 0.2 mm".into(), anchors: vec![fd::Anchor { doc: "pcb".into(), uuid: "v1".into(), ..Default::default() }], ..Default::default() };
        p.erc.sched.disk_rev = "e1".into(); p.drc.sched.disk_rev = "d1".into();
        p.ext.insert("fabtest".into(), Ext { counts: fd::counts(&[f.clone()]), findings: vec![f], rev: "e1d1".into(), meta: json!({"label": "Fab rules", "judged": false}), ..Default::default() });
        let all = all_findings(&p, ""); assert_eq!(all.len(), 2); assert_eq!(all_findings(&p, "fabtest").len(), 1); assert!(all_findings(&p, "drc").is_empty());
        let v = summary("/p/a.kicad_pro", &p, 0);
        assert_eq!(v["sources"]["fabtest"]["label"], "Fab rules"); assert_eq!(v["sources"]["fabtest"]["counts"]["warning"], 1); assert_eq!(v["sources"]["fabtest"]["state"], "fresh");
        p.drc.sched.disk_rev = "d2".into();
        assert_eq!(summary("/p/a.kicad_pro", &p, 0)["sources"]["fabtest"]["state"], "stale", "the design changed since the set");
        let pj = panel_json(&p, v); assert_eq!(pj["total"], 2); assert_eq!(pj["badge"]["n"], 2);
        assert!(pj["groups"].as_array().unwrap().iter().any(|g| g["source"] == "fabtest" && g["rule"] == "min_drill"));
        assert!(known_source("fabtest") && known_source("drc") && !known_source("nope"));
    }
    #[test] fn only_the_design_goes_to_another_machine() {
        let d = crate::plat::tmp::dir("checkrun-files");
        std::fs::create_dir_all(d.join("sheets")).unwrap(); std::fs::create_dir_all(d.join("backups")).unwrap(); std::fs::create_dir_all(d.join("mfg")).unwrap();
        for f in ["a.kicad_pro", "a.kicad_pcb", "a.kicad_dru", "a.kicad_sch", "sheets/io.kicad_sch", "backups/old.kicad_sch", "fp-lib-table", "mfg/a.zip", "a.kicad_prl", "notes.md"] { std::fs::write(d.join(f), "x").unwrap() }
        let s = |p: &str| d.join(p).to_string_lossy().to_string();
        let files = project::Files { pro: s("a.kicad_pro"), pcb: s("a.kicad_pcb"), sch: s("a.kicad_sch"), all: vec![] };
        let mut got = design_files(&d, &files); got.sort();
        assert_eq!(got, ["a.kicad_dru", "a.kicad_pcb", "a.kicad_pro", "a.kicad_sch", "fp-lib-table", "sheets/io.kicad_sch"]);
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn the_state_word() {
        let mut x = Src::default();
        assert_eq!(state_word(&x), "none");
        x.had = true; x.sched.ran_rev = "a".into(); x.sched.disk_rev = "a".into(); assert_eq!(state_word(&x), "fresh");
        x.sched.disk_rev = "b".into(); assert_eq!(state_word(&x), "stale");
        x.sched.running_since = 5; assert_eq!(state_word(&x), "running");
        x.sched.running_since = 0; x.state = "failed".into(); assert_eq!(state_word(&x), "failed");
    }
}

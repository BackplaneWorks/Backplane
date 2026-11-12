//! The live DRC/ERC state of each project a client looks at (the desktop's checks panel and chip, the phones beside their viewers).
//! Per project and check: state (none / running / fresh / stale / failed), counts by severity, when it ran, how long, the design
//! revision it ran on, and its findings (kicad/findings.rs). Published as info `kicad.checks.<digest>` (the summary, small) and served
//! by RPC (the list):
//! - `kicad.checks.watch {project}`: the project's checks are kept live while some client asks at least every WATCH_MS; answers the summary.
//! - `kicad.checks.run {project, source?: "drc"|"erc"}`: run now (the Run button).
//! - `kicad.checks.list {project, source?, severity?, q?, offset?, limit?}`: {summary, total, findings: [...]}, errors first.
//! - `kicad.checks.focus {project, id}`: cross-probe the finding into the open KiCad (its items selected and zoomed, in the board for
//!   DRC, the schematic for ERC: `editor.kicad.select` with `show`) and light it in the viewers.
//! - `kicad.checks.fix {project, id}`: {text}: the message "Fix with agent" sends.
//! Source: `kicad-cli pcb drc` / `sch erc` (JSON) on the saved files, one run at a time in the whole hub (a large board's DRC takes
//! gigabytes), at low priority, cancelled when a newer save arrives. When it runs is findings::next (saves debounced, held while the
//! person works in KiCad or an agent's turn holds the document). Results are kept in `<home>/cache/checks/` (never beside the design),
//! so a restart shows the last result at once, fresh or stale.
use super::{cad, embed, findings::{self as fd, Counts, Do, Finding, Sched}, ipc, kserve as ks, project};
use crate::{api::{Caller, Fut, RpcHandler}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::{collections::HashMap, path::{Path, PathBuf}, sync::{Arc, Mutex, OnceLock, Weak}, time::Duration};

/// A project stays live this long after a client last asked (clients ask again while their panel or chip shows).
const WATCH_MS: u64 = 10 * 60_000;
const TICK_MS: u64 = 1000;
const RUN_SECS: u64 = 600;
pub const SOURCES: [&str; 2] = ["erc", "drc"];

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
}
#[derive(Default)]
struct Proj { files: project::Files, drc: Src, erc: Src, asked: u64, loaded: bool }
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
/// One run at a time in the whole hub.
static SLOT: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

pub fn start(hub: &Arc<Hub>) {
    *HUB.lock().unwrap() = Some(Arc::downgrade(hub));
    let _ = WAKE.set(tokio::sync::Notify::new());
    hub.register_rpc(Arc::new(ChecksRpc));
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
    let one = |s: &Src| json!({"rev": s.sched.ran_rev, "at": s.at, "took": s.took, "had": s.had, "failed": if s.state == "failed" { s.reason.clone() } else { String::new() }, "findings": s.findings.iter().map(fd::to_json).collect::<Vec<_>>()});
    let v = json!({"pro": pro, "drc": one(&p.drc), "erc": one(&p.erc)});
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
        if !o["had"].as_bool().unwrap_or(false) { continue }
        src.findings = o["findings"].as_array().into_iter().flatten().map(fd::from_json).collect();
        src.counts = fd::counts(&src.findings);
        src.sched.ran_rev = o["rev"].as_str().unwrap_or("").into();
        src.at = o["at"].as_u64().unwrap_or(0); src.took = o["took"].as_u64().unwrap_or(0); src.had = true;
        let failed = o["failed"].as_str().unwrap_or("");
        if !failed.is_empty() { src.state = "failed".into(); src.reason = failed.into(); }
    }
}

// Summary ------------------------------------------------------------------------------------------------------------------------

fn src_json(p: &Proj, s: &str, now: u64) -> Value {
    let x = if s == "drc" { &p.drc } else { &p.erc };
    if !p.has(s) { return Value::Null }
    let st = state_word(x);
    let running = x.sched.running_since > 0;
    json!({"state": st, "reason": x.reason, "running": running, "fraction": if running { fd::fraction(now.saturating_sub(x.sched.running_since), x.took) } else { -1.0 },
        "counts": x.counts.to_json(), "at": x.at, "took_ms": x.took, "rev": x.sched.ran_rev, "has_result": x.had})
}
/// none (no result yet), running, fresh (the result is of the design on disk), stale (the design changed since), failed.
fn state_word(x: &Src) -> &'static str {
    if x.sched.running_since > 0 { "running" } else if x.state == "failed" { "failed" } else if !x.had { "none" } else if x.sched.ran_rev != x.sched.disk_rev && !x.sched.disk_rev.is_empty() { "stale" } else { "fresh" }
}
fn summary(pro: &str, p: &Proj, now: u64) -> Value {
    let pick = |s: &str| { let x = if s == "drc" { &p.drc } else { &p.erc }; p.has(s).then(|| (if x.state == "failed" && x.sched.running_since == 0 { "failed" } else { "ok" }, x.counts, x.had)) };
    json!({"project": pro, "pcb": p.files.pcb, "sch": p.files.sch, "drc": src_json(p, "drc", now), "erc": src_json(p, "erc", now), "chip": fd::chip(pick("drc"), pick("erc"))})
}
async fn publish(hub: &Arc<Hub>, pro: &str) {
    let v = { let m = state().lock().unwrap(); match m.get(pro) { Some(p) => summary(pro, p, now_ms()), None => return } };
    hub.set_info(&info_key(pro), &v.to_string()).await;
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
        m.retain(|_, p| now.saturating_sub(p.asked) < WATCH_MS || p.drc.sched.running_since > 0 || p.erc.sched.running_since > 0);
        m.iter().map(|(k, p)| (k.clone(), p.files.clone())).collect()
    };
    if pros.is_empty() { return 60_000 }
    let mut sleep = TICK_MS * 5;
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
                x.sched.agent_since = agent; x.sched.person_at = person;
                match fd::next(now, &x.sched) {
                    Do::Run => { x.sched.demand = false; x.sched.running_since = now; x.sched.running_rev = rev.clone(); let c = Arc::new(tokio::sync::Notify::new()); x.cancel = Some(c.clone()); acts.push((s, rev.clone(), c)); changed = true; }
                    Do::Cancel => { if let Some(c) = &x.cancel { c.notify_one() } }
                    Do::Wait(ms) => sleep = sleep.min(ms.max(100)),
                    Do::Nothing => {}
                }
                if x.sched.running_since > 0 { sleep = sleep.min(TICK_MS) }
            }
        }
        if changed { publish(hub, &pro).await }
        for (s, rev, c) in acts { let (h, p, f) = (hub.clone(), pro.clone(), files.clone()); tokio::spawn(async move { run(&h, &p, &f, s, &rev, c).await }); }
    }
    sleep
}

/// What one run found: its findings, or why it failed (None: cancelled).
type Outcome = Option<Result<Vec<Finding>, String>>;

async fn run(hub: &Arc<Hub>, pro: &str, f: &project::Files, s: &'static str, rev: &str, cancel: Arc<tokio::sync::Notify>) {
    let slot = tokio::select! { g = SLOT.get_or_init(|| tokio::sync::Mutex::new(())).lock() => g, _ = cancel.notified() => { return finish(hub, pro, s, rev, None, 0).await } };
    // waiting for the slot is not running: the bar's clock starts now
    if let Some(p) = state().lock().unwrap().get_mut(pro) { p.src(s).sched.running_since = now_ms() }
    publish(hub, pro).await;
    let t0 = now_ms();
    let o = run_cli(hub, pro, f, s, &cancel).await;
    let took = now_ms() - t0;
    drop(slot);
    finish(hub, pro, s, rev, o, took).await
}

async fn run_cli(hub: &Arc<Hub>, pro: &str, f: &project::Files, s: &str, cancel: &tokio::sync::Notify) -> Outcome {
    let cli = ipc::runtime(hub).await.cli;
    if cli.is_empty() { return Some(Err("KiCad is not installed on this machine (no kicad-cli)".into())) }
    let dir = cache_dir(hub).join(ks::digest(pro));
    let _ = std::fs::create_dir_all(&dir);
    let c = cad::CHECKS.iter().find(|c| c.id == s)?;
    let argv = cad::fill(c.argv, &[("cli", &cli), ("pcb", &f.pcb), ("sch", &f.sch), ("out", &dir.to_string_lossy())]);
    let report = dir.join(c.report);
    let _ = std::fs::remove_file(&report);
    let mut cmd = low_priority(&argv);
    cmd.kill_on_drop(true).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let child = match cmd.spawn() { Ok(c) => c, Err(e) => return Some(Err(format!("could not start kicad-cli: {e}"))) };
    let res = tokio::select! {
        r = tokio::time::timeout(Duration::from_secs(RUN_SECS), child.wait_with_output()) => r,
        _ = cancel.notified() => return None, // the child is killed as it is dropped
    };
    let o = match res { Ok(Ok(o)) => o, Ok(Err(e)) => return Some(Err(format!("kicad-cli: {e}"))), Err(_) => return Some(Err(format!("{} took longer than {RUN_SECS} s", s.to_uppercase()))) };
    let text = std::fs::read_to_string(&report).unwrap_or_default();
    let parsed = if s == "drc" { fd::drc(&text) } else { fd::erc(&text) };
    match parsed {
        Some(fs) => Some(Ok(fs)),
        None => {
            let why = String::from_utf8_lossy(&o.stderr).trim().lines().last().unwrap_or("").to_string();
            let why = if why.is_empty() { String::from_utf8_lossy(&o.stdout).trim().lines().last().unwrap_or("").to_string() } else { why };
            Some(Err(format!("kicad-cli {} failed (exit {}){}", s, o.status.code().unwrap_or(-1), if why.is_empty() { String::new() } else { format!(": {why}") })))
        }
    }
}
/// kicad-cli at a low priority: `nice -n 15` on Unix, below-normal on Windows. Its folder is the root: every path is absolute.
fn low_priority(argv: &[String]) -> tokio::process::Command {
    #[cfg(unix)] let mut c = { let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, "nice"); c.arg("-n").arg("15").args(argv); c };
    #[cfg(windows)] let mut c = { const BELOW_NORMAL: u32 = 0x0000_4000; let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, &argv[0]); c.args(&argv[1..]); c.creation_flags(crate::plat::proc::NO_WINDOW | BELOW_NORMAL); c };
    c.current_dir(std::path::MAIN_SEPARATOR_STR);
    c
}

async fn finish(hub: &Arc<Hub>, pro: &str, s: &str, rev: &str, o: Outcome, took: u64) {
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
        if let Some(e) = m.get_mut(&pro) { for s in SOURCES { let (a, b) = (e.src(s), tmp.src(s)); if !a.had && b.had { std::mem::swap(a, b) } } }
        drop(m);
        wake();
    }
    Ok(pro)
}
fn with<T>(pro: &str, f: impl FnOnce(&Proj) -> T) -> Option<T> { state().lock().unwrap().get(pro).map(f) }
fn find(pro: &str, id: &str) -> Option<Finding> {
    with(pro, |p| p.drc.findings.iter().chain(p.erc.findings.iter()).find(|f| f.id == id).cloned()).flatten()
}

pub async fn request(hub: &Arc<Hub>, method: &str, p: &Value) -> Result<Value, String> {
    let pro = named(hub, p).await?;
    let src = p["source"].as_str().unwrap_or("");
    if !src.is_empty() && !SOURCES.contains(&src) { return Err(format!("unknown check {src}: drc or erc")) }
    match method {
        "kicad.checks.watch" => Ok(with(&pro, |x| summary(&pro, x, now_ms())).unwrap_or(Value::Null)),
        "kicad.checks.run" => {
            { let mut m = state().lock().unwrap(); if let Some(x) = m.get_mut(&pro) { for s in SOURCES { if (src.is_empty() || s == src) && x.has(s) { x.src(s).sched.demand = true } } } }
            wake();
            Ok(json!({"project": pro, "queued": true}))
        }
        "kicad.checks.list" => {
            let (sev, q) = (p["severity"].as_str().unwrap_or(""), p["q"].as_str().unwrap_or(""));
            let offset = p["offset"].as_u64().unwrap_or(0) as usize; let limit = p["limit"].as_u64().unwrap_or(500).min(2000) as usize;
            Ok(with(&pro, |x| {
                let all: Vec<Finding> = SOURCES.iter().filter(|s| src.is_empty() || **s == src).flat_map(|s| if *s == "drc" { x.drc.findings.clone() } else { x.erc.findings.clone() }).collect();
                let l = fd::filter(&all, sev, q);
                json!({"summary": summary(&pro, x, now_ms()), "total": l.len(), "offset": offset, "findings": l.iter().skip(offset).take(limit).map(|f| fd::to_json(f)).collect::<Vec<_>>()})
            }).unwrap_or(Value::Null))
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

pub struct ChecksRpc;
impl RpcHandler for ChecksRpc {
    fn methods(&self) -> &'static [&'static str] { &["kicad.checks.watch", "kicad.checks.run", "kicad.checks.list", "kicad.checks.focus", "kicad.checks.fix"] }
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
    #[test] fn the_state_word() {
        let mut x = Src::default();
        assert_eq!(state_word(&x), "none");
        x.had = true; x.sched.ran_rev = "a".into(); x.sched.disk_rev = "a".into(); assert_eq!(state_word(&x), "fresh");
        x.sched.disk_rev = "b".into(); assert_eq!(state_word(&x), "stale");
        x.sched.running_since = 5; assert_eq!(state_word(&x), "running");
        x.sched.running_since = 0; x.state = "failed".into(); assert_eq!(state_word(&x), "failed");
    }
}

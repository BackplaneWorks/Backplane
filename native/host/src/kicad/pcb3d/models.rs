//! A footprint's 3D model references to converted models, cached by content.
//!
//! 1. `Env` + `resolve`: a `(model "...")` path to a file (KiCad's variables, the board's folder, a STEP beside a `.wrl`).
//! 2. `key`: sha256 of the file's bytes, memoized per (path, dev, ino, size, mtime), so a board opened again hashes nothing.
//! 3. `Cache`: `<dir>/part-v1-<key>.glb` (mm, Z up, the converter's own format: freecad_gui.py's), put in place only by rename;
//!    a failure is `part-v1-<key>.fail` holding the message and is retried after a day.
//! 4. `convert`: a batch of (key, file) jobs. Hits are reported at once; the misses go to K = clamp(cores/4, 1, 8) workers and each
//!    result is reported the moment it is done (completion order). FreeCAD workers are one process each converting many files
//!    (files/scripts/freecad_batch.py); `Step2glb` runs one process per file on K threads; a `.wrl` is read natively (`wrl_to_glb`).
//!    Blocking, but `stop` is polled (every ~25 ms) and the workers are killed when it is set. A tool that fails a job hands it to the
//!    next tool of `tools`; only when all failed is the failure recorded and reported.
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};
use sha2::{Digest, Sha256};
use super::super::stepglb::{self, Mesher};

const BATCH_PY: &str = include_str!("../../files/scripts/freecad_batch.py");
const POLL: Duration = Duration::from_millis(25);
const FAIL_TTL: Duration = Duration::from_secs(24 * 3600);
const STALL_SECS: u64 = 600;
const ROUNDS: usize = 5;

// ---------- Env and resolution ----------

/// The variables a model path is expanded with.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Env { vars: BTreeMap<String, String> }

fn is_model_var(n: &str) -> Option<u32> {
    let rest = n.strip_prefix("KICAD")?.strip_suffix("_3DMODEL_DIR")?;
    if rest.is_empty() { Some(0) } else { rest.parse().ok() }
}
fn version_key(v: &str) -> Vec<u64> { v.split('.').map(|p| p.parse().unwrap_or(0)).collect() }

impl Env {
    pub fn empty() -> Env { Env::default() }
    pub fn with(mut self, k: &str, v: &str) -> Env { self.vars.insert(k.into(), v.into()); self }
    /// A variable; a `KICADn_3DMODEL_DIR` that is not set falls back to the highest-numbered other one that is (as KiCad's resolver does).
    pub fn get(&self, name: &str) -> Option<&str> {
        if let Some(v) = self.vars.get(name).filter(|v| !v.is_empty()) { return Some(v) }
        if is_model_var(name).is_some() || name == "KISYS3DMOD" {
            return self.vars.iter().filter(|(k, v)| is_model_var(k).is_some() && !v.is_empty()).max_by_key(|(k, _)| is_model_var(k)).map(|(_, v)| v.as_str());
        }
        None
    }
    /// Process environment, then KiCad's `kicad_common.json` (newest version first), then the default 3D model dirs.
    /// `kicad_cli` adds `<its dir>/../share/kicad/3dmodels` to the places looked in.
    pub fn system(kicad_cli: Option<&Path>) -> Env {
        let home = std::env::var("HOME").unwrap_or_default();
        let mut roots = vec![];
        if let Ok(x) = std::env::var("XDG_CONFIG_HOME") { if !x.is_empty() { roots.push(PathBuf::from(x).join("kicad")) } }
        roots.push(PathBuf::from(&home).join(".config/kicad"));
        roots.push(PathBuf::from(&home).join("Library/Preferences/kicad"));
        roots.push(PathBuf::from(&home).join(".var/app/org.kicad.KiCad/config/kicad"));
        let mut dirs: Vec<PathBuf> = ["/usr/share/kicad/3dmodels", "/usr/local/share/kicad/3dmodels", "/var/lib/flatpak/exports/share/kicad/3dmodels",
            "/var/lib/flatpak/app/org.kicad.KiCad/current/active/files/share/kicad/3dmodels", "/Applications/KiCad/KiCad.app/Contents/SharedSupport/3dmodels",
            "/Applications/KiCad.app/Contents/SharedSupport/3dmodels"].iter().map(PathBuf::from).collect();
        dirs.push(PathBuf::from(&home).join(".local/share/flatpak/app/org.kicad.KiCad/current/active/files/share/kicad/3dmodels"));
        if let Some(c) = kicad_cli.and_then(|c| c.parent()) { dirs.insert(0, c.join("../share/kicad/3dmodels")) }
        Env::build(std::env::vars().collect(), &roots, &dirs)
    }
    /// `process` wins over every `kicad_common.json` under the `config_roots` (`<root>/<version>/kicad_common.json`, newest first), which win over `dirs`.
    pub fn build(process: Vec<(String, String)>, config_roots: &[PathBuf], dirs: &[PathBuf]) -> Env {
        let mut vars = BTreeMap::new();
        for (k, v) in process { vars.insert(k, v); }
        let mut files: Vec<(Vec<u64>, PathBuf)> = vec![];
        for r in config_roots {
            for e in std::fs::read_dir(r).into_iter().flatten().flatten() {
                let p = e.path().join("kicad_common.json");
                if p.is_file() { files.push((version_key(&e.file_name().to_string_lossy()), p)) }
            }
        }
        files.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, p) in files {
            let Some(j) = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) else { continue };
            for (k, v) in j.pointer("/environment/vars").and_then(|v| v.as_object()).into_iter().flatten() {
                if let Some(s) = v.as_str() { if !s.is_empty() { vars.entry(k.clone()).or_insert_with(|| s.to_string()); } }
            }
        }
        if let Some(d) = dirs.iter().find(|d| d.is_dir()) {
            let d = d.to_string_lossy().into_owned();
            for n in 6..=10 { vars.entry(format!("KICAD{n}_3DMODEL_DIR")).or_insert_with(|| d.clone()); }
            vars.entry("KISYS3DMOD".into()).or_insert(d);
        }
        Env { vars }
    }
}

/// `${VAR}` and `$(VAR)` expanded (`KIPRJMOD` is the board's folder); None when a variable is not set.
pub fn expand(raw: &str, board_dir: &Path, env: &Env) -> Option<String> { expand_at(raw, board_dir, env, 0) }
fn expand_at(raw: &str, board_dir: &Path, env: &Env, depth: u8) -> Option<String> {
    if depth > 4 { return None }
    let mut out = String::new(); let mut rest = raw;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let close = match after.chars().next() { Some('{') => '}', Some('(') => ')', _ => { out.push('$'); rest = after; continue } };
        let end = after.find(close)?;
        let name = &after[1..end];
        if name == "KIPRJMOD" { out.push_str(&board_dir.to_string_lossy()) }
        else { out.push_str(&expand_at(env.get(name)?, board_dir, env, depth + 1)?) }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// A model reference to the file to convert: variables expanded, a relative path from the board's folder (else under the 3D model dir),
/// a STEP of the same stem preferred over a `.wrl` (as kicad-cli's `--subst-models`), else the file as named. None: no such file.
pub fn resolve(raw: &str, board_dir: &Path, env: &Env) -> Option<PathBuf> {
    let s = expand(raw.trim(), board_dir, env)?;
    if s.is_empty() { return None }
    let p = PathBuf::from(&s);
    let mut cands = vec![];
    if p.is_absolute() { cands.push(p) } else {
        cands.push(board_dir.join(&p));
        if let Some(m) = env.get("KICAD10_3DMODEL_DIR") { cands.push(Path::new(m).join(&p)) }
    }
    for c in cands {
        if is_wrl(&c) { for e in ["step", "stp", "STEP", "STP"] { let s = c.with_extension(e); if s.is_file() { return Some(s) } } }
        if c.is_file() { return Some(c) }
    }
    None
}
fn is_wrl(p: &Path) -> bool { p.extension().map_or(false, |e| e.eq_ignore_ascii_case("wrl")) }

// ---------- content key ----------

type Stamp = (PathBuf, u64, u64, u64, i128);
static KEYS: Mutex<Option<HashMap<Stamp, String>>> = Mutex::new(None);
static HASHED: AtomicUsize = AtomicUsize::new(0);
/// How many files `key` has read so far (a test's proof of memoization).
pub fn hashed() -> usize { HASHED.load(Ordering::Relaxed) }

fn stamp(p: &Path) -> std::io::Result<Stamp> {
    let m = std::fs::metadata(p)?;
    #[cfg(unix)] let (dev, ino) = { use std::os::unix::fs::MetadataExt; (m.dev(), m.ino()) };
    #[cfg(not(unix))] let (dev, ino) = (0u64, 0u64);
    let ns = m.modified().ok().map(|t| match t.duration_since(SystemTime::UNIX_EPOCH) { Ok(d) => d.as_nanos() as i128, Err(e) => -(e.duration().as_nanos() as i128) }).unwrap_or(0);
    Ok((p.to_path_buf(), dev, ino, m.len(), ns))
}
/// The model's content key: sha256 of its bytes, hex. Memoized by (path, dev, ino, size, mtime_ns) for the process.
pub fn key(p: &Path) -> std::io::Result<String> {
    let st = stamp(p)?;
    if let Some(k) = KEYS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&st)) { return Ok(k.clone()) }
    let bytes = std::fs::read(p)?;
    HASHED.fetch_add(1, Ordering::Relaxed);
    let k: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    KEYS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(st, k.clone());
    Ok(k)
}

// ---------- cache ----------

/// The converted models on disk. `Cache::standard()` is `stepglb::cache_dir()/parts`; tests give a folder of their own.
#[derive(Clone, Debug)]
pub struct Cache { pub dir: PathBuf, pub fail_ttl: Duration }
impl Cache {
    pub fn new(dir: impl Into<PathBuf>) -> Cache { Cache { dir: dir.into(), fail_ttl: FAIL_TTL } }
    pub fn standard() -> Cache { Cache::new(format!("{}/parts", stepglb::cache_dir())) }
    pub fn glb(&self, key: &str) -> PathBuf { self.dir.join(format!("part-v1-{key}.glb")) }
    pub fn fail_file(&self, key: &str) -> PathBuf { self.dir.join(format!("part-v1-{key}.fail")) }
    /// What the cache already knows of a model: its GLB, or a failure not yet a day old. None: convert it.
    pub fn lookup(&self, key: &str) -> Option<Result<PathBuf, String>> {
        let g = self.glb(key);
        if stepglb::have(&g.to_string_lossy()) { return Some(Ok(g)) }
        let f = self.fail_file(key);
        let age = std::fs::metadata(&f).ok()?.modified().ok().and_then(|t| t.elapsed().ok());
        match age { Some(a) if a < self.fail_ttl => Some(Err(std::fs::read_to_string(&f).unwrap_or_default())), _ => { let _ = std::fs::remove_file(&f); None } }
    }
    /// A finished file into place in one rename (never a partial file under the final name).
    pub fn publish(&self, key: &str, from: &Path) -> Result<PathBuf, String> {
        let to = self.glb(key);
        let r = stepglb::publish(stepglb::Ran::default(), from, &to.to_string_lossy());
        if r.code == 0 { let _ = std::fs::remove_file(self.fail_file(key)); Ok(to) } else { Err(r.out) }
    }
    pub fn record_fail(&self, key: &str, msg: &str) { let _ = std::fs::create_dir_all(&self.dir); let _ = std::fs::write(self.fail_file(key), msg); }
}

// ---------- batch conversion ----------

/// Worker count: clamp(cores / 4, 1, 8), at most `jobs`.
pub fn workers(jobs: usize) -> usize {
    #[cfg(test)] { let n = TEST_WORKERS.load(Ordering::Relaxed); if n > 0 { return n.min(jobs.max(1)) } }
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    (cores / 4).clamp(1, 8).min(jobs.max(1))
}

/// Tests: a fixed worker count (a runner with 4 cores gets one worker, and a test of completion order needs two).
#[cfg(test)] static TEST_WORKERS: AtomicUsize = AtomicUsize::new(0);
type Job = (String, PathBuf);
type Failed = Vec<(Job, String)>;

/// Converts every (key, file) not in the cache and reports each (`on_ready(key, glb path or why not)`) the moment it is known: hits
/// first, then misses in completion order. Blocks until done or `stop` is set (workers are then killed; unfinished jobs are not reported).
pub fn convert<F: Fn(&str, Result<PathBuf, String>) + Send + Sync>(cache: &Cache, jobs: Vec<Job>, tools: &[Mesher], stop: &AtomicBool, on_ready: F) {
    convert_with(Opts::default(), cache, jobs, tools, stop, &on_ready)
}

/// How a conversion runs: `workers` FreeCAD processes at most (None: `workers()`), each `nice` lower. A prewarm is one process at 19.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opts { pub workers: Option<usize>, pub nice: i32 }
impl Default for Opts { fn default() -> Self { Opts { workers: None, nice: 10 } } }
impl Opts { pub fn background() -> Opts { Opts { workers: Some(1), nice: 19 } } }

/// The models being converted now, in this process: a second caller of one waits for the first instead of starting FreeCAD for it again
/// (a prewarm and the 3D view asking at once).
fn inflight() -> &'static (std::sync::Mutex<std::collections::HashSet<String>>, std::sync::Condvar) {
    static I: std::sync::OnceLock<(std::sync::Mutex<std::collections::HashSet<String>>, std::sync::Condvar)> = std::sync::OnceLock::new();
    I.get_or_init(Default::default)
}
fn release(keys: &[String]) {
    let (m, cv) = inflight(); let mut s = m.lock().unwrap_or_else(|e| e.into_inner());
    for k in keys { s.remove(k); }
    cv.notify_all();
}
/// The keys this call converts: let go as each is reported, and all of them when the call ends however it ends.
struct Claim(Vec<String>);
impl Drop for Claim { fn drop(&mut self) { release(&self.0) } }
/// Whether `key` is still being converted by another caller (waits up to `ms` for it to finish).
fn busy(key: &str, ms: u64) -> bool {
    let (m, cv) = inflight(); let s = m.lock().unwrap_or_else(|e| e.into_inner());
    if !s.contains(key) { return false }
    let (s, _) = cv.wait_timeout(s, Duration::from_millis(ms)).unwrap_or_else(|e| e.into_inner());
    s.contains(key)
}

/// `convert` with its options (`Opts`).
pub fn convert_with(opts: Opts, cache: &Cache, jobs: Vec<Job>, tools: &[Mesher], stop: &AtomicBool, on_ready: &(dyn Fn(&str, Result<PathBuf, String>) + Send + Sync)) {
    let mut seen = std::collections::HashSet::new();
    let mut misses: Vec<Job> = vec![];
    for (k, p) in jobs {
        if !seen.insert(k.clone()) { continue }
        match cache.lookup(&k) { Some(r) => on_ready(&k, r), None => misses.push((k, p)) }
    }
    if misses.is_empty() || stop.load(Ordering::Relaxed) { return }
    let (mut mine, mut theirs): (Vec<Job>, Vec<Job>) = (vec![], vec![]);
    { let (m, _) = inflight(); let mut s = m.lock().unwrap_or_else(|e| e.into_inner());
      for j in misses { if s.insert(j.0.clone()) { mine.push(j) } else { theirs.push(j) } } }
    let _claim = Claim(mine.iter().map(|j| j.0.clone()).collect());
    let mine_ready = |k: &str, r: Result<PathBuf, String>| { release(&[k.to_string()]); on_ready(k, r) };
    std::thread::scope(|s| {
        // another caller's models: each waited for, then taken from the cache; one it gave up (stopped) is converted here after all
        s.spawn(|| {
            let mut left = vec![];
            for (k, p) in theirs {
                while busy(&k, 100) { if stop.load(Ordering::Relaxed) { return } }
                match cache.lookup(&k) { Some(r) => on_ready(&k, r), None => left.push((k, p)) }
            }
            if !left.is_empty() && !stop.load(Ordering::Relaxed) { convert_with(opts, cache, left, tools, stop, on_ready) }
        });
        run(opts, cache, mine, tools, stop, &mine_ready);
    });
}

/// The models not in the cache, converted: native VRML here, the rest by each tool in turn.
fn run(opts: Opts, cache: &Cache, mut misses: Vec<Job>, tools: &[Mesher], stop: &AtomicBool, on_ready: &(dyn Fn(&str, Result<PathBuf, String>) + Sync)) {
    if misses.is_empty() || stop.load(Ordering::Relaxed) { return }
    let _ = std::fs::create_dir_all(&cache.dir);
    misses.sort_by_key(|(_, p)| std::cmp::Reverse(std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)));
    let (wrl, mut rest): (Vec<Job>, Vec<Job>) = misses.into_iter().partition(|(_, p)| is_wrl(p));
    std::thread::scope(|s| {
        s.spawn(move || for (k, p) in wrl {
            if stop.load(Ordering::Relaxed) { return }
            let r = std::fs::read(&p).map_err(|e| e.to_string()).and_then(|b| wrl_to_glb(&String::from_utf8_lossy(&b)))
                .and_then(|g| { let t = cache.dir.join(format!(".{k}.{}.wrl.tmp", std::process::id())); std::fs::write(&t, g).map_err(|e| e.to_string())?; cache.publish(&k, &t) });
            if let Err(e) = &r { cache.record_fail(&k, e) }
            on_ready(&k, r);
        });
        let mut last = stepglb::NO_MESHER.to_string(); let mut tried = false;
        for t in tools {
            if rest.is_empty() || stop.load(Ordering::Relaxed) { break }
            let failed = match t {
                Mesher::FreeCad { gui, cmd } => {
                    let mut failed: Failed = vec![];
                    let mut left = std::mem::take(&mut rest);
                    for (mode, prog) in [(true, gui), (false, cmd)] {
                        if prog.is_empty() || left.is_empty() { continue }
                        failed = freecad(opts, cache, mode, prog, left, stop, on_ready);
                        left = failed.iter().map(|(j, _)| j.clone()).collect();
                    }
                    failed
                }
                Mesher::Step2glb(tool) => step2glb(cache, tool, std::mem::take(&mut rest), stop, on_ready),
            };
            tried = true;
            if let Some((_, m)) = failed.last() { last = m.clone() }
            rest = failed.into_iter().map(|(j, _)| j).collect();
        }
        if stop.load(Ordering::Relaxed) { return }
        for (k, _) in rest {
            if tried { cache.record_fail(&k, &last) }
            on_ready(&k, Err(last.clone()));
        }
    });
}

fn nice_cmd(program: &str, nice: i32) -> std::process::Command {
    let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, "nice");
    c.args(["-n", &nice.clamp(0, 19).to_string(), program]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).current_dir("/");
    // a group of its own: FreeCAD's AppImage forks a helper that holds its FUSE mount, and a stop must take that down too (`end`)
    #[cfg(unix)] { use std::os::unix::process::CommandExt; c.process_group(0); }
    c
}
/// A worker ended: first left to exit by itself (it may be writing its last model), then its whole group asked to stop, then made to.
/// Killing only the child left the AppImage's mount helper behind, one stale FUSE mount a worker.
fn end(kids: &mut [std::process::Child], grace: Duration) {
    let t = Instant::now();
    while t.elapsed() < grace && kids.iter_mut().any(|c| matches!(c.try_wait(), Ok(None))) { std::thread::sleep(POLL) }
    for sig in [15, 9] { // SIGTERM, then SIGKILL
        let live: Vec<u32> = kids.iter_mut().filter_map(|c| matches!(c.try_wait(), Ok(None)).then(|| c.id())).collect();
        #[cfg(not(unix))] let _ = &live;
        #[cfg(unix)] for pid in &live { unsafe { libc::kill(-(*pid as i32), sig); } }
        #[cfg(not(unix))] if sig == 9 { for c in kids.iter_mut() { let _ = c.kill(); } }
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(2) && kids.iter_mut().any(|c| matches!(c.try_wait(), Ok(None))) { std::thread::sleep(POLL) }
    }
    for c in kids.iter_mut() { let _ = c.wait(); }
    // the group's helper outlives its leader by a moment: its own group, so one more word to the group on the way out
    #[cfg(unix)] for c in kids.iter() { unsafe { libc::kill(-(c.id() as i32), 15); } }
}

fn step2glb(cache: &Cache, tool: &str, jobs: Vec<Job>, stop: &AtomicBool, on_ready: &(dyn Fn(&str, Result<PathBuf, String>) + Sync)) -> Failed {
    let next = AtomicUsize::new(0); let failed = Mutex::new(vec![]);
    let jobs = &jobs;
    std::thread::scope(|s| for _ in 0..workers(jobs.len()) {
        s.spawn(|| loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            let Some((k, p)) = jobs.get(i) else { return };
            if stop.load(Ordering::Relaxed) { return }
            let tmp = cache.dir.join(format!(".{k}.{}.s2g.tmp", std::process::id()));
            let r = stepglb::convert(&[Mesher::Step2glb(tool.into())], &p.to_string_lossy(), &tmp.to_string_lossy(), Some(stepglb::NORMAL), true);
            let r = if r.code == 0 { cache.publish(k, &tmp) } else { Err(r.out.trim().to_string()) };
            let _ = std::fs::remove_file(&tmp);
            match r { Ok(g) => on_ready(k, Ok(g)), Err(e) => failed.lock().unwrap_or_else(|e| e.into_inner()).push(((k.clone(), p.clone()), e)) }
        });
    });
    failed.into_inner().unwrap_or_default()
}

/// One FreeCAD flavour over `jobs`: K worker processes sharing a job file, outputs polled and published as they appear.
fn freecad(opts: Opts, cache: &Cache, gui: bool, program: &str, jobs: Vec<Job>, stop: &AtomicBool, on_ready: &(dyn Fn(&str, Result<PathBuf, String>) + Sync)) -> Failed {
    let Ok(stage) = stepglb::stage(&cache.dir.join("batch").to_string_lossy(), "b-") else { return jobs.into_iter().map(|j| (j, "cannot make a folder in the model cache".into())).collect() };
    let py = stage.join("batch.py"); let list = stage.join("jobs.txt");
    let outs: Vec<PathBuf> = jobs.iter().map(|(k, _)| stage.join(format!("{k}.glb"))).collect();
    let text: String = jobs.iter().zip(&outs).map(|((_, p), o)| format!("{}\t{}\n", p.display(), o.display())).collect();
    let tol = stepglb::NORMAL.linear.max(0.25).to_string();
    let mut failed: Failed = vec![];
    if std::fs::write(&py, BATCH_PY).is_err() || std::fs::write(&list, &text).is_err() {
        let _ = std::fs::remove_dir_all(&stage);
        return jobs.into_iter().map(|j| (j, "cannot write the job file".into())).collect();
    }
    let mut pending: Vec<usize> = (0..jobs.len()).collect();
    let spawn = |n: usize| -> Vec<std::process::Child> {
        (0..n).filter_map(|_| {
            let mut c = nice_cmd(program, opts.nice);
            if gui { stepglb::soft_gl(&mut c); c.arg(&py).env("QT_QPA_PLATFORM", "offscreen").env("BP_GUI", "1"); }
            else { c.args(["-c", "import os; exec(open(os.environ['BP_PY']).read())"]).env("BP_PY", &py); }
            c.env("BP_JOBS", &list).env("BP_TOL", &tol).spawn().ok()
        }).collect()
    };
    let mut rounds = 0;
    'rounds: while !pending.is_empty() && rounds < ROUNDS {
        rounds += 1;
        let mut kids = spawn(opts.workers.unwrap_or_else(|| workers(pending.len())).clamp(1, pending.len().max(1)));
        if kids.is_empty() { for i in pending.drain(..) { failed.push((jobs[i].clone(), format!("could not start {program}"))) } break }
        let mut progress = Instant::now(); let mut got = 0;
        loop {
            let stopped = stop.load(Ordering::Relaxed);
            pending.retain(|&i| {
                let (k, _) = &jobs[i]; let o = &outs[i];
                if o.is_file() {
                    match cache.publish(k, o) { Ok(g) => on_ready(k, Ok(g)), Err(e) => failed.push((jobs[i].clone(), e)) }
                    got += 1; progress = Instant::now(); return false;
                }
                let n = std::fs::read_to_string(format!("{}.n", o.display())).unwrap_or_default();
                if n.starts_with("0 ") {
                    failed.push((jobs[i].clone(), format!("FreeCAD could not mesh {}: {}", jobs[i].1.display(), n.trim_start_matches("0 ").trim())));
                    got += 1; progress = Instant::now(); return false;
                }
                true
            });
            let alive = kids.iter_mut().any(|c| matches!(c.try_wait(), Ok(None)));
            let timed_out = progress.elapsed() > Duration::from_secs(stall_secs());
            if pending.is_empty() || stopped || timed_out || !alive {
                end(&mut kids, if stopped || timed_out { Duration::ZERO } else { Duration::from_secs(5) });
                if stopped { pending.clear(); break 'rounds }
                if !pending.is_empty() && !alive && !timed_out && got > 0 {
                    // a worker died on a model (a crash): that one was claimed and is lost; the unclaimed ones go to a fresh worker
                    pending.retain(|&i| {
                        let claimed = Path::new(&format!("{}.claim", outs[i].display())).exists();
                        if claimed { failed.push((jobs[i].clone(), format!("FreeCAD stopped while meshing {}", jobs[i].1.display()))) }
                        !claimed
                    });
                    // the final poll above already ran for outputs written just before the exit
                    continue 'rounds;
                }
                if timed_out { for i in pending.drain(..) { failed.push((jobs[i].clone(), "FreeCAD timed out".into())) } }
                break;
            }
            std::thread::sleep(POLL);
        }
        if !pending.is_empty() { for i in pending.drain(..) { failed.push((jobs[i].clone(), format!("FreeCAD stopped without meshing {}", jobs[i].1.display()))) } }
    }
    let _ = std::fs::remove_dir_all(&stage);
    failed
}
fn stall_secs() -> u64 { std::env::var("BP_MODELS_STALL_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(STALL_SECS) }

// ---------- native VRML2 (.wrl) ----------

#[derive(Debug, Clone)]
enum V { Nums(Vec<f64>), Node(Box<N>), Nodes(Vec<N>), Word(String) }
#[derive(Debug, Clone, Default)]
struct N { ty: String, fields: Vec<(String, V)> }
impl N { fn f(&self, n: &str) -> Option<&V> { self.fields.iter().find(|(k, _)| k == n).map(|(_, v)| v) }
    fn nums(&self, n: &str) -> Option<&[f64]> { match self.f(n) { Some(V::Nums(v)) => Some(v), _ => None } }
    fn node(&self, n: &str) -> Option<&N> { match self.f(n) { Some(V::Node(b)) => Some(b), _ => None } } }

struct Lex<'a> { t: Vec<&'a str>, at: usize, defs: HashMap<String, N> }
fn lex(s: &str) -> Vec<&str> {
    let b = s.as_bytes(); let mut out = vec![]; let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'#' { while i < b.len() && b[i] != b'\n' { i += 1 } }
        else if c.is_ascii_whitespace() || c == b',' { i += 1 }
        else if matches!(c, b'{' | b'}' | b'[' | b']') { out.push(&s[i..i + 1]); i += 1 }
        else if c == b'"' { let st = i; i += 1; while i < b.len() && b[i] != b'"' { i += 1 } i = (i + 1).min(b.len()); out.push(&s[st..i]) }
        else { let st = i; while i < b.len() && !b[i].is_ascii_whitespace() && !matches!(b[i], b',' | b'{' | b'}' | b'[' | b']' | b'#') { i += 1 } out.push(&s[st..i]) }
    }
    out
}
impl<'a> Lex<'a> {
    fn peek(&self) -> Option<&'a str> { self.t.get(self.at).copied() }
    fn num(&self) -> Option<f64> { self.peek().and_then(|t| t.parse().ok()) }
    /// a node starting at its type word (after any DEF name)
    fn node(&mut self, depth: u32) -> Option<N> {
        if depth > 64 { return None }
        let mut ty = self.peek()?; self.at += 1;
        let mut def = None;
        if ty == "DEF" { def = Some(self.peek()?.to_string()); self.at += 1; ty = self.peek()?; self.at += 1 }
        else if ty == "USE" { let n = self.peek()?; self.at += 1; return self.defs.get(n).cloned() }
        if self.peek()? != "{" { return None }
        self.at += 1;
        let mut node = N { ty: ty.to_string(), fields: vec![] };
        while let Some(t) = self.peek() {
            if t == "}" { self.at += 1; break }
            self.at += 1;
            let name = t.to_string();
            let v = self.value(depth + 1)?;
            node.fields.push((name, v));
        }
        if let Some(d) = def { self.defs.insert(d, node.clone()); }
        Some(node)
    }
    fn value(&mut self, depth: u32) -> Option<V> {
        match self.peek()? {
            "[" => {
                self.at += 1;
                let mut nums = vec![]; let mut nodes = vec![];
                loop {
                    match self.peek()? {
                        "]" => { self.at += 1; break }
                        _ => if let Some(x) = self.num() { self.at += 1; nums.push(x) } else if self.peek()?.starts_with('"') { self.at += 1 } else { nodes.push(self.node(depth + 1)?) }
                    }
                }
                Some(if nodes.is_empty() { V::Nums(nums) } else { V::Nodes(nodes) })
            }
            t if t.parse::<f64>().is_ok() => { let mut v = vec![]; while let Some(x) = self.num() { v.push(x); self.at += 1 } Some(V::Nums(v)) }
            "TRUE" | "FALSE" => { let w = self.peek()?.to_string(); self.at += 1; Some(V::Word(w)) }
            t if t.starts_with('"') => { self.at += 1; Some(V::Word(t.trim_matches('"').into())) }
            _ => Some(V::Node(Box::new(self.node(depth)?))),
        }
    }
}

type M = [[f64; 4]; 3];
const ID: M = [[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.]];
fn mul(a: &M, b: &M) -> M {
    let mut r = [[0.; 4]; 3];
    for i in 0..3 { for j in 0..4 { r[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j] + if j == 3 { a[i][3] } else { 0. }; } }
    r
}
fn transform(n: &N) -> M {
    let t = n.nums("translation").filter(|v| v.len() >= 3).map_or([0.; 3], |v| [v[0], v[1], v[2]]);
    let s = n.nums("scale").filter(|v| v.len() >= 3).map_or([1.; 3], |v| [v[0], v[1], v[2]]);
    let r = n.nums("rotation").filter(|v| v.len() >= 4).map_or([0., 0., 1., 0.], |v| [v[0], v[1], v[2], v[3]]);
    let l = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    let (x, y, z) = if l > 0. { (r[0] / l, r[1] / l, r[2] / l) } else { (0., 0., 1.) };
    let (c, sn) = (r[3].cos(), r[3].sin()); let k = 1. - c;
    let rot = [[c + x * x * k, x * y * k - z * sn, x * z * k + y * sn, 0.], [y * x * k + z * sn, c + y * y * k, y * z * k - x * sn, 0.], [z * x * k - y * sn, z * y * k + x * sn, c + z * z * k, 0.]];
    let sc = [[s[0], 0., 0., 0.], [0., s[1], 0., 0.], [0., 0., s[2], 0.]];
    let tr = [[1., 0., 0., t[0]], [0., 1., 0., t[1]], [0., 0., 1., t[2]]];
    mul(&tr, &mul(&rot, &sc))
}

type Groups = Vec<([u8; 3], Vec<[f32; 3]>, Vec<u32>)>;
fn shapes(n: &N, m: &M, g: &mut Groups, depth: u32) {
    if depth > 64 { return }
    match n.ty.as_str() {
        "Transform" | "Group" | "Anchor" | "Collision" => {
            let m2 = if n.ty == "Transform" { mul(m, &transform(n)) } else { *m };
            match n.f("children") { Some(V::Nodes(c)) => for c in c { shapes(c, &m2, g, depth + 1) }, Some(V::Node(c)) => shapes(c, &m2, g, depth + 1), _ => {} }
        }
        "Shape" => {
            let Some(geo) = n.node("geometry").filter(|g| g.ty == "IndexedFaceSet") else { return };
            let col = n.node("appearance").and_then(|a| a.node("material")).and_then(|m| m.nums("diffuseColor")).filter(|c| c.len() >= 3).map_or([0.8, 0.8, 0.8], |c| [c[0], c[1], c[2]]);
            let q = [(col[0] * 255.).round().clamp(0., 255.) as u8, (col[1] * 255.).round().clamp(0., 255.) as u8, (col[2] * 255.).round().clamp(0., 255.) as u8];
            let Some(pts) = geo.node("coord").and_then(|c| c.nums("point")) else { return };
            let Some(ci) = geo.nums("coordIndex") else { return };
            let gi = match g.iter().position(|x| x.0 == q) { Some(i) => i, None => { g.push((q, vec![], vec![])); g.len() - 1 } };
            let base = g[gi].1.len() as u32;
            for p in pts.chunks_exact(3) {
                let v: Vec<f64> = (0..3).map(|i| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2] + m[i][3]).collect();
                g[gi].1.push([(v[0] * 2.54) as f32, (v[1] * 2.54) as f32, (v[2] * 2.54) as f32]);
            }
            let np = pts.len() / 3; let mut face: Vec<u32> = vec![];
            for &i in ci.iter().chain(std::iter::once(&-1.0)) {
                if i < 0. { for w in 1..face.len().saturating_sub(1) { g[gi].2.extend([face[0], face[w], face[w + 1]]) } face.clear() }
                else if (i as usize) < np { face.push(base + i as u32) }
            }
        }
        _ => {}
    }
}

/// KiCad's VRML2 model (Transform / Group / Shape, Appearance Material diffuseColor, IndexedFaceSet; DEF/USE; units of 0.1 inch) as the
/// converter's GLB: mm, Z up, one primitive per colour.
pub fn wrl_to_glb(text: &str) -> Result<Vec<u8>, String> {
    let mut lx = Lex { t: lex(text), at: 0, defs: HashMap::new() };
    let mut groups: Groups = vec![];
    while lx.peek().is_some() {
        let Some(n) = lx.node(0) else { if lx.at < lx.t.len() { lx.at += 1; continue } else { break } };
        shapes(&n, &ID, &mut groups, 0);
    }
    groups.retain(|g| !g.2.is_empty());
    if groups.is_empty() { return Err("no faces in the VRML model".into()) }
    let lin = |x: f64| if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) };
    let (mut bin, mut views, mut accs, mut mats, mut prims) = (vec![], vec![], vec![], vec![], vec![]);
    for (c, pts, idx) in &groups {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in pts { for i in 0..3 { lo[i] = lo[i].min(p[i]); hi[i] = hi[i].max(p[i]); } }
        views.push(format!(r#"{{"buffer":0,"byteOffset":{},"byteLength":{},"target":34962}}"#, bin.len(), 12 * pts.len()));
        for p in pts { for x in p { bin.extend(x.to_le_bytes()) } }
        accs.push(format!(r#"{{"bufferView":{},"componentType":5126,"count":{},"type":"VEC3","min":[{},{},{}],"max":[{},{},{}]}}"#, views.len() - 1, pts.len(), lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]));
        views.push(format!(r#"{{"buffer":0,"byteOffset":{},"byteLength":{},"target":34963}}"#, bin.len(), 4 * idx.len()));
        for i in idx { bin.extend(i.to_le_bytes()) }
        accs.push(format!(r#"{{"bufferView":{},"componentType":5125,"count":{},"type":"SCALAR"}}"#, views.len() - 1, idx.len()));
        mats.push(format!(r#"{{"pbrMetallicRoughness":{{"baseColorFactor":[{},{},{},1.0],"metallicFactor":0.0,"roughnessFactor":0.8}}}}"#, lin(c[0] as f64 / 255.), lin(c[1] as f64 / 255.), lin(c[2] as f64 / 255.)));
        prims.push(format!(r#"{{"attributes":{{"POSITION":{}}},"indices":{},"material":{}}}"#, accs.len() - 2, accs.len() - 1, mats.len() - 1));
    }
    let mut js = format!(r#"{{"asset":{{"version":"2.0","generator":"backplane-wrl2glb","extras":{{"unit":"mm","up":"Z"}}}},"buffers":[{{"byteLength":{}}}],"bufferViews":[{}],"accessors":[{}],"materials":[{}],"meshes":[{{"primitives":[{}]}}],"nodes":[{{"name":"wrl","mesh":0}}],"scenes":[{{"nodes":[0]}}],"scene":0}}"#,
        bin.len(), views.join(","), accs.join(","), mats.join(","), prims.join(",")).into_bytes();
    while js.len() % 4 != 0 { js.push(b' ') }
    while bin.len() % 4 != 0 { bin.push(0) }
    let mut out = Vec::with_capacity(28 + js.len() + bin.len());
    out.extend(0x46546C67u32.to_le_bytes()); out.extend(2u32.to_le_bytes()); out.extend(((28 + js.len() + bin.len()) as u32).to_le_bytes());
    out.extend((js.len() as u32).to_le_bytes()); out.extend(0x4E4F534Au32.to_le_bytes()); out.extend(&js);
    out.extend((bin.len() as u32).to_le_bytes()); out.extend(0x004E4942u32.to_le_bytes()); out.extend(&bin);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    fn dir(tag: &str) -> PathBuf {
        let base = std::env::var("BP_TMP").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
        let d = base.join(format!("bp-models-{tag}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d
    }
    fn touch(p: &Path) { std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, "x").unwrap(); }

    #[test] fn variables_expand_in_both_syntaxes() {
        let env = Env::empty().with("A", "/a").with("B", "${A}/b");
        let bd = Path::new("/board");
        assert_eq!(expand("${A}/x.step", bd, &env).as_deref(), Some("/a/x.step"));
        assert_eq!(expand("$(A)/x.step", bd, &env).as_deref(), Some("/a/x.step"));
        assert_eq!(expand("${B}/x", bd, &env).as_deref(), Some("/a/b/x"));
        assert_eq!(expand("${KIPRJMOD}/m/x.step", bd, &env).as_deref(), Some("/board/m/x.step"));
        assert_eq!(expand("${NOPE}/x", bd, &env), None);
        assert_eq!(expand("a$b", bd, &env).as_deref(), Some("a$b"));
    }
    #[test] fn resolve_finds_files_and_prefers_step_over_wrl() {
        let d = dir("res"); let (lib, board) = (d.join("lib"), d.join("proj"));
        touch(&lib.join("a.3dshapes/r.step")); touch(&lib.join("a.3dshapes/r.wrl")); touch(&lib.join("a.3dshapes/w.wrl")); touch(&lib.join("a.3dshapes/u.WRL")); touch(&lib.join("a.3dshapes/u.stp"));
        touch(&board.join("mine/m.step")); touch(&board.join("mine/n.wrl"));
        let env = Env::empty().with("KICAD10_3DMODEL_DIR", &lib.to_string_lossy());
        let ok = |raw: &str| resolve(raw, &board, &env);
        assert_eq!(ok("${KICAD10_3DMODEL_DIR}/a.3dshapes/r.wrl"), Some(lib.join("a.3dshapes/r.step")), "step of the same stem wins");
        assert_eq!(ok("$(KICAD10_3DMODEL_DIR)/a.3dshapes/r.step"), Some(lib.join("a.3dshapes/r.step")));
        assert_eq!(ok("${KICAD10_3DMODEL_DIR}/a.3dshapes/w.wrl"), Some(lib.join("a.3dshapes/w.wrl")), "no step: the wrl as named");
        assert_eq!(ok("${KICAD10_3DMODEL_DIR}/a.3dshapes/u.WRL"), Some(lib.join("a.3dshapes/u.stp")), "stp and a capital WRL");
        assert_eq!(ok("${KIPRJMOD}/mine/m.step"), Some(board.join("mine/m.step")));
        assert_eq!(ok("mine/m.step"), Some(board.join("mine/m.step")), "relative to the board");
        assert_eq!(ok("mine/n.wrl"), Some(board.join("mine/n.wrl")));
        assert_eq!(ok(&lib.join("a.3dshapes/r.step").to_string_lossy()), Some(lib.join("a.3dshapes/r.step")), "absolute");
        assert_eq!(ok("${KICAD10_3DMODEL_DIR}/a.3dshapes/missing.step"), None);
        assert_eq!(ok("${UNSET}/x.step"), None);
        assert_eq!(ok("a.3dshapes/r.step"), Some(lib.join("a.3dshapes/r.step")), "a relative path not beside the board is looked for in the model dir");
        let old = Env::empty().with("KICAD7_3DMODEL_DIR", &lib.to_string_lossy());
        assert_eq!(resolve("${KICAD10_3DMODEL_DIR}/a.3dshapes/r.step", &board, &old), Some(lib.join("a.3dshapes/r.step")), "another version's dir stands in");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn env_layers_process_then_kicad_common_then_defaults() {
        let d = dir("env"); let cfg = d.join("kicad"); let m = d.join("models"); std::fs::create_dir_all(&m).unwrap();
        for (v, val) in [("9.0", "/old"), ("10.0", "/new")] {
            std::fs::create_dir_all(cfg.join(v)).unwrap();
            std::fs::write(cfg.join(v).join("kicad_common.json"), format!(r#"{{"environment":{{"vars":{{"MYLIB":"{val}","KICAD10_3DMODEL_DIR":"/from-config"}}}}}}"#)).unwrap();
        }
        let e = Env::build(vec![("KICAD10_3DMODEL_DIR".into(), "/from-process".into())], &[cfg.clone()], &[d.join("none"), m.clone()]);
        assert_eq!(e.get("KICAD10_3DMODEL_DIR"), Some("/from-process"));
        assert_eq!(e.get("MYLIB"), Some("/new"), "newest version first");
        assert_eq!(e.get("KICAD8_3DMODEL_DIR"), Some(m.to_str().unwrap()), "default: the first install dir that exists");
        assert_eq!(e.get("KISYS3DMOD"), Some(m.to_str().unwrap()));
        let e = Env::build(vec![], &[cfg], &[m]);
        assert_eq!(e.get("KICAD10_3DMODEL_DIR"), Some("/from-config"));
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn the_key_is_content_and_memoized() {
        let d = dir("key"); let (a, b) = (d.join("a.step"), d.join("b.step"));
        std::fs::write(&a, "hello").unwrap(); std::fs::write(&b, "hello").unwrap();
        let before = hashed();
        let ka = key(&a).unwrap();
        assert_eq!(ka, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
        assert_eq!(key(&a).unwrap(), ka); assert_eq!(key(&a).unwrap(), ka);
        assert_eq!(hashed() - before, 1, "read once");
        assert_eq!(key(&b).unwrap(), ka, "same bytes, same key"); assert_eq!(hashed() - before, 2);
        std::fs::write(&a, "hello!").unwrap(); // size changed: rehashed
        assert_ne!(key(&a).unwrap(), ka); assert_eq!(hashed() - before, 3);
        assert!(key(&d.join("none")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn publish_is_a_rename_and_a_failure_is_remembered_for_a_day() {
        let d = dir("cache"); let c = Cache::new(d.join("parts")); std::fs::create_dir_all(&c.dir).unwrap();
        assert!(c.lookup("k").is_none());
        let t = c.dir.join("tmp"); std::fs::write(&t, "glb").unwrap();
        let g = c.publish("k", &t).unwrap();
        assert_eq!(g, c.dir.join("part-v1-k.glb")); assert!(!t.exists()); assert_eq!(c.lookup("k"), Some(Ok(g)));
        std::fs::write(&t, "").unwrap(); assert!(c.publish("j", &t).is_err() && !c.glb("j").exists(), "an empty file is not a model");
        c.record_fail("f", "nope");
        assert_eq!(c.lookup("f"), Some(Err("nope".into())));
        let mut old = c.clone(); old.fail_ttl = Duration::ZERO;
        assert!(old.lookup("f").is_none() && !c.fail_file("f").exists(), "retried after the ttl");
        c.record_fail("g", "x"); let t2 = c.dir.join("t2"); std::fs::write(&t2, "glb").unwrap(); c.publish("g", &t2).unwrap();
        assert!(!c.fail_file("g").exists(), "a success clears the failure");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn worker_count_is_clamped() { assert_eq!(workers(1), 1); assert!(workers(100) >= 1 && workers(100) <= 8); }

    const CUBE: &str = "#VRML V2.0 utf8\nDEF M Material { diffuseColor 1 0 0 }\nTransform { translation 1 0 0 scale 1 1 1 children [ Shape { appearance Appearance { material USE M } geometry IndexedFaceSet { coord Coordinate { point [ 0 0 0, 1 0 0, 1 1 0, 0 1 0, 0 0 1, 1 0 1, 1 1 1, 0 1 1 ] } coordIndex [ 0 1 2 3 -1 4 5 6 7 -1 ] } } ] }\n";
    #[test] fn a_wrl_becomes_a_glb_in_mm() {
        let g = wrl_to_glb(CUBE).unwrap();
        let t = super::super::super::glb::tris(&g).unwrap();
        assert_eq!(t.len(), 4);
        let xs: Vec<f32> = t.iter().flat_map(|t| [t.a[0], t.b[0], t.c[0]]).collect();
        assert!((xs.iter().cloned().fold(f32::MIN, f32::max) - 5.08).abs() < 1e-4, "(1+1) * 2.54 mm");
        assert!((xs.iter().cloned().fold(f32::MAX, f32::min) - 2.54).abs() < 1e-4);
        assert!(wrl_to_glb("#VRML V2.0 utf8\nShape {}").is_err());
    }

    /// a fake step2glb: sleeps as long as the source file says (ms), then writes a GLB-ish file; "bad" fails
    fn fake_tool(d: &Path) -> String {
        use crate::plat::fs::PermissionsExt;
        let t = d.join("fake"); std::fs::write(&t, "#!/bin/sh\nms=$(cat \"$1\")\nif [ \"$ms\" = bad ]; then echo broken >&2; exit 1; fi\nsleep $(awk \"BEGIN{print $ms/1000}\")\nprintf glb > \"$2\"\n").unwrap();
        std::fs::set_permissions(&t, std::fs::Permissions::from_mode(0o755)).unwrap(); t.to_string_lossy().into_owned()
    }
    #[test] fn convert_streams_in_completion_order_and_caches() {
        TEST_WORKERS.store(2, Ordering::Relaxed); // never reset: more workers only make the other tests faster
        let d = dir("conv"); let tool = fake_tool(&d); let c = Cache::new(d.join("parts"));
        let mk = |n: &str, body: &str| { let p = d.join(n); std::fs::write(&p, body).unwrap(); (n.to_string(), p) };
        let jobs = vec![mk("slow", "1500"), mk("fast", "10"), mk("bad", "bad"), mk("wrl", ""), ];
        std::fs::write(&jobs[3].1, CUBE).unwrap(); let mut jobs = jobs; jobs[3].1 = d.join("c.wrl"); std::fs::write(&jobs[3].1, CUBE).unwrap();
        let got: Mutex<Vec<(String, bool, Instant)>> = Mutex::new(vec![]);
        let t0 = Instant::now();
        let tools = [Mesher::Step2glb(tool.clone())];
        convert(&c, jobs.clone(), &tools, &AtomicBool::new(false), |k, r| got.lock().unwrap().push((k.into(), r.is_ok(), Instant::now())));
        let got = got.into_inner().unwrap();
        let names: Vec<&str> = got.iter().map(|g| g.0.as_str()).collect();
        assert_eq!(names.len(), 4); assert_eq!(names[..3], ["wrl", "fast", "slow"], "{names:?}"); assert_eq!(names[3], "bad", "a failure is reported once every tool had it");
        assert!(got.iter().find(|g| g.0 == "fast").unwrap().2 < got.iter().find(|g| g.0 == "slow").unwrap().2);
        assert!(got.iter().find(|g| g.0 == "bad").map(|g| !g.1).unwrap() && got.iter().find(|g| g.0 == "wrl").map(|g| g.1).unwrap());
        assert!(t0.elapsed() >= Duration::from_millis(1500));
        assert!(c.lookup("bad").unwrap().unwrap_err().contains("broken"), "failure recorded");
        // warm: everything is reported at once, no tool run
        let n = Mutex::new(0); let t1 = Instant::now();
        convert(&c, jobs.clone(), &[], &AtomicBool::new(false), |_, _| *n.lock().unwrap() += 1);
        assert_eq!(*n.lock().unwrap(), 4); assert!(t1.elapsed() < Duration::from_millis(500));
        // duplicates are one job; stop before starting reports nothing
        let n = Mutex::new(0); let stop = AtomicBool::new(true);
        convert(&Cache::new(d.join("other")), vec![jobs[0].clone(), jobs[0].clone()], &tools, &stop, |_, _| *n.lock().unwrap() += 1);
        assert_eq!(*n.lock().unwrap(), 0);
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn a_model_two_callers_want_at_once_is_converted_once() {
        // a prewarm in the background and the 3D view asking for the same model: one conversion, both told
        let d = dir("once"); let c = Cache::new(d.join("parts"));
        use crate::plat::fs::PermissionsExt;
        let t = d.join("count"); std::fs::write(&t, format!("#!/bin/sh\necho run >> {}\nsleep 0.6\nprintf glb > \"$2\"\n", crate::plat::fs::script_path(&d.join("runs")))).unwrap();
        std::fs::set_permissions(&t, std::fs::Permissions::from_mode(0o755)).unwrap();
        let tools = [Mesher::Step2glb(t.to_string_lossy().into_owned())];
        let src = d.join("m.step"); std::fs::write(&src, "x").unwrap();
        let job = || vec![("k1-once".to_string(), src.clone())];
        let (a, b) = (Mutex::new(None), Mutex::new(None));
        std::thread::scope(|s| {
            s.spawn(|| convert_with(Opts::background(), &c, job(), &tools, &AtomicBool::new(false), &|_, r| *a.lock().unwrap() = Some(r.is_ok())));
            std::thread::sleep(Duration::from_millis(150));
            s.spawn(|| convert(&c, job(), &tools, &AtomicBool::new(false), |_, r| *b.lock().unwrap() = Some(r.is_ok())));
        });
        assert_eq!((*a.lock().unwrap(), *b.lock().unwrap()), (Some(true), Some(true)));
        assert_eq!(std::fs::read_to_string(d.join("runs")).unwrap().lines().count(), 1, "the second caller waited for the first");
        assert!(!inflight().0.lock().unwrap().contains("k1-once"), "its claim let go");
        let _ = std::fs::remove_dir_all(&d);
    }
    fn real_tools() -> Vec<Mesher> { stepglb::machine(Path::new(&std::env::var("HOME").unwrap())) }
    fn secs(t: Instant) -> f64 { t.elapsed().as_secs_f64() }

    #[test] #[ignore] fn real_models_convert_and_stream() {
        let d = dir("real"); let c = Cache::new(d.join("parts"));
        let root = Path::new("/usr/share/kicad/3dmodels");
        let want = ["Resistor_SMD.3dshapes/R_0402_1005Metric.step", "Package_SO.3dshapes/SOIC-8_3.9x4.9mm_P1.27mm.step", "Package_QFP.3dshapes/LQFP-48_7x7mm_P0.5mm.step", "Connector_USB.3dshapes/USB_C_Receptacle_GCT_USB4085.step"];
        let jobs: Vec<Job> = want.iter().map(|w| { let p = root.join(w); assert!(p.is_file(), "{p:?}"); (key(&p).unwrap(), p) }).collect();
        let names: HashMap<String, String> = jobs.iter().zip(want).map(|((k, _), w)| (k.clone(), w.to_string())).collect();
        let tools = real_tools(); eprintln!("tools {tools:?}");
        let t0 = Instant::now(); let got = Mutex::new(vec![]);
        convert(&c, jobs.clone(), &tools, &AtomicBool::new(false), |k, r| { let t = secs(t0); eprintln!("{:6.2}s {} {:?}", t, names[k], r); got.lock().unwrap().push((k.to_string(), r, t)) });
        let got = got.into_inner().unwrap();
        assert_eq!(got.len(), 4);
        let (first, last) = (got.iter().map(|g| g.2).fold(f64::MAX, f64::min), got.iter().map(|g| g.2).fold(0., f64::max));
        eprintln!("first {first:.2}s last {last:.2}s workers {}", workers(4));
        for (k, r, _) in &got {
            let p = r.as_ref().unwrap_or_else(|e| panic!("{}: {e}", names[k]));
            let bytes = std::fs::read(p).unwrap();
            let t = super::super::super::glb::tris(&bytes).unwrap();
            let colours: std::collections::HashSet<u32> = t.iter().map(|t| t.color).collect();
            eprintln!("{} tris {} colours {}", names[k], t.len(), colours.len());
            assert!(!t.is_empty() && colours.len() > 1, "{} has colours", names[k]);
        }
        let t1 = Instant::now(); let n = Mutex::new(0);
        convert(&c, jobs, &tools, &AtomicBool::new(false), |_, r| { assert!(r.is_ok()); *n.lock().unwrap() += 1 });
        eprintln!("warm {:.3}s", secs(t1)); assert_eq!(*n.lock().unwrap(), 4);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test] #[ignore] fn bench_boards() {
        let boards = std::env::var("BACKPLANE_TEST_BOARD").expect("BACKPLANE_TEST_BOARD");
        let env = Env::system(None); let tools = real_tools();
        for b in boards.split(':').filter(|b| !b.is_empty()) {
            let text = std::fs::read_to_string(b).unwrap(); let bd = Path::new(b).parent().unwrap();
            let mut raw = std::collections::BTreeSet::new();
            for part in text.split("(model ").skip(1) {
                let part = part.trim_start();
                if let Some(rest) = part.strip_prefix('"') { if let Some(e) = rest.find('"') { raw.insert(rest[..e].to_string()); } }
            }
            let (mut files, mut missing) = (std::collections::BTreeSet::new(), 0);
            for r in &raw { match resolve(r, bd, &env) { Some(p) => { files.insert(p); } None => { missing += 1; eprintln!("missing {r}"); } } }
            let t = Instant::now();
            let jobs: Vec<Job> = files.iter().map(|p| (key(p).unwrap(), p.clone())).collect();
            let unique: std::collections::HashSet<&String> = jobs.iter().map(|j| &j.0).collect();
            eprintln!("{b}: refs {} files {} unique {} missing {missing}; resolve+key {:.2}s", raw.len(), files.len(), unique.len(), secs(t));
            let d = dir("bench"); let c = Cache::new(d.join("parts"));
            let names: HashMap<String, String> = jobs.iter().map(|(k, p)| (k.clone(), p.file_name().unwrap().to_string_lossy().into_owned())).collect();
            let t0 = Instant::now(); let (mut ok, mut bad) = (0, 0); let cnt = Mutex::new((0, 0)); let _ = (&mut ok, &mut bad);
            convert(&c, jobs.clone(), &tools, &AtomicBool::new(false), |k, r| { let mut g = cnt.lock().unwrap(); if r.is_ok() { g.0 += 1 } else { g.1 += 1 }
                eprintln!("{:7.2}s {} {}", secs(t0), names[k], match &r { Ok(_) => "ok".into(), Err(e) => format!("FAIL {e}") }) });
            let (ok, bad) = *cnt.lock().unwrap();
            eprintln!("cold {:.2}s ok {ok} failed {bad}", secs(t0));
            let t1 = Instant::now(); let t2 = Instant::now();
            let jobs2: Vec<Job> = files.iter().map(|p| (key(p).unwrap(), p.clone())).collect();
            convert(&c, jobs2, &tools, &AtomicBool::new(false), |_, _| {});
            eprintln!("warm {:.3}s (key {:.3}s)", secs(t1), secs(t2));
            let _ = std::fs::remove_dir_all(&d);
        }
    }
}

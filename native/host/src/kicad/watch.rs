//! server/plot's actor in Rust: the board, schematic and 3D sources the clients watch (`kicad.watch`), polled every
//! 150 ms while any is watched, each changed file read, the plot sent to each client as the delta against what that client
//! holds. Every source (boards, schematics, sheet trees, info lookups: kicad/plotread.rs; 3D models:
//! kicad/solidread.rs) is read in this process, the same bytes as child, on blocking threads, at most `READERS` at once and one
//! 3D model at a time. `BACKPLANE_PLOT_NATIVE=0` sends them all to the child again (`backplane --plot KIND FILE INFOS`, `BACKPLANE_PLOT_BIN`,
//! default `backplane-serve` beside the hub).
//! Not ported yet: web stamps. A split view (`kicad.watch` kind "split") watches the schematic, board and 3D model at once (`watch_split`).
//!
//! Kept so a press shows at once: a board or schematic is plotted once per content (`Made`: by the bytes' digest, so the same file in
//! another worktree, or saved again unchanged, is not plotted again), sources no one watches stay (`KEPT`, `KEPT_BYTES`), a sheet tree stays
//! while its files do, and `ahead` reads a thread's board, schematic, sheets and 3D model when the thread is shown (one round at a time: a
//! newer round cancels what an older one has not started). A client that keeps its plots by source (`held` in `kicad.watch`: the GPUI window,
//! up to `CLIENT_SOURCES`) gets each frame as the delta against what it holds of that source (`Mirror`), so a reopen is a few bytes.
use super::{plot::{self, Plot}, plotgen, plotread, project, solid, solidread, stepglb};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, path::PathBuf, sync::{atomic::{AtomicU64, Ordering::Relaxed}, Arc, Mutex}, time::Duration};

const READERS: usize = 2;
const POLL_MS: u64 = 150;
const KEPT: usize = 32; // sources no client watches are kept (plots included), the least recently used dropped first: Src.kept
const KEPT_BYTES: usize = 192 << 20; // ... while their plots come to no more than this
const VERSIONS_KEPT: usize = 8; // saved versions no view watches: Src.versions.kept
const MADE_BYTES: usize = 128 << 20; // plots kept by the bytes they are of
const TREES_KEPT: usize = 32;
const AHEAD_SHEETS: usize = 16;
/// A board's 3D model is exported ahead only while the machine has this much memory available: kicad-cli takes 1.6 GB for the CM5
/// carrier (4 MB board, 22 s) and 3.5 GB for Thunderscope (23 MB board, 25 s). One model at a time, at nice 10.
const AHEAD_MEM: u64 = 6 << 30;
/// A board's bare model (body, copper, mask, silk) goes first only for a board up to this much file: then it is quick (CM5 carrier,
/// 4 MB: 5.8 s against 22 s with the parts); a bigger board's copper is most of the export (Thunderscope, 23 MB: 18 s against 25 s),
/// so its model waits for the parts rather than coming 18 s later.
const BARE_FIRST_MOST: u64 = 8 << 20;
/// MemAvailable, in bytes (u64::MAX where there is no /proc/meminfo: nothing is held back for it)
fn mem_available() -> u64 {
    std::fs::read_to_string("/proc/meminfo").ok().and_then(|t| t.lines().find(|l| l.starts_with("MemAvailable:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())).map_or(u64::MAX, |kb| kb << 10)
}
/// The GPUI window's plot store (native/gpui/src/screens/plot.rs `Store`): it keeps this many sources by key and drops them all when one
/// more comes; a frame over `CLIENT_FRAME_MOST` it refuses, and that empties it too. A `kicad.watch` with `held` = this asks for keyed deltas.
pub const CLIENT_SOURCES: u64 = 6;
const CLIENT_FRAME_MOST: usize = 32 << 20;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Stamp { rev: String, ok: bool }
pub fn stamp(path: &str) -> Stamp {
    use std::os::unix::fs::MetadataExt;
    // FS.revision: "dev:ino:size:mtime s:ns:ctime s:ns" (a same-size save within a second still changes it)
    match std::fs::metadata(path) { Ok(m) => Stamp { rev: format!("{}:{}:{}:{}:{}:{}:{}", m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec()), ok: true }, Err(_) => Stamp::default() }
}
/// The stamp of a file not written in the last two seconds (None while it may still be being saved): what a kept sheet tree is checked against.
fn settled(path: &str) -> Option<Stamp> {
    let m = std::fs::metadata(path);
    if let Ok(m) = &m { if m.modified().ok().and_then(|t| t.elapsed().ok()).map_or(true, |e| e < Duration::from_secs(2)) { return None } }
    Some(stamp(path))
}
/// A saved version or a comparison: immutable, read once.
pub fn is_version(path: &str) -> bool { path.starts_with("git\t") || path.starts_with("diff\n") }
pub fn solid_kind(k: u32) -> bool { matches!(k, 2..=4) }
pub fn key(kind: u32, root: &str, path: &str) -> String { format!("{kind}|{root}|{path}") }
pub fn kind_of(name: &str) -> Option<u32> { Some(match name { "board" => 0, "schematic" => 1, "3d" => 2, "3d-bare" => 3, "3d-full" => 4, _ => return None }) }
/// FNV-1a of a key (Key.hash), the info file's name.
pub fn fnv(s: &str) -> u32 { s.chars().fold(2166136261u32, |h, c| (h ^ c as u32).wrapping_mul(16777619)) }
/// What a plot holds in memory, near enough to bound what is kept.
fn weight(p: &Plot) -> usize { 64 + p.cs.iter().map(|c| c.d.len() + c.info.len() + 48).sum::<usize>() + p.solid.as_ref().map_or(0, |s| s.d.len()) }

#[derive(Default)]
struct Src { kind: u32, file: String, stamp: Option<Stamp>, busy: bool, plot: Option<Arc<Plot>>, again: bool, used: u64, tried: Option<Stamp> }
/// One client's view of a source: the plot it was last sent (its chunks are what it holds, in order: kicad.info's `at` indexes them).
struct Sub { key: String, held: Option<Arc<Plot>>, mesh: Option<Arc<Plot>>, fresh: bool, base: String }
/// What a keyed client holds, as its store keeps it: the sources in the order they came, the last one applied; `synced` once it was told
/// to drop what it held before (a store from an earlier connection is not known here).
#[derive(Default)]
struct Mirror { held: Vec<(String, Arc<Plot>)>, last: String, synced: bool }
impl Mirror {
    fn get(&self, k: &str) -> Option<&Arc<Plot>> { self.held.iter().find(|h| h.0 == k).map(|h| &h.1) }
    /// Store.receive: a frame of `k` applied (a frame it refuses for its size empties it)
    fn took(&mut self, k: &str, p: &Arc<Plot>, bytes: usize) {
        if bytes > CLIENT_FRAME_MOST { self.held.clear(); self.last.clear(); return }
        if self.held.len() >= CLIENT_SOURCES as usize && self.get(k).is_none() { self.held.clear() }
        match self.held.iter_mut().find(|h| h.0 == k) { Some(h) => h.1 = p.clone(), None => self.held.push((k.to_string(), p.clone())) }
        self.last = k.to_string();
    }
    /// What a frame of `k` is the delta against: `k` as held, else the newest held source of the same kind and file name (the same sheet
    /// or board in another worktree shares its chunks), else of the same kind, else the last applied ("" when nothing is held: every
    /// chunk then goes whole).
    fn base(&self, k: &str) -> String {
        if self.get(k).is_some() { return k.to_string() }
        let kind = |x: &str| x.split('|').next().unwrap_or("").to_string();
        let name = |x: &str| x.splitn(3, '|').nth(2).unwrap_or("").rsplit('/').next().unwrap_or("").to_string(); // "" for the project's own file
        let same = |x: &str, file: bool| kind(x) == kind(k) && (!file || name(x) == name(k));
        [true, false].iter().find_map(|f| self.held.iter().rev().find(|h| same(&h.0, *f)).map(|h| h.0.clone())).unwrap_or_else(|| self.last.clone())
    }
    /// Store.fail: a `none` for `k` drops it
    fn dropped(&mut self, k: &str) { self.held.retain(|h| h.0 != k); if self.last == k { self.last.clear() } }
}
struct MadeOne { plot: Arc<Plot>, info: Arc<String>, bytes: usize, used: u64 }
/// Plots by (kind, digest of the file's bytes), least recently used dropped past MADE_BYTES.
struct Made { m: HashMap<(u32, (u64, u64)), MadeOne>, bytes: usize, tick: u64, cap: usize }
impl Default for Made { fn default() -> Made { Made { m: HashMap::new(), bytes: 0, tick: 0, cap: MADE_BYTES } } }
impl Made {
    fn get(&mut self, k: &(u32, (u64, u64))) -> Option<(Arc<Plot>, Arc<String>)> {
        self.tick += 1; let t = self.tick;
        self.m.get_mut(k).map(|o| { o.used = t; (o.plot.clone(), o.info.clone()) })
    }
    fn put(&mut self, k: (u32, (u64, u64)), plot: Arc<Plot>, info: Arc<String>) {
        self.tick += 1;
        let bytes = weight(&plot) + info.len();
        if let Some(o) = self.m.insert(k, MadeOne { plot, info, bytes, used: self.tick }) { self.bytes -= o.bytes }
        self.bytes += bytes;
        while self.bytes > self.cap && self.m.len() > 1 {
            let Some(old) = self.m.iter().min_by_key(|(_, o)| o.used).map(|(k, _)| *k) else { break };
            if let Some(o) = self.m.remove(&old) { self.bytes -= o.bytes }
        }
    }
}
#[derive(Default)]
struct State { srcs: HashMap<String, Src>, subs: HashMap<u64, Vec<Sub>>, polling: bool, mirrors: HashMap<u64, Mirror>, trees: HashMap<String, (Vec<(String, Stamp)>, Arc<plotread::Tree>, u64)>, tick: u64 }

pub struct Plots { gen: AtomicU64, st: Mutex<State>, bin: PathBuf, dir: PathBuf, home: PathBuf, readers: tokio::sync::Semaphore, solid: tokio::sync::Mutex<()>, made: Mutex<Made>, round: AtomicU64 }

/// The hub's one plot actor (kicad::start), for the views to read ahead through.
pub static PLOTS: std::sync::OnceLock<Arc<Plots>> = std::sync::OnceLock::new();
/// A thread was shown on a view: its drawings are read ahead (see `Plots::ahead`).
pub async fn shown(hub: &Arc<Hub>, thread: &str) {
    let Some(p) = PLOTS.get() else { return };
    let root = { let m = hub.model().await; m.threads.get(thread).map(|t| if !t.worktree.is_empty() { t.worktree.clone() } else { m.projects.get(&t.project).filter(|p| !p.removed).map(|p| p.root.clone()).unwrap_or_default() }).unwrap_or_default() };
    if !root.is_empty() { p.ahead(hub, &root) }
}

impl Plots {
    pub fn new(home: &std::path::Path) -> Arc<Plots> {
        let bin = std::env::var_os("BACKPLANE_PLOT_BIN").map(PathBuf::from).unwrap_or_else(|| std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("backplane-serve"))).unwrap_or_else(|| "backplane-serve".into()));
        let dir = home.join("plot"); let _ = std::fs::create_dir_all(&dir);
        Arc::new(Plots { gen: Default::default(), st: Mutex::new(State::default()), bin, dir, home: home.to_path_buf(), readers: tokio::sync::Semaphore::new(READERS), solid: tokio::sync::Mutex::new(()), made: Default::default(), round: Default::default() })
    }
    fn infos(&self, key: &str) -> PathBuf { self.dir.join(format!("{}.info", fnv(key))) }

    /// kicad.watch: kind "board", "schematic", "3d", "3d-bare" or "3d-full" shows it, anything else stops. The client's
    /// other views are dropped; a new plot view starts from the chunks of the one it replaces (Sub.carry), a 3D view also
    /// watches the board's plot beside the model. `keyed`: the client keeps its plots by source (`held`), so frames are deltas against those.
    pub fn watch(self: &Arc<Self>, hub: &Arc<Hub>, client: u64, kind: Option<u32>, root: &str, path: &str, tree: &str, keyed: bool) {
        let Some(kind) = kind else { self.stop(client); return };
        let pf = if path.is_empty() || kind == 1 { Some(project::main(root)) } else { None };
        let board = |k: u32| if path.is_empty() { pf.as_ref().map(|f| if k == 1 { f.sch.clone() } else { f.pcb.clone() }).unwrap_or_default() } else { path.to_string() };
        let file = board(kind);
        if file.is_empty() { // Pv.watch.k none: nothing to show
            self.drop_subs(client);
            self.tell_none(hub, client, &key(kind, root, path), if kind == 1 { "No schematic in this project yet" } else { "No board in this project yet" });
            return
        }
        let is_board = file.ends_with(".kicad_pcb"); // Pv.watch.go: only a board's model has a board beside it
        let mut views = vec![(kind, key(kind, root, path), file)];
        if solid_kind(kind) && is_board { views.push((0, key(0, root, path), board(0))) }
        let todo = self.subscribe(hub, client, views, true, keyed);
        if kind == 1 { self.sheets(hub, client, root, &pf.map(|f| f.sch).unwrap_or_default(), path, tree) }
        for ky in todo { self.fan_one(hub, &ky, Some(client)) }
    }
    /// kicad.watch kind "split": the schematic (`path` is its sheet, "" the root), the board and the board's 3D model at once, each its own
    /// plot (full chunks, no carry: the panes are separate sources) and the 3D model's board plot shared with the board pane.
    pub fn watch_split(self: &Arc<Self>, hub: &Arc<Hub>, client: u64, root: &str, path: &str, tree: &str, keyed: bool) {
        let f = project::main(root);
        let sch = if path.is_empty() { f.sch.clone() } else { path.to_string() };
        let mut views: Vec<(u32, String, String)> = vec![];
        if !sch.is_empty() { views.push((1, key(1, root, path), sch.clone())) }
        if !f.pcb.is_empty() { views.push((0, key(0, root, ""), f.pcb.clone())); views.push((2, key(2, root, ""), f.pcb.clone())) }
        let todo = self.subscribe(hub, client, views, false, keyed);
        if !sch.is_empty() { self.sheets(hub, client, root, &f.sch, path, tree) }
        for ky in todo { self.fan_one(hub, &ky, Some(client)) }
    }
    /// The client's views replaced by these (kind, key, file): their sources made or freshened, the poll started; the keys to bring them up to.
    fn subscribe(self: &Arc<Self>, hub: &Arc<Hub>, client: u64, views: Vec<(u32, String, String)>, carry_on: bool, keyed: bool) -> Vec<String> {
        let mut todo = vec![];
        let mut st = self.st.lock().unwrap();
        st.mirrors.retain(|c, _| hub.is_attached(*c));
        if keyed { st.mirrors.entry(client).or_default(); }
        let carry = if carry_on { st.subs.get(&client).and_then(|v| v.iter().find(|s| s.key.starts_with("0|") || s.key.starts_with("1|"))).map(|s| (s.held.clone(), s.base.clone())).unwrap_or_default() } else { Default::default() };
        st.subs.remove(&client);
        st.tick += 1; let tick = st.tick;
        for (i, (k, ky, f)) in views.into_iter().enumerate() {
            let carries = carry_on && i == 0 && !solid_kind(k);
            st.subs.entry(client).or_default().push(Sub { key: ky.clone(), held: if carries { carry.0.clone() } else { None }, mesh: None, fresh: true, base: if carries { carry.1.clone() } else { String::new() } });
            let s = st.srcs.entry(ky.clone()).or_insert_with(|| Src { kind: k, file: f.clone(), ..Default::default() });
            s.file = f; s.used = tick;
            if !s.busy && !is_version(&s.file) && s.stamp.as_ref() != Some(&stamp(&s.file)) { s.plot = None } // Pv.fresh: a plot of an older file is not sent
            todo.push(ky);
        }
        self.gc(&mut st);
        if !st.polling { st.polling = true; let me = self.clone(); let hub = hub.clone(); tokio::spawn(async move { me.poll(hub).await }); }
        todo
    }
    pub fn stop(&self, client: u64) { self.drop_subs(client) }
    fn drop_subs(&self, client: u64) {
        let mut st = self.st.lock().unwrap(); st.subs.remove(&client); self.gc(&mut st);
    }
    /// `{t:"plot", key, none}` to one client; a keyed client's store drops that source.
    fn tell_none(&self, hub: &Arc<Hub>, client: u64, ky: &str, why: &str) -> bool {
        if let Some(m) = self.st.lock().unwrap().mirrors.get_mut(&client) { m.dropped(ky) }
        hub.push(client, json!({"t":"plot","key":ky,"none":why}))
    }
    /// Sources no one watches: the least recently used go first, past KEPT or past KEPT_BYTES of plots (saved versions on their own count).
    fn gc(&self, st: &mut State) {
        let watched: HashSet<String> = st.subs.values().flatten().map(|s| s.key.clone()).collect();
        for (limit, versions) in [(KEPT, false), (VERSIONS_KEPT, true)] {
            let mut idle: Vec<(u64, String, usize)> = st.srcs.iter().filter(|(k, s)| !watched.contains(*k) && is_version(&s.file) == versions).map(|(k, s)| (s.used, k.clone(), s.plot.as_deref().map_or(0, weight))).collect();
            idle.sort_by(|a, b| b.0.cmp(&a.0)); // newest first
            let mut total = 0usize;
            for (i, (_, k, w)) in idle.iter().enumerate() { total += w; if i >= limit || (i > 0 && total > KEPT_BYTES) { st.srcs.remove(k); } }
        }
    }
    /// kicad.warm / PWarm: a saved version plotted ahead and kept, no view watching it (scrubbing the history comes back to it at once).
    pub fn warm(self: &Arc<Self>, hub: &Arc<Hub>, kind: u32, root: &str, path: &str) {
        if path.is_empty() || kind > 1 { return }
        let ky = key(kind, root, path);
        let need = { let mut st = self.st.lock().unwrap(); st.tick += 1; let tick = st.tick;
            let s = st.srcs.entry(ky.clone()).or_insert_with(|| Src { kind, file: path.to_string(), ..Default::default() });
            s.used = tick; let need = s.plot.is_none() && !s.busy; self.gc(&mut st); need };
        if need { self.read(hub, &ky, 0) }
    }
    /// A thread's drawings read before they are asked for: its board, its schematic and the sheets below it, then the board's 3D model (while AHEAD_MEM is available),
    /// each kept as an idle source, so the first press shows at once. A newer round (another thread shown) cancels what this one has
    /// not started; a source already read at its file's revision, or that failed at it, is not read again.
    pub fn ahead(self: &Arc<Self>, hub: &Arc<Hub>, root: &str) {
        if root.is_empty() || !native() { return }
        let round = self.round.fetch_add(1, Relaxed) + 1;
        let (me, hub, root) = (self.clone(), hub.clone(), root.to_string());
        tokio::spawn(async move {
            let (m2, r) = (me.clone(), root.clone());
            let Ok((f, sheets)) = tokio::task::spawn_blocking(move || {
                let f = project::main(&r);
                let sheets: Vec<String> = if f.sch.is_empty() { vec![] } else { let t = m2.tree_of(&f.sch); let mut seen = HashSet::new();
                    t.rows.lines().filter_map(|l| { let c: Vec<&str> = l.split('\t').collect(); (c.len() >= 4 && c[0] != "0").then(|| c[3].to_string()) }).filter(|p| !is_version(p) && seen.insert(p.clone())).take(AHEAD_SHEETS).collect() };
                (f, sheets)
            }).await else { return };
            if me.round.load(Relaxed) != round { return }
            let mut want: Vec<(u32, String, String)> = vec![];
            if !f.pcb.is_empty() { want.push((0, key(0, &root, ""), f.pcb.clone())) }
            if !f.sch.is_empty() { want.push((1, key(1, &root, ""), f.sch.clone())) }
            for s in sheets { want.push((1, key(1, &root, &s), s)) }
            if f.pcb.ends_with(".kicad_pcb") { want.push((2, key(2, &root, ""), f.pcb.clone())) }
            let todo: Vec<String> = { let mut st = me.st.lock().unwrap(); st.tick += 1; let tick = st.tick; let mut todo = vec![];
                for (k, ky, file) in want {
                    let s = st.srcs.entry(ky.clone()).or_insert_with(|| Src { kind: k, file: file.clone(), ..Default::default() });
                    if s.file != file { s.file = file; s.plot = None; s.stamp = None; s.tried = None }
                    s.used = s.used.max(tick.saturating_sub(1));
                    if s.busy { continue }
                    let now = stamp(&s.file);
                    if s.plot.is_some() && s.stamp.as_ref() == Some(&now) { continue }
                    if s.tried.as_ref() == Some(&now) { continue }
                    s.plot = None; todo.push(ky);
                }
                me.gc(&mut st); todo };
            for ky in todo { me.read(&hub, &ky, round) }
        });
    }

    /// Bring watchers of `key` (or just `only`) up to its plot; a source with none yet is read.
    fn fan_one(self: &Arc<Self>, hub: &Arc<Hub>, ky: &str, only: Option<u64>) {
        let (plot, need_read, kind) = { let st = self.st.lock().unwrap(); match st.srcs.get(ky) { Some(s) => (s.plot.clone(), s.plot.is_none() && !s.busy, s.kind), None => return } };
        if let Some(p) = plot { self.send_to(hub, ky, &p, only); self.revision(hub, ky, only); if solid_kind(kind) { self.note(hub, ky, only) } }
        if need_read { self.read(hub, ky, 0) }
    }
    /// Pv.revision.ready: the exact revision of the file the plot is of, to the watchers (nothing for a version).
    fn revision(&self, hub: &Arc<Hub>, ky: &str, only: Option<u64>) {
        let (rev, who): (String, Vec<u64>) = { let st = self.st.lock().unwrap();
            (st.srcs.get(ky).and_then(|s| s.stamp.as_ref()).map(|s| s.rev.clone()).unwrap_or_default(),
             st.subs.iter().filter(|(c, v)| only.map_or(true, |o| o == **c) && v.iter().any(|s| s.key == ky)).map(|(c, _)| *c).collect()) };
        if rev.is_empty() { return }
        for c in who { hub.push(c, json!({"t":"plotrevision","key":ky,"revision":rev})); }
    }
    /// Pv.note: a 3D model's note (in its source's info file: the parts with no 3D model on this machine) to its watchers
    fn note(&self, hub: &Arc<Hub>, ky: &str, only: Option<u64>) {
        let text = std::fs::read_to_string(self.infos(ky)).unwrap_or_default();
        let who: Vec<u64> = { let st = self.st.lock().unwrap(); st.subs.iter().filter(|(c, v)| only.map_or(true, |o| o == **c) && v.iter().any(|s| s.key == ky)).map(|(c, _)| *c).collect() };
        for c in who { hub.push(c, json!({"t":"plotnote","key":ky,"text":text.trim()})); }
    }
    fn send_to(&self, hub: &Arc<Hub>, ky: &str, p: &Arc<Plot>, only: Option<u64>) {
        let mut gone = vec![];
        { let mut guard = self.st.lock().unwrap(); let st = &mut *guard;
          for (client, subs) in st.subs.iter_mut() {
              if only.map_or(false, |o| o != *client) { continue }
              for sub in subs.iter_mut().filter(|s| s.key == ky) {
                  let ok = if let Some(so) = &p.solid { // Pv.send.solid
                      if sub.mesh.as_ref().map_or(false, |m| Arc::ptr_eq(m, p) || m.solid.as_ref().map(|x| &x.d) == Some(&so.d)) { true } else {
                          let t0 = std::time::Instant::now(); let f = plot::solid_frame(ky, so);
                          plot::times(|| format!("{{\"stage\":\"send_solid\",\"key\":{:?},\"frame_ms\":{:.1},\"frame_bytes\":{}}}", ky, t0.elapsed().as_secs_f64() * 1e3, f.len()));
                          sub.mesh = Some(p.clone()); hub.push_raw(*client, f) }
                  } else if let Some(m) = st.mirrors.get_mut(client) {
                      // a keyed client: the delta against what its store holds of this source, else of the last one applied
                      let t0 = std::time::Instant::now();
                      let reset = !m.synced; if reset { m.held.clear(); m.last.clear(); m.synced = true }
                      let base = m.base(ky);
                      let held: &[plot::Chunk] = m.get(&base).map_or(&[], |h| &h.cs[..]);
                      let rs = plot::delta(&p.cs, held);
                      let quiet = !sub.fresh && base == ky && plot::quiet(&rs, held.len());
                      let t1 = std::time::Instant::now();
                      let f = if quiet { None } else { Some(plot::frame(ky, &family(ky), &base, p, &rs)) };
                      plot::times(|| format!("{{\"stage\":\"send\",\"key\":{:?},\"keyed\":true,\"base\":{:?},\"held\":{},\"chunks\":{},\"fresh\":{},\"delta_ms\":{:.1},\"frame_ms\":{:.1},\"frame_bytes\":{}}}", ky, base, held.len(), p.cs.len(), rs.iter().filter(|r| matches!(r, plot::Ref::Fresh(_))).count(), (t1 - t0).as_secs_f64() * 1e3, t1.elapsed().as_secs_f64() * 1e3, f.as_ref().map_or(0, |f| f.len())));
                      // a store from before this connection is emptied first (Store.fail with no key), then this frame lands on nothing unknown
                      let mut ok = !reset || hub.push(*client, json!({"t":"plot","key":"","none":""}));
                      if let Some(f) = f { let n = f.len(); ok = ok && hub.push_raw(*client, f); m.took(ky, p, n) }
                      sub.held = Some(p.clone()); sub.fresh = false; sub.base = ky.to_string(); ok
                  } else {
                      let t0 = std::time::Instant::now();
                      let held: &[plot::Chunk] = sub.held.as_ref().map_or(&[], |h| &h.cs[..]);
                      let rs = plot::delta(&p.cs, held);
                      let t1 = std::time::Instant::now();
                      let f = if !sub.fresh && plot::quiet(&rs, held.len()) { None } else { Some(plot::frame(ky, &family(ky), &sub.base, p, &rs)) };
                      plot::times(|| format!("{{\"stage\":\"send\",\"key\":{:?},\"held\":{},\"chunks\":{},\"fresh\":{},\"delta_ms\":{:.1},\"frame_ms\":{:.1},\"frame_bytes\":{}}}", ky, held.len(), p.cs.len(), rs.iter().filter(|r| matches!(r, plot::Ref::Fresh(_))).count(), (t1 - t0).as_secs_f64() * 1e3, t1.elapsed().as_secs_f64() * 1e3, f.as_ref().map_or(0, |f| f.len())));
                      let ok = match f { None => true, Some(f) => hub.push_raw(*client, f) };
                      sub.held = Some(p.clone()); sub.fresh = false; sub.base = ky.to_string(); ok
                  };
                  if !ok { gone.push(*client) }
              }
          } }
        for c in gone { self.stop(c) }
    }

    /// Read a source on a blocking thread and send it to its watchers. `round` 0 is a watcher's read; another is an `ahead` round's,
    /// dropped before it starts when a newer round has begun (a model's waits for no other model: it gives way instead).
    fn read(self: &Arc<Self>, hub: &Arc<Hub>, ky: &str, round: u64) {
        let (kind, file) = { let mut st = self.st.lock().unwrap(); let Some(s) = st.srcs.get_mut(ky) else { return }; if s.busy { if round == 0 { s.again = true } return } s.busy = true; (s.kind, s.file.clone()) };
        let (me, hub, ky) = (self.clone(), hub.clone(), ky.to_string());
        tokio::spawn(async move {
            let q0 = std::time::Instant::now();
            let ahead_solid = round != 0 && solid_kind(kind);
            if ahead_solid { tokio::time::sleep(Duration::from_millis(500)).await } // the drawings first
            // a model ahead waits for no one and needs memory to spare (an export it cannot make now is made when it is asked for)
            let cancelled = |me: &Plots| round != 0 && (me.round.load(Relaxed) != round || (ahead_solid && !solidread::ready(kind, &file) && mem_available() < AHEAD_MEM));
            let _slot = if ahead_solid || cancelled(&me) { None } else { me.readers.acquire().await.ok() };
            // Pv.read.at: a model whose GLB is known too big is refused at once, not behind another model's read
            let known = if solid_kind(kind) { solidread::known(kind, &file) } else { 0 };
            let _one = if solid_kind(kind) && solid::fits(known) && !cancelled(&me) { if ahead_solid { me.solid.try_lock().ok() } else { Some(me.solid.lock().await) } } else { None };
            if cancelled(&me) || (ahead_solid && solid::fits(known) && _one.is_none()) {
                // given up: a watcher that came meanwhile (it found the source busy) still gets it read
                let again = { let mut g = me.st.lock().unwrap(); let st = &mut *g; let watched = st.subs.values().flatten().any(|s| s.key == ky);
                    st.srcs.get_mut(&ky).map_or(false, |s| { s.busy = false; std::mem::take(&mut s.again) || watched }) };
                if again { me.read(&hub, &ky, 0) }
                return
            }
            let r0 = std::time::Instant::now();
            let before = if is_version(&file) { Stamp::default() } else { stamp(&file) };
            // a board's model not made yet: the bare board (body, copper, mask, silk: a quarter of the export's time for the CM5 carrier;
            // a board up to BARE_FIRST_MOST) is made first and goes out whole to whoever watches, then the board with its parts replaces it under the same key
            let mut first: Option<Arc<Plot>> = None;
            if kind == 2 && native() && solid::fits(known) && file.ends_with(".kicad_pcb") && std::fs::metadata(&file).map_or(false, |m| m.len() <= BARE_FIRST_MOST) && !solidread::ready(2, &file) {
                if let Some((0, _, Some(bare))) = me.run_native(&hub, 3, &file, &ky).await {
                    if bare.solid.is_some() && stamp(&file) == before {
                        { let mut st = me.st.lock().unwrap(); if let Some(s) = st.srcs.get_mut(&ky) { s.plot = Some(bare.clone()) } }
                        plot::times(|| format!("{{\"stage\":\"solid_first\",\"key\":{:?},\"ms\":{:.1}}}", ky, r0.elapsed().as_secs_f64() * 1e3));
                        me.send_to(&hub, &ky, &bare, None); first = Some(bare);
                    }
                }
            }
            let out = if !solid::fits(known) { Some((3, solid::big(known), None)) } else if native() { me.run_native(&hub, kind, &file, &ky).await } else { me.run_child(kind, &file, &ky).await.map(|(c, t)| (c, t, None)) };
            // the parts could not be added: the bare board stays what is shown
            let out = match (out, first) { (Some((c, _, _)), Some(b)) if c != 0 && c != 2 => Some((0, String::new(), Some(b))), (o, _) => o };
            let after = if is_version(&file) { Stamp::default() } else { stamp(&file) };
            plot::times(|| format!("{{\"stage\":\"read\",\"key\":{:?},\"round\":{round},\"queue_ms\":{:.1},\"run_ms\":{:.1}}}", ky, (r0 - q0).as_secs_f64() * 1e3, r0.elapsed().as_secs_f64() * 1e3));
            let (code, text, made) = match out { Some((c, t, m)) => if before != after { (2, String::new(), None) } else { (c, t, m) }, None => (1, "cannot run the plot reader".into(), None) };
            let mut again = false;
            let plot = if code == 0 { Some(made.unwrap_or_else(|| Arc::new(plot::parse_child(&text)))) } else { None };
            { let mut st = me.st.lock().unwrap(); st.tick += 1; let tick = st.tick;
              if let Some(s) = st.srcs.get_mut(&ky) { s.busy = false; s.stamp = Some(before.clone()); again = std::mem::take(&mut s.again); s.used = s.used.max(tick);
                  if let Some(p) = &plot { s.plot = Some(p.clone()); s.tried = None } else if code != 2 { s.tried = Some(before) } }
              me.gc(&mut st); }
            match (&plot, code) {
                (Some(p), _) => { me.send_to(&hub, &ky, p, None); me.revision(&hub, &ky, None) }
                (None, 0 | 2) => {} // half written or read again: the next poll sees the file change
                (None, _) => { // Pv.fail: every watcher told why, and holding nothing
                    let why: String = text.trim().chars().take(200).collect();
                    let who: Vec<u64> = { let mut st = me.st.lock().unwrap(); let mut who = vec![];
                        for (c, subs) in st.subs.iter_mut() { for s in subs.iter_mut().filter(|s| s.key == ky) { s.held = None; who.push(*c) } } who };
                    let gone: Vec<u64> = who.into_iter().filter(|c| !me.tell_none(&hub, *c, &ky, &why)).collect();
                    for c in gone { me.stop(c) } }
            }
            if matches!(code, 0 | 2) && (ky.starts_with("2|") || ky.starts_with("3|")) { me.note(&hub, &ky, None) } // Pv.handle PGot
            if again { me.read(&hub, &ky, 0) }
        });
    }
    /// A board or schematic as its plot: the file's bytes read and digested, the plot of those bytes kept (Made) or made. A comparison is
    /// made each time (it is read once, being immutable). The same exits as plotread::read.
    fn plot_2d(&self, kind: u32, path: &str, infos: &str) -> (i32, String, Option<Arc<Plot>>) {
        let done = |r: plotgen::Read| match r {
            plotgen::Read::Plot(p, t) => { plotgen::write_infos(infos, &t); (0, String::new(), Some((Arc::new(*p), t))) }
            plotgen::Read::Half => (2, String::new(), None), plotgen::Read::Fail(m) => (3, m, None) };
        if plotread::kind_of(path) == 2 { let (c, m, p) = done(plotread::read(kind, path)); return (c, m, p.map(|x| x.0)) }
        let fixed = plotread::kind_of(path) != 0;
        let t0 = std::time::Instant::now();
        let b = plotread::version(path);
        if b.is_empty() { let (c, m, p) = done(if fixed { plotgen::Read::Fail(plotgen::UNREAD.into()) } else { plotgen::empty(path) }); return (c, m, p.map(|x| x.0)) }
        let d = (kind, plot::digest(&b));
        let hit = self.made.lock().unwrap().get(&d);
        plot::times(|| format!("{{\"stage\":\"digest\",\"kind\":{kind},\"bytes\":{},\"read_digest_ms\":{:.1},\"hit\":{}}}", b.len(), t0.elapsed().as_secs_f64() * 1e3, hit.is_some()));
        if let Some((p, info)) = hit { plotgen::write_infos(infos, &info); return (0, String::new(), Some(p)) }
        let (c, m, p) = done(plotgen::of_bytes(kind, &b, fixed));
        (c, m, p.map(|(p, t)| { self.made.lock().unwrap().put(d, p.clone(), Arc::new(t)); p }))
    }
    /// `Pv.child` in this process (kicad/plotread.rs, kicad/solidread.rs): the plot, its info file written, and the child's exit code and
    /// message. A panic in a reader is an unread file, never the hub's end; a big file's memory goes back when it is done.
    async fn run_native(self: &Arc<Self>, hub: &Arc<Hub>, kind: u32, file: &str, ky: &str) -> Option<(i32, String, Option<Arc<Plot>>)> {
        let (f, infos) = (file.to_string(), self.infos(ky));
        if solid_kind(kind) {
            let (cli, home) = (super::ipc::runtime(hub).await.cli, self.home.clone());
            return tokio::task::spawn_blocking(move || {
                let i = infos.to_string_lossy().into_owned();
                let t0 = std::time::Instant::now();
                let r = std::panic::catch_unwind(|| solidread::read(kind, &f, &cli, &stepglb::machine(&home), &i));
                plot::times(|| format!("{{\"stage\":\"solid\",\"kind\":{kind},\"file\":{:?},\"ms\":{:.1}}}", f, t0.elapsed().as_secs_f64() * 1e3));
                let o = match r { Ok(g) => (g.code, g.text, g.plot.map(Arc::new)), Err(_) => (3, plotgen::UNREAD.to_string(), None) };
                #[cfg(target_env = "gnu")] unsafe { libc::malloc_trim(0); }
                o
            }).await.ok()
        }
        let me = self.clone();
        tokio::task::spawn_blocking(move || {
            let t0 = std::time::Instant::now();
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| me.plot_2d(kind, &f, &infos.to_string_lossy())));
            plot::times(|| format!("{{\"stage\":\"plotread\",\"kind\":{kind},\"ms\":{:.1}}}", t0.elapsed().as_secs_f64() * 1e3));
            let o = r.unwrap_or_else(|_| (3, plotgen::UNREAD.to_string(), None));
            #[cfg(target_env = "gnu")] unsafe { libc::malloc_trim(0); }
            o
        }).await.ok()
    }
    async fn run_child(&self, kind: u32, file: &str, ky: &str) -> Option<(i32, String)> {
        let mut c = if solid_kind(kind) { let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, "nice"); c.args(["-n", "10"]).arg(&self.bin); c } else { crate::proc::tokio_cmd(crate::proc::Kind::Local, &self.bin) };
        c.arg("--plot").arg(kind.to_string()).arg(file).arg(self.infos(ky)).stdin(std::process::Stdio::null()).kill_on_drop(false);
        let o = c.output().await.ok()?;
        let code = o.status.code().unwrap_or(1);
        let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
        if code != 0 && text.trim().is_empty() { text = String::from_utf8_lossy(&o.stderr).into_owned() }
        Some((code, text))
    }

    async fn poll(self: Arc<Self>, hub: Arc<Hub>) {
        loop {
            tokio::time::sleep(Duration::from_millis(POLL_MS)).await;
            let mut changed = vec![];
            { let mut st = self.st.lock().unwrap();
              // clients that went away
              let gone: Vec<u64> = st.subs.keys().copied().filter(|c| !hub.is_attached(*c)).collect();
              for c in gone { st.subs.remove(&c); st.mirrors.remove(&c); }
              self.gc(&mut st);
              if st.subs.is_empty() { st.polling = false; return }
              let watched: HashSet<String> = st.subs.values().flatten().map(|s| s.key.clone()).collect();
              for (k, s) in st.srcs.iter_mut() {
                  if !watched.contains(k) || s.busy || is_version(&s.file) { continue }
                  let now = stamp(&s.file);
                  if s.stamp.as_ref() != Some(&now) { changed.push(k.clone()) }
              } }
            for k in changed { self.read(&hub, &k, 0) }
        }
    }

    /// A schematic's sheet tree (plotread::tree), kept while every file it read is as it was.
    fn tree_kept(&self, file: &str) -> Option<Arc<plotread::Tree>> {
        let deps = { let st = self.st.lock().unwrap(); st.trees.get(file).map(|t| (t.0.clone(), t.1.clone())) };
        let (deps, t) = deps?;
        deps.iter().all(|(p, s)| stamp(p) == *s).then_some(t)
    }
    fn tree_of(&self, file: &str) -> Arc<plotread::Tree> {
        if let Some(t) = self.tree_kept(file) { return t }
        let t = Arc::new(plotread::tree(file));
        // the files it read: the root and every sheet in its rows (a version is immutable: nothing to check)
        let mut files: Vec<String> = vec![file.to_string()];
        for l in t.rows.lines() { if let Some(f) = l.split('\t').nth(3) { if !files.iter().any(|x| x == f) { files.push(f.to_string()) } } }
        let deps: Option<Vec<(String, Stamp)>> = if is_version(file) { Some(vec![]) } else { files.into_iter().map(|f| settled(&f).map(|s| (f, s))).collect() };
        if let Some(deps) = deps {
            let mut st = self.st.lock().unwrap(); st.tick += 1; let tick = st.tick;
            if st.trees.len() >= TREES_KEPT && !st.trees.contains_key(file) { if let Some(old) = st.trees.iter().min_by_key(|t| (t.1).2).map(|t| t.0.clone()) { st.trees.remove(&old); } }
            st.trees.insert(file.to_string(), (deps, t.clone(), tick));
        }
        t
    }
    /// Pv.tree: a schematic's sheets (and the boxes a tap on a sheet symbol opens), read by `--plot-tree FILE` and sent as
    /// `{"t":"sheets", root, slot, gen, rows, boxes, ids, top, refs}`; `gen` only grows (the client takes a reply only if newer).
    /// A tree kept from before goes at once, ahead of the plot.
    fn sheets(self: &Arc<Self>, hub: &Arc<Hub>, client: u64, root: &str, sch: &str, path: &str, tree: &str) {
        let gen = self.gen.fetch_add(1, Relaxed) + 1;
        let file = tree_root(sch, path, tree);
        if file.is_empty() { return }
        let (me, hub, root, slot) = (self.clone(), hub.clone(), root.to_string(), tree_slot(path));
        let send = move |hub: &Arc<Hub>, t: &plotread::Tree| { hub.push(client, json!({"t":"sheets","root":root,"slot":slot,"gen":gen.to_string(),"rows":t.rows,"boxes":t.boxes,"ids":t.ids,"top":t.top,"refs":t.refs})); };
        if native() { if let Some(t) = self.tree_kept(&file) { send(&hub, &t); return } }
        tokio::spawn(async move {
            if native() {
                let (m2, f) = (me.clone(), file.clone());
                let t0 = std::time::Instant::now();
                let Ok(Ok(t)) = tokio::task::spawn_blocking(move || std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| m2.tree_of(&f)))).await else { return };
                plot::times(|| format!("{{\"stage\":\"tree\",\"ms\":{:.1}}}", t0.elapsed().as_secs_f64() * 1e3));
                send(&hub, &t);
                return
            }
            let Some(o) = crate::proc::tokio_cmd(crate::proc::Kind::Local, &me.bin).arg("--plot-tree").arg(&file).stdin(std::process::Stdio::null()).output().await.ok() else { return };
            if !o.status.success() { return }
            let text = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let j: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            let g = |k: &str, d: &str| j.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
            send(&hub, &plotread::Tree { rows: g("rows", text.trim_end()), boxes: g("boxes", ""), ids: g("ids", ""), top: g("top", ""), refs: g("refs", "") });
        });
    }
    /// kicad.probe: the pieces of the chunks this client holds for `key` whose info has `name=value` (a net, a part), as (chunk index, piece index): the cross-probe's light.
    pub async fn probe(&self, client: u64, key_: &str, name: &str, value: &str) -> Vec<(usize, usize)> {
        let sigs: Vec<u32> = { let st = self.st.lock().unwrap(); st.subs.get(&client).and_then(|v| v.iter().find(|s| s.key == key_)).and_then(|s| s.held.as_ref()).map(|h| h.cs.iter().map(|c| c.sig).collect()).unwrap_or_default() };
        if sigs.is_empty() || name.is_empty() || value.is_empty() { return vec![] }
        let (f, name, value) = (self.infos(key_), name.to_string(), value.to_string());
        let hits = tokio::task::spawn_blocking(move || { let t = std::fs::read(f).unwrap_or_default(); plotgen::info_scan(&String::from_utf8_lossy(&t), &name, &value) }).await.unwrap_or_default();
        let set: std::collections::HashMap<u32, Vec<usize>> = hits.into_iter().fold(Default::default(), |mut m, (s, i)| { m.entry(s).or_default().push(i); m });
        let mut out = vec![];
        for (at, sig) in sigs.iter().enumerate() { if let Some(ix) = set.get(sig) { for i in ix { out.push((at, *i)); if out.len() >= 20000 { return out } } } }
        out
    }
    /// kicad.info: what a tapped piece is, of the chunk the phone holds at `at` for `key`, by the child that reads the info file.
    pub async fn info(&self, client: u64, key_: &str, at: usize, idx: usize) -> String {
        let sig = { let st = self.st.lock().unwrap(); st.subs.get(&client).and_then(|v| v.iter().find(|s| s.key == key_)).and_then(|s| s.held.as_ref()).and_then(|h| h.cs.get(at)).map(|c| c.sig) };
        let Some(sig) = sig else { return String::new() };
        if native() {
            let f = self.infos(key_);
            return tokio::task::spawn_blocking(move || { let t = std::fs::read(f).unwrap_or_default(); plotgen::info_find(&String::from_utf8_lossy(&t), sig, idx).trim_end().to_string() }).await.unwrap_or_default();
        }
        let o = crate::proc::tokio_cmd(crate::proc::Kind::Local, &self.bin).arg("--plot-info").arg(self.infos(key_)).arg(sig.to_string()).arg(idx.to_string()).stdin(std::process::Stdio::null()).output().await;
        match o { Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim_end().to_string(), _ => String::new() }
    }
}
/// Every source is read in this process unless BACKPLANE_PLOT_NATIVE=0 sends it to child.
pub fn native() -> bool { std::env::var("BACKPLANE_PLOT_NATIVE").map_or(true, |v| v != "0") }
/// Hist.kind: 0 a live file, 1 a file in a commit, 2 a comparison.
pub fn hist_kind(p: &str) -> u8 { if p.starts_with("git\t") { 1 } else if p.starts_with("diff\n") { 2 } else { 0 } }
/// Hist.rebase: another live file (abs) in the same commit as version p; abs itself when p is live.
pub fn rebase(p: &str, abs: &str) -> String { if hist_kind(p) == 1 { let f: Vec<&str> = p.split('\t').collect(); format!("git\t{}\t{}\t{}\t{abs}", f.get(1).unwrap_or(&""), f.get(2).unwrap_or(&""), f.get(3).unwrap_or(&"")) } else { abs.to_string() } }
/// Tree.root with Pv.tree.path: the file whose sheets are read (the project's root sheet, or `tree`, in the version shown; the file shown when there is none).
pub fn tree_root(sch: &str, path: &str, tree: &str) -> String {
    let shown = if hist_kind(path) == 2 { "" } else { path };
    let top = if tree.is_empty() { sch } else { tree };
    if top.is_empty() { shown.to_string() } else { rebase(shown, top) }
}
/// Pv.tree.slot: which tree it is: "" the live design's, else "<gitdir>\t<commit>" of the version watched.
pub fn tree_slot(path: &str) -> String { if hist_kind(path) == 1 { let f: Vec<&str> = path.split('\t').collect(); format!("{}\t{}", f.get(1).unwrap_or(&""), f.get(2).unwrap_or(&"")) } else { String::new() } }
/// Hist.key.family for a live source: "kind|root|path" with the path's family (the live file a version is of).
pub fn family(ky: &str) -> String {
    let mut it = ky.splitn(3, '|'); let (k, r, p) = (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or(""));
    let fam = if let Some(rest) = p.strip_prefix("git\t") { rest.split('\t').nth(3).map(|x| x.to_string()).unwrap_or_default() }
              else if let Some(rest) = p.strip_prefix("diff\n") { let new = rest.split('\n').nth(1).unwrap_or(""); new.strip_prefix("git\t").and_then(|x| x.split('\t').nth(3)).map(|x| x.to_string()).unwrap_or_else(|| new.to_string()) } else { p.to_string() };
    format!("{k}|{r}|{fam}")
}

/// What each thread's viewer shows, as the client reports it (`viewer.state`): kind, source key, part, sheet, layer, selection; with a counter that moves at every report.
static VIEWER_STATE: std::sync::OnceLock<Mutex<HashMap<String, (u64, Value)>>> = std::sync::OnceLock::new();
fn vs() -> &'static Mutex<HashMap<String, (u64, Value)>> { VIEWER_STATE.get_or_init(Default::default) }
pub fn viewer_report(thread: &str, state: Value) { let mut m = vs().lock().unwrap(); if m.len() > 512 && !m.contains_key(thread) { m.clear(); } let n = m.get(thread).map_or(0, |x| x.0) + 1; m.insert(thread.to_string(), (n, state)); }
/// (report counter, state) of a thread's viewer; (0, null) before any report.
pub fn viewer_state(thread: &str) -> (u64, Value) { vs().lock().unwrap().get(thread).cloned().unwrap_or((0, Value::Null)) }

pub struct KicadRpc(pub Arc<Plots>);
impl RpcHandler for KicadRpc {
    fn methods(&self) -> &'static [&'static str] { &["kicad.watch", "kicad.info", "kicad.warm", "kicad.probe", "viewer.state", "history.steps"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            match method {
                "kicad.watch" => {
                    let (kind, root, path) = (s("kind"), s("root"), s("path"));
                    // PR #960: an active request with neither an absolute root nor an absolute source has no folder and is refused (Hub.folder.out Watch)
                    if !kind.is_empty() && !(crate::files::threadpath::absolute(&root) || crate::files::threadpath::absolute(&path)) { return vec![crate::files::threadpath::refusal(id)] }
                    if !kind.is_empty() && !super::known_root(hub, &root, &path).await { return vec![json!({"t":"reply","id":id,"ok":false,"error":"that folder is not in this project"})] }
                    // `held`: the client keeps this many sources by key (the GPUI window's store), so frames may be deltas against any of them
                    let keyed = p.get("held").and_then(Value::as_u64) == Some(CLIENT_SOURCES);
                    let t0 = std::time::Instant::now();
                    if kind == "split" { self.0.watch_split(hub, who.client, &root, &path, &s("tree"), keyed) }
                    else { self.0.watch(hub, who.client, kind_of(&kind), &root, &path, &s("tree"), keyed); }
                    plot::times(|| format!("{{\"stage\":\"watch_rpc\",\"kind\":{kind:?},\"path\":{path:?},\"ms\":{:.1}}}", t0.elapsed().as_secs_f64() * 1e3));
                    vec![json!({"t":"reply","id":id,"ok":true})]
                }
                "kicad.warm" => { // the saved versions before and after a step, plotted ahead
                    let (kind, root) = (s("kind"), s("root"));
                    for path in [s("before"), s("after")] {
                        if path.is_empty() || !super::known_root(hub, &root, &path).await { continue }
                        self.0.warm(hub, if kind == "schematic" { 1 } else { 0 }, &root, &path);
                    }
                    vec![json!({"t":"reply","id":id,"ok":true})]
                }
                "history.steps" => vec![json!({"t":"reply","id":id,"ok":true,"result": super::histsteps::steps(hub, &s("thread")).await})],
                "viewer.state" => { // the client says what its viewer shows (the thread is the raw id)
                    viewer_report(&s("thread"), p.get("state").cloned().unwrap_or(Value::Null));
                    vec![json!({"t":"reply","id":id,"ok":true})]
                }
                "kicad.probe" => {
                    let m = self.0.probe(who.client, &s("key"), &s("name"), &s("value")).await;
                    vec![json!({"t":"reply","id":id,"ok":true,"result":{"matches": m.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>()}})]
                }
                _ => { // kicad.info
                    let num = |k: &str| p.get(k).map(|v| v.as_u64().or_else(|| v.as_str().and_then(|x| x.parse().ok())).unwrap_or(0)).unwrap_or(0) as usize;
                    let text = self.0.info(who.client, &s("key"), num("at"), num("idx")).await;
                    vec![json!({"t":"reply","id":id,"ok":true,"result":{"info":text}})]
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// the GPUI window's Store.receive and Store.fail as far as which sources it holds (native/gpui/src/screens/plot.rs)
    #[derive(Default)] struct Store { held: Vec<String> }
    impl Store {
        fn receive(&mut self, k: &str, base: &str) -> bool { if !base.is_empty() && !self.held.iter().any(|h| h == base) { self.held.retain(|h| h != k); return false }
            if self.held.len() >= CLIENT_SOURCES as usize && !self.held.iter().any(|h| h == k) { self.held.clear() } if !self.held.iter().any(|h| h == k) { self.held.push(k.into()) } true }
        fn fail(&mut self, k: &str) { if k.is_empty() { self.held.clear() } else { self.held.retain(|h| h != k) } }
    }
    fn plot_of(n: u32) -> Arc<Plot> { Arc::new(Plot { cs: (0..n).map(|i| plot::Chunk { sig: i, layer: 1, color: 2, alpha: 3, mode: 1, width: 4, d: vec![i as u8; 8], info: String::new() }).collect(), b: Default::default(), thick: 0, solid: None, edge: Default::default() }) }
    // a keyed client's mirror names only sources its store still holds, whatever came before (an earlier connection's sources included)
    #[test] fn the_mirror_holds_what_the_window_holds() {
        let mut store = Store::default(); for k in ["0|/a|", "1|/a|", "1|/a|/a/s.kicad_sch", "0|/b|", "1|/b|"] { store.receive(k, ""); } // left from before
        let mut m = Mirror::default(); let p = plot_of(3);
        let keys: Vec<String> = (0..9).map(|i| format!("{}|/r{}|", i % 2, i % 4)).collect();
        let mut x = 7u32;
        for step in 0..400 {
            x = x.wrapping_mul(1103515245).wrapping_add(12345); let k = &keys[(x >> 16) as usize % keys.len()];
            if !m.synced { store.fail(""); m.held.clear(); m.last.clear(); m.synced = true }
            if (x >> 8) % 11 == 0 { m.dropped(k); store.fail(k); continue }
            let base = m.base(k);
            assert!(store.receive(k, &base), "step {step}: {k} against {base} that the window no longer holds");
            m.took(k, &p, if (x >> 4) % 37 == 0 { CLIENT_FRAME_MOST + 1 } else { 100 });
            if (x >> 4) % 37 == 0 { store.fail("") } // too big: Store.too_big empties it
            let mut a: Vec<&String> = m.held.iter().map(|h| &h.0).collect(); let mut b: Vec<&String> = store.held.iter().collect(); a.sort(); b.sort(); assert_eq!(a, b, "step {step}");
        }
    }
    // the mirror is exact only while it keeps the window's own rules: its store's size and its frame limit (in the same tree)
    #[test] fn the_mirror_has_the_windows_numbers() {
        let store = include_str!("../../../gpui/src/screens/plot.rs"); let sock = include_str!("../../../gpui/src/hub.rs");
        assert!(store.contains(&format!("const MAX_SOURCES: usize = {CLIENT_SOURCES};")), "native/gpui's Store keeps another number of sources: change CLIENT_SOURCES with it");
        assert!(CLIENT_FRAME_MOST == 32 << 20 && sock.contains("b.len()<=(32<<20)"), "native/gpui takes plot frames up to another size: change CLIENT_FRAME_MOST with it");
        assert!(store.contains("if self.held.len() >= MAX_SOURCES && !self.held.contains_key(&key) { self.held.clear(); self.frames.clear(); }"), "native/gpui's Store drops its sources another way: Mirror::took follows it");
    }
    #[test] fn a_source_not_held_is_sent_against_its_likeness() {
        let mut m = Mirror::default(); let p = plot_of(2);
        for k in ["0|/a|", "1|/a|/a/pcie.kicad_sch", "1|/a|/a/power.kicad_sch", "1|/a|"] { m.took(k, &p, 10) }
        assert_eq!(m.base("1|/a|"), "1|/a|"); assert_eq!(m.base("1|/w|/w/pcie.kicad_sch"), "1|/a|/a/pcie.kicad_sch"); assert_eq!(m.base("1|/w|"), "1|/a|");
        m.took("1|/a|/a/power.kicad_sch", &p, 10); assert_eq!(m.base("1|/w|"), "1|/a|", "the root sheet against the root sheet");
        assert_eq!(m.base("1|/w|/w/new.kicad_sch"), "1|/a|"); assert_eq!(m.base("0|/w|"), "0|/a|"); assert_eq!(Mirror::default().base("0|/w|"), "");
        m.took("0|/a|", &p, 10); assert_eq!(m.base("2|/a|"), "0|/a|", "no likeness: the last applied");
    }
    #[test] fn plots_kept_by_content_are_bounded_and_the_oldest_go() {
        let mut made = Made { cap: 3 * weight(&plot_of(100)) + 10, ..Default::default() };
        for i in 0..5u64 { made.put((0, (i, 0)), plot_of(100), Arc::new(String::new())); if i == 3 { assert!(made.get(&(0, (1, 0))).is_some()) } }
        assert!(made.bytes <= made.cap); assert_eq!(made.m.len(), 3);
        assert!(made.get(&(0, (1, 0))).is_some(), "used lately: kept"); assert!(made.get(&(0, (0, 0))).is_none() && made.get(&(0, (2, 0))).is_none());
    }
    #[test] fn the_same_board_in_another_folder_is_not_plotted_again() {
        let d = std::env::temp_dir().join(format!("bp-watch-made-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d);
        for w in ["a", "b"] { std::fs::create_dir_all(d.join(w)).unwrap(); std::fs::write(d.join(w).join("x.kicad_pcb"), "(kicad_pcb (version 20240108) (gr_line (start 0 0) (end 10 5) (layer \"F.Cu\") (width 0.2)))\n").unwrap() }
        let plots = Plots::new(&d.join("home")); let f = |w: &str| d.join(w).join("x.kicad_pcb").to_string_lossy().into_owned();
        let (ia, ib) = (d.join("ia").to_string_lossy().into_owned(), d.join("ib").to_string_lossy().into_owned());
        let (ca, _, pa) = plots.plot_2d(0, &f("a"), &ia); let (cb, _, pb) = plots.plot_2d(0, &f("b"), &ib);
        assert_eq!((ca, cb), (0, 0)); let (pa, pb) = (pa.unwrap(), pb.unwrap()); assert!(Arc::ptr_eq(&pa, &pb)); assert!(!pa.cs.is_empty());
        assert_eq!(std::fs::read(&ia).unwrap(), std::fs::read(&ib).unwrap(), "each source has its info file");
        let direct = match plotread::read(0, &f("a")) { plotgen::Read::Plot(p, _) => *p, _ => panic!() }; assert_eq!(*pa, direct, "the kept plot is the reader's");
        std::fs::write(d.join("b").join("x.kicad_pcb"), "(kicad_pcb (version 20240108) (gr_line (start 0 0) (end 10 6) (layer \"F.Cu\") (width 0.2)))\n").unwrap();
        let (_, _, pc) = plots.plot_2d(0, &f("b"), &ib); assert!(!Arc::ptr_eq(&pa, &pc.unwrap()), "other bytes, another plot");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn sheet_tree_roots() {
        assert_eq!(tree_root("/p/top.kicad_sch", "", ""), "/p/top.kicad_sch"); assert_eq!(tree_root("/p/top.kicad_sch", "/p/sub.kicad_sch", "/p/other.kicad_sch"), "/p/other.kicad_sch");
        assert_eq!(tree_root("", "/p/x.kicad_sch", ""), "/p/x.kicad_sch"); assert_eq!(tree_root("/p/top.kicad_sch", "diff\na\nb", ""), "/p/top.kicad_sch"); assert_eq!(tree_root("", "diff\na\nb", ""), "");
        let v = "git\t/g\tabc\t2\t/p/sub.kicad_sch"; assert_eq!(tree_root("/p/top.kicad_sch", v, ""), "git\t/g\tabc\t2\t/p/top.kicad_sch"); assert_eq!(tree_slot(v), "/g\tabc"); assert_eq!(tree_slot("/p/x"), "");
    }
    #[test] fn keys_and_families() {
        assert_eq!(key(0, "/p", ""), "0|/p|"); assert_eq!(fnv(""), 2166136261); assert_eq!(fnv("a"), 0xe40c292c);
        assert_eq!(family("0|/p|b.kicad_pcb"), "0|/p|b.kicad_pcb");
        assert_eq!(family("0|/r|git\t/r/.git\tabc\t2\t/r/b.kicad_pcb"), "0|/r|/r/b.kicad_pcb");
        assert_eq!(family("0|/r|diff\ngit\tg\told\t2\t/r/b.kicad_pcb\ngit\tg\tnew\t2\t/r/b.kicad_pcb"), "0|/r|/r/b.kicad_pcb");
        assert_eq!(kind_of("3d-full"), Some(4)); assert_eq!(kind_of(""), None); assert_eq!(kind_of("split:sb3"), None);
        assert!(is_version("git\tx") && is_version("diff\na") && !is_version("/p/b.kicad_pcb"));
    }
}

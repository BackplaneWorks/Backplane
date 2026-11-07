//! server/plot's actor in Rust: the board, schematic and 3D sources the clients watch (`kicad.watch`), polled every
//! 150 ms while any is watched, each changed file read, the plot sent to each client as the delta against what that client
//! holds. Every source (boards, schematics, sheet trees, info lookups: kicad/plotread.rs; 3D models:
//! kicad/solidread.rs) is read in this process, the same bytes as child, on blocking threads, at most `READERS` at once and one
//! 3D model at a time. `BACKPLANE_PLOT_NATIVE=0` sends them all to the child again (`backplane --plot KIND FILE INFOS`, `BACKPLANE_PLOT_BIN`,
//! default `backplane-serve` beside the hub).
//! Not ported yet: web stamps. A split view (`kicad.watch` kind "split") watches the schematic, board and 3D model at once (`watch_split`).
use super::{plot::{self, Plot}, plotgen, plotread, project, solid, solidread, stepglb};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, sync::{Arc, Mutex}, time::Duration};

const READERS: usize = 2;
const POLL_MS: u64 = 150;
const KEPT: usize = 4; // sources no client watches are kept (plots included): Src.kept
const VERSIONS_KEPT: usize = 8; // saved versions no view watches: Src.versions.kept

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Stamp { rev: String, ok: bool }
pub fn stamp(path: &str) -> Stamp {
    use crate::plat::fs::MetadataExt;
    // FS.revision: "dev:ino:size:mtime s:ns:ctime s:ns" (a same-size save within a second still changes it)
    match std::fs::metadata(path) { Ok(m) => Stamp { rev: format!("{}:{}:{}:{}:{}:{}:{}", m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec()), ok: true }, Err(_) => Stamp::default() }
}
/// A saved version or a comparison: immutable, read once.
pub fn is_version(path: &str) -> bool { path.starts_with("git\t") || path.starts_with("diff\n") }
pub fn solid_kind(k: u32) -> bool { matches!(k, 2..=4) }
pub fn key(kind: u32, root: &str, path: &str) -> String { format!("{kind}|{root}|{path}") }
pub fn kind_of(name: &str) -> Option<u32> { Some(match name { "board" => 0, "schematic" => 1, "3d" => 2, "3d-bare" => 3, "3d-full" => 4, _ => return None }) }
/// FNV-1a of a key (Key.hash), the info file's name.
pub fn fnv(s: &str) -> u32 { s.chars().fold(2166136261u32, |h, c| (h ^ c as u32).wrapping_mul(16777619)) }

#[derive(Default)]
struct Src { kind: u32, file: String, stamp: Option<Stamp>, busy: bool, plot: Option<Arc<Plot>>, again: bool }
/// `kept`: the id (`plot::mesh_id`) of the model the client already holds from an earlier visit; that model is not sent again
struct Sub { key: String, held: Vec<plot::Chunk>, mesh: Vec<u8>, fresh: bool, base: String, kept: u64 }
#[derive(Default)]
struct State { srcs: HashMap<String, Src>, subs: HashMap<u64, Vec<Sub>>, polling: bool, idle: Vec<String> }

pub struct Plots { gen: std::sync::atomic::AtomicU64, st: Mutex<State>, bin: PathBuf, dir: PathBuf, home: PathBuf, readers: tokio::sync::Semaphore, solid: tokio::sync::Mutex<()> }

impl Plots {
    pub fn new(home: &std::path::Path) -> Arc<Plots> {
        let bin = std::env::var_os("BACKPLANE_PLOT_BIN").map(PathBuf::from).unwrap_or_else(|| std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("backplane-serve"))).unwrap_or_else(|| "backplane-serve".into()));
        let dir = home.join("plot"); let _ = std::fs::create_dir_all(&dir);
        Arc::new(Plots { gen: Default::default(), st: Mutex::new(State::default()), bin, dir, home: home.to_path_buf(), readers: tokio::sync::Semaphore::new(READERS), solid: tokio::sync::Mutex::new(()) })
    }
    fn infos(&self, key: &str) -> PathBuf { self.dir.join(format!("{}.info", fnv(key))) }

    /// kicad.watch: kind "board", "schematic", "3d", "3d-bare" or "3d-full" shows it, anything else stops. The client's
    /// other views are dropped; a new plot view starts from the chunks of the one it replaces (Sub.carry), a 3D view also
    /// watches the board's plot beside the model. `kept` names the model the client has cached (0: none).
    pub fn watch(self: &Arc<Self>, hub: &Arc<Hub>, client: u64, kind: Option<u32>, root: &str, path: &str, tree: &str, kept: u64) {
        let Some(kind) = kind else { self.stop(client); return };
        let board = |k: u32| if path.is_empty() { let f = project::read(root); if k == 1 { f.sch } else { f.pcb } } else { path.to_string() };
        let file = board(kind);
        if file.is_empty() { // Pv.watch.k none: nothing to show
            self.drop_subs(client);
            hub.push(client, json!({"t":"plot","key":key(kind, root, path),"none": if kind == 1 { "No schematic in this project yet" } else { "No board in this project yet" }}));
            return
        }
        let is_board = file.ends_with(".kicad_pcb"); // Pv.watch.go: only a board's model has a board beside it
        let mut views = vec![(kind, key(kind, root, path), file)];
        if solid_kind(kind) && is_board { views.push((0, key(0, root, path), board(0))) }
        let mut todo = vec![];
        { let mut st = self.st.lock().unwrap();
          let carry = st.subs.get(&client).and_then(|v| v.iter().find(|s| s.key.starts_with("0|") || s.key.starts_with("1|"))).map(|s| (s.held.clone(), s.base.clone())).unwrap_or_default();
          st.subs.remove(&client);
          for (i, (k, ky, f)) in views.into_iter().enumerate() {
              let carries = i == 0 && !solid_kind(k);
              st.subs.entry(client).or_default().push(Sub { key: ky.clone(), held: if carries { carry.0.clone() } else { vec![] }, mesh: vec![], fresh: true, base: if carries { carry.1.clone() } else { String::new() }, kept: if solid_kind(k) { kept } else { 0 } });
              let s = st.srcs.entry(ky.clone()).or_insert_with(|| Src { kind: k, file: f.clone(), ..Default::default() });
              s.file = f;
              if !s.busy && !is_version(&s.file) && s.stamp.as_ref() != Some(&stamp(&s.file)) { s.plot = None } // Pv.fresh: a plot of an older file is not sent
              todo.push(ky);
          }
          self.gc(&mut st);
          if !st.polling { st.polling = true; let me = self.clone(); let hub = hub.clone(); tokio::spawn(async move { me.poll(hub).await }); } }
        for ky in todo { self.fan_one(hub, &ky, Some(client)) }
        if kind == 1 { self.sheets(hub, client, root, path, tree) }
    }
    /// kicad.watch kind "split": the schematic (`path` is its sheet, "" the root), the board and the board's 3D model at once, each its own
    /// plot (full chunks, no carry: the panes are separate sources) and the 3D model's board plot shared with the board pane.
    pub fn watch_split(self: &Arc<Self>, hub: &Arc<Hub>, client: u64, root: &str, path: &str, tree: &str) {
        let f = project::read(root);
        let sch = if path.is_empty() { f.sch.clone() } else { path.to_string() };
        let mut views: Vec<(u32, String, String)> = vec![];
        if !sch.is_empty() { views.push((1, key(1, root, path), sch.clone())) }
        if !f.pcb.is_empty() { views.push((0, key(0, root, ""), f.pcb.clone())); views.push((2, key(2, root, ""), f.pcb.clone())) }
        let mut todo = vec![];
        { let mut st = self.st.lock().unwrap();
          st.subs.remove(&client);
          for (k, ky, file) in views {
              st.subs.entry(client).or_default().push(Sub { key: ky.clone(), held: vec![], mesh: vec![], fresh: true, base: String::new(), kept: 0 });
              let s = st.srcs.entry(ky.clone()).or_insert_with(|| Src { kind: k, file: file.clone(), ..Default::default() });
              s.file = file;
              if !s.busy && !is_version(&s.file) && s.stamp.as_ref() != Some(&stamp(&s.file)) { s.plot = None }
              todo.push(ky);
          }
          self.gc(&mut st);
          if !st.polling { st.polling = true; let me = self.clone(); let hub = hub.clone(); tokio::spawn(async move { me.poll(hub).await }); } }
        for ky in todo { self.fan_one(hub, &ky, Some(client)) }
        if !sch.is_empty() { self.sheets(hub, client, root, path, tree) }
    }
    pub fn stop(&self, client: u64) { self.drop_subs(client) }
    fn drop_subs(&self, client: u64) {
        let mut st = self.st.lock().unwrap(); st.subs.remove(&client); self.gc(&mut st);
    }
    fn gc(&self, st: &mut State) {
        let watched: std::collections::HashSet<String> = st.subs.values().flatten().map(|s| s.key.clone()).collect();
        let mut keep_idle: Vec<String> = vec![];
        for (limit, versions) in [(KEPT, false), (VERSIONS_KEPT, true)] {
            let mut idle: Vec<String> = st.srcs.iter().filter(|(k, s)| !watched.contains(*k) && is_version(&s.file) == versions).map(|(k, _)| k.clone()).collect();
            idle.sort_by_key(|k| !st.idle.contains(k)); // already idle ones are the older
            let drop = idle.len().saturating_sub(limit);
            for k in idle.iter().take(drop) { st.srcs.remove(k); }
            keep_idle.extend(idle.into_iter().skip(drop));
        }
        st.idle = keep_idle;
    }
    /// kicad.warm / PWarm: a saved version plotted ahead and kept, no view watching it (scrubbing the history comes back to it at once).
    pub fn warm(self: &Arc<Self>, hub: &Arc<Hub>, kind: u32, root: &str, path: &str) {
        if path.is_empty() || kind > 1 { return }
        let ky = key(kind, root, path);
        let need = { let mut st = self.st.lock().unwrap();
            let s = st.srcs.entry(ky.clone()).or_insert_with(|| Src { kind, file: path.to_string(), ..Default::default() });
            let need = s.plot.is_none() && !s.busy; self.gc(&mut st); need };
        if need { self.read(hub, &ky) }
    }

    /// Bring watchers of `key` (or just `only`) up to its plot; a source with none yet is read.
    fn fan_one(self: &Arc<Self>, hub: &Arc<Hub>, ky: &str, only: Option<u64>) {
        let (plot, need_read, kind) = { let st = self.st.lock().unwrap(); match st.srcs.get(ky) { Some(s) => (s.plot.clone(), s.plot.is_none() && !s.busy, s.kind), None => return } };
        if let Some(p) = plot { self.send_to(hub, ky, &p, only); self.revision(hub, ky, only); if solid_kind(kind) { self.note(hub, ky, only) } }
        if need_read { self.read(hub, ky) }
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
        { let mut st = self.st.lock().unwrap();
          for (client, subs) in st.subs.iter_mut() {
              if only.map_or(false, |o| o != *client) { continue }
              for sub in subs.iter_mut().filter(|s| s.key == ky) {
                  let ok = if let Some(so) = &p.solid { // Pv.send.solid
                      if so.d == sub.mesh { true }
                      else if sub.kept != 0 && plot::mesh_id(&so.d) == sub.kept { sub.mesh = so.d.clone(); sub.kept = 0; true } // the client's cached model is this one
                      else { let ok = hub.push_raw(*client, plot::solid_frame(ky, so)); sub.mesh = so.d.clone(); sub.kept = 0; ok }
                  } else {
                      let rs = plot::delta(&p.cs, &sub.held);
                      let ok = if !sub.fresh && plot::quiet(&rs, sub.held.len()) { true } else { hub.push_raw(*client, plot::frame(ky, &family(ky), &sub.base, p, &rs)) };
                      sub.held = p.cs.clone(); sub.fresh = false; sub.base = ky.to_string(); ok
                  };
                  if !ok { gone.push(*client) }
              }
          } }
        for c in gone { self.stop(c) }
    }

    fn read(self: &Arc<Self>, hub: &Arc<Hub>, ky: &str) {
        let (kind, file) = { let mut st = self.st.lock().unwrap(); let Some(s) = st.srcs.get_mut(ky) else { return }; if s.busy { s.again = true; return } s.busy = true; (s.kind, s.file.clone()) };
        let (me, hub, ky) = (self.clone(), hub.clone(), ky.to_string());
        tokio::spawn(async move {
            let _slot = me.readers.acquire().await.ok();
            // Pv.read.at: a model whose GLB is known too big is refused at once, not behind another model's read
            let known = if solid_kind(kind) { solidread::known(kind, &file) } else { 0 };
            let _one = if solid_kind(kind) && solid::fits(known) { Some(me.solid.lock().await) } else { None };
            let before = if is_version(&file) { Stamp::default() } else { stamp(&file) };
            let out = if !solid::fits(known) { Some((3, solid::big(known), None)) } else if native() { me.run_native(&hub, kind, &file, &ky).await } else { me.run_child(kind, &file, &ky).await.map(|(c, t)| (c, t, None)) };
            let after = if is_version(&file) { Stamp::default() } else { stamp(&file) };
            let (code, text, made) = match out { Some((c, t, m)) => if before != after { (2, String::new(), None) } else { (c, t, m) }, None => (1, "cannot run the plot reader".into(), None) };
            let mut again = false;
            let plot = if code == 0 { Some(Arc::new(made.unwrap_or_else(|| plot::parse_child(&text)))) } else { None };
            { let mut st = me.st.lock().unwrap();
              if let Some(s) = st.srcs.get_mut(&ky) { s.busy = false; s.stamp = Some(before); again = std::mem::take(&mut s.again); if let Some(p) = &plot { s.plot = Some(p.clone()) } } }
            match (&plot, code) {
                (Some(p), _) => { me.send_to(&hub, &ky, p, None); me.revision(&hub, &ky, None) }
                (None, 0 | 2) => {} // half written or read again: the next poll sees the file change
                (None, _) => { // Pv.fail: every watcher told why, and holding nothing
                    let why: String = text.trim().chars().take(200).collect();
                    let mut st = me.st.lock().unwrap(); let mut gone = vec![];
                    for (c, subs) in st.subs.iter_mut() { for s in subs.iter_mut().filter(|s| s.key == ky) { s.held.clear(); if !hub.push(*c, json!({"t":"plot","key":ky,"none":why})) { gone.push(*c) } } }
                    drop(st); for c in gone { me.stop(c) } }
            }
            if matches!(code, 0 | 2) && (ky.starts_with("2|") || ky.starts_with("3|")) { me.note(&hub, &ky, None) } // Pv.handle PGot
            if again { me.read(&hub, &ky) }
        });
    }
    /// `Pv.child` in this process (kicad/plotread.rs, kicad/solidread.rs): the plot, its info file written, and the child's exit code and
    /// message. A panic in a reader is an unread file, never the hub's end; a big file's memory goes back when it is done.
    async fn run_native(&self, hub: &Arc<Hub>, kind: u32, file: &str, ky: &str) -> Option<(i32, String, Option<Plot>)> {
        let (f, infos) = (file.to_string(), self.infos(ky));
        if solid_kind(kind) {
            let (cli, home) = (super::ipc::runtime(hub).await.cli, self.home.clone());
            return tokio::task::spawn_blocking(move || {
                let i = infos.to_string_lossy().into_owned();
                let r = std::panic::catch_unwind(|| solidread::read(kind, &f, &cli, &stepglb::machine(&home), &i));
                let o = match r { Ok(g) => (g.code, g.text, g.plot), Err(_) => (3, plotgen::UNREAD.to_string(), None) };
                #[cfg(all(target_os = "linux", target_env = "gnu"))] unsafe { libc::malloc_trim(0); }
                o
            }).await.ok()
        }
        tokio::task::spawn_blocking(move || {
            let r = std::panic::catch_unwind(|| plotread::read(kind, &f));
            let o = match r {
                Ok(plotgen::Read::Plot(p, t)) => { plotgen::write_infos(&infos.to_string_lossy(), &t); (0, String::new(), Some(*p)) }
                Ok(plotgen::Read::Half) => (2, String::new(), None),
                Ok(plotgen::Read::Fail(m)) => (3, m, None),
                Err(_) => (3, plotgen::UNREAD.to_string(), None),
            };
            #[cfg(all(target_os = "linux", target_env = "gnu"))] unsafe { libc::malloc_trim(0); }
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
              for c in gone { st.subs.remove(&c); }
              self.gc(&mut st);
              if st.subs.is_empty() { st.polling = false; return }
              let watched: std::collections::HashSet<String> = st.subs.values().flatten().map(|s| s.key.clone()).collect();
              for (k, s) in st.srcs.iter_mut() {
                  if !watched.contains(k) || s.busy || is_version(&s.file) { continue }
                  let now = stamp(&s.file);
                  if s.stamp.as_ref() != Some(&now) { changed.push(k.clone()) }
              } }
            for k in changed { self.read(&hub, &k) }
        }
    }

    /// Pv.tree: a schematic's sheets (and the boxes a tap on a sheet symbol opens), read by `--plot-tree FILE` and sent as
    /// `{"t":"sheets", root, slot, gen, rows, boxes, ids, top, refs}`; `gen` only grows (the client takes a reply only if newer).
    fn sheets(self: &Arc<Self>, hub: &Arc<Hub>, client: u64, root: &str, path: &str, tree: &str) {
        let gen = self.gen.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let file = tree_root(&project::read(root).sch, path, tree);
        if file.is_empty() { return }
        let (me, hub, root, slot) = (self.clone(), hub.clone(), root.to_string(), tree_slot(path));
        tokio::spawn(async move {
            if native() {
                let f = file.clone();
                let Ok(Ok(t)) = tokio::task::spawn_blocking(move || std::panic::catch_unwind(|| plotread::tree(&f))).await else { return };
                hub.push(client, json!({"t":"sheets","root":root,"slot":slot,"gen":gen.to_string(),"rows":t.rows,"boxes":t.boxes,"ids":t.ids,"top":t.top,"refs":t.refs}));
                return
            }
            let Some(o) = crate::proc::tokio_cmd(crate::proc::Kind::Local, &me.bin).arg("--plot-tree").arg(&file).stdin(std::process::Stdio::null()).output().await.ok() else { return };
            if !o.status.success() { return }
            let text = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let j: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            let g = |k: &str, d: &str| j.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
            hub.push(client, json!({"t":"sheets","root":root,"slot":slot,"gen":gen.to_string(),"rows":g("rows", text.trim_end()),"boxes":g("boxes",""),"ids":g("ids",""),"top":g("top",""),"refs":g("refs","")}));
        });
    }
    /// kicad.probe: the pieces of the chunks this client holds for `key` whose info has `name=value` (a net, a part), as (chunk index, piece index): the cross-probe's light.
    pub async fn probe(&self, client: u64, key_: &str, name: &str, value: &str) -> Vec<(usize, usize)> {
        let sigs: Vec<u32> = { let st = self.st.lock().unwrap(); st.subs.get(&client).and_then(|v| v.iter().find(|s| s.key == key_)).map(|s| s.held.iter().map(|c| c.sig).collect()).unwrap_or_default() };
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
        let sig = { let st = self.st.lock().unwrap(); st.subs.get(&client).and_then(|v| v.iter().find(|s| s.key == key_)).and_then(|s| s.held.get(at)).map(|c| c.sig) };
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
                    if kind == "split" { self.0.watch_split(hub, who.client, &root, &path, &s("tree")) }
                    else { self.0.watch(hub, who.client, kind_of(&kind), &root, &path, &s("tree"), u64::from_str_radix(&s("kept"), 16).unwrap_or(0)); }
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

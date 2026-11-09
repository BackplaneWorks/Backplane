//! server/plot's actor in Rust: the board, schematic and 3D sources the clients watch (`kicad.watch`), polled every
//! 150 ms while any is watched, each changed file read, the plot sent to each client as the delta against what that client
//! holds. Every source (boards, schematics, sheet trees, info lookups: kicad/plotread.rs; 3D models:
//! kicad/solidread.rs) is read in this process, the same bytes as child, on blocking threads, at most `READERS` at once and one
//! 3D model at a time. `BACKPLANE_PLOT_NATIVE=0` sends them all to the child again (`backplane --plot KIND FILE INFOS`, `BACKPLANE_PLOT_BIN`,
//! default `backplane-serve` beside the hub).
//! Not ported yet: web stamps. A split view (`kicad.watch` kind "split") watches the schematic, board and 3D model at once (`watch_split`).
use super::{pcb3d, plot::{self, Plot}, plotgen, plotread, project, solid, solidread, stepglb};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, sync::{Arc, Mutex}, time::Duration};

const READERS: usize = 2;
/// The most Mech parts meshed ahead when one is shown.
const WARM_MECH: usize = 24;
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
/// What the hub is doing to make a 3D source's model, in the person's words: `native` a board made here (pcb3d), `cached` a converted model already kept.
pub fn stage_text(kind: u32, file: &str, native: bool, cached: bool, tools: &[stepglb::Mesher]) -> String {
    let path = file.rsplit(['\t', '\n']).next().unwrap_or(file);
    let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tool = match tools.first() { Some(stepglb::Mesher::Step2glb(_)) => "OpenCascade", Some(stepglb::Mesher::FreeCad { .. }) => "FreeCAD", None => "" };
    if native { return format!("Making the board's 3D model from {name}…") }
    if cached || path.ends_with(".glb") { return format!("Reading {name}…") }
    if solidread::is_step(path) { return if tool.is_empty() { format!("Meshing {name}…") } else { format!("Meshing {name} with {tool}…") } }
    let _ = kind; format!("Exporting the board's 3D model with KiCad (kicad-cli)…")
}
pub fn key(kind: u32, root: &str, path: &str) -> String { format!("{kind}|{root}|{path}") }
pub fn kind_of(name: &str) -> Option<u32> { Some(match name { "board" => 0, "schematic" => 1, "3d" => 2, "3d-bare" => 3, "3d-full" => 4, _ => return None }) }
/// FNV-1a of a key (Key.hash), the info file's name.
pub fn fnv(s: &str) -> u32 { s.chars().fold(2166136261u32, |h, c| (h ^ c as u32).wrapping_mul(16777619)) }

#[derive(Default)]
/// `parts`: a board model's part frames so far (the one that starts them first), `stop` its stream's (set when another replaces it)
struct Src { kind: u32, file: String, stamp: Option<Stamp>, busy: bool, plot: Option<Arc<Plot>>, again: bool, parts: Vec<Arc<Vec<u8>>>, stop: Option<Arc<std::sync::atomic::AtomicBool>> }
/// `kept`: the id (`plot::mesh_id`) of the model the client already holds from an earlier visit; that model is not sent again
/// `parts_at`: how many of its source's part frames this watcher has been sent
struct Sub { key: String, held: Vec<plot::Chunk>, mesh: Vec<u8>, fresh: bool, base: String, kept: u64, parts_at: usize }
#[derive(Default)]
/// `prewarmed`: each board whose part models were queued for conversion, by the revision it was then (pcb3d::stream::prewarm)
/// `took`: how long the last read of each 3D source's file took (ms), told as the estimate of the next one
/// `mech_warming`: the projects whose Mech parts are being meshed ahead (warm_mech)
struct State { srcs: HashMap<String, Src>, subs: HashMap<u64, Vec<Sub>>, polling: bool, idle: Vec<String>, prewarmed: HashMap<String, String>, took: HashMap<String, u64>, mech_warming: std::collections::HashSet<String> }

pub struct Plots { gen: std::sync::atomic::AtomicU64, st: Mutex<State>, bin: PathBuf, dir: PathBuf, home: PathBuf, readers: tokio::sync::Semaphore, solid: tokio::sync::Mutex<()> }

impl Plots {
    pub fn new(home: &std::path::Path) -> Arc<Plots> {
        let bin = std::env::var_os("BACKPLANE_PLOT_BIN").map(PathBuf::from).unwrap_or_else(|| std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(format!("backplane-serve{}", std::env::consts::EXE_SUFFIX)))).unwrap_or_else(|| "backplane-serve".into()));
        let dir = home.join("plot"); let _ = std::fs::create_dir_all(&dir);
        Arc::new(Plots { gen: Default::default(), st: Mutex::new(State::default()), bin, dir, home: home.to_path_buf(), readers: tokio::sync::Semaphore::new(READERS), solid: tokio::sync::Mutex::new(()) })
    }
    fn infos(&self, key: &str) -> PathBuf { self.dir.join(format!("{}.info", fnv(key))) }

    /// kicad.watch: kind "board", "schematic", "3d", "3d-bare" or "3d-full" shows it, anything else stops. The client's
    /// other views are dropped; a new plot view starts from the chunks of the one it replaces (Sub.carry), a 3D view also
    /// watches the board's plot beside the model. `kept` names the model the client has cached: its key ("" for any of this view's) and id (0: none).
    /// Returns the keys of the plots this view shows, so a client can drop what arrives late for the view it left.
    pub fn watch(self: &Arc<Self>, hub: &Arc<Hub>, client: u64, kind: Option<u32>, root: &str, path: &str, tree: &str, kept: (&str, u64)) -> Vec<String> {
        let Some(kind) = kind else { self.stop(client); return vec![] };
        let board = |k: u32| if path.is_empty() { let f = project::read(root); if k == 1 { f.sch } else { f.pcb } } else { path.to_string() };
        let file = board(kind);
        if file.is_empty() { // Pv.watch.k none: nothing to show
            self.drop_subs(client);
            hub.push(client, json!({"t":"plot","key":key(kind, root, path),"none": if kind == 1 { "No schematic in this project yet" } else { "No board in this project yet" }}));
            return vec![key(kind, root, path)]
        }
        let is_board = file.ends_with(".kicad_pcb"); // Pv.watch.go: only a board's model has a board beside it
        if !solid_kind(kind) { self.prewarm(hub, &if is_board { file.clone() } else { project::read(root).pcb }) } // a 3D view converts its parts itself, at full speed
        if solid_kind(kind) && solidread::is_step(&file) && !is_version(&file) { self.warm_mech(root, &file) }
        let mut views = vec![(kind, key(kind, root, path), file)];
        if solid_kind(kind) && is_board { views.push((0, key(0, root, path), board(0))) }
        let keys: Vec<String> = views.iter().map(|v| v.1.clone()).collect();
        let mut todo = vec![];
        { let mut st = self.st.lock().unwrap();
          let carry = st.subs.get(&client).and_then(|v| v.iter().find(|s| s.key.starts_with("0|") || s.key.starts_with("1|"))).map(|s| (s.held.clone(), s.base.clone())).unwrap_or_default();
          // a model this client was already sent for the same source stays sent: a second watch of it (the Mech page's own beside the
          // view's) does not send its frames again
          let mut had: Vec<(String, Vec<u8>, usize)> = st.subs.remove(&client).unwrap_or_default().into_iter().filter(|s| !s.mesh.is_empty()).map(|s| (s.key, s.mesh, s.parts_at)).collect();
          for (i, (k, ky, f)) in views.into_iter().enumerate() {
              let carries = i == 0 && !solid_kind(k);
              let (mesh, parts_at) = had.iter().position(|h| h.0 == ky).map(|i| { let h = had.swap_remove(i); (h.1, h.2) }).unwrap_or_default();
              st.subs.entry(client).or_default().push(Sub { key: ky.clone(), held: if carries { carry.0.clone() } else { vec![] }, mesh, fresh: true, base: if carries { carry.1.clone() } else { String::new() }, kept: if solid_kind(k) && (kept.0.is_empty() || kept.0 == ky) { kept.1 } else { 0 }, parts_at });
              let s = st.srcs.entry(ky.clone()).or_insert_with(|| Src { kind: k, file: f.clone(), ..Default::default() });
              s.file = f;
              if !s.busy && !is_version(&s.file) && s.stamp.as_ref() != Some(&stamp(&s.file)) { s.plot = None } // Pv.fresh: a plot of an older file is not sent
              todo.push(ky);
          }
          self.gc(&mut st);
          if !st.polling { st.polling = true; let me = self.clone(); let hub = hub.clone(); tokio::spawn(async move { me.poll(hub).await }); } }
        for ky in todo { self.fan_one(hub, &ky, Some(client)) }
        if kind == 1 { self.sheets(hub, client, root, path, tree) }
        keys
    }
    /// kicad.watch kind "split": the schematic (`path` is its sheet, "" the root), the board and the board's 3D model at once, each its own
    /// plot (full chunks, no carry: the panes are separate sources) and the 3D model's board plot shared with the board pane.
    pub fn watch_split(self: &Arc<Self>, hub: &Arc<Hub>, client: u64, root: &str, path: &str, tree: &str) -> Vec<String> {
        let f = project::read(root);
        let sch = if path.is_empty() { f.sch.clone() } else { path.to_string() };
        let mut views: Vec<(u32, String, String)> = vec![];
        if !sch.is_empty() { views.push((1, key(1, root, path), sch.clone())) }
        if !f.pcb.is_empty() { views.push((0, key(0, root, ""), f.pcb.clone())); views.push((2, key(2, root, ""), f.pcb.clone())) }
        let keys: Vec<String> = views.iter().map(|v| v.1.clone()).collect();
        let mut todo = vec![];
        { let mut st = self.st.lock().unwrap();
          st.subs.remove(&client);
          for (k, ky, file) in views {
              st.subs.entry(client).or_default().push(Sub { key: ky.clone(), held: vec![], mesh: vec![], fresh: true, base: String::new(), kept: 0, parts_at: 0 });
              let s = st.srcs.entry(ky.clone()).or_insert_with(|| Src { kind: k, file: file.clone(), ..Default::default() });
              s.file = file;
              if !s.busy && !is_version(&s.file) && s.stamp.as_ref() != Some(&stamp(&s.file)) { s.plot = None }
              todo.push(ky);
          }
          self.gc(&mut st);
          if !st.polling { st.polling = true; let me = self.clone(); let hub = hub.clone(); tokio::spawn(async move { me.poll(hub).await }); } }
        for ky in todo { self.fan_one(hub, &ky, Some(client)) }
        if !sch.is_empty() { self.sheets(hub, client, root, path, tree) }
        keys
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
            for k in idle.iter().take(drop) { if let Some(f) = st.srcs.remove(k).and_then(|s| s.stop) { f.store(true, std::sync::atomic::Ordering::Relaxed) } }
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
        if let Some(p) = plot { self.send_to(hub, ky, &p, only); self.sync_parts(hub, ky, only); self.revision(hub, ky, only); if solid_kind(kind) { self.note(hub, ky, only) } }
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
    /// A 3D source's loading stage to its watchers: what the hub is doing for it now (`text`, "" when it is done), for how long already
    /// (`ms`) and, when known, about how long it took the last time (`about`, ms; 0 unknown). The window shows the text with the time running.
    /// A frame like a plot's (binary to a window), with neither `mesh`, `none` nor `parts`: a phone that does not know it passes it by.
    fn stage(&self, hub: &Arc<Hub>, ky: &str, text: &str, ms: u64, about: u64) {
        let who: Vec<u64> = { let st = self.st.lock().unwrap(); st.subs.iter().filter(|(_, v)| v.iter().any(|s| s.key == ky)).map(|(c, _)| *c).collect() };
        for c in who { hub.push(c, json!({"t":"plot","key":ky,"stage":text,"ms":ms,"about":about})); }
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

    /// Each watcher of `key` (or just `only`) sent the part frames it has not had yet, in order.
    fn sync_parts(&self, hub: &Arc<Hub>, ky: &str, only: Option<u64>) {
        let mut gone = vec![];
        { let mut st = self.st.lock().unwrap();
          let frames = match st.srcs.get(ky) { Some(s) if !s.parts.is_empty() => s.parts.clone(), _ => return };
          for (client, subs) in st.subs.iter_mut() {
              if only.map_or(false, |o| o != *client) { continue }
              for sub in subs.iter_mut().filter(|s| s.key == ky) {
                  while sub.parts_at < frames.len() { if !hub.push_raw(*client, frames[sub.parts_at].to_vec()) { gone.push(*client); break } sub.parts_at += 1; }
              }
          } }
        for c in gone { self.stop(c) }
    }
    /// A board model's parts, streamed: the plan's frames go onto the source as they are made and out to its watchers. A newer model's
    /// stream stops this one (its flag), and so does the source leaving.
    fn start_parts(self: &Arc<Self>, hub: &Arc<Hub>, ky: &str, file: &str, rev: String, made: Arc<(pcb3d::stream::Plan, u64)>) {
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        { let mut st = self.st.lock().unwrap();
          let Some(s) = st.srcs.get_mut(ky) else { return };
          if let Some(old) = s.stop.replace(stop.clone()) { old.store(true, std::sync::atomic::Ordering::Relaxed) }
          s.parts.clear();
          for subs in st.subs.values_mut() { for sub in subs.iter_mut().filter(|s| s.key == ky) { sub.parts_at = 0 } } }
        let (me, hub, ky, file, home) = (self.clone(), hub.clone(), ky.to_string(), file.to_string(), self.home.clone());
        let rt = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            let tools = stepglb::machine(&home);
            let cache = pcb3d::models::Cache::standard();
            let emit = |f: Vec<u8>| {
                { let mut st = me.st.lock().unwrap();
                  if stop.load(std::sync::atomic::Ordering::Relaxed) { return } // checked under the lock that replaces it: no frame of an old stream lands after a new one starts
                  let Some(s) = st.srcs.get_mut(&ky) else { return };
                  s.parts.push(Arc::new(f)); }
                let (me, hub, ky) = (me.clone(), hub.clone(), ky.clone());
                rt.spawn(async move { me.sync_parts(&hub, &ky, None) });
            };
            let t0 = std::time::Instant::now();
            let say = |text: String| { if !stop.load(std::sync::atomic::Ordering::Relaxed) { me.stage(&hub, &ky, &text, t0.elapsed().as_millis() as u64, 0) } };
            let t = pcb3d::stream::parts(&ky, &rev, &made.0, made.1, &cache, &tools, &stop, &emit, &say);
            if !stop.load(std::sync::atomic::Ordering::Relaxed) { me.stage(&hub, &ky, "", t0.elapsed().as_millis() as u64, 0) }
            eprintln!("pcb3d: {} parts: first {} ms, all {:.0} ms ({} part frames, {} instances boxed, {} models failed)", file, t.first.map_or("-".into(), |f| format!("{f:.0}")), t.all, t.models, t.boxes, t.failed);
        });
    }
    /// A project's board opened (any of its views) or changed: its part models converted in the background, once a revision, so its 3D
    /// view finds them ready. Not for a saved version, nor when kicad-cli makes the 3D models (`viewer.board3d`).
    fn prewarm(self: &Arc<Self>, hub: &Arc<Hub>, pcb: &str) {
        if !pcb.ends_with(".kicad_pcb") || is_version(pcb) { return }
        let rev = stamp(pcb).rev;
        if rev.is_empty() { return }
        { let mut st = self.st.lock().unwrap();
          if st.prewarmed.get(pcb) == Some(&rev) { return }
          // a 3D view of this board is open: its own stream converts the parts, with every worker (a prewarm's one would hold it back)
          if st.subs.values().flatten().any(|s| solid_kind(s.key.split('|').next().and_then(|k| k.parse().ok()).unwrap_or(0)) && st.srcs.get(&s.key).map_or(false, |x| x.file == pcb)) { return }
          if st.prewarmed.len() > 64 { st.prewarmed.clear() }
          st.prewarmed.insert(pcb.to_string(), rev); }
        let (hub, pcb, home) = (hub.clone(), pcb.to_string(), self.home.clone());
        tokio::spawn(async move {
            if !Self::board3d_native(&hub).await { return }
            let cli = super::ipc::runtime(&hub).await.cli;
            let _ = tokio::task::spawn_blocking(move || {
                let bytes = std::fs::read(&pcb).unwrap_or_default();
                // the board's own model too, kept by content, so its first 3D view reads it back instead of meshing it (Thunderscope: 1.5 s)
                let rev = stamp(&pcb).rev;
                let _ = std::panic::catch_unwind(|| if let Ok(made) = pcb3d::stream::board_model_kept(&bytes, &std::path::Path::new(&stepglb::cache_dir()).join("boards"), !stepglb::have(&pcb3d::stream::glb_path(&pcb, &rev).to_string_lossy())) {
                    if !made.layers.is_empty() { let name = std::path::Path::new(&pcb).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(); pcb3d::stream::keep_glb(&made.layers, &name, &pcb, &rev) }
                });
                #[cfg(all(target_os = "linux", target_env = "gnu"))] unsafe { libc::malloc_trim(0); } // the meshing's memory goes back
                let r = std::panic::catch_unwind(|| pcb3d::stream::prewarm(&bytes, &pcb, &cli, &stepglb::machine(&home), &pcb3d::models::Cache::standard()));
                match r { Ok(Ok((n, fresh, ms))) if fresh > 0 => eprintln!("pcb3d: prewarm {pcb}: {fresh} of {n} part models converted in {ms:.0} ms"), Ok(Err(e)) => eprintln!("pcb3d: prewarm {pcb}: {e}"), _ => {} }
            }).await;
        });
    }
    /// A Mech part shown: the project's other STEP parts (its mech/ folder) meshed ahead, one at a time, at a low priority, so a press on
    /// their chip shows them in a moment instead of after FreeCAD's seconds. A project at a time; parts already meshed are skipped, and at
    /// most WARM_MECH of them (a big library folder is not meshed whole).
    fn warm_mech(self: &Arc<Self>, root: &str, shown: &str) {
        if root.is_empty() || !self.st.lock().unwrap().mech_warming.insert(root.to_string()) { return }
        let (me, root, shown, home) = (self.clone(), root.to_string(), shown.to_string(), self.home.clone());
        tokio::task::spawn_blocking(move || {
            let tools = stepglb::machine(&home);
            let (t, mut n) = (std::time::Instant::now(), 0);
            for rel in crate::present::viewer::mech_parts(&root).into_iter().take(WARM_MECH) {
                let abs = format!("{root}/{rel}");
                if abs == shown || !solidread::is_step(&abs) { continue }
                if solidread::warm(&abs, &tools) { n += 1 }
            }
            if n > 0 { eprintln!("mech: {root}: {n} parts meshed ahead in {:.0} ms", t.elapsed().as_secs_f64() * 1000.0) }
            me.st.lock().unwrap().mech_warming.remove(&root);
        });
    }
    /// Whether a board's 3D model is made here (pcb3d) rather than by kicad-cli: the setting `viewer.board3d` (`native`, the default, or `kicad-cli`).
    async fn board3d_native(hub: &Arc<Hub>) -> bool { hub.setting("viewer.board3d").await.as_deref() != Some("kicad-cli") }
    /// pcb3d: the bare board from its file (a saved version's too), and for a model with parts the plan of its parts, with the note of the
    /// missing ones written. The GLB goes into the models cache behind it, by the file's revision.
    async fn run_board3d(&self, hub: &Arc<Hub>, kind: u32, file: &str, ky: &str) -> Option<Result<(Plot, Option<Arc<(pcb3d::stream::Plan, u64)>>), String>> {
        let (f, infos, cli) = (file.to_string(), self.infos(ky), super::ipc::runtime(hub).await.cli);
        tokio::task::spawn_blocking(move || {
            let r = std::panic::catch_unwind(|| {
                let t = std::time::Instant::now();
                let bytes = plotread::version(&f);
                let abs = f.rsplit(['\t', '\n']).next().unwrap_or(&f).to_string();
                // kept by content: a reopened board (after a restart, in another worktree, an old version) is read back, not meshed again;
                // a file whose own GLB is not kept yet is meshed (its layers make that GLB)
                let glb_kept = is_version(&f) || stepglb::have(&pcb3d::stream::glb_path(&abs, &stamp(&f).rev).to_string_lossy());
                let made = pcb3d::stream::board_model_kept(&bytes, &std::path::Path::new(&stepglb::cache_dir()).join("boards"), !glb_kept)?;
                let tris = made.plot.solid.as_ref().map_or(0, |s| s.n as u64);
                eprintln!("pcb3d: {abs} board {:.0} ms (parse {:.0}, mesh {:.0}), {tris} triangles", t.elapsed().as_secs_f64() * 1000.0, made.ms.0, made.ms.1);
                let plan = if kind == 3 { None } else {
                    let env = pcb3d::models::Env::system((!cli.is_empty()).then(|| std::path::Path::new(&cli)));
                    let dir = pcb3d::stream::board_dir(&abs);
                    let emb = pcb3d::models::embedded(&bytes, &pcb3d::models::Cache::standard().embedded_dir()); // KiCad 9's models inside the board
                    let p = pcb3d::stream::plan(&made.board, made.seat, &|raw: &str| pcb3d::models::resolve_in(raw, &dir, &env, &emb));
                    plotgen::write_infos(&infos.to_string_lossy(), &pcb3d::stream::missing_note(&p));
                    Some(Arc::new((p, tris)))
                };
                if !is_version(&f) && !made.layers.is_empty() { // the GLB, kept by revision: one per board
                    let (layers, name, rev) = (made.layers, std::path::Path::new(&abs).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), stamp(&f).rev);
                    std::thread::spawn(move || pcb3d::stream::keep_glb(&layers, &name, &abs, &rev));
                }
                Ok((made.plot, plan))
            });
            #[cfg(all(target_os = "linux", target_env = "gnu"))] unsafe { libc::malloc_trim(0); }
            r.unwrap_or_else(|_| Err(plotgen::UNREAD.to_string()))
        }).await.ok()
    }

    fn read(self: &Arc<Self>, hub: &Arc<Hub>, ky: &str) {
        let (kind, file) = { let mut st = self.st.lock().unwrap(); let Some(s) = st.srcs.get_mut(ky) else { return }; if s.busy { s.again = true; return } s.busy = true; (s.kind, s.file.clone()) };
        let (me, hub, ky) = (self.clone(), hub.clone(), ky.to_string());
        tokio::spawn(async move {
            let t0 = std::time::Instant::now();
            let solid3 = solid_kind(kind);
            let about = if solid3 { me.st.lock().unwrap().took.get(&file).copied().unwrap_or(0) } else { 0 };
            let say = |text: &str| if solid3 { me.stage(&hub, &ky, text, t0.elapsed().as_millis() as u64, about) };
            if solid3 && me.readers.available_permits() == 0 { say("Waiting for another drawing to finish…") }
            let _slot = me.readers.acquire().await.ok();
            // Pv.read.at: a model whose GLB is known too big is refused at once, not behind another model's read
            let native3d = solid3 && file.ends_with(".kicad_pcb") && !file.starts_with("diff\n") && Self::board3d_native(&hub).await;
            let known = if solid3 && !native3d { solidread::known(kind, &file) } else { 0 };
            let _one = if solid3 && !native3d && solid::fits(known) {
                let g = match me.solid.try_lock() { Ok(g) => g, Err(_) => { say("Waiting for another 3D model to finish…"); me.solid.lock().await } };
                Some(g)
            } else { None };
            if solid3 { say(&stage_text(kind, &file, native3d, known > 0, &stepglb::machine(&me.home))) }
            let before = if is_version(&file) { Stamp::default() } else { stamp(&file) };
            let mut parts = None;
            let out = if native3d { me.run_board3d(&hub, kind, &file, &ky).await.map(|r| match r { Ok((p, plan)) => { parts = plan; (0, String::new(), Some(p)) } Err(e) => (3, e, None) }) }
                else if !solid::fits(known) { Some((3, solid::big(known), None)) } else if native() { me.run_native(&hub, kind, &file, &ky).await } else { me.run_child(kind, &file, &ky).await.map(|(c, t)| (c, t, None)) };
            let after = if is_version(&file) { Stamp::default() } else { stamp(&file) };
            let (code, text, made) = match out { Some((c, t, m)) => if before != after { (2, String::new(), None) } else { (c, t, m) }, None => (1, "cannot run the plot reader".into(), None) };
            let mut again = false;
            let plot = if code == 0 { Some(Arc::new(made.unwrap_or_else(|| plot::parse_child(&text)))) } else { None };
            { let mut st = me.st.lock().unwrap();
              if let Some(s) = st.srcs.get_mut(&ky) { s.busy = false; s.stamp = Some(before.clone()); again = std::mem::take(&mut s.again); if let Some(p) = &plot { s.plot = Some(p.clone()) } } }
            if solid3 {
                let ms = t0.elapsed().as_millis() as u64;
                if code == 0 && !is_version(&file) { me.st.lock().unwrap().took.insert(file.clone(), ms); }
                let mb = plot.as_ref().and_then(|p| p.solid.as_ref()).map_or(0.0, |s| s.d.len() as f64 / 1048576.0);
                if mb >= 2.0 { say(&format!("Sending the model ({mb:.1} MB)…")) }
            }
            match (&plot, code) {
                (Some(p), _) => {
                    me.send_to(&hub, &ky, p, None); me.revision(&hub, &ky, None);
                    if let Some(plan) = parts.take() { me.start_parts(&hub, &ky, &file, before.rev.clone(), plan) }
                }
                (None, 0 | 2) => {} // half written or read again: the next poll sees the file change
                (None, _) => { // Pv.fail: every watcher told why, and holding nothing
                    let why: String = text.trim().chars().take(200).collect();
                    let mut st = me.st.lock().unwrap(); let mut gone = vec![];
                    for (c, subs) in st.subs.iter_mut() { for s in subs.iter_mut().filter(|s| s.key == ky) { s.held.clear(); if !hub.push(*c, json!({"t":"plot","key":ky,"none":why})) { gone.push(*c) } } }
                    drop(st); for c in gone { me.stop(c) } }
            }
            if matches!(code, 0 | 2) && (ky.starts_with("2|") || ky.starts_with("3|")) { me.note(&hub, &ky, None) } // Pv.handle PGot
            if solid3 { say("") }
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
            for k in changed {
                let file = { let st = self.st.lock().unwrap(); st.srcs.get(&k).map(|s| s.file.clone()).unwrap_or_default() };
                self.prewarm(&hub, &file); // a board that changed may use new models
                self.read(&hub, &k)
            }
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
/// `kept` of a watch: "<key>\t<model id in hex>", or an older phone's id alone (any of the view's models). A model is skipped only for that key.
pub fn kept_of(k: &str) -> (&str, u64) { let (key, id) = k.rsplit_once('\t').unwrap_or(("", k)); (key, u64::from_str_radix(id, 16).unwrap_or(0)) }
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
    fn methods(&self) -> &'static [&'static str] { &["kicad.watch", "kicad.info", "kicad.warm", "kicad.probe", "kicad.open", "kicad.install", "viewer.state", "history.steps"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            match method {
                "kicad.watch" => {
                    let (kind, root, path) = (s("kind"), s("root"), s("path"));
                    // PR #960: an active request with neither an absolute root nor an absolute source has no folder and is refused (Hub.folder.out Watch)
                    if !kind.is_empty() && !(crate::files::threadpath::absolute(&root) || crate::files::threadpath::absolute(&path)) { return vec![crate::files::threadpath::refusal(id)] }
                    if !kind.is_empty() && !super::known_root(hub, &root, &path).await { return vec![json!({"t":"reply","id":id,"ok":false,"error":"that folder is not in this project"})] }
                    // kept: "<key>\t<id>" (an older phone sends the id alone)
                    let k = s("kept");
                    let keys = if kind == "split" { self.0.watch_split(hub, who.client, &root, &path, &s("tree")) }
                    else { self.0.watch(hub, who.client, kind_of(&kind), &root, &path, &s("tree"), kept_of(&k)) };
                    vec![json!({"t":"reply","id":id,"ok":true,"result":{"keys":keys}})]
                }
                "kicad.warm" => { // the saved versions before and after a step, plotted ahead
                    let (kind, root) = (s("kind"), s("root"));
                    for path in [s("before"), s("after")] {
                        if path.is_empty() || !super::known_root(hub, &root, &path).await { continue }
                        self.0.warm(hub, if kind == "schematic" { 1 } else { 0 }, &root, &path);
                    }
                    vec![json!({"t":"reply","id":id,"ok":true})]
                }
                // Open in KiCad: the thread's design in the desktop KiCad (kicad/open.rs); the reply and a line in the thread say how it went
                "kicad.open" => match super::open::open(hub, &s("thread")).await {
                    Ok(what) => vec![json!({"t":"reply","id":id,"ok":true,"result":{"opened":what}})],
                    Err(why) => vec![json!({"t":"reply","id":id,"ok":false,"error":why})],
                },
                // Install KiCad (Settings > CAD): refused, or started; how it goes is the info key `kicad.install` (kicad/install.rs)
                "kicad.install" => match super::install::install(hub, p.get("consent").and_then(Value::as_bool).unwrap_or(false)).await {
                    Ok(what) => vec![json!({"t":"reply","id":id,"ok":true,"result":{"started":what}})],
                    Err(why) => vec![json!({"t":"reply","id":id,"ok":false,"error":why})],
                },
                "history.steps" => vec![json!({"t":"reply","id":id,"ok":true,"result": super::histsteps::steps(hub, &s("thread"), &s("kind"), &s("file")).await})],
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
    #[test] fn a_3d_source_says_what_the_hub_does_for_it() {
        let fc = [stepglb::Mesher::FreeCad { gui: "/g".into(), cmd: "/c".into() }]; let occ = [stepglb::Mesher::Step2glb("/s".into())];
        assert_eq!(stage_text(2, "/p/b.kicad_pcb", true, false, &fc), "Making the board's 3D model from b.kicad_pcb…");
        assert_eq!(stage_text(2, "/p/mech/lid.step", false, false, &fc), "Meshing lid.step with FreeCAD…");
        assert_eq!(stage_text(2, "/p/mech/lid.STEP", false, false, &occ), "Meshing lid.STEP with OpenCascade…");
        assert_eq!(stage_text(2, "/p/mech/lid.step", false, true, &fc), "Reading lid.step…", "a mesh already made is only read");
        assert_eq!(stage_text(2, "git\tabc\t/p/b.kicad_pcb", true, false, &fc), "Making the board's 3D model from b.kicad_pcb…", "a saved version names its file");
        assert_eq!(stage_text(2, "/p/b.kicad_pcb", false, false, &fc), "Exporting the board's 3D model with KiCad (kicad-cli)…");
        assert_eq!(stage_text(2, "/p/m.glb", false, false, &[]), "Reading m.glb…");
    }
    #[test] fn kept_names_a_key_and_a_model() {
        assert_eq!(kept_of("2|/a|\t00000000000000ff"), ("2|/a|", 255));
        assert_eq!(kept_of("ff"), ("", 255), "an older phone: the id alone");
        assert_eq!(kept_of(""), ("", 0)); assert_eq!(kept_of("2|/a|\tzz"), ("2|/a|", 0));
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

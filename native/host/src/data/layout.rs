//! data/1 docking layout (feature `layout`): the hub's tile layout (`present::layout`) for phones, tablets and browsers.
//!
//! - `viewer.layout {thread, bounds:[w,h], page?, diff?}` → the device's layout for `thread`, fitted to the viewer area in CSS px.
//! - `viewer.layout.act {thread, op, bounds, …}` → the same reply after the op, or a refusal `{code, key, message}`.
//!
//! One shared layout per (device, thread): the TREE is stored, never the fit; every connection sends its own bounds and gets its own fit
//! (`present::layout_fit`). A change is pushed as `{t:"layout", thread, rev}` to the device's connections watching the thread; a client that sees a rev it
//! does not hold asks again. Multi-tile layouts of PAIRED devices (a native or browser device) survive a hub restart (a small file under the hub's home, mode 0600,
//! dropped when the device is retired or its session has expired). A connection with the HUB token (device `hub`) has its layout in memory only: nothing of it is
//! written to the file, and a restart forgets it.
//!
//! The device's layout is NEVER a desktop session's: the hub's pure layout operations run over a synthetic session built here from this device's own
//! drafts, and only their `@view*:<thread>` drafts come back into this device's store. Nothing reads or writes a presenter session.
use super::*;
use crate::present::layout::{self, Layout, Refusal};
use crate::present::layout_fit::{self, Fitted};
use crate::present::viewer::{self, LayoutOut};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Most tiles one layout holds: the hub's own limit.
pub const MAX_TILES: usize = layout::MAX_TILES;
/// A device keeps layouts for this many threads (least recently used first out) and the hub for this many devices.
const THREADS_PER_DEVICE: usize = 32;
const DEVICES: usize = 64;
/// The page kinds a data/1 layout holds: viewers only. A KiCad frame, the Simulator, the terminal and the hub's browser pane are never tiles.
const TILE_KINDS: &[&str] = &["board", "schematic", "3d", "gerbers", "mech", "files", "diff", "review"];
/// The ops of `viewer.layout.act`.
const OPS: &[&str] = &["add", "close", "focus", "resize", "swap", "move", "overlay", "unsplit", "maximize", "restore", "show"];

/// One device's layout of one thread: the synthetic session drafts the hub's layout code reads (the layout, the page, the picked board), the
/// focus recency (most recent first) and the revision clients compare.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub drafts: serde_json::Map<String, Value>,
    pub recent: Vec<String>,
    pub rev: u64,
    stamp: u64,
}

pub struct Store {
    entries: Mutex<HashMap<(String, String), Entry>>,
    /// connection → the device it belongs to
    clients: Mutex<HashMap<u64, String>>,
    /// (device, thread) → the connections that read this layout and are told when it changes
    watchers: Mutex<HashMap<(String, String), BTreeSet<u64>>>,
    /// connections holding a layout's watch set (`viewer.watch {kind:"layout"}`): another watch on one lets the set go
    holding: Mutex<HashMap<u64, String>>,
    file: Option<PathBuf>,
    /// the memory differs from the file (a write is owed); `io` serialises writes, `written` is the body last written (an unchanged store writes nothing)
    dirty: std::sync::atomic::AtomicBool,
    io: Mutex<String>,
    scheduled: std::sync::atomic::AtomicBool,
    tick: std::sync::atomic::AtomicU64,
    pruned: std::sync::atomic::AtomicU64,
}

/// One store per hub home (a process may run several hubs, as the tests do), each loaded from its home's file on first use.
static STORES: Mutex<Vec<(PathBuf, &'static Store)>> = Mutex::new(Vec::new());

/// A device's layouts are gone (revoked): from the hub's store and its file.
pub fn forget_device_file(home: &Path, device: &str) {
    let st = global(home);
    st.forget_device(device);
    st.forget_device(&format!("browser:{device}"));
}

/// The hub's store, loaded from its home on first use.
pub fn global(home: &Path) -> &'static Store {
    let mut g = STORES.lock().unwrap();
    if let Some((_, st)) = g.iter().find(|(h, _)| h == home) { return st }
    let st: &'static Store = Box::leak(Box::new(Store::open(Some(home.join("viewer-layouts.json")))));
    g.push((home.to_path_buf(), st));
    st
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }

impl Store {
    pub fn new() -> Store { Store::open(None) }
    pub fn open(file: Option<PathBuf>) -> Store {
        let st = Store { entries: Mutex::new(HashMap::new()), clients: Mutex::new(HashMap::new()), watchers: Mutex::new(HashMap::new()), holding: Mutex::new(HashMap::new()), file, dirty: false.into(), io: Mutex::new(String::new()), scheduled: false.into(), tick: 0.into(), pruned: 0.into() };
        if let Some(f) = &st.file { st.load(f) }
        st
    }
    fn load(&self, file: &Path) {
        let text = match std::fs::read_to_string(file) { Ok(t) => t, Err(e) if e.kind() == std::io::ErrorKind::NotFound => return, Err(e) => { eprintln!("viewer layouts: {} is unreadable ({e}); starting empty", file.display()); return } };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            // a file that is not ours (or cut short): kept as `.bad` for whoever looks, never overwritten silently, and the store starts empty
            let bad = file.with_extension("json.bad");
            match std::fs::copy(file, &bad) {
                Ok(_) => eprintln!("viewer layouts: {} is not valid JSON; kept as {} and starting empty", file.display(), bad.display()),
                Err(e) => eprintln!("viewer layouts: {} is not valid JSON and could not be kept as {} ({e}); starting empty", file.display(), bad.display()),
            }
            return;
        };
        let mut g = self.entries.lock().unwrap();
        for (device, threads) in v["devices"].as_object().into_iter().flatten().filter(|(d, _)| *d != "hub") {
            for (thread, e) in threads.as_object().into_iter().flatten() {
                let drafts = e["drafts"].as_object().cloned().unwrap_or_default();
                // only what this module writes is read back: `@view*:<thread>` drafts holding a multi-tile layout
                let drafts: serde_json::Map<String, Value> = drafts.into_iter().filter(|(k, _)| persisted_key(k, thread)).collect();
                if !has_layout(&drafts, thread) { continue }
                let recent = e["recent"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
                g.insert((device.clone(), thread.clone()), Entry { drafts, recent, rev: e["rev"].as_u64().unwrap_or(1).max(1), stamp: self.tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed) });
            }
        }
        // the limits hold for what is read back as for what is written
        Self::bound(&mut g, None);
    }
    /// Keep the store bounded: per device (`device`, when one just wrote) the least recently used thread out past the limit, then the least recent device.
    fn bound(g: &mut HashMap<(String, String), Entry>, device: Option<&str>) {
        let devices: BTreeSet<String> = match device { Some(d) => [d.to_string()].into(), None => g.keys().map(|(d, _)| d.clone()).collect() };
        for d in devices {
            loop {
                let mine: Vec<_> = g.iter().filter(|((x, _), _)| *x == d).map(|(k, v)| (k.clone(), v.stamp)).collect();
                if mine.len() <= THREADS_PER_DEVICE { break }
                if let Some((k, _)) = mine.into_iter().min_by_key(|(_, st)| *st) { g.remove(&k); }
            }
        }
        loop {
            let devices: BTreeSet<String> = g.keys().map(|(d, _)| d.clone()).collect();
            if devices.len() <= DEVICES { break }
            let Some(old) = devices.into_iter().min_by_key(|d| g.iter().filter(|((x, _), _)| x == d).map(|(_, v)| v.stamp).max().unwrap_or(0)) else { break };
            g.retain(|(d, _), _| *d != old);
        }
    }
    /// Write the multi-tile layouts now (atomic: a 0600 temporary file and a rename), serialised with every other write; the LATEST tree is what is written, and
    /// nothing is written when it equals what the file holds. Single-tile layouts are the page the device is on; they are not kept. Failures are logged.
    pub fn flush(&self) {
        let Some(file) = &self.file else { return };
        let mut written = self.io.lock().unwrap();
        self.dirty.store(false, std::sync::atomic::Ordering::SeqCst);
        let mut devices = serde_json::Map::new();
        for ((device, thread), e) in self.entries.lock().unwrap().iter() {
            // the hub token's layout (device `hub`) is never written
            if device == "hub" || !has_layout(&e.drafts, thread) { continue }
            let slot = devices.entry(device.clone()).or_insert_with(|| json!({}));
            // only what is root-relative stays on disk: the layout and the picked board (`@view.file` & co. hold the absolute path the desktop derives)
            let kept: serde_json::Map<String, Value> = e.drafts.iter().filter(|(k, _)| persisted_key(k, thread)).map(|(k, v)| (k.clone(), v.clone())).collect();
            slot[thread] = json!({"drafts": kept, "recent": e.recent, "rev": e.rev});
        }
        let body = json!({"version": 1, "devices": devices}).to_string();
        if *written == body { return }
        // device ids and project-relative paths: the temporary file is created 0600 (as the device registry's is), then renamed over the file
        use crate::plat::fs::OpenOptionsExt;
        use std::io::Write;
        let tmp = file.with_extension(format!("json.{}.{}.tmp", std::process::id(), self.tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        let r = (|| -> std::io::Result<()> {
            let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?;
            f.write_all(body.as_bytes())?; f.sync_all()?; drop(f);
            crate::plat::fs::rename(&tmp, file)
        })();
        match r {
            Ok(()) => *written = body,
            Err(e) => { let _ = std::fs::remove_file(&tmp); self.dirty.store(true, std::sync::atomic::Ordering::SeqCst); eprintln!("viewer layouts: could not write {}: {e}", file.display()); }
        }
    }
    /// A change is owed to the file. No I/O here (this runs inside the state's lock): `persist_soon` writes it, off that lock.
    fn touch(&self) { self.dirty.store(true, std::sync::atomic::Ordering::SeqCst); }
    /// Write what is owed after a short pause (one writer at a time; a burst of drag positions is one write of the last tree). Not under any hub lock.
    pub fn persist_soon(&'static self) {
        if self.file.is_none() || !self.dirty.load(std::sync::atomic::Ordering::SeqCst) { return }
        if self.scheduled.swap(true, std::sync::atomic::Ordering::SeqCst) { return }
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            self.scheduled.store(false, std::sync::atomic::Ordering::SeqCst);
            self.flush();
        });
    }

    pub fn entry(&self, device: &str, thread: &str) -> Option<Entry> { self.entries.lock().unwrap().get(&(device.to_string(), thread.to_string())).cloned() }
    pub fn len(&self) -> usize { self.entries.lock().unwrap().len() }
    /// Store `e` for (device, thread), keeping the store bounded. Memory only: the file follows (`persist_soon`).
    pub fn put(&self, device: &str, thread: &str, mut e: Entry) {
        let mut g = self.entries.lock().unwrap();
        // what is already held is not written again, and a read of it is not a use that moves it up the recency list
        if let Some(old) = g.get(&(device.to_string(), thread.to_string())) { let mut same = e.clone(); same.stamp = old.stamp; if *old == same { return } }
        e.stamp = self.tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        g.insert((device.to_string(), thread.to_string()), e);
        Self::bound(&mut g, Some(device));
        drop(g);
        self.touch();
    }
    /// A device's layouts are gone: revoked, logged out, expired.
    pub fn forget_device(&self, device: &str) {
        self.entries.lock().unwrap().retain(|(d, _), _| d != device);
        self.watchers.lock().unwrap().retain(|(d, _), _| d != device);
        // a retired device is gone from the file before this returns: the latest tree is written, so a write still owed cannot bring it back
        self.touch();
        self.flush();
    }
    /// Drop every device not in `live` (the hub token's `hub` always stays).
    pub fn prune(&self, live: &BTreeSet<String>) -> usize {
        let gone: BTreeSet<String> = self.entries.lock().unwrap().keys().map(|(d, _)| d.clone()).filter(|d| d != "hub" && !live.contains(d)).collect();
        for d in &gone { self.forget_device(d) }
        gone.len()
    }

    pub fn register_client(&self, client: u64, device: &str) { self.clients.lock().unwrap().insert(client, device.to_string()); }
    /// Forget the layouts of every device the registry no longer holds (expired, revoked while the hub was down), at most once a minute.
    /// LOCK ORDER: `ids` is handed in, already read from the registry (its lock released); the store's locks are never held while calling into the registry, because
    /// `Registry::retire` calls the store with the registry's mutex held.
    pub fn prune_to(&self, ids: &[String], now_ms: u64) {
        let last = self.pruned.swap(now_ms, std::sync::atomic::Ordering::Relaxed);
        if now_ms.saturating_sub(last) < 60_000 && last != 0 { self.pruned.store(last, std::sync::atomic::Ordering::Relaxed); return }
        let live: BTreeSet<String> = ids.iter().flat_map(|i| [i.clone(), format!("browser:{i}")]).collect();
        self.prune(&live);
    }
    pub fn device_of(&self, client: u64) -> Option<String> { self.clients.lock().unwrap().get(&client).cloned() }
    pub fn forget_client(&self, client: u64) {
        self.clients.lock().unwrap().remove(&client);
        for w in self.watchers.lock().unwrap().values_mut() { w.remove(&client); }
        self.holding.lock().unwrap().remove(&client);
    }
    /// The connection now holds a layout watch set for `thread` (None: it holds none).
    pub fn hold(&self, client: u64, thread: Option<&str>) { let mut h = self.holding.lock().unwrap(); match thread { Some(t) => { h.insert(client, t.to_string()); } None => { h.remove(&client); } } }
    /// The thread whose layout watch set the connection holds.
    pub fn held(&self, client: u64) -> Option<String> { self.holding.lock().unwrap().get(&client).cloned() }
    fn watch(&self, device: &str, thread: &str, client: u64) { self.watchers.lock().unwrap().entry((device.to_string(), thread.to_string())).or_default().insert(client); }
    /// The connections of `device` watching `thread`.
    pub fn watchers_of(&self, device: &str, thread: &str) -> Vec<u64> { self.watchers.lock().unwrap().get(&(device.to_string(), thread.to_string())).map(|w| w.iter().copied().collect()).unwrap_or_default() }
}

/// Whether a draft key is one this module owns for `thread`: the page, the stored layout and what they derive from.
fn owned_key(key: &str, thread: &str) -> bool {
    ["@view", "@view.layout", "@view.board", "@view.file", "@view.sheet", "@view.gerbers", "@mech.part", "@files.dir"].iter().any(|k| key == format!("{k}:{thread}"))
}
/// The drafts kept on disk: the stored layout and the picked board, both root-relative.
fn persisted_key(key: &str, thread: &str) -> bool { key == format!("@view.layout:{thread}") || key == format!("@view.board:{thread}") }
fn has_layout(drafts: &serde_json::Map<String, Value>, thread: &str) -> bool {
    drafts.get(&format!("@view.layout:{thread}")).and_then(Value::as_str).is_some_and(|t| serde_json::from_str::<Layout>(t).map_or(false, |l| l.tiles().len() >= 2))
}

/// The synthetic session the hub's layout code reads for this device: its own drafts and nothing else (no `info`, no other thread's drafts).
pub fn session_for(thread: &str, entry: Option<&Entry>, page: &str, diff: bool) -> Value {
    // Diff takes precedence over the page it was opened from (as the rail's `diff` flag does): the Diff panel is on and no page is
    let diff = diff && entry.is_none();
    let page = if diff { "" } else { page };
    let mut drafts = serde_json::Map::new();
    match entry { Some(e) => for (k, v) in &e.drafts { if owned_key(k, thread) { drafts.insert(k.clone(), v.clone()); } },
        None => {
            // no layout yet: the page the device is on is the one tile (`gerbers` is a Board page with the Gerber drafts set)
            let (open, gerbers) = if page == "gerbers" { ("board", "1") } else { (page, "") };
            if !open.is_empty() { drafts.insert(format!("@view:{thread}"), json!(open)); }
            if !gerbers.is_empty() { drafts.insert(format!("@view.gerbers:{thread}"), json!(gerbers)); }
        } }
    if diff { return json!({"sel": thread, "rid": 1, "drafts": drafts, "panel": true, "info": {format!("view.kind:{thread}"): "diff", format!("panel:{thread}"): "1"}}) }
    json!({"sel": thread, "rid": 1, "drafts": drafts, "panel": false, "info": {}})
}

/// `bounds` as a client sends it: `[w, h]` of numbers (strings, a missing one or a wrong count are an invalid request); each side is then clamped.
pub fn parse_bounds(v: &Value) -> Result<(f32, f32), Refused> {
    let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(|| Refused::of(Refusal::InvalidInput))?;
    let n = |x: &Value| x.as_f64();
    match (n(&a[0]), n(&a[1])) { (Some(w), Some(h)) => Ok(layout_fit::clamp_bounds(w, h)), _ => Err(Refused::of(Refusal::InvalidInput)) }
}

/// The `page` a client says it is on: one of the tile kinds or nothing (a passive read must not seed a page this protocol cannot hold).
fn check_page(page: &str) -> Result<(), Refused> {
    if page.is_empty() || TILE_KINDS.contains(&page) { Ok(()) } else { Err(extra("notOnDevice", "viewer.layout.refusal.notondevice")) }
}

/// Mark the tiles whose named source no longer resolves INSIDE the project (a link changed after it was saved): they are not watched, and say so.
fn mark_unavailable(out: &mut Value, l: &Layout, root: &str) {
    if root.is_empty() { return }
    let bad: BTreeSet<String> = unresolved(l, root);
    for t in out["tiles"].as_array_mut().into_iter().flatten() { if t["id"].as_str().is_some_and(|i| bad.contains(i)) { t["unavailable"] = json!(true); } }
}
/// The ids of the tiles with a tile or overlay source that does not resolve inside `root` now.
fn unresolved(l: &Layout, root: &str) -> BTreeSet<String> {
    l.tiles().into_iter().filter(|t| std::iter::once(t.source.as_str()).chain(t.overlay.iter().map(|o| o.source.as_str())).any(|src| !src.is_empty() && safe_source(root, src).is_err()))
        .map(|t| t.id.clone()).collect()
}
/// The layout with every named source replaced by its canonical path inside `root` (resolved now, at this use), so what is opened is what was checked.
fn resolved(l: &Layout, root: &str) -> Layout {
    fn walk(v: &mut Value, root: &str) {
        if let Some(src) = v.get("source").and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_string) { if let Ok(c) = safe_source(root, &src) { v["source"] = json!(c) } }
        if v.get("overlay").is_some_and(Value::is_object) { walk(&mut v["overlay"], root) }
        for k in ["a", "b", "root"] { if v.get(k).is_some_and(Value::is_object) { walk(&mut v[k], root) } }
    }
    let mut v = serde_json::to_value(l).unwrap_or(Value::Null);
    walk(&mut v, root);
    serde_json::from_value(v).unwrap_or_else(|_| l.clone())
}

/// A refusal as data/1 answers it: `{code, key, message}`, the message from the catalog in the hub's language.
#[derive(Clone, Debug, PartialEq)]
pub struct Refused { pub code: &'static str, pub key: &'static str, pub text: &'static str }
impl Refused {
    pub fn of(r: Refusal) -> Refused { Refused::of_for(r, "en") }
    pub fn of_for(r: Refusal, locale: &str) -> Refused { Refused { code: r.code(), key: r.i18n_key(), text: r.text_for(locale) } }
    pub fn to_value(&self) -> Value { json!({"code": self.code, "key": self.key, "message": self.text}) }
}

/// Refusals only a device can meet, with their own catalog keys (the text is the catalog's, in the hub's language, set when the refusal leaves `call`).
fn extra(code: &'static str, key: &'static str) -> Refused { Refused { code, key, text: layout::catalog_text(key, "en").unwrap_or(code) } }

/// Check a `source` a client names: it must be a path inside the project's folder (never absolute, never leaving it, no symlink out) that exists.
pub fn safe_source(root: &str, source: &str) -> Result<String, Refused> {
    if source.is_empty() { return Ok(String::new()) }
    let bad = || Refused::of(Refusal::InvalidSource);
    if !layout::is_relative_source(source) || source.contains('\0') || source.contains('\\') || source.split('/').any(|x| x == ".." ) || crate::plat::fs::is_abs(source) { return Err(bad()) }
    let rel = source.trim_start_matches("./").to_string();
    if root.is_empty() { return Err(bad()) }
    let (Ok(base), Ok(full)) = (std::fs::canonicalize(root), std::fs::canonicalize(Path::new(root).join(&rel))) else { return Err(bad()) };
    if !full.starts_with(&base) { return Err(bad()) }
    // what is kept is the canonical path (a link inside the project is followed to where it really is, still inside it), so what is opened later is what was checked
    let canon = full.strip_prefix(&base).ok().and_then(|p| p.to_str()).map(|p| p.replace(std::path::MAIN_SEPARATOR, "/")).ok_or_else(bad)?;
    if canon.is_empty() || !layout::is_relative_source(&canon) { return Err(bad()) }
    let _ = rel;
    Ok(canon)
}

/// The tile JSON of one tile: no field of it is an absolute path.
fn tile_json(t: &layout::Tile, focused: bool, rect: Option<layout::Rect>) -> Value {
    let rel = |p: &str| if layout::is_relative_source(p) { p.to_string() } else { String::new() };
    let mut v = json!({"id": t.id, "kind": t.kind.as_str(), "source": rel(&t.source), "explicit": t.explicit, "focused": focused, "visible": rect.is_some(),
        "rect": rect.map(|r| json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h}))});
    if let Some(ver) = &t.version { v["version"] = json!({"sourceId": ver.source_id, "id": ver.id, "label": ver.label}) }
    if let Some(ov) = &t.overlay {
        v["overlay"] = json!({"kind": ov.kind.as_str(), "source": rel(&ov.source), "mode": match ov.mode { layout::OverlayMode::Old => "old", layout::OverlayMode::Diff => "diff", layout::OverlayMode::New => "new" }});
        if let Some(ver) = &ov.version { v["overlay"]["version"] = json!({"sourceId": ver.source_id, "id": ver.id, "label": ver.label}) }
    }
    v
}

/// The `viewer.layout` reply: the stored tree fitted to `bounds`.
pub fn reply(thread: &str, rev: u64, layout: &Layout, fitted: &Fitted) -> Value {
    let rect_of = |id: &str| fitted.visible.iter().find(|(i, _)| i == id).map(|(_, r)| *r);
    let tiles: Vec<Value> = layout.tiles().into_iter().map(|t| tile_json(t, layout.focus.as_deref() == Some(t.id.as_str()), rect_of(&t.id))).collect();
    let axis = fitted.axis.map(|a| match a { layout::Axis::Row => "row", layout::Axis::Col => "col" });
    let splits: Vec<Value> = fitted.splits.iter().map(|sp| json!({"id": sp.id, "axis": match sp.axis { layout::Axis::Row => "row", layout::Axis::Col => "col" }, "ratio": sp.ratio,
        "rect": {"x": sp.rect.x, "y": sp.rect.y, "w": sp.rect.w, "h": sp.rect.h}})).collect();
    json!({"thread": thread, "rev": rev, "mode": fitted.mode.as_str(), "axis": axis, "bounds": [fitted.bounds.0, fitted.bounds.1],
        "focus": layout.focus, "maximized": layout.maximized, "order": fitted.order, "tiles": tiles, "splits": splits,
        "limits": {"maxTiles": MAX_TILES, "minTilePx": layout::MIN_TILE_PX}})
}

/// The layout the hub derives for a session (the desktop's own derivation: the stored tree, else the one open page).
fn current(st: &crate::state::State, session: &Value) -> Layout {
    viewer::viewer_json(st, &json!({}), session).and_then(|v| serde_json::from_value::<Layout>(v["layout"].clone()).ok()).unwrap_or_else(Layout::empty)
}

/// What `viewer.layout` answers: the device's tree fitted to `bounds`. The page the device is on (`page`, `diff`) is what a device with no layout of several
/// tiles is looking at: it is that one tile. A read is PASSIVE: it never changes what is stored nor the `rev` (a later `act` carries the page again to start from
/// it); a thread of another machine shows the one derived tile, never stored, and every `act` on it is refused.
pub fn read(st: &crate::state::State, store: &Store, device: &str, thread: &str, bounds: (f32, f32), page: &str, diff: bool, root: &str) -> Result<Value, Refused> {
    check_page(page)?;
    if thread.contains('~') {
        let l = current(st, &session_for(thread, None, page, diff));
        return Ok(reply(thread, 1, &l, &layout_fit::fit(&l, &[], bounds)));
    }
    check_thread(st, thread)?;
    let entry = store.entry(device, thread);
    let multi = entry.as_ref().is_some_and(|e| has_layout(&e.drafts, thread));
    // several tiles are the device's own tree; otherwise the page it says it is on (or the one kept from its last change)
    // several tiles are the device's own tree; otherwise the page it says it is on, or Diff (which takes precedence over a page, and needs none)
    let session = if multi || (page.is_empty() && !diff) { session_for(thread, entry.as_ref(), "", false) } else { session_for(thread, None, page, diff) };
    let l = current(st, &session);
    let (rev, recent) = entry.map_or((1, vec![]), |e| (e.rev, e.recent));
    let mut out = reply(thread, rev, &l, &layout_fit::fit(&l, &recent, bounds));
    mark_unavailable(&mut out, &l, root);
    Ok(out)
}

fn check_thread(st: &crate::state::State, thread: &str) -> Result<(), Refused> {
    if thread.contains('~') { return Err(extra("linked", "viewer.layout.refusal.linked")) }
    if !st.threads.contains_key(thread) { return Err(Refused::of(Refusal::NotFound)) }
    Ok(())
}

/// Map an op and its fields to the hub's layout action and its JSON payload. `bounds` is passed to `resize` (the hub clamps the ratio by tile minimums there).
fn action_of(op: &str, p: &Value, root: &str, bounds: (f32, f32)) -> Result<(&'static str, Value), Refused> {
    let bad = || Refused::of(Refusal::InvalidInput);
    let str_of = |k: &str| -> Result<Option<String>, Refused> { match p.get(k) { None | Some(Value::Null) => Ok(None), Some(v) => v.as_str().map(|x| Some(x.to_string())).ok_or_else(bad) } };
    let kind_ok = |k: &str| TILE_KINDS.contains(&k);
    Ok(match op {
        "add" => {
            let kind = str_of("kind")?.ok_or_else(bad)?;
            if !kind_ok(&kind) { return Err(extra("notOnDevice", "viewer.layout.refusal.notondevice")) }
            let mut v = json!({"kind": kind});
            if let Some(side) = str_of("side")? { v["side"] = json!(side) }
            if let Some(t) = str_of("target")? { v["target"] = json!(t) }
            if let Some(src) = str_of("source")? { v["source"] = json!(safe_source(root, &src)?) }
            if let Some(ver) = p.get("version").filter(|x| !x.is_null()) { v["version"] = ver.clone() }
            ("layout-add", v)
        }
        "close" => ("layout-close", json!({"tile": str_of("tile")?.ok_or_else(bad)?})),
        "focus" => ("layout-focus", json!({"tile": str_of("tile")?.ok_or_else(bad)?})),
        "maximize" => ("layout-maximize", json!({"tile": str_of("tile")?.ok_or_else(bad)?})),
        "restore" => ("layout-restore", json!({})),
        // split an overlay back out of `tile` (to `side`, right by default)
        "unsplit" => { let mut v = json!({"tile": str_of("tile")?.ok_or_else(bad)?}); if let Some(side) = str_of("side")? { v["side"] = json!(side) } ("layout-unsplit", v) }
        "resize" => {
            let ratio = p.get("ratio").and_then(Value::as_f64).filter(|r| r.is_finite()).ok_or_else(|| Refused::of(Refusal::InvalidRatio))?;
            ("layout-resize", json!({"split": str_of("split")?.ok_or_else(bad)?, "ratio": ratio, "bounds": [bounds.0, bounds.1]}))
        }
        "swap" => ("layout-swap", json!({"a": str_of("a")?.ok_or_else(bad)?, "b": str_of("b")?.ok_or_else(bad)?})),
        "move" => {
            let mut v = json!({"tile": str_of("tile")?.ok_or_else(bad)?, "target": str_of("target")?.ok_or_else(bad)?});
            if let Some(side) = str_of("side")? { v["side"] = json!(side) }
            ("layout-move", v)
        }
        // `overlay` is null (clear it), the id of another tile (merge it over `tile`) or `{kind, source?, mode?, version?}` (a source over it)
        "overlay" => {
            let mut v = json!({"base": str_of("tile")?.ok_or_else(bad)?});
            if let Some(mode) = str_of("mode")? { v["mode"] = json!(mode) }
            match p.get("overlay") {
                None | Some(Value::Null) => { v["overlay"] = Value::Null }
                Some(Value::String(tile)) => { v["overlay"] = json!(tile) }
                Some(ov) if ov.is_object() => {
                    let mut o = ov.clone();
                    let kind = ov["kind"].as_str().ok_or_else(bad)?;
                    if !kind_ok(kind) { return Err(extra("notOnDevice", "viewer.layout.refusal.notondevice")) }
                    if let Some(src) = ov.get("source").and_then(Value::as_str) { o["source"] = json!(safe_source(root, src)?) }
                    v["overlay"] = o;
                }
                _ => return Err(bad()),
            }
            ("layout-overlay", v)
        }
        _ => return Err(bad()),
    })
}

/// Apply `op` for `device` on `thread`: the hub's pure layout code over this device's own synthetic session, the result stored, the watchers told.
/// `root` is the project's design folder (sources are checked against it); the hub's locale decides the refusal text.
pub fn act(st: &crate::state::State, store: &Store, device: &str, thread: &str, op: &str, p: &Value, root: &str, locale: &str, bounds: (f32, f32)) -> Result<Value, Refused> {
    check_thread(st, thread)?;
    if !OPS.contains(&op) { return Err(Refused::of_for(Refusal::InvalidInput, locale)) }
    let entry = store.entry(device, thread);
    let page = s(p, "page");
    check_page(page)?;
    let session = session_for(thread, entry.as_ref(), page, p["diff"] == true);
    let before = current(st, &session);
    // a layout never holds more tiles than the limit, whatever an op adds
    let out = if op == "show" {
        let kind = s(p, "kind");
        if kind != "split" && !TILE_KINDS.contains(&kind) { return Err(extra("notOnDevice", "viewer.layout.refusal.notondevice")) }
        // the source-aware idempotent show (an agent's `viewer_show`'s own path): it applies the kind WITH its source, focuses or retargets, and never closes
        let src = match p.get("source").and_then(Value::as_str) { Some(src) => safe_source(root, src)?, None => String::new() };
        viewer::viewer_show_action(st, "", &session, None, locale, kind, &src)
    } else {
        let (action, mut payload) = action_of(op, p, root, bounds).map_err(|r| Refused { text: Refusal::InvalidInput.text_for(locale), ..r })?;
        // the guard fields are the desktop's own: a data/1 client holds no KiCad, so nothing it sends can name a lock or a KiCad tile
        if let Some(o) = payload.as_object_mut() { o.remove("locked"); o.remove("kicadTile"); }
        viewer::layout_action(st, "", &session, None, locale, action, &payload.to_string())
    };
    let vo = match out {
        Some(LayoutOut::Ok(vo)) => vo,
        Some(LayoutOut::Refused { code, key, text }) => return Err(Refused { code, key, text }),
        None => return Err(Refused::of_for(Refusal::InvalidInput, locale)),
    };
    let mut after_session = session.clone();
    crate::session::patch::merge(&mut after_session, &vo.patch);
    let after = current(st, &after_session);
    if after.tiles().len() > MAX_TILES { return Err(Refused::of_for(Refusal::TooManyTiles, locale)) }
    // only this device's own `@view*:<thread>` drafts come back; whatever else a patch carries is not stored
    let mut drafts = serde_json::Map::new();
    for (k, v) in after_session["drafts"].as_object().into_iter().flatten() { if owned_key(k, thread) && v.as_str().map_or(true, |t| !t.is_empty()) { drafts.insert(k.clone(), v.clone()); } }
    let changed = serde_json::to_value(&before).ok() != serde_json::to_value(&after).ok();
    let mut e = entry.unwrap_or(Entry { rev: 1, ..Default::default() });
    e.drafts = drafts;
    if changed { e.rev += 1; }
    if let Some(f) = after.focus.clone() { e.recent.retain(|x| *x != f); e.recent.insert(0, f); }
    e.recent.retain(|x| after.contains_tile(x));
    e.recent.truncate(MAX_TILES);
    let rev = e.rev;
    let fitted = layout_fit::fit(&after, &e.recent, bounds);
    store.put(device, thread, e);
    let mut r = reply(thread, rev, &after, &fitted);
    mark_unavailable(&mut r, &after, root);
    r["changed"] = json!(changed);
    Ok(r)
}

/// The connections of `device` to tell that `thread`'s layout moved to `rev`.
pub fn pushes(store: &Store, device: &str, thread: &str, rev: u64) -> Vec<(u64, Value)> {
    store.watchers_of(device, thread).into_iter().map(|c| (c, json!({"t": "layout", "thread": thread, "rev": rev}))).collect()
}

/// Who a connection is for the store: a paired device's id, a browser device (`browser:<id>`), or the hub token (`hub`).
pub fn device_key(native_device: Option<String>, browser_device: Option<&str>) -> String {
    match (browser_device, native_device) { (Some(b), _) => format!("browser:{b}"), (None, Some(d)) => d, (None, None) => "hub".to_string() }
}

/// The `viewer.layout` and `viewer.layout.act` requests, for the connection `c` of the hub `hub`.
pub async fn call(hub: &Arc<Hub>, c: &mut crate::hub::Conn, m: &str, p: &Value) -> Result<Value, Value> {
    let store = global(&hub.cfg.home);
    let device = store.device_of(c.client).unwrap_or_else(|| "hub".to_string());
    let locale = hub.setting("ui.lang").await.unwrap_or_default();
    let th = s(p, "thread").to_string();
    let bounds = parse_bounds(p.get("bounds").unwrap_or(&Value::Null)).map_err(|r| localized(r, &locale))?;
    // the project's design folder, as `viewer.files` answers it, for the sources a client names
    let root = if th.contains('~') { String::new() } else { crate::files::kicad::canonical::design_root(&super::ext::root_of(hub, &th).await.unwrap_or_default()) };
    let r = crate::statehost::with_state_of(hub, |st| match m {
        "viewer.layout" => read(st, store, &device, &th, bounds, s(p, "page"), p["diff"] == true, &root),
        _ => act(st, store, &device, &th, s(p, "op"), p, &root, &locale, bounds),
    });
    // whatever the state's lock saw changed is written now, off that lock
    store.persist_soon();
    let r = match r { Some(r) => r, None => return Err(localized(extra("starting", "viewer.layout.refusal.starting"), &locale)) };
    match r {
        Ok(mut v) => {
            // a connection that read or changed the layout hears of its changes
            store.watch(&device, &th, c.client);
            if m == "viewer.layout.act" && v["changed"] == true {
                // every connection of the device watching this thread, the sender included (its reply carries the same rev)
                for (client, frame) in pushes(store, &device, &th, v["rev"].as_u64().unwrap_or(0)) { hub.push(client, frame); }
                // a connection holding THIS thread's watch set gets the new set with the answer; a failure is told, never dropped
                if store.held(c.client).as_deref() == Some(th.as_str()) {
                    match install(hub, c, store, &device, &th, bounds, false, &locale).await { Ok(w) => v["watch"] = w, Err(e) => v["watchError"] = e }
                }
            }
            Ok(v)
        }
        Err(r) => Err(localized(r, &locale)),
    }
}

/// The plot sources of the tiles of `device`'s layout of `thread` that are visible at `bounds`, as `kicad.watch.set` takes them (the hub builds the paths: they
/// are never sent to the device), each source resolved inside the project NOW. The set is the visible plot tiles whatever their number: one Board tile has its
/// source, and a layout whose visible tiles draw nothing (Files in a phone stack) has none.
fn watch_set_sources(st: &crate::state::State, store: &Store, device: &str, thread: &str, bounds: (f32, f32), root: &str, page: &str, diff: bool, force: bool) -> Result<Vec<Value>, Refused> {
    check_thread(st, thread)?;
    check_page(page)?;
    let entry = store.entry(device, thread);
    let multi = entry.as_ref().is_some_and(|e| has_layout(&e.drafts, thread));
    // several tiles are the device's own tree; otherwise the page it says it is on, or Diff (which takes precedence over a page, and needs none)
    let session = if multi || (page.is_empty() && !diff) { session_for(thread, entry.as_ref(), "", false) } else { session_for(thread, None, page, diff) };
    let l = current(st, &session);
    // every named source is resolved again against the folder now: a link changed since it was saved is not followed (its tile is not watched)
    let bad = unresolved(&l, root);
    let l = resolved(&l, root);
    let fitted = layout_fit::fit(&l, entry.as_ref().map(|e| e.recent.as_slice()).unwrap_or(&[]), bounds);
    let visible: BTreeSet<&str> = fitted.visible.iter().map(|(i, _)| i.as_str()).collect();
    Ok(viewer::watch_sources(&l, root, false, force).into_iter().filter(|e| e["tile"].as_str().is_some_and(|t| visible.contains(t) && !bad.contains(t))).collect())
}

/// Install `device`'s visible plot tiles as this connection's watch set: `{thread, tiles: {tileId: [keys]}, keys: [...]}`. The set is sent EVERY time, empty
/// included (the hub then stops what it was streaming). The connection holds the set for `thread` only after the handler answered.
async fn install(hub: &Arc<Hub>, c: &mut crate::hub::Conn, store: &Store, device: &str, th: &str, bounds: (f32, f32), force: bool, locale: &str) -> Result<Value, Value> {
    install_page(hub, c, store, device, th, bounds, force, locale, "", false).await
}
async fn install_page(hub: &Arc<Hub>, c: &mut crate::hub::Conn, store: &Store, device: &str, th: &str, bounds: (f32, f32), force: bool, locale: &str, page: &str, diff: bool) -> Result<Value, Value> {
    if th.contains('~') { return Err(localized(extra("linked", "viewer.layout.refusal.linked"), locale)) }
    let root = crate::files::kicad::canonical::design_root(&super::ext::root_of(hub, th).await.unwrap_or_default());
    let sources = match crate::statehost::with_state_of(hub, |st| watch_set_sources(st, store, device, th, bounds, &root, page, diff, force)) {
        Some(r) => r.map_err(|r| localized(r, locale))?,
        None => return Err(localized(extra("starting", "viewer.layout.refusal.starting"), locale)),
    };
    let r = match super::rpc(hub, c, "kicad.watch.set", json!({"root": root, "sources": sources})).await {
        Ok(r) => r,
        Err(e) if e.as_str().is_some_and(|t| t.contains("too many")) => return Err(localized(Refused::of(Refusal::TooManySources), locale)),
        Err(e) => return Err(e),
    };
    // the hub holds the set now: only now does the connection
    store.hold(c.client, Some(th));
    let tiles = r["result"]["tiles"].clone();
    let mut keys: Vec<Value> = vec![];
    for ks in tiles.as_object().into_iter().flat_map(|m| m.values()) { for k in ks.as_array().into_iter().flatten() { if !keys.contains(k) { keys.push(k.clone()) } } }
    Ok(json!({"thread": th, "tiles": if tiles.is_object() { tiles } else { json!({}) }, "keys": keys}))
}

fn localized(r: Refused, locale: &str) -> Value { Refused { text: layout::catalog_text(r.key, locale).unwrap_or(r.text), ..r }.to_value() }

/// `viewer.watch {thread, kind: "layout", bounds, page?, diff?, force?}`: the plot sources of the device's visible tiles watched at once; `bounds` is required.
/// The answer names each tile's keys (an array per tile: a 3D tile draws from two). A connection calling it is told of the layout's changes, as a reader is.
/// Another `viewer.watch` on this connection lets the set go (`leave`, in `ext.rs`).
pub async fn watch(hub: &Arc<Hub>, c: &mut crate::hub::Conn, p: &Value) -> Result<Value, Value> {
    let store = global(&hub.cfg.home);
    let device = store.device_of(c.client).unwrap_or_else(|| "hub".to_string());
    let locale = hub.setting("ui.lang").await.unwrap_or_default();
    let th = s(p, "thread");
    let bounds = parse_bounds(p.get("bounds").unwrap_or(&Value::Null)).map_err(|r| localized(r, &locale))?;
    let out = install_page(hub, c, store, &device, th, bounds, p["force"] == true, &locale, s(p, "page"), p["diff"] == true).await?;
    store.watch(&device, th, c.client);
    Ok(out)
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;

//! Where the person is, kept across a relaunch: the view the hub last drew (its target and session: thread, panel, viewer, folds), the scroll
//! place per thread, and the sidebar. One small file (0600) beside the token, like the drafts, which stay in their own file and out of this one.
//! Written a moment after each change, and on quit; SIGTERM quits cleanly, so a new build taking over loses nothing (scripts/gpui/dev-reload.sh).
//! A scripted run starts clean and saves nothing, unless BACKPLANE_GPUI_PLACE=1 asks for it (the reload test).
use crate::{hub::Config, Backplane};
use gpui::{point, px, Context};
use serde_json::{json, Map, Value};
use std::{collections::HashMap, path::{Path, PathBuf}, sync::{atomic::{AtomicBool, Ordering}, mpsc, Mutex, OnceLock}, time::{Duration, Instant}};

const MAX_FILE: u64 = 1 << 20;
const MAX_SESSION: usize = 256 * 1024;
const MAX_OFFSETS: usize = 256;
const DEBOUNCE: Duration = Duration::from_millis(300);
/// How often the window's own part (scroll, sidebar) is looked at. No frame is asked for: nothing is drawn unless something else changed.
const LOOK: Duration = Duration::from_millis(500);
/// How near its end (px) the timeline counts as there: a thread left at its end opens at its end, new rows and all.
const END_SLACK: f32 = 4.;
/// How long a restored place is held against the first pictures, whatever they do to the scroll, before the list has been measured.
const HOLD: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, PartialEq)]
pub struct Place { pub target: Value, pub session: Value, pub offsets: HashMap<String, f32>, pub sidebar: bool }
impl Default for Place { fn default() -> Self { Self { target: Value::Null, session: Value::Null, offsets: HashMap::new(), sidebar: true } } }

/// The session as kept: drafts live in gpui-drafts.json, the request id is the hub's to count afresh, and a session too big to keep loses its scratch.
pub fn kept_session(session: &Value) -> Value {
    let Some(o) = session.as_object() else { return Value::Null };
    let mut s: Map<String, Value> = o.iter().filter(|(k, _)| !["drafts", "rid"].contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
    if serde_json::to_string(&s).map_or(true, |t| t.len() > MAX_SESSION) { s.remove("info"); }
    Value::Object(s)
}

/// A session to open with: what was kept, with the fields every session has.
pub fn opened_session(kept: &Value, rid: u64) -> Value {
    let mut s = json!({"sel": "", "rid": rid, "drafts": {}, "open": {}, "adding": false, "panel": false, "info": {}});
    for (k, v) in kept.as_object().into_iter().flatten() { s[k] = v.clone(); }
    s["rid"] = json!(rid); s["drafts"] = json!({}); s
}

/// Bounded, and anything unreadable is no place at all: the client opens as it does on a first run.
pub fn load(path: &Path) -> Option<Place> {
    if std::fs::metadata(path).ok()?.len() > MAX_FILE { return None; }
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    if v["version"] != 1 || !v["target"].is_object() || !v["session"].is_object() { return None; }
    let offsets = v["offsets"].as_object().into_iter().flatten().filter_map(|(k, y)| Some((k.clone(), y.as_f64().filter(|y| y.is_finite())? as f32))).collect();
    Some(Place { target: v["target"].clone(), session: v["session"].clone(), offsets, sidebar: v["sidebar"].as_bool().unwrap_or(true) })
}

/// Through a temp file (0600: a session can name what the person was looking at), so a crash never leaves half a file.
pub fn save(path: &Path, p: &Place) {
    if p.target.is_null() { return; }
    let offsets: Map<String, Value> = if p.offsets.len() > MAX_OFFSETS { Map::new() } else { p.offsets.iter().map(|(k, y)| (k.clone(), json!(y))).collect() };
    let Ok(text) = serde_json::to_string(&json!({"version": 1, "target": p.target, "session": p.session, "offsets": offsets, "sidebar": p.sidebar})) else { return };
    if text.len() as u64 > MAX_FILE { return; }
    let tmp = path.with_extension("tmp");
    let ok = {
        use std::io::Write;
        let mut o = std::fs::OpenOptions::new(); o.write(true).create(true).truncate(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; o.mode(0o600); }
        o.open(&tmp).and_then(|mut f| f.write_all(text.as_bytes())).is_ok()
    };
    if ok { let _ = std::fs::rename(tmp, path); }
}

enum Msg { View(Value, Value), Window(HashMap<String, f32>, bool), Flush(mpsc::Sender<()>) }

/// The background writer: a note is cheap, the file is written after DEBOUNCE of quiet.
struct Writer { tx: Mutex<mpsc::Sender<Msg>> }
impl Writer {
    fn start(path: PathBuf, mut place: Place) -> Self {
        let (tx, rx) = mpsc::channel::<Msg>();
        std::thread::Builder::new().name("place".into()).spawn(move || {
            let mut dirty = false;
            loop {
                match rx.recv_timeout(DEBOUNCE) {
                    Ok(Msg::View(t, s)) => { let s = kept_session(&s); if t != place.target || s != place.session { place.target = t; place.session = s; dirty = true; } }
                    Ok(Msg::Window(o, b)) => { if o != place.offsets || b != place.sidebar { place.offsets = o; place.sidebar = b; dirty = true; } }
                    Ok(Msg::Flush(ack)) => { if dirty { save(&path, &place); dirty = false; } let _ = ack.send(()); }
                    Err(mpsc::RecvTimeoutError::Timeout) => { if dirty { save(&path, &place); dirty = false; } }
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
        }).expect("place writer");
        Self { tx: Mutex::new(tx) }
    }
    fn send(&self, m: Msg) -> bool { self.tx.lock().map_or(false, |t| t.send(m).is_ok()) }
    fn flush(&self) { let (a, r) = mpsc::channel(); if self.send(Msg::Flush(a)) { let _ = r.recv_timeout(Duration::from_secs(1)); } }
}

static WRITER: OnceLock<Writer> = OnceLock::new();
/// What was saved, until each part has been handed over: the view to the hub worker, the rest to the window.
static SAVED_VIEW: Mutex<Option<(Value, Value)>> = Mutex::new(None);
static SAVED_WINDOW: Mutex<Option<(HashMap<String, f32>, bool)>> = Mutex::new(None);
/// A scroll place put back and not yet painted: the thread, the offset, until when.
static HELD: Mutex<Option<(String, f32, Instant)>> = Mutex::new(None);
static TERM: AtomicBool = AtomicBool::new(false);

/// Called once, before the hub worker starts.
pub fn open(config: &Config) {
    if config.script.is_some() && std::env::var_os("BACKPLANE_GPUI_PLACE").is_none() { return; }
    let path = config.token_file.with_file_name("gpui-place.json");
    let saved = load(&path);
    if let Some(p) = &saved {
        eprintln!("GPUI_LOG {}", json!({"place": {"target": p.target["kind"], "offsets": p.offsets.len(), "sidebar": p.sidebar}}));
        if let Ok(mut v) = SAVED_VIEW.lock() { *v = Some((p.target.clone(), p.session.clone())); }
        if let Ok(mut w) = SAVED_WINDOW.lock() { *w = Some((p.offsets.clone(), p.sidebar)); }
    }
    let _ = WRITER.set(Writer::start(path, saved.unwrap_or_default()));
}
/// The hub worker's first view: the target and session to open with.
pub fn take_view() -> Option<(Value, Value)> { SAVED_VIEW.lock().ok()?.take() }
/// The hub answered a view: this is where the person is now.
pub fn note_view(target: &Value, session: &Value) { if let Some(w) = WRITER.get() { w.send(Msg::View(target.clone(), session.clone())); } }

#[cfg(unix)] fn on_term() {
    unsafe extern "C" { fn signal(signum: i32, handler: usize) -> usize; }
    const SIGTERM: i32 = 15;
    extern "C" fn term(_: i32) { TERM.store(true, Ordering::SeqCst); unsafe { signal(SIGTERM, 0); } } // a second TERM ends a window too stuck to quit
    unsafe { signal(SIGTERM, term as extern "C" fn(i32) as usize); }
}
#[cfg(not(unix))] fn on_term() {}

/// The window's half: look at the scroll and sidebar now and then, save on quit, and quit on SIGTERM.
pub fn watch(cx: &mut Context<Backplane>) {
    if WRITER.get().is_none() { return; }
    on_term();
    cx.on_app_quit(|this, _| { this.place_note(); if let Some(w) = WRITER.get() { w.flush(); } async {} }).detach();
    cx.spawn(async move |weak, cx| loop {
        cx.background_executor().timer(LOOK).await;
        if weak.update(cx, |this, _| this.place_note()).is_err() { break; }
        if TERM.load(Ordering::SeqCst) { let _ = cx.update(|cx| cx.quit()); break; }
    }).detach();
}

/// What a held place does with this picture: keep the offset, or let go because the person moved, the list has a range now, or time is up.
pub fn hold_on(held: &str, raw: &str, range: f32, expired: bool) -> bool { held == raw && range <= 0. && !expired }
/// The offset to keep for a thread: none at its end, so it opens at its end.
pub fn kept_offset(y: f32, range: f32) -> Option<f32> { if -y >= range - END_SLACK { None } else { Some(y) } }

impl Backplane {
    /// A thread's picture landed. The first after a start brings the scroll places and the sidebar back; until the list is measured the place is put
    /// back after each picture, since a picture may send the timeline to its end first.
    pub(crate) fn place_restore(&mut self, raw: &str) {
        if let Some((offsets, sidebar)) = SAVED_WINDOW.lock().ok().and_then(|mut w| w.take()) {
            self.screens.offsets.extend(offsets); self.sidebar_open = sidebar; self.ui_epoch += 1;
            let y = self.screens.offsets.get(raw).copied();
            if let (Some(y), Ok(mut h)) = (y, HELD.lock()) { *h = Some((raw.to_string(), y, Instant::now() + HOLD)); }
            self.m("place", json!({"thread": raw, "y": y, "sidebar": sidebar}));
        }
        let Ok(mut h) = HELD.lock() else { return };
        let Some((thread, y, until)) = h.clone() else { return };
        if hold_on(&thread, raw, f32::from(self.scroll.max_offset().height), Instant::now() >= until) { self.scroll.set_offset(point(px(0.), px(y))); } else { *h = None; }
    }
    fn place_note(&mut self) {
        let Some(w) = WRITER.get() else { return };
        if HELD.lock().map_or(true, |h| h.is_some()) { return; } // the place was just put back; nothing has measured it yet
        let mut offsets = self.screens.offsets.clone();
        if !self.selected.is_empty() {
            match kept_offset(f32::from(self.scroll.offset().y), f32::from(self.scroll.max_offset().height)) { Some(y) => { offsets.insert(self.selected.clone(), y); } None => { offsets.remove(&self.selected); } }
        }
        w.send(Msg::Window(offsets, self.sidebar_open));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn file(n: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("gpui-place-{n}-{}", std::process::id())); std::fs::create_dir_all(&d).unwrap(); d.join("gpui-place.json") }
    fn place() -> Place {
        let mut offsets = HashMap::new(); offsets.insert("t1".to_string(), -640.5);
        Place { target: json!({"kind": "thread", "id": "t1"}), session: json!({"sel": "t1", "panel": true, "open": {"fold:a": "1"}, "info": {"view.kind:t1": "board"}}), offsets, sidebar: false }
    }
    #[test] fn a_place_comes_back_as_it_went() {
        let f = file("rt"); save(&f, &place()); assert_eq!(load(&f), Some(place()));
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600); }
    }
    #[test] fn drafts_never_reach_the_file() {
        let s = kept_session(&json!({"sel": "t1", "rid": 9, "drafts": {"t1": "a pasted secret"}, "panel": true}));
        assert_eq!(s, json!({"sel": "t1", "panel": true}));
        let f = file("dr"); save(&f, &Place { session: s, ..place() }); assert!(!std::fs::read_to_string(&f).unwrap().contains("secret"));
    }
    #[test] fn a_session_opens_with_every_field_and_a_fresh_request_id() {
        let s = opened_session(&json!({"sel": "t1", "panel": true, "rid": 4, "drafts": {"t1": "x"}}), 77);
        assert_eq!(s, json!({"sel": "t1", "rid": 77, "drafts": {}, "open": {}, "adding": false, "panel": true, "info": {}}));
    }
    #[test] fn a_huge_session_loses_its_scratch_and_keeps_the_thread() {
        let s = kept_session(&json!({"sel": "t1", "info": {"x": "y".repeat(MAX_SESSION + 1)}}));
        assert_eq!(s, json!({"sel": "t1"}));
    }
    #[test] fn a_bad_file_is_no_place() {
        let f = file("bad");
        for text in ["", "not json", "{}", r#"{"version":2,"target":{},"session":{}}"#, r#"{"version":1,"target":null,"session":{}}"#] { std::fs::write(&f, text).unwrap(); assert_eq!(load(&f), None, "{text}"); }
        std::fs::write(&f, vec![b' '; MAX_FILE as usize + 1]).unwrap(); assert_eq!(load(&f), None);
        assert_eq!(load(&file("none").with_file_name("absent.json")), None);
    }
    #[test] fn a_place_is_held_until_the_list_is_measured_the_person_moves_or_time_is_up() {
        assert!(hold_on("t1", "t1", 0., false));
        assert!(!hold_on("t1", "t1", 900., false), "the list has a range: the scroll is its own again");
        assert!(!hold_on("t1", "t2", 0., false), "the person went to another thread");
        assert!(!hold_on("t1", "t1", 0., true), "a list that never gets a range does not hold the place forever");
    }
    #[test] fn a_thread_at_its_end_keeps_no_offset_and_opens_at_its_end() {
        assert_eq!(kept_offset(-1000., 1000.), None); assert_eq!(kept_offset(-997., 1000.), None); assert_eq!(kept_offset(0., 0.), None);
        assert_eq!(kept_offset(-700., 1000.), Some(-700.)); assert_eq!(kept_offset(0., 1000.), Some(0.));
    }
    #[test] fn nothing_is_written_before_the_hub_has_drawn_a_view() {
        let f = file("none2"); save(&f, &Place::default()); assert!(!f.exists());
    }
    #[test] fn the_writer_waits_for_quiet_and_flush_writes_now() {
        let f = file("w"); let w = Writer::start(f.clone(), Place::default());
        for i in 0..20 { w.send(Msg::View(json!({"kind": "thread", "id": format!("t{i}")}), json!({"sel": format!("t{i}"), "rid": i}))); }
        assert!(!f.exists(), "written before the debounce");
        w.flush(); let p = load(&f).unwrap(); assert_eq!(p.target["id"], "t19"); assert_eq!(p.session, json!({"sel": "t19"}));
        w.send(Msg::Window(place().offsets, false)); w.flush(); let p = load(&f).unwrap(); assert_eq!(p.offsets["t1"], -640.5); assert!(!p.sidebar);
    }
}

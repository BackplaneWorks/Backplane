//! The one resident recognizer (docs/voice.md, "Kept loaded"): while any audio feature runs on the local model (dictation from every client, the Pebble ring and
//! webhook recordings), the hub keeps one `backplane-voice serve` alive from its start, and every `live` and `file` helper decodes through its socket instead of
//! building its own recognizer (about a second, and ~700 MB, each time). Nothing loads for the cloud engine. It is unloaded when no feature uses the local model,
//! or under real memory pressure (never mid-session), and loads again once there is room. If it is not up, a helper builds its own, as before.
use super::{session, vdict};
use crate::{hub::Hub, proc::{self, Kind}};
use serde_json::{json, Value};
use std::{collections::HashMap, path::{Path, PathBuf}, process::Stdio, sync::{atomic::{AtomicUsize, Ordering}, Arc, Mutex}, time::{Duration, Instant}};
use tokio::{io::{AsyncBufReadExt, BufReader}, sync::Notify};

/// How often the hub looks again (a setting, an install or a remove also wakes it at once).
const TICK: Duration = Duration::from_secs(5);
/// Below this much available memory the recognizer is let go (when no session is using it)...
pub const LOW_MB: u64 = 1024;
/// ...and it loads only with this much: building it peaks near 1 GB.
pub const ROOM_MB: u64 = 2560;
/// A server that died is started again after this, doubling to MAX_WAIT.
const FIRST_WAIT: Duration = Duration::from_secs(5);
const MAX_WAIT: Duration = Duration::from_secs(300);

#[derive(Debug, PartialEq, Eq)]
pub enum Do { Nothing, Start, Stop }
/// What to do now. `want`: the model the audio features need, if the local engine is theirs; `running`: the model of the server there is (up or loading);
/// `avail_mb`: memory available (None: unknown, never pressure); `busy`: a session or recording is decoding. Second: whether the load is held back by memory.
pub fn decide(want: Option<&str>, running: Option<&str>, avail_mb: Option<u64>, busy: bool) -> (Do, bool) {
    let low = avail_mb.is_some_and(|m| m < LOW_MB); let room = avail_mb.map_or(true, |m| m >= ROOM_MB);
    match (want, running) {
        (None, None) => (Do::Nothing, false),
        (None, Some(_)) => (Do::Stop, false),
        (Some(w), Some(r)) if w != r => (Do::Stop, false),
        (Some(_), Some(_)) => if low && !busy { (Do::Stop, true) } else { (Do::Nothing, false) },
        (Some(_), None) => if room { (Do::Start, false) } else { (Do::Nothing, true) },
    }
}

/// MemAvailable, in MB (Linux; None elsewhere).
fn avail_mb() -> Option<u64> {
    let m = std::fs::read_to_string("/proc/meminfo").ok()?;
    m.lines().find(|l| l.starts_with("MemAvailable:"))?.split_whitespace().nth(1)?.parse::<u64>().ok().map(|kb| kb / 1024)
}
fn rss_kb(pid: u32) -> u64 {
    std::fs::read_to_string(format!("/proc/{pid}/status")).ok().and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse().ok())).unwrap_or(0)
}
/// The socket, `<home>/voice/engine.sock`; None where a Unix socket cannot be (Windows) or the path is too long for one.
pub fn sock_path(home: &Path) -> Option<PathBuf> { let p = home.join("voice/engine.sock"); (cfg!(unix) && p.as_os_str().len() < 100).then_some(p) }

#[derive(Default)]
struct St { turn_tried: bool, model: Option<String>, up: bool, gen: u64, stdin: Option<tokio::process::ChildStdin>, pid: u32, held: bool, fails: u32, retry: Option<Instant>, ms: u64, rss_mb: u64, said: String }
#[derive(Default)]
pub struct Resident { st: Mutex<St>, busy: AtomicUsize, kick: Notify }
/// A session or recording decoding through the server: while one is held, memory pressure does not take the server away.
pub struct Hold(Arc<Resident>);
impl Drop for Hold { fn drop(&mut self) { self.0.busy.fetch_sub(1, Ordering::SeqCst); } }

static ALL: Mutex<Option<HashMap<PathBuf, Arc<Resident>>>> = Mutex::new(None);
/// The hub's at HOME, if it started one.
pub fn of(home: &Path) -> Option<Arc<Resident>> { ALL.lock().unwrap().as_ref().and_then(|m| m.get(home).cloned()) }

impl Resident {
    /// The socket to hand a helper for MODEL, when the server is up with it; and the hold that keeps it.
    pub fn server(self: &Arc<Self>, home: &Path, model: &str) -> Option<(String, Hold)> {
        let st = self.st.lock().unwrap();
        if !st.up || st.model.as_deref() != Some(vdict::model_id(model)) { return None }
        let p = sock_path(home)?; self.busy.fetch_add(1, Ordering::SeqCst);
        Some((p.to_string_lossy().into_owned(), Hold(self.clone())))
    }
    /// The speaker check was turned on: its model may be fetched now (the one try a run allows is given back).
    pub fn may_fetch(&self) { self.st.lock().unwrap().turn_tried = false; self.kick() }
    /// Look again now (a voice setting changed, a model was installed or removed).
    pub fn kick(&self) { self.kick.notify_one() }
    /// What Settings shows: `{"state": "off|loading|loaded|held", "model", "rss_mb", "ms"}`.
    fn state(&self) -> Value {
        let s = self.st.lock().unwrap();
        let state = if s.up { "loaded" } else if s.model.is_some() { "loading" } else if s.held { "held" } else { "off" };
        json!({"state": state, "model": s.model.clone().unwrap_or_default(), "rss_mb": s.rss_mb, "ms": s.ms})
    }
    async fn publish(&self, hub: &Hub) {
        let v = self.state(); let t = v.to_string();
        { let mut s = self.st.lock().unwrap(); if s.said == t { return } s.said = t.clone(); }
        hub.set_info("voice.resident", if v["state"] == "off" { "" } else { &t }).await;
    }
    fn stop(&self) { let mut s = self.st.lock().unwrap(); s.stdin = None; s.up = false; } // its stdin's end is its stop; the reader clears the rest when it is gone

    async fn wanted(hub: &Hub) -> Option<String> {
        if crate::copy::on() || session::exe().is_none() || sock_path(&hub.cfg.home).is_none() { return None }
        let pr = session::prefs(hub).await; let model = pr.get("voice.model").cloned().unwrap_or_default();
        let ready = session::ready(&hub.cfg.home, &model);
        // every audio feature (dictation, the ring, webhook recordings) goes by voice.engine; none can be switched off by itself
        (ready && session::engine_of(pr.get("voice.engine").map_or("auto", String::as_str), ready) == "local").then(|| vdict::model_id(&model).to_string())
    }
    async fn step(self: &Arc<Self>, hub: &Arc<Hub>) {
        let want = Self::wanted(hub).await;
        // the end-of-turn model (native/voice/src/turn.rs) came after many installs: fetched once, then the server starts again to load it
        // the speaker model (always-on's speaker check, and learning the voiceprint from dictation) likewise
        // (the speaker model only when the speaker check is turned on: it is off unless asked for)
        let speaker = super::always::Cfg::of(&session::prefs(hub).await).speaker;
        let missing: Vec<&str> = [("smart-turn-v3.2-cpu.onnx", "local-turn", true), ("nemo_en_titanet_small.onnx", "local-speaker", speaker)].iter().filter(|(f, _, w)| *w && !hub.cfg.home.join("voice/local").join(f).exists()).map(|x| x.1).collect();
        if want.is_some() && !missing.is_empty() && !std::mem::replace(&mut self.st.lock().unwrap().turn_tried, true) {
            if let Some(bin) = session::exe() {
                let (me, dir) = (self.clone(), hub.cfg.home.join("voice/local"));
                let cmds: Vec<String> = missing.iter().map(|s| s.to_string()).collect();
                tokio::spawn(async move {
                    let mut any = false;
                    for cmd in cmds {
                        let mut c = proc::tokio_cmd(Kind::Net, &bin); c.args([cmd.as_str(), "--local-dir", &dir.to_string_lossy()]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true);
                        let ok = tokio::time::timeout(Duration::from_secs(300), c.status()).await.ok().and_then(|r| r.ok()).map_or(false, |s| s.success());
                        eprintln!("voice: {cmd} {}", if ok { "fetched its model; the resident recognizer loads it" } else { "could not fetch its model; dictation goes on without it" }); any |= ok;
                    }
                    if any && me.busy.load(Ordering::SeqCst) == 0 { me.stop() } me.kick();
                });
            }
        }
        let (running, pid, up) = { let s = self.st.lock().unwrap(); (s.model.clone(), s.pid, s.up) };
        let busy = self.busy.load(Ordering::SeqCst) > 0;
        let (d, held) = decide(want.as_deref(), running.as_deref(), avail_mb(), busy);
        { let mut s = self.st.lock().unwrap(); s.held = held; if up { s.rss_mb = rss_kb(pid) / 1024 } }
        match d {
            Do::Nothing => {}
            Do::Stop => { if held { eprintln!("voice: memory is short: the resident recognizer is let go until there is room") } self.stop() }
            Do::Start => { let ok = self.st.lock().unwrap().retry.map_or(true, |t| Instant::now() >= t); if ok { if let Some(m) = want { self.spawn(hub, &m) } } }
        }
        self.publish(hub).await;
    }
    fn spawn(self: &Arc<Self>, hub: &Arc<Hub>, model: &str) {
        let (Some(bin), Some(sock)) = (session::exe(), sock_path(&hub.cfg.home)) else { return };
        let home = hub.cfg.home.clone(); crate::net::botnet::mkdir(&home.join("voice"));
        let mut c = proc::tokio_cmd(Kind::Local, &bin);
        c.args(["serve", "--local-dir", &home.join("voice/local").to_string_lossy(), "--model", model, "--socket", &sock.to_string_lossy()]).current_dir("/")
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true);
        let Ok(mut child) = c.spawn() else { let mut s = self.st.lock().unwrap(); s.fails += 1; s.retry = Some(Instant::now() + wait(s.fails)); return };
        let (stdin, stdout) = (child.stdin.take(), child.stdout.take());
        let gen = { let mut s = self.st.lock().unwrap(); s.gen += 1; s.model = Some(model.to_string()); s.up = false; s.stdin = stdin; s.pid = child.id().unwrap_or(0); s.gen };
        let (me, hub) = (self.clone(), hub.clone());
        tokio::spawn(async move {
            let t0 = Instant::now();
            if let Some(out) = stdout {
                let mut lines = BufReader::new(out).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    let Ok(o) = serde_json::from_str::<Value>(&l) else { continue };
                    if o["event"] == "loaded" {
                        { let mut s = me.st.lock().unwrap(); if s.gen == gen { s.up = true; s.fails = 0; s.retry = None; s.ms = o["ms"].as_u64().unwrap_or(0); s.rss_mb = o["rss_kb"].as_u64().unwrap_or(0) / 1024; } }
                        eprintln!("voice: the resident recognizer is loaded ({} ms, {} MB)", o["ms"], o["rss_kb"].as_u64().unwrap_or(0) / 1024);
                        me.publish(&hub).await;
                    }
                }
            }
            if tokio::time::timeout(Duration::from_secs(3), child.wait()).await.is_err() { let _ = child.start_kill(); let _ = child.wait().await; }
            {
                let mut s = me.st.lock().unwrap();
                if s.gen == gen {
                    // not ours to stop (its stdin is still held): it died; wait before the next one, longer each time
                    if s.stdin.is_some() { s.fails += 1; s.retry = Some(Instant::now() + wait(s.fails)); eprintln!("voice: the resident recognizer stopped after {} s", t0.elapsed().as_secs()) }
                    s.model = None; s.up = false; s.stdin = None; s.pid = 0;
                }
            }
            me.kick();
        });
    }
}
fn wait(fails: u32) -> Duration { FIRST_WAIT.saturating_mul(1 << fails.saturating_sub(1).min(8)).min(MAX_WAIT) }

/// Starts the hub's resident recognizer keeper (it loads nothing unless the local engine is the audio features').
pub fn start(hub: &Arc<Hub>) -> Arc<Resident> {
    let r = Arc::new(Resident::default());
    ALL.lock().unwrap().get_or_insert_with(HashMap::new).insert(hub.cfg.home.clone(), r.clone());
    let (me, hub) = (r.clone(), Arc::downgrade(hub));
    tokio::spawn(async move { loop { let Some(h) = hub.upgrade() else { me.stop(); return }; me.step(&h).await; drop(h); let _ = tokio::time::timeout(TICK, me.kick.notified()).await; } });
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn loaded_while_the_local_engine_is_wanted_let_go_only_under_pressure_and_not_mid_session() {
        let m = Some("parakeet-v2");
        assert_eq!(decide(None, None, Some(9000), false), (Do::Nothing, false), "the cloud engine: nothing loads");
        assert_eq!(decide(m, None, Some(9000), false), (Do::Start, false));
        assert_eq!(decide(m, None, None, false), (Do::Start, false), "memory unknown: never pressure");
        assert_eq!(decide(m, m, Some(9000), false), (Do::Nothing, false), "no idle unload");
        assert_eq!(decide(None, m, Some(9000), true), (Do::Stop, false), "no feature uses it");
        assert_eq!(decide(Some("parakeet-v3"), m, Some(9000), false), (Do::Stop, false), "another model: this one goes, that one comes next");
        assert_eq!(decide(m, m, Some(LOW_MB - 1), false), (Do::Stop, true), "pressure");
        assert_eq!(decide(m, m, Some(LOW_MB - 1), true), (Do::Nothing, false), "but not under a session");
        assert_eq!(decide(m, None, Some(LOW_MB + 1), false), (Do::Nothing, true), "held back until there is room");
        assert_eq!(decide(m, None, Some(ROOM_MB), false), (Do::Start, false), "and loaded again once there is");
    }
    #[test] fn a_dead_server_waits_longer_each_time() { assert_eq!([wait(1), wait(2), wait(3), wait(20)], [FIRST_WAIT, FIRST_WAIT * 2, FIRST_WAIT * 4, MAX_WAIT]); }
}

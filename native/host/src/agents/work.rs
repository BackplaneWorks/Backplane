//! Watched work: `work_run` starts a shell command Backplane owns, `work_watch` watches a pid or a file, `work_list`, `work_cancel`, `work_unwatch`.
//! The rows are the thread's `watch` WorkSet, the end is told to the thread as a system message (Watch.text) and wakes it, a command's output goes to
//! `<home>/work/<id>/out`. work_run's `lock` is locks.rs: the row waits for its grant, the job holds the lock until it ends. Not here yet: wake subscriptions on work (`wake_on` kind work). A command is its own process group and
//! survives a restart of the hub: its row keeps `pid:starttime` in the ident field, and at start it is watched again (cancel still reaches its group); when it is gone, or the pid is another
//! process now, it ends with the outcome unknown. On a copy of a log all of this is off: the tools are refused and nothing is re-armed.
use super::{bg, hub_id, lines::s, locks, now_s, Agents};
use serde_json::{json, Value};
use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use tokio::sync::watch;

fn fail(m: &str) -> (bool, Value) { (false, json!({"error":m})) }
/// M.Bg.enc: a command or path kept whole on one tab-free line.
pub fn enc(t: &str) -> String { format!("\\e{}", t.replace('\\', "\\b").replace('\t', "\\t").replace('\n', "\\n").replace('\r', "\\r")) }
pub fn dec(t: &str) -> String { match t.strip_prefix("\\e") { None => t.to_string(), Some(r) => { let mut o = String::new(); let mut it = r.chars(); while let Some(c) = it.next() { if c == '\\' { match it.next() { Some('b') => o.push('\\'), Some('t') => o.push('\t'), Some('n') => o.push('\n'), Some('r') => o.push('\r'), Some(x) => { o.push('\\'); o.push(x) } None => o.push('\\') } } else { o.push(c) } } o } } }
/// Watch.line
pub fn row(id: &str, state: &str, note: &str, kind: &str, target: &str, ident: &str, deadline: u64) -> String { format!("{}\t\t{}\twatch\t{}\t{}\t{}\t{}", bg::line(id, state), bg::clean(note), bg::clean(kind), enc(target), bg::clean(ident), deadline) }
pub fn kind(l: &str) -> String { bg::field(l, 5) }
pub fn target(l: &str) -> String { dec(&bg::field(l, 6)) }
pub fn deadline(l: &str) -> u64 { bg::field(l, 8).parse().unwrap_or(0) }
/// Watch.text
pub fn text(note: &str, word: &str, detail: &str) -> String { format!("[Backplane] Watched work \"{note}\" ended: {word}{}", if detail.is_empty() { String::new() } else { format!("\n{detail}") }) }
/// Watch.word
pub fn word(status: &str, cancelling: bool) -> String { if status == "gone" && cancelling { "cancelled".into() } else if status == "gone" { "it ended, outcome unknown (not known to have succeeded)".into() } else { status.into() } }

pub struct Ctl { pub stop: watch::Sender<bool> }
/// What a copy of a log says to every work tool (SIDE-BY-SIDE.md): nothing is launched, watched or re-armed there.
pub const COPY_LINE: &str = "watched work is off on a copy of a log (BACKPLANE_COPY=1): nothing is started or watched here";
/// A process's start time (clock ticks since boot, /proc/<pid>/stat field 22): with the pid it names one process, so a reused pid is not mistaken for it (Watch.ident).
#[cfg(target_os = "linux")]
pub fn start_time(pid: u32) -> Option<u64> { let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?; let rest = s.rsplit_once(')')?.1; rest.split_whitespace().nth(19)?.parse().ok() }
/// No /proc: `ps -o lstart=` names the start second (the text is hashed); a pid that is not there prints nothing.
#[cfg(all(unix, not(target_os = "linux")))]
pub fn start_time(pid: u32) -> Option<u64> {
    let o = crate::proc::std_cmd(crate::proc::Kind::Local, "ps").args(["-o", "lstart=", "-p", &pid.to_string()]).env("LC_ALL", "C").stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()?;
    let t = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if !o.status.success() || t.is_empty() { return None }
    Some(t.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3)))
}
/// Windows: the process's creation time.
#[cfg(windows)]
pub fn start_time(pid: u32) -> Option<u64> { crate::plat::proc::start_time(pid) }
pub fn ident_of(pid: u32) -> String { start_time(pid).map_or_else(|| "-".into(), |st| format!("{pid}:{st}")) }
/// Whether the process an ident names is still that process.
pub fn ident_alive(ident: &str) -> bool { let Some((p, st)) = ident.split_once(':') else { return false }; match (p.parse::<u32>(), st.parse::<u64>()) { (Ok(p), Ok(st)) => start_time(p) == Some(st), _ => false } }

impl Agents {
    async fn watch_rows(&self, th: &str) -> Vec<String> { let w = self.world().await; bg::lines(w.setting(&format!("watch.{th}"))) }
    async fn put_watch(&self, th: &str, rows: &[String]) { self.commit_events(vec![self.rows_event(th, rows)]).await }
    pub(super) fn rows_event(&self, th: &str, rows: &[String]) -> Value { json!({"$":"WorkSet","thread":th,"kind":"watch","rows":bg::show(rows),"at":now_s()}) }
    pub(super) async fn rows_of(&self, th: &str) -> Vec<String> { self.watch_rows(th).await }
    pub(super) async fn put_rows(&self, th: &str, rows: &[String]) { self.put_watch(th, rows).await }
    fn secs(&self, w: &super::world::World, a: &Value) -> u64 { let raw = match a.get("timeoutSec") { Some(Value::Number(n)) => n.to_string(), Some(Value::String(x)) => x.clone(), _ => String::new() }; let ok = |x: &str| !x.is_empty() && x.len() <= 10 && x.chars().all(|c| c.is_ascii_digit()); if ok(&raw) { raw.parse().unwrap_or(1200) } else { let d = w.setting("work.timeout.seconds"); if ok(d) { d.parse().unwrap_or(1200) } else { 1200 } } }

    pub async fn tool_work_run(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        if crate::copy::on() { return fail(COPY_LINE) }
        let cmd = s(a, "command").to_string(); if cmd.trim().is_empty() { return fail("command is required") }
        let lock = s(a, "lock").to_string(); if !lock.is_empty() && !locks::name_ok(&lock) { return fail(locks::BAD_NAME) }
        let _g = self.work_lock.lock().await;
        let (cwd, secs) = { let w = self.world().await; if !w.threads.get(thread).is_some_and(|t| !t.archived && !t.deleted) { return fail("unknown thread") } (w.cwd(thread), self.secs(&w, a)) };
        let (now, mut rows) = (now_s(), self.watch_rows(thread).await); let id = format!("w-{thread}-{}-{}", super::now_ms(), self.seq_n());
        let note = if s(a, "note").is_empty() { cmd.clone() } else { s(a, "note").to_string() }; let dl = if secs == 0 { 0 } else { now + secs };
        // the job's intent and its place in the queue are one commit: a queued row always has its party, and a restart finds both
        let (state, pos) = if lock.is_empty() { rows.push(row(&id, "launching", &note, "run", &cmd, "-", dl)); self.put_watch(thread, &rows).await; ("launching".to_string(), None) } else {
            let l = self.lock_enter(&lock, locks::Party { id: id.clone(), kind: "work".into(), thread: thread.into(), note: note.clone(), since: now, lease: 0 }, now).await;
            let held = l.owner() == id && l.ready; let state = if held { "launching".to_string() } else { locks::wait_word(&lock, l.position(&id)) };
            rows.push(row(&id, &state, &note, "run", &cmd, "-", dl));
            self.commit_events(vec![json!({"$":"LockChanged","name":lock,"lock":l.json(),"at":now}), self.rows_event(thread, &rows)]).await;
            (state, Some(l.position(&id))) };
        if state == "launching" { self.work_launch(thread, &id, cmd, cwd, dl, Some(lock.as_str()).filter(|l| !l.is_empty())) }
        let mut r = json!({"id":id,"state":state,"next":"End your turn: you will be woken with the outcome."}); if let Some(p) = pos { r["position"] = json!(p) }
        (true, r)
    }
    /// The job starts: its stop switch, then its task. `lock`: the name whose flock the job takes with it, so exclusion ends when the command does, even if the hub is gone by then.
    pub(super) fn work_launch(self: &Arc<Self>, th: &str, id: &str, cmd: String, cwd: String, dl: u64, lock: Option<&str>) {
        let (tx, rx) = watch::channel(false); self.works.lock().unwrap().insert(id.to_string(), Ctl { stop: tx });
        let guard = lock.and_then(|n| self.guard_file(n, id));
        let me = self.clone(); let (th, id) = (th.to_string(), id.to_string());
        tokio::spawn(async move { me.run_work(th, id, cmd, cwd, dl, rx, guard).await });
    }
    async fn run_work(self: Arc<Self>, th: String, id: String, cmd: String, cwd: String, dl: u64, mut stop: watch::Receiver<bool>, guard: Option<std::fs::File>) {
        let dir: PathBuf = self.hub.cfg.home.join("work").join(&id); let _ = std::fs::create_dir_all(&dir);
        let outp = dir.join("out"); let _ = std::fs::write(dir.join("cmd"), &cmd);
        let (so, se) = match std::fs::File::create(&outp).and_then(|f| f.try_clone().map(|g| (f, g))) { Ok(x) => x, Err(e) => { self.work_end(&th, &id, "gone", &format!("could not open the output file: {e}")).await; return } };
        // in a memory scope of its own (memscope.rs), the command in an inner sh: the outer one stays 0.6 s after it, so the scope is still there
        // when its watcher looks for a kill (a fast eater's sh ends with it, and the scope with them)
        let scope = crate::memscope::wrap("work"); let scoped = scope.is_some();
        let mut c = crate::proc::tokio_cmd_in(crate::proc::Kind::Shell, "sh", scope); crate::memscope::raise_oom(&mut c);
        if scoped { c.args(["-c", "sh -c \"$0\"; e=$?; sleep 0.6; exit $e", cmd.as_str()]); } else { c.args(["-c", cmd.as_str()]); }
        c.current_dir(if cwd.is_empty() { self.hub.cfg.home.clone() } else { PathBuf::from(&cwd) }).env("BACKPLANE_LOCK_HOME", &self.hub.cfg.home).stdin(Stdio::null()).stdout(so).stderr(se).kill_on_drop(false);
        for (k, v) in super::kicadsel::job_env(&self.kicad_env.lock().unwrap(), std::env::var_os("PATH")) { c.env(k, v); } // the KiCad the agent was given (kicadsel.rs)
        crate::plat::proc::own_group_tokio(&mut c);
        if let Some(f) = &guard { crate::plat::proc::inherit(&mut c, f) } // the job inherits its lock's flock
        let mut child = match c.spawn() { Ok(c) => c, Err(e) => { self.work_end(&th, &id, "gone", &format!("could not start: {e}")).await; return } };
        let pid = child.id().unwrap_or(0); crate::plat::proc::adopt(pid, false); drop(guard);
        let killed: Arc<std::sync::Mutex<Option<String>>> = Arc::default(); // a process of the job killed for memory in its scope (memscope.rs): said with its end
        { let k = killed.clone(); crate::memscope::watch(pid, move |w| { *k.lock().unwrap() = Some(w); }); }
        { let _g = self.work_lock.lock().await; let rows = self.watch_rows(&th).await;
          if let Some(l) = rows.iter().find(|l| bg::field(l, 0) == bg::clean(&id)) { if bg::doing(l) == "launching" { // only this row, only if it is still launching (a cancel meanwhile keeps "cancelling")
              let nr: Vec<String> = rows.iter().map(|x| if bg::field(x, 0) == bg::clean(&id) { row(&id, "watching", &bg::work_title(x), "run", &target(x), &ident_of(pid), deadline(x)) } else { x.clone() }).collect(); self.put_watch(&th, &nr).await } } }
        let mut timed_out = dl == 0;
        let status = loop {
            let left = if timed_out { Duration::from_secs(3600) } else { Duration::from_secs(dl.saturating_sub(now_s()).max(1)) };
            tokio::select! {
                r = child.wait() => break match r { Ok(st) => format!("exit {}", crate::plat::exit_code(&st)), Err(_) => "gone".into() },
                r = stop.changed() => { if r.is_err() || !*stop.borrow() { let st = child.wait().await; break match st { Ok(st) => format!("exit {}", crate::plat::exit_code(&st)), Err(_) => "gone".into() } } // unwatched: the command goes on, nobody is told
                    if pid > 0 { crate::plat::proc::kill_group(pid, crate::plat::proc::TERM); tokio::spawn(async move { tokio::time::sleep(Duration::from_secs(3)).await; crate::plat::proc::kill_group(pid, crate::plat::proc::KILL); }); } let _ = child.wait().await; break "gone".into() }
                _ = tokio::time::sleep(left), if !timed_out => { timed_out = true; self.work_end(&th, &id, "timeout", &tail(&outp)).await; }
            }
        };
        if pid > 0 { crate::plat::proc::kill_group(pid, crate::plat::proc::KILL); } // anything the command left running in its group
        tokio::time::sleep(Duration::from_millis(150)).await; // the watcher's last word lands before the end is told
        let detail = match killed.lock().unwrap().take() { Some(w) => format!("{w}\n{}", tail(&outp)), None => tail(&outp) };
        self.work_end(&th, &id, &status, &detail).await;
    }
    /// Watch.end: the row is dropped and the thread told once (a timeout keeps the row, ended with the outcome unknown).
    pub async fn work_end(self: &Arc<Self>, th: &str, id: &str, status: &str, detail: &str) {
        let _g = self.work_lock.lock().await;
        let timeout = status == "timeout"; let held = self.lock_of(id).await;
        if !timeout { if let Some((name, _)) = &held { if !self.lock_free(name, id).await { self.lock_leave(name, id).await; } } } // whatever became of the row: the job is over, so is its claim
        let rows = self.watch_rows(th).await; let Some(l) = rows.iter().find(|l| bg::field(l, 0) == bg::clean(id)).cloned() else { return };
        if timeout && held.is_some() { // a timeout cannot free a lock over live work: the row stays, watched, and the thread hears it once
            if bg::doing(&l) == locks::TIMED_OUT { return }
            let nr: Vec<String> = rows.iter().map(|x| if bg::field(x, 0) == bg::clean(id) { row(id, locks::TIMED_OUT, &bg::work_title(x), "run", &target(x), &bg::field(x, 7), 0) } else { x.clone() }).collect();
            self.put_watch(th, &nr).await; self.work_tell(th, text(&bg::work_title(&l), "timed out, still running, lock retained", detail)).await; return
        }
        if bg::ended(&l) && !(bg::doing(&l).strip_prefix(bg::ENDED) == Some("unknown") && !timeout) { return } // told already
        let note = bg::work_title(&l); let cancelling = bg::doing(&l) == "cancelling";
        let (rows, say) = if timeout { (bg::end(&rows, id, "unknown"), text(&note, "timed out, still running, outcome unknown", detail)) } else { (bg::drop(&rows, id), text(&note, &word(status, cancelling), detail)) };
        if !timeout { self.works.lock().unwrap().remove(id); }
        self.put_watch(th, &rows).await;
        self.work_tell(th, say).await;
    }
    /// An agent process of `th` watched in its memory scope: a process it started that the kernel kills for memory is told to the thread at once.
    pub(super) fn memory_watch(self: &Arc<Self>, th: &str, pid: u32) {
        let (me, th) = (self.clone(), th.to_string()); let rt = tokio::runtime::Handle::current();
        crate::memscope::watch(pid, move |w| { let (me, th) = (me.clone(), th.clone()); rt.spawn(async move { me.work_tell(&th, w).await }); });
    }
    /// The outcome told: a stopped thread hears it without waking; an archived or deleted one is told nothing.
    pub(super) async fn work_tell(self: &Arc<Self>, th: &str, say: String) {
        let (live, interrupted) = { let w = self.world().await; (w.threads.get(th).map_or(false, |t| !t.archived && !t.deleted), w.threads.get(th).map_or(false, |t| t.turn == "interrupted")) };
        if !live { return }
        let msg = format!("t:backplane:{}", hub_id("l", now_s(), self.seq_n()));
        if interrupted { self.commit_events(vec![json!({"$":"MessagePosted","thread":th,"msg":msg,"role":"system","text":say,"at":now_s()})]).await }
        else { let _ = self.inbound("system", th, &msg, &say, ("backplane".into(), "system".into(), "work".into()), vec![]).await; }
    }

    pub async fn tool_work_watch(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        if crate::copy::on() { return fail(COPY_LINE) }
        let k = s(a, "kind").to_string(); if k != "pid" && k != "file" { return fail("kind must be pid or file") }
        let tgt = s(a, "target").to_string(); if tgt.trim().is_empty() { return fail("target is required") }
        let _g = self.work_lock.lock().await;
        let secs = { let w = self.world().await; if !w.threads.contains_key(thread) { return fail("unknown thread") } self.secs(&w, a) };
        let (now, mut rows) = (now_s(), self.watch_rows(thread).await); let id = format!("w-{thread}-{}-{}", super::now_ms(), self.seq_n());
        let note = if s(a, "note").is_empty() { tgt.clone() } else { s(a, "note").to_string() }; let dl = if secs == 0 { 0 } else { now + secs };
        rows.push(row(&id, "watching", &note, &k, &tgt, if k == "file" { &id } else { "-" }, dl)); self.put_watch(thread, &rows).await;
        self.spawn_poll(thread.into(), id.clone(), k.clone(), tgt, dl, if k == "file" { id.clone() } else { "-".into() });
        (true, json!({"id":id,"token":id,"next":if k == "file" { "Make the work write the token into the file when it is done, then end your turn." } else { "End your turn: you will be woken when it ends." }}))
    }
    fn spawn_poll(self: &Arc<Self>, th: String, id: String, kind: String, target: String, dl: u64, ident: String) {
        let (tx, mut stop) = watch::channel(false); self.works.lock().unwrap().insert(id.clone(), Ctl { stop: tx }); let me = self.clone();
        tokio::spawn(async move {
            let mut noted = dl == 0;
            loop {
                tokio::select! { r = stop.changed() => {
                    if r.is_ok() && *stop.borrow() && kind == "run" { if let Some(p) = ident.split(':').next().and_then(|x| x.parse::<i32>().ok()) { if ident_alive(&ident) && p > 1 { crate::plat::proc::kill_group(p as u32, crate::plat::proc::TERM); tokio::spawn(async move { tokio::time::sleep(Duration::from_secs(3)).await; crate::plat::proc::kill_group(p as u32, crate::plat::proc::KILL); }); } } } // a re-armed command: cancel reaches its group
                    me.work_end(&th, &id, "gone", "").await; return } _ = tokio::time::sleep(Duration::from_secs(1)) => {} }
                let done = match kind.as_str() { "run" => !ident_alive(&ident), "pid" => target.parse::<i32>().map_or(true, |p| p <= 0 || !crate::plat::proc::exists(p as u32)), _ => has_token(&target, &id) };
                if done { me.work_end(&th, &id, if kind == "file" { "found" } else { "gone" }, "").await; return }
                if !noted && now_s() >= dl { noted = true; me.work_end(&th, &id, "timeout", "").await }
            }
        });
    }

    pub async fn tool_work_list(self: &Arc<Self>, thread: &str) -> (bool, Value) {
        let rows = self.watch_rows(thread).await;
        (true, json!({"work": rows.iter().map(|l| json!({"id":bg::field(l, 0),"state":if bg::ended(l) { bg::doing(l).trim_start_matches(bg::ENDED).to_string() } else { bg::doing(l) },"note":bg::work_title(l),"kind":kind(l),"target":target(l)})).collect::<Vec<_>>()}))
    }
    pub async fn tool_work_cancel(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        if crate::copy::on() { return fail(COPY_LINE) }
        let id = s(a, "id").to_string(); let _g = self.work_lock.lock().await;
        let rows = self.watch_rows(thread).await; let l = rows.iter().find(|l| bg::field(l, 0) == bg::clean(&id)).cloned();
        match l { Some(l) if locks::queued_row(&bg::doing(&l)) => { // never launched: its place in the queue goes with its row
            if let Some((name, lk)) = self.lock_of(&id).await { if lk.owner() == id { self.lock_free(&name, &id).await; } else { self.lock_leave(&name, &id).await; } }
            self.put_watch(thread, &bg::drop(&self.watch_rows(thread).await, &id)).await; self.work_tell(thread, text(&bg::work_title(&l), "cancelled", "")).await;
            (true, json!({"id":id,"state":"cancelled"})) }
            Some(l) if !bg::ended(&l) => {
            self.put_watch(thread, &bg::set(&rows, &id, "cancelling")).await;
            if let Some(c) = self.works.lock().unwrap().get(&id) { let _ = c.stop.send(true); }
            (true, json!({"id":id,"state":"cancelling"})) }
            _ => fail("no such work running") }
    }
    pub async fn tool_work_unwatch(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        let id = s(a, "id").to_string(); let _g = self.work_lock.lock().await;
        let rows = self.watch_rows(thread).await; let Some(l) = rows.iter().find(|l| bg::field(l, 0) == bg::clean(&id)).cloned() else { return fail("no such work, or it owns a lock; cancel locked work instead") };
        if self.lock_of(&id).await.is_some() { return fail("no such work, or it owns a lock; cancel locked work instead") }
        self.put_watch(thread, &bg::drop(&rows, &id)).await;
        if kind(&l) != "run" { if let Some(c) = self.works.lock().unwrap().remove(&id) { c.stop.send_replace(true); } } else { self.works.lock().unwrap().remove(&id); } // an unwatched command may still run
        (true, json!({"id":id,"state":"no longer watched (it may still be running)"}))
    }
    /// At start (Watch.rearm): every open row is watched again. A `run` command is its own process group and may have outlived the hub: its `pid:starttime` ident is watched (cancel reaches its group);
    /// gone, or another process now, it ends with the outcome unknown. Nothing happens on a copy of a log (the rows stay as they are, nothing is polled or delivered).
    pub async fn work_rearm(self: &Arc<Self>) {
        if crate::copy::on() { return }
        let all: Vec<(String, Vec<String>)> = { let w = self.world().await; w.st.iter().filter_map(|(k, v)| k.strip_prefix("watch.").map(|t| (t.to_string(), bg::lines(v)))).collect() };
        for (th, rows) in all { for l in rows.iter().filter(|l| !bg::ended(l)) {
            if locks::queued_row(&bg::doing(l)) { continue } // it never started: it waits for its grant
            let id = bg::field(l, 0); let (k, ident) = (kind(l), bg::field(l, 7));
            if k == "run" && !ident_alive(&ident) { self.work_end(&th, &id, "gone", "it was not running when Backplane started again").await; continue }
            self.spawn_poll(th.clone(), id, k, target(l), deadline(l), ident);
        } }
    }
}
/// The end of an output file: the last 4 KiB read as it is (lossy for bytes that are not UTF-8), at most 1500 characters of it; never the whole file.
fn tail(p: &std::path::Path) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(p) else { return String::new() }; let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = f.seek(SeekFrom::Start(len.saturating_sub(4096))); let mut b = vec![]; let _ = f.take(4096).read_to_end(&mut b);
    let t = String::from_utf8_lossy(&b); let t = t.trim_end(); let n = t.chars().count(); t.chars().skip(n.saturating_sub(1500)).collect()
}
/// Whether a file holds the token: its last 1 MiB, read lossy (a file that is not UTF-8 is still found; a huge one is not read whole every second).
fn has_token(p: &str, token: &str) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(p) else { return false }; let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = f.seek(SeekFrom::Start(len.saturating_sub(1 << 20))); let mut b = vec![]; let _ = f.take(1 << 20).read_to_end(&mut b); String::from_utf8_lossy(&b).contains(token)
}

//! Named locks (docs/locks.md): one holder and a FIFO queue per name, `lock_acquire` / `lock_release` / `lock_cancel` / `lock_list`, `work_run`'s `lock`, the clients' `lock.release` / `lock.cancel`.
//! The record is the log's `LockChanged` (`lock.<name>`, `locks.names`), written whole after each transition. Every transition runs under `work_lock`, as the watch rows do: a read of the record and its
//! write cannot interleave, so one holder is structural. Nothing here blocks an MCP call: a queued thread ends its turn and a message wakes it; a queued job's row waits and is launched on its grant.
//! A reservation also owns the OS flock of `<lock dir>/bp-<name>.lock`, so a raw `flock` is excluded and an existing one holds the grant back (position 0). A job inherits that flock: it outlives the hub with the job.
//! `locks_tick` (every 15 s, and at start) drops parties that are gone, walks the leases and retries a grant. A copy of a log does none of it: the tools are refused, nothing is taken, nothing is re-armed.
pub use super::lockrec::*;
use super::{bg, lines::s, now_s, work, Agents};
use serde_json::{json, Value};
use crate::plat::fs::OpenOptionsExt;
use std::{path::PathBuf, sync::Arc, time::Duration};

pub const COPY_LINE: &str = "named locks are off on a copy of a log (BACKPLANE_COPY=1): nothing is taken, released or queued here";
pub const BAD_NAME: &str = "lock name must contain 1..64 letters, digits, hyphens or underscores";
/// Seconds a warned, idle holder keeps its reservation; and a client's (the shell helper's) lease.
const GRACE: u64 = 60;
pub const CLIENT_LEASE: u64 = 90;
const TICK: Duration = Duration::from_secs(15);
/// A timed-out job that holds a lock: the row stays, watched, until the command really ends.
pub const TIMED_OUT: &str = "watching (timed out, still running)";

fn change(name: &str, l: &Lock, at: u64) -> Value { json!({"$":"LockChanged","name":name,"lock":l.json(),"at":at}) }
fn fail(m: &str) -> (bool, Value) { (false, json!({"error":m})) }
/// Who holds a flock on this file, from /proc/locks: {"pid","cmd","since"}. Elsewhere (no /proc), or with the holder out of sight, nobody is named.
fn who_holds(path: &std::path::Path, now: u64) -> Option<Value> {
    use crate::plat::fs::MetadataExt;
    let ino = std::fs::metadata(path).ok()?.ino(); let locks = std::fs::read_to_string("/proc/locks").ok()?;
    let pid: u32 = locks.lines().find_map(|l| { let f: Vec<&str> = l.split_whitespace().collect(); (f.get(1) == Some(&"FLOCK") && f.get(5).and_then(|d| d.rsplit(':').next()) == Some(ino.to_string().as_str())).then(|| f[4].parse().ok()).flatten() })?;
    let cmd = std::fs::read(format!("/proc/{pid}/cmdline")).map(|b| String::from_utf8_lossy(&b).replace('\0', " ").trim().to_string()).unwrap_or_default();
    Some(json!({"pid":pid,"cmd":cmd.chars().take(200).collect::<String>(),"since":now}))
}

/// The flocks this hub holds: name to (holder, the open file). The lock goes with the file, or with a job that inherited it.
pub type Guards = std::sync::Mutex<std::collections::HashMap<String, (String, std::fs::File)>>;

impl Agents {
    pub(super) async fn lock_get(&self, name: &str) -> Lock { let w = self.world().await; Lock::read(w.setting(&format!("lock.{name}"))) }
    pub(super) async fn lock_names(&self) -> Vec<String> { let w = self.world().await; bg::lines(w.setting("locks.names")) }
    async fn lease_at(&self, now: u64) -> u64 { let w = self.world().await; let m: u64 = w.setting("locks.lease.minutes").parse().unwrap_or(0); now + if m == 0 { 20 } else { m } * 60 }
    fn lock_file(&self, name: &str) -> PathBuf {
        if let Some(d) = std::env::var_os("BACKPLANE_LOCK_DIR").filter(|d| !d.is_empty()) { return PathBuf::from(d).join(format!("bp-{name}.lock")) }
        if cfg!(test) { return self.hub.cfg.home.join(format!("bp-{name}.lock")) } // a test never touches the real /tmp/bp-<name>.lock
        if cfg!(unix) { PathBuf::from("/tmp").join(format!("bp-{name}.lock")) } else { std::env::temp_dir().join(format!("bp-{name}.lock")) } // /tmp: what scripts/bp-lock.sh takes
    }
    /// Take the flock for `id`, not blocking. false: someone outside holds it. A file that cannot be opened holds nobody back: the hub's own record still serialises.
    fn guard_take(&self, name: &str, id: &str) -> bool {
        let mut g = self.guards.lock().unwrap();
        if g.get(name).is_some_and(|(h, _)| h == id) { return true }
        g.remove(name);
        let Ok(f) = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).mode(0o600).open(self.lock_file(name)) else { return true };
        if !crate::plat::hub::lock(&f) { return false }
        g.insert(name.to_string(), (id.to_string(), f)); true
    }
    /// The unlock acts on the open file, so a stray process that inherited it from a job holds nothing back.
    fn guard_drop(&self, name: &str, id: &str) { let mut g = self.guards.lock().unwrap(); if g.get(name).is_some_and(|(h, _)| h == id) { if let Some((_, f)) = g.remove(name) { let _ = f.unlock(); } } }
    /// A descriptor of the flock for a job to inherit.
    pub fn guard_file(&self, name: &str, id: &str) -> Option<std::fs::File> { self.guards.lock().unwrap().get(name).filter(|(h, _)| h == id).and_then(|(_, f)| f.try_clone().ok()) }

    /// The lock a job holds or waits for.
    pub async fn lock_of(&self, id: &str) -> Option<(String, Lock)> {
        let w = self.world().await;
        bg::lines(w.setting("locks.names")).into_iter().find_map(|n| { let l = Lock::read(w.setting(&format!("lock.{n}"))); l.has(id).then_some((n, l)) })
    }
    /// A new party. Holder of an empty lock with the flock free: held at once. Otherwise queued, or the head waiting on a flock outside. Writes nothing of its own: the caller commits `l`.
    pub(super) async fn lock_enter(&self, name: &str, p: Party, now: u64) -> Lock {
        let mut l = self.lock_get(name).await; let id = p.id.clone(); l.acquire(p);
        if l.owner() == id { if self.guard_take(name, &id) { l.ready = true; let at = self.lease_at(now).await; if let Some(h) = &mut l.holder { if h.kind != "client" { h.lease = at } } } else { l.outside = who_holds(&self.lock_file(name), now) } }
        l
    }

    /// Every lock the thread holds or waits for (as itself: its own claims, not its work's, which work_cancel frees).
    pub(super) async fn locks_drop(self: &Arc<Self>, th: &str) {
        for n in self.lock_names().await {
            if !self.lock_get(&n).await.has(th) { continue }
            let _g = self.work_lock.lock().await;
            if self.lock_get(&n).await.owner() == th { self.lock_free(&n, th).await; } else { self.lock_leave(&n, th).await; }
        }
    }
    pub async fn tool_lock_acquire(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        if crate::copy::on() { return fail(COPY_LINE) }
        let name = s(a, "name"); if !name_ok(name) { return fail(BAD_NAME) }
        let _g = self.work_lock.lock().await;
        { let w = self.world().await; if !w.threads.get(thread).is_some_and(|t| !t.archived && !t.deleted) { return fail("unknown thread") } }
        let had = self.lock_get(name).await; if had.has(thread) { return (true, had.answer(thread)) } // a repeat keeps its place and appends nothing
        let now = now_s(); let l = self.lock_enter(name, Party { id: thread.into(), kind: "thread".into(), thread: thread.into(), note: s(a, "note").into(), since: now, lease: 0 }, now).await;
        self.commit_events(vec![change(name, &l, now)]).await;
        (true, l.answer(thread))
    }
    pub async fn tool_lock_release(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        if crate::copy::on() { return fail(COPY_LINE) }
        let _g = self.work_lock.lock().await; (true, json!({"released":self.lock_free(s(a, "name"), thread).await}))
    }
    pub async fn tool_lock_cancel(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        if crate::copy::on() { return fail(COPY_LINE) }
        let _g = self.work_lock.lock().await; (true, json!({"cancelled":self.lock_leave(s(a, "name"), thread).await}))
    }
    pub async fn tool_lock_list(&self) -> (bool, Value) {
        let w = self.world().await;
        (true, json!({"locks":bg::lines(w.setting("locks.names")).iter().map(|n| { let l = Lock::read(w.setting(&format!("lock.{n}"))); let mut v = l.json(); v["name"] = json!(n); v }).collect::<Vec<_>>()}))
    }
    /// The shell helper's calls (scripts/bp-lock.sh): `acquire` and `renew` keep a lease of CLIENT_LEASE seconds, `status` reads; a client the record no longer holds is queued again and told so. `wait` long-polls on this connection only.
    pub async fn lock_http(self: &Arc<Self>, op: &str, a: &Value) -> Value {
        if crate::copy::on() { return json!({"error":COPY_LINE}) }
        let (name, holder) = (s(a, "name"), s(a, "holder")); if !name_ok(name) { return json!({"error":BAD_NAME}) }
        if !holder.starts_with("pid:") { return json!({"error":"holder must begin pid:"}) }
        let now = now_s();
        let mut r = { let _g = self.work_lock.lock().await; match op {
            "release" => return json!({"released":self.lock_free(name, holder).await}),
            "cancel" => return json!({"cancelled":self.lock_leave(name, holder).await}),
            _ => {
                let mut l = self.lock_get(name).await; let lost = !l.has(holder);
                if lost { l = self.lock_enter(name, Party { id: holder.into(), kind: "client".into(), thread: String::new(), note: s(a, "note").into(), since: now, lease: now + CLIENT_LEASE }, now).await; self.commit_events(vec![change(name, &l, now)]).await }
                else if op != "status" { for p in l.holder.iter_mut().chain(l.waiters.iter_mut()).filter(|p| p.id == holder) { p.lease = now + CLIENT_LEASE } self.commit_events(vec![change(name, &l, now)]).await }
                let mut v = l.answer(holder); if lost && op != "acquire" { v["requeued"] = json!(true) } v } } };
        if a["wait"].as_bool().unwrap_or(false) { let end = now + 25; while r["held"] != true && now_s() < end { tokio::time::sleep(Duration::from_millis(200)).await; r = self.lock_get(name).await.answer(holder) } }
        r
    }
    /// The holder `id` lets go: the next in line is granted. Under `work_lock`. false for anyone but the holder.
    pub(super) async fn lock_free(self: &Arc<Self>, name: &str, id: &str) -> bool {
        let mut l = self.lock_get(name).await; if !l.release(id) { return false }
        self.guard_drop(name, id); self.commit_events(vec![change(name, &l, now_s())]).await; self.lock_grant(name).await; true
    }
    /// `id` leaves the queue. Under `work_lock`.
    pub(super) async fn lock_leave(self: &Arc<Self>, name: &str, id: &str) -> bool {
        let mut l = self.lock_get(name).await; if !l.cancel(id) { return false }
        self.commit_events(vec![change(name, &l, now_s())]).await; self.lock_words(name, &l).await; true
    }
    /// A queued job's row says where it stands (Locks.wait).
    async fn lock_words(&self, name: &str, l: &Lock) {
        for (i, p) in l.waiters.iter().enumerate().filter(|(_, p)| p.kind == "work") {
            let rows = self.rows_of(&p.thread).await; let word = wait_word(name, i + 1);
            if rows.iter().any(|r| bg::field(r, 0) == bg::clean(&p.id) && queued_row(&bg::doing(r)) && bg::doing(r) != word) { self.put_rows(&p.thread, &bg::set(&rows, &p.id, &word)).await }
        }
    }
    /// The holder is told its lock is its own, once: `ready` is written before anything else, so a retry finds nothing to do. One held back by a flock outside stays not ready for the tick to retry.
    /// A holder that is gone is skipped: the next in line comes up in the same pass. Under `work_lock`.
    pub(super) async fn lock_grant(self: &Arc<Self>, name: &str) {
        loop {
            let mut l = self.lock_get(name).await; self.lock_words(name, &l).await;
            let Some(p) = l.holder.clone() else { return }; if l.ready { return }
            if !self.party_live(&p, now_s()).await { l.promote(); self.commit_events(vec![change(name, &l, now_s())]).await; continue }
            let now = now_s();
            if !self.guard_take(name, &p.id) {
                let o = who_holds(&self.lock_file(name), now); let same = |a: &Option<Value>, b: &Option<Value>| a.as_ref().map(|v| &v["pid"]) == b.as_ref().map(|v| &v["pid"]);
                if !same(&o, &l.outside) { l.outside = o; self.commit_events(vec![change(name, &l, now)]).await }
                if p.kind == "work" { let rows = self.rows_of(&p.thread).await; let word = wait_word(name, 0); if rows.iter().any(|r| bg::field(r, 0) == bg::clean(&p.id) && bg::doing(r) != word) { self.put_rows(&p.thread, &bg::set(&rows, &p.id, &word)).await } }
                return
            }
            l.ready = true; l.outside = None;
            if p.kind != "client" { let at = self.lease_at(now).await; if let Some(h) = &mut l.holder { h.lease = at } }
            match p.kind.as_str() {
                "work" => self.work_granted(name, &p, &l, now).await,
                "thread" => { self.commit_events(vec![change(name, &l, now)]).await; self.lock_tell(&p.thread, name, &format!("Lock {name} is yours (waited {}). Release it with lock_release when done.", age(now, p.since))).await }
                _ => self.commit_events(vec![change(name, &l, now)]).await,
            }
            return
        }
    }
    /// A queued job's turn: its row and the grant in one commit, then the launch. Its timeout starts now: the queue does not eat it.
    async fn work_granted(self: &Arc<Self>, name: &str, p: &Party, l: &Lock, now: u64) {
        let rows = self.rows_of(&p.thread).await; let Some(r) = rows.iter().find(|r| bg::field(r, 0) == bg::clean(&p.id)).cloned() else { return };
        let dl = match work::deadline(&r) { 0 => 0, d => now + d.saturating_sub(p.since) };
        let nr: Vec<String> = rows.iter().map(|x| if bg::field(x, 0) == bg::clean(&p.id) { work::row(&p.id, "launching", &bg::work_title(x), "run", &work::target(x), "-", dl) } else { x.clone() }).collect();
        self.commit_events(vec![change(name, l, now), self.rows_event(&p.thread, &nr)]).await;
        let cwd = self.world().await.cwd(&p.thread); self.work_launch(&p.thread, &p.id, work::target(&r), cwd, dl, Some(name));
    }
    /// The hub's word to a thread about its lock, under the lock's own name; a stopped thread is told without being woken.
    pub(super) async fn lock_tell(self: &Arc<Self>, th: &str, name: &str, say: &str) {
        let (live, stopped) = { let w = self.world().await; (w.threads.get(th).is_some_and(|t| !t.archived && !t.deleted), w.threads.get(th).is_some_and(|t| t.turn == "interrupted")) };
        if !live { return }
        let msg = format!("t:lock:{}", super::hub_id("l", now_s(), self.seq_n())); let from = format!("lock:{name}");
        let post = json!({"$":"MessagePosted","thread":th,"msg":msg,"role":"system","text":say,"at":now_s()});
        if stopped { self.commit_events(vec![post]).await } else if self.inbound("system", th, &msg, say, (from, "system".into(), "lock".into()), vec![]).await.is_err() { self.commit_events(vec![post]).await }
    }
    /// Locks.live: a thread's party while its thread is live, a job's while its row is open, a client's while its lease runs.
    async fn party_live(&self, p: &Party, now: u64) -> bool {
        if p.kind == "client" { return p.lease > now }
        let w = self.world().await; if !w.threads.get(&p.thread).is_some_and(|t| !t.archived && !t.deleted) { return false }
        p.kind != "work" || bg::lines(w.setting(&format!("watch.{}", p.thread))).iter().any(|r| bg::field(r, 0) == bg::clean(&p.id) && !bg::ended(r))
    }
    /// Locks.lease.busy: a running turn, or work of its own at work.
    pub(super) async fn party_busy(&self, p: &Party) -> bool { let w = self.world().await; w.running(&p.thread) || bg::lines(w.setting(&format!("watch.{}", p.thread))).iter().any(|r| !bg::ended(r)) || bg::running(&bg::lines(w.setting(&format!("bg.{}", p.thread)))) > 0 }

    /// One pass over the locks: parties that are gone leave, leases are walked, a grant held back is tried again.
    pub async fn locks_tick(self: &Arc<Self>) {
        if crate::copy::on() { return }
        let _g = self.work_lock.lock().await; let now = now_s();
        for name in self.lock_names().await {
            let l = self.lock_get(&name).await; if l.idle() { continue }
            let mut n = l.clone(); let mut keep = vec![];
            for p in std::mem::take(&mut n.waiters) { if self.party_live(&p, now).await { keep.push(p) } } n.waiters = keep;
            let gone = match &n.holder { Some(h) if !self.party_live(h, now).await => Some(h.id.clone()), _ => None };
            if let Some(id) = &gone { n.release(id); self.guard_drop(&name, id); }
            if n != l { self.commit_events(vec![change(&name, &n, now)]).await }
            if n.holder.is_some() && !n.ready { self.lock_grant(&name).await; continue }
            let Some(h) = n.holder.clone() else { continue };
            if h.kind != "work" && !self.guards.lock().unwrap().contains_key(&name) && !self.guard_take(&name, &h.id) { let o = who_holds(&self.lock_file(&name), now); if n.outside.is_none() && o.is_some() { n.outside = o; self.commit_events(vec![change(&name, &n, now)]).await } } // a restart lost the flock of a thread's lock
            if super::design::doc_lock_idle(self, &name, &h, now).await { continue } // a document's lock goes with its holder's turn
            if h.kind != "client" { self.lock_lease(&name, &n, &h, now).await }
        }
    }
    /// Lease policy never stops a process or moves exclusion off busy work: a stale holder is warned once, and only an idle one is released, a grace after the warning.
    async fn lock_lease(self: &Arc<Self>, name: &str, l: &Lock, h: &Party, now: u64) {
        if h.lease == 0 { let mut n = l.clone(); let at = self.lease_at(now).await; if let Some(x) = &mut n.holder { x.lease = at } self.commit_events(vec![change(name, &n, now)]).await; return }
        if !lease_expired(h.lease, now) { return }
        let key = format!("lock.warning.{name}"); let warned: Value = { let w = self.world().await; serde_json::from_str(w.setting(&key)).unwrap_or(Value::Null) };
        if !(warned["owner"] == json!(h.id) && warned["lease"] == json!(h.lease)) {
            self.commit_events(vec![json!({"$":"SettingSet","key":key,"value":json!({"owner":h.id,"lease":h.lease,"at":now}).to_string(),"at":now})]).await;
            self.lock_tell(&h.thread, name, &format!("Lock {name} has no recent progress heartbeat. Report progress or release it. Busy work keeps its lock; an idle reservation is released after a 60-second warning grace period.")).await;
            return
        }
        if now < warned["at"].as_u64().unwrap_or(now) + GRACE || self.party_busy(h).await { return }
        let waiters = l.waiters.clone();
        if !self.lock_free(name, &h.id).await { return }
        self.lock_tell(&h.thread, name, &format!("Lock {name} lease expired after the warning grace period while you had no running turn or owned work; the reservation was released.")).await;
        for p in waiters.iter().filter(|p| p.kind != "client" && !p.thread.is_empty()) { self.lock_tell(&p.thread, name, &format!("Lock {name} had a stale idle holder; its lease was released. Your FIFO position is preserved.")).await }
    }
    /// After a restart every client gets a fresh lease: the downtime must not cost a live helper its place.
    async fn locks_extend(&self) {
        let _g = self.work_lock.lock().await; let now = now_s();
        for name in self.lock_names().await {
            let mut l = self.lock_get(&name).await; let mut moved = false;
            for p in l.holder.iter_mut().chain(l.waiters.iter_mut()).filter(|p| p.kind == "client") { p.lease = now + CLIENT_LEASE; moved = true }
            if moved { self.commit_events(vec![change(&name, &l, now)]).await }
        }
    }
    /// At start, and every TICK after. What the log says is the truth: a restart with a holder and waiters goes on from it.
    pub fn locks_start(self: &Arc<Self>) {
        if crate::copy::on() { return }
        let me = self.clone(); tokio::spawn(async move {
            me.locks_extend().await;
            loop {
                if !me.lock_tick_manual.load(std::sync::atomic::Ordering::SeqCst) { me.locks_tick().await }
                me.lock_ticks.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                tokio::time::sleep(TICK).await
            }
        });
    }
}

/// `/lock/<op>`: loopback HTTP for shell helpers, behind the hub's pairing credential like every other route.
pub struct Http(pub Arc<Agents>);
impl crate::api::HttpHandler for Http {
    fn routes(&self) -> &'static [&'static str] { &["/lock/acquire", "/lock/status", "/lock/renew", "/lock/release", "/lock/cancel"] }
    fn takes_body(&self) -> bool { true }
    fn call<'a>(&'a self, _hub: &'a Arc<crate::hub::Hub>, _req: &'a crate::auth::Req) -> crate::api::Fut<'a, crate::api::HttpReply> { Box::pin(async { crate::api::HttpReply::text("405 Method Not Allowed", "POST only") }) }
    fn call_body<'a>(&'a self, _hub: &'a Arc<crate::hub::Hub>, req: &'a crate::auth::Req, mut body: crate::api::Body<'a>) -> crate::api::Fut<'a, crate::api::HttpReply> {
        Box::pin(async move {
            use tokio::io::AsyncReadExt;
            if req.method != "POST" { return crate::api::HttpReply::text("405 Method Not Allowed", "POST only") }
            if body.len > 4096 { return crate::api::HttpReply::text("413 Payload Too Large", "too long") }
            let _ = body.proceed().await; let mut b = vec![]; if body.read_to_end(&mut b).await.is_err() { return crate::api::HttpReply::text("400 Bad Request", "no body") }
            let a: Value = serde_json::from_slice(&b).unwrap_or(Value::Null);
            let op = req.target.split('?').next().unwrap_or("").rsplit('/').next().unwrap_or("");
            crate::api::HttpReply::ok("application/json", self.0.lock_http(op, &a).await.to_string().into_bytes())
        })
    }
}

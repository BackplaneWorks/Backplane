//! The guard's files. Everything lives in `<home>/guard/` and is plain text or JSON, so a client on the machine, the hub and a person with `cat` all read the same thing:
//!   lock            one guard per home
//!   status.json     what the guard is doing now (written whole, renamed over)
//!   request         one request from a client (written whole, renamed over; taken by rename)
//!   incidents.jsonl what failed and what was done, newest last
//!   good            builds that started cleanly on this home, oldest first
//!   launch.json     how the guard was started, so `backplane hub restart` can start it again
//!   phase           written by the hub: where its start has got to
//!   listeners       written by the hub: the addresses it listens on, one a line (rescue answers on the same ones)
//!   peers.json      written by the hub: the ids of the owner's linked machines, for rescue
//! And beside the builds (`$DEV`): `BAD` (id, tab, why) and `.fresh-bad` (the id alone, for a watcher that reads only that), `.pins/<port>` (builds nothing may prune).
use serde_json::{json, Value};
use std::{fs, io::Write, path::{Path, PathBuf}};

pub fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) }

#[derive(Clone)]
pub struct Dir(pub PathBuf);
impl Dir {
    pub fn of(home: &Path) -> Dir { Dir(home.join("guard")) }
    pub fn path(&self, name: &str) -> PathBuf { self.0.join(name) }
    pub fn make(&self) { let _ = fs::create_dir_all(&self.0); }
    /// Written beside and renamed over, so a reader never sees half. A full disk loses the write, never the file that was there.
    pub fn put(&self, name: &str, text: &str) -> std::io::Result<()> {
        let tmp = self.path(&format!(".{name}.{}", std::process::id()));
        let r = fs::write(&tmp, text).and_then(|_| fs::rename(&tmp, self.path(name)));
        if r.is_err() { let _ = fs::remove_file(&tmp); }
        r
    }
    pub fn text(&self, name: &str) -> String { fs::read_to_string(self.path(name)).unwrap_or_default() }
    pub fn json(&self, name: &str) -> Value { serde_json::from_str(&self.text(name)).unwrap_or(Value::Null) }
    pub fn lines(&self, name: &str) -> Vec<String> { self.text(name).lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect() }

    pub fn incident(&self, what: &str, why: &str, did: &str, build: &str) -> Value {
        let v = json!({"at": now(), "what": what, "why": why, "did": did, "build": build});
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(self.path("incidents.jsonl")) { let _ = writeln!(f, "{v}"); }
        v
    }
    /// An alert stays open until a record that `clears` it: the app and the phones stop showing it then (native/host/src/guard.rs `notice`).
    pub fn clear(&self, what: &str, why: &str, cleared: &str, build: &str) -> Value {
        let v = json!({"at": now(), "what": what, "why": why, "did": "", "clears": cleared, "build": build});
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(self.path("incidents.jsonl")) { let _ = writeln!(f, "{v}"); }
        v
    }
    /// The newest record about `alert` is the alert itself, not a clear of it.
    pub fn open_alert(&self, alert: &str) -> bool {
        self.lines("incidents.jsonl").iter().rev().filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .find(|v| v["what"] == alert || v["clears"] == alert).map_or(false, |v| v["what"] == alert)
    }
    pub fn incidents(&self, n: usize) -> Vec<Value> { let l = self.lines("incidents.jsonl"); l[l.len().saturating_sub(n)..].iter().filter_map(|x| serde_json::from_str(x).ok()).collect() }

    pub fn good(&self) -> Vec<String> { self.lines("good") }
    /// `build` goes last; the newest 16 are kept.
    pub fn add_good(&self, build: &str) {
        let mut g: Vec<String> = self.good().into_iter().filter(|b| b != build).collect(); g.push(build.to_string());
        let from = g.len().saturating_sub(16); let _ = self.put("good", &(g[from..].join("\n") + "\n"));
    }
    /// A request taken once: it is renamed before it is read, so two readers cannot both act on it.
    pub fn take_request(&self) -> Option<Value> {
        let (r, t) = (self.path("request"), self.path(".request.taken"));
        fs::rename(&r, &t).ok()?;
        let v = fs::read_to_string(&t).ok().and_then(|s| serde_json::from_str(&s).ok()); let _ = fs::remove_file(&t); v
    }
    /// One request at a time: one already waiting is never replaced (the asker hears "busy"), unless it is so old that no guard is taking requests.
    pub fn ask(&self, action: &str, build: &str, from: &str, id: &str) -> std::io::Result<()> {
        self.make();
        let (r, t) = (self.path("request"), self.path(&format!(".request.{id}")));
        fs::write(&t, json!({"id": id, "action": action, "build": build, "from": from, "at": now()}).to_string())?;
        let out = match fs::hard_link(&t, &r) {
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let stale = fs::metadata(&r).and_then(|m| m.modified()).ok().and_then(|m| m.elapsed().ok()).map_or(true, |a| a.as_secs() >= 30);
                if stale { fs::rename(&t, &r) } else { Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "another request is waiting for the guard: try again in a moment")) }
            }
            o => o,
        };
        let _ = fs::remove_file(&t); out
    }
}

/// The builds folder (`$DEV`, with builds/<id>/ and CURRENT): None for a hub run from one folder of its own.
#[derive(Clone)]
pub struct Builds(pub PathBuf);
impl Builds {
    pub fn dir(&self, id: &str) -> PathBuf { self.0.join("builds").join(id) }
    pub fn current(&self) -> String { fs::read_to_string(self.0.join("CURRENT")).unwrap_or_default().lines().next().unwrap_or("").trim().to_string() }
    pub fn bad(&self) -> Vec<String> { fs::read_to_string(self.0.join("BAD")).unwrap_or_default().lines().filter_map(|l| l.split('\t').next()).filter(|l| !l.is_empty()).map(str::to_string).collect() }
    /// One line more in BAD; `.fresh-bad` holds the newest alone, which is all an older watcher reads.
    pub fn mark_bad(&self, id: &str, why: &str) {
        if self.bad().iter().any(|b| b == id) { return }
        let why: String = why.chars().filter(|c| !c.is_control()).take(200).collect();
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(self.0.join("BAD")) { let _ = writeln!(f, "{id}\t{why}"); }
        let _ = fs::write(self.0.join(".fresh-bad"), format!("{id}\n"));
    }
    /// The one writer of CURRENT in a guarded install: a whole file, renamed over.
    pub fn publish(&self, id: &str) { let t = self.0.join(format!(".CURRENT.{}", std::process::id())); if fs::write(&t, format!("{id}\n")).and_then(|_| fs::rename(&t, self.0.join("CURRENT"))).is_err() { let _ = fs::remove_file(&t); } }
    /// Take the mark off: the person says this build is fine.
    pub fn clear_bad(&self, id: &str) {
        let kept: Vec<String> = fs::read_to_string(self.0.join("BAD")).unwrap_or_default().lines().filter(|l| l.split('\t').next() != Some(id)).map(str::to_string).collect();
        let _ = fs::write(self.0.join("BAD"), if kept.is_empty() { String::new() } else { kept.join("\n") + "\n" });
        if fs::read_to_string(self.0.join(".fresh-bad")).unwrap_or_default().trim() == id { let _ = fs::remove_file(self.0.join(".fresh-bad")); }
    }
    /// Builds a running guard needs: dev-cut.sh prunes none of them, whatever the count.
    pub fn pin(&self, port: u16, ids: &[String]) { let d = self.0.join(".pins"); let _ = fs::create_dir_all(&d); let _ = fs::write(d.join(port.to_string()), ids.join("\n") + "\n"); }
    pub fn unpin(&self, port: u16) { let _ = fs::remove_file(self.0.join(".pins").join(port.to_string())); }
}

/// `~/.config/backplane/guard-home`: the home `backplane hub ...` means when none is named. Windows has USERPROFILE.
pub fn pointer() -> Option<PathBuf> { std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(|h| PathBuf::from(h).join(".config").join("backplane").join("guard-home")) }

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("bp-guard-{n}-{}", std::process::id())); let _ = fs::remove_dir_all(&d); fs::create_dir_all(&d).unwrap(); d }
    #[test] fn a_request_is_taken_once() {
        let d = Dir::of(&tmp("req")); d.ask("restart", "", "cli", "r1").unwrap();
        assert_eq!(d.take_request().unwrap()["action"], "restart"); assert!(d.take_request().is_none());
    }
    #[test] fn a_torn_request_is_dropped_not_fatal() { let d = Dir::of(&tmp("torn")); d.make(); fs::write(d.path("request"), "{\"act").unwrap(); assert!(d.take_request().is_none()); assert!(d.take_request().is_none()); }
    #[test] fn good_builds_keep_their_order_and_their_limit() {
        let d = Dir::of(&tmp("good")); d.make(); for i in 0..20 { d.add_good(&format!("b{i}")); } d.add_good("b5");
        let g = d.good(); assert_eq!(g.len(), 16); assert_eq!(g.last().unwrap(), "b5");
    }
    #[test] fn a_bad_mark_reaches_both_readers_and_comes_off() {
        let b = Builds(tmp("bad")); b.mark_bad("x", "it\npanicked"); b.mark_bad("x", "again"); b.mark_bad("y", "too");
        assert_eq!(b.bad(), vec!["x", "y"]); assert_eq!(fs::read_to_string(b.0.join(".fresh-bad")).unwrap().trim(), "y");
        assert_eq!(fs::read_to_string(b.0.join("BAD")).unwrap().lines().count(), 2, "one line each, the why on it");
        b.clear_bad("y"); assert_eq!(b.bad(), vec!["x"]); assert!(!b.0.join(".fresh-bad").exists());
    }
    #[test] fn an_alert_is_open_until_a_clear_of_it() {
        let d = Dir::of(&tmp("clr")); d.make();
        assert!(!d.open_alert("Disk nearly full"));
        d.incident("Disk nearly full", "7 GB", "free space", "b"); d.incident("Hub crashed", "x", "y", "b");
        assert!(d.open_alert("Disk nearly full"), "another incident after it does not close it");
        d.clear("Disk has room again", "200 GB", "Disk nearly full", "b");
        assert!(!d.open_alert("Disk nearly full")); assert!(d.open_alert("Hub crashed"));
        d.incident("Disk nearly full", "6 GB", "free space", "b"); assert!(d.open_alert("Disk nearly full"), "full again");
    }
    #[test] fn incidents_read_back_newest_last() { let d = Dir::of(&tmp("inc")); d.make(); d.incident("a", "w", "d", "b"); d.incident("c", "w", "d", "b"); assert_eq!(d.incidents(1)[0]["what"], "c"); assert_eq!(d.incidents(9).len(), 2); }
}

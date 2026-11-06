//! The hub's updater: `update.check`, `update.status`, `app.update`, the hourly check, and what every client is told (info `update.status`, one JSON line; the clients draw it and decide nothing).
//!
//! Only a hub the guard runs from a builds folder updates itself (`BACKPLANE_BUILDS`); a package install or a hub run from a plain folder says `update: "package"` and does nothing here.
//! An update never cuts a turn: "install" is a wish that is carried out once no turn runs, whoever asked and from wherever.
use super::{rel, run};
use crate::{api::{Caller, Fut, RpcHandler, Ticker}, hub::{now_ms, Hub}, proc::{self, Kind}};
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}, sync::{atomic::{AtomicBool, Ordering::SeqCst}, Arc, Mutex}};

const EVERY: u64 = 3600;

pub fn builds() -> Option<PathBuf> { std::env::var_os("BACKPLANE_BUILDS").filter(|v| !v.is_empty()).map(PathBuf::from) }
/// `BACKPLANE_NO_UPDATE=1`, or a copy of a real log, which acts on nothing outside its home.
fn off() -> bool { crate::copy::on() || std::env::var("BACKPLANE_NO_UPDATE").map_or(false, |v| v == "1") }
/// The feed. A flag or the environment may change it, never a setting: any paired client can write a setting, and this names what a hub runs next. The signature still has to match.
fn feed() -> String { std::env::var("BACKPLANE_UPDATE_FEED").ok().filter(|v| !v.is_empty()).unwrap_or_else(|| run::FEED.to_string()) }

/// What every client shows. `state`: package, off, idle, checking, available, downloading, waiting, installing, failed.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Status { pub state: String, pub current: String, pub available: String, pub channel: String, pub note: String, pub checked: u64 }

/// Whether a turn is running anywhere. Queued work counts through its running turn; between turns an update may go.
pub fn busy<'a>(mut turns: impl Iterator<Item = &'a str>) -> bool { turns.any(|t| t == "running") }
/// The hourly check is due: never checked, or an hour on. A clock that went back does not stop it for good.
pub fn due(now: u64, last: u64) -> bool { last == 0 || now < last || now - last >= EVERY }

fn curl(url: &str, out: &Path, max: u64) -> Result<(), String> {
    let mut c = proc::std_cmd(Kind::Net, "curl");
    c.args(["-fsSL", "--max-time", "1800", "--max-filesize", &max.to_string(), "-A", "backplane-hub", "-o"]).arg(out);
    if url.starts_with("https://") { c.args(["--proto", "=https", "--proto-redir", "=https"]); } // the real feed never falls to plain http on a redirect
    let o = c.arg(url).output().map_err(|e| format!("curl: {e}"))?;
    if o.status.success() { Ok(()) } else { Err(format!("could not reach the release feed (curl {})", o.status.code().unwrap_or(-1))) }
}
/// Listed and checked (`run::members_ok`) before anything is written, then unpacked owning nothing but what this hub owns.
fn tar(a: &Path, d: &Path) -> Result<(), String> {
    let list = |flag: &str| -> Result<String, String> {
        let o = proc::std_cmd(Kind::Local, "tar").arg(flag).arg(a).env("LC_ALL", "C").output().map_err(|e| format!("tar: {e}"))?;
        if o.status.success() { Ok(String::from_utf8_lossy(&o.stdout).into_owned()) } else { Err("the archive would not unpack".into()) }
    };
    run::members_ok(&list("-tzf")?, &list("-tvzf")?)?;
    let o = proc::std_cmd(Kind::Local, "tar").args(["--no-same-owner", "--no-same-permissions", "-xzf"]).arg(a).arg("-C").arg(d).output().map_err(|e| format!("tar: {e}"))?;
    if o.status.success() { Ok(()) } else { Err("the archive would not unpack".into()) }
}
/// `BAD`, the guard's list: id, tab, why.
fn bad_lines(builds: &Path) -> Vec<(String, String)> {
    fs::read_to_string(builds.join("BAD")).unwrap_or_default().lines().filter_map(|l| { let (i, w) = l.split_once('\t').unwrap_or((l, "")); (!i.is_empty()).then(|| (i.to_string(), w.to_string())) }).collect()
}
fn lines_of(p: &Path) -> Vec<String> { fs::read_to_string(p).unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect() }
fn prune(home: &Path, b: &Path) {
    let all: Vec<String> = fs::read_dir(b.join("builds")).map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    let pins: Vec<String> = fs::read_dir(b.join(".pins")).map(|d| d.filter_map(|e| e.ok()).flat_map(|e| lines_of(&e.path())).collect()).unwrap_or_default();
    let cur = lines_of(&b.join("CURRENT")).into_iter().next().unwrap_or_default();
    if cur.is_empty() { return } // nothing says what runs: delete nothing
    for x in run::prunable(&all, &cur, &pins, &lines_of(&home.join("guard/good"))) { if rel::tag_ok(&x) { let _ = fs::remove_dir_all(b.join("builds").join(x)); } }
}

#[derive(Default)]
pub struct Updates { st: Mutex<Status>, working: AtomicBool, wanted: AtomicBool, last: Mutex<u64> }
impl Updates {
    fn pending_path(hub: &Hub) -> PathBuf { hub.cfg.home.join("update").join("pending") }
    async fn tell(&self, hub: &Arc<Hub>, f: impl FnOnce(&mut Status)) {
        let line = { let mut s = self.st.lock().unwrap(); f(&mut s); serde_json::to_string(&*s).unwrap_or_default() };
        hub.set_info("update.status", &line).await;
    }
    async fn channel(hub: &Hub) -> String { rel::channel(&hub.setting("update.channel").await.unwrap_or_default()).to_string() }
    async fn turns_run(hub: &Arc<Hub>) -> bool { let g = hub.model().await; busy(g.threads.values().map(|t| t.turn.as_str())) }

    /// At start: what this install is, and how an update asked for before the restart ended.
    pub async fn start(self: &Arc<Self>, hub: &Arc<Hub>) {
        let (current, channel) = (hub.cfg.version.clone(), Self::channel(hub).await);
        let Some(b) = builds() else { return self.tell(hub, |s| { s.state = "package".into(); s.current = current; s.channel = channel; }).await };
        hub.set_info("update", "managed").await;
        let mut state = if off() { "off" } else { "idle" }.to_string(); let (mut note, mut available) = (String::new(), String::new());
        let p = Self::pending_path(hub);
        if let Some(tag) = lines_of(&p).into_iter().next() {
            match run::outcome(&tag, &current, &bad_lines(&b)) {
                run::Outcome::Installed => { note = format!("Updated to {tag}."); let _ = fs::remove_file(&p); prune(&hub.cfg.home, &b); }
                run::Outcome::RolledBack(why) => { state = "failed".into(); note = if why.is_empty() { format!("{tag} failed its health check, so this build stayed.") } else { format!("{tag} failed its health check, so this build stayed: {why}") }; let _ = fs::remove_file(&p); }
                // not yet: the guard has not moved to it, and has not given up on it. Neither done nor failed: the pending line stays, and the next start asks again.
                run::Outcome::Waiting => { state = "installing".into(); available = tag.clone(); note = format!("{tag} has not started yet."); }
            }
        }
        self.tell(hub, |s| { s.state = state; s.current = current; s.channel = channel; s.note = note; s.available = available; }).await;
    }

    /// One pass: look, and when `install` is wanted and no turn runs, fetch and hand over. A pass already going is left to finish.
    pub async fn pass(self: &Arc<Self>, hub: &Arc<Hub>, now: u64) {
        let Some(b) = builds() else { return };
        if off() || self.working.swap(true, SeqCst) { return }
        let channel = Self::channel(hub).await; let current = hub.cfg.version.clone(); *self.last.lock().unwrap() = now;
        self.tell(hub, |s| { s.state = "checking".into(); s.channel = channel.clone(); s.note.clear(); }).await;
        let scratch = hub.cfg.home.join("update"); let (c2, ch2, b2, s2) = (current.clone(), channel.clone(), b.clone(), scratch.clone());
        let found = tokio::task::spawn_blocking(move || -> Result<Option<run::Found>, String> {
            fs::create_dir_all(&s2).map_err(|e| e.to_string())?;
            let target = rel::target().ok_or("no build is made for this machine")?;
            let list = s2.join("releases.json"); curl(&feed(), &list, 8 << 20)?;
            let l = run::listing(&fs::read(&list).map_err(|e| e.to_string())?)?;
            let bad: Vec<String> = bad_lines(&b2).into_iter().map(|x| x.0).collect();
            run::find(&run::Ask { current: &c2, channel: &ch2, target, keys: &rel::keys(), bad: &bad }, &l, &s2, &curl)
        }).await.unwrap_or_else(|e| Err(e.to_string()));
        let at = now;
        let found = match found {
            Err(e) => { self.wanted.store(false, SeqCst); self.tell(hub, |s| { s.state = "failed".into(); s.note = e; s.checked = at; }).await; self.working.store(false, SeqCst); return } // the person presses again, or the next hour comes: no retry every minute
            Ok(None) => { self.wanted.store(false, SeqCst); self.tell(hub, |s| { s.state = "idle".into(); s.available.clear(); s.checked = at; }).await; self.working.store(false, SeqCst); return }
            Ok(Some(f)) => f,
        };
        let tag = found.tag.clone();
        // automatic like 0.14: nothing has to be pressed, but a running turn always wins
        if Self::turns_run(hub).await { let t = tag.clone(); self.tell(hub, |s| { s.state = "waiting".into(); s.available = t; s.note = "Waiting for running turns to finish.".into(); s.checked = at; }).await; self.wanted.store(true, SeqCst); self.working.store(false, SeqCst); return }
        let t = tag.clone(); self.tell(hub, |s| { s.state = "downloading".into(); s.available = t; s.checked = at; }).await;
        let (f2, b3) = (found.clone(), b.clone());
        let staged = tokio::task::spawn_blocking(move || run::stage(&f2, &b3, &curl, &tar)).await.unwrap_or_else(|e| Err(e.to_string()));
        let r = staged.and_then(|_| { fs::write(Self::pending_path(hub), format!("{tag}\n")).map_err(|e| e.to_string())?; run::request(&hub.cfg.home, &format!("update-{now}"), &tag, now) });
        match r {
            Ok(()) => self.tell(hub, |s| { s.state = "installing".into(); }).await,
            Err(e) => { let _ = fs::remove_file(Self::pending_path(hub)); self.tell(hub, |s| { s.state = "failed".into(); s.note = e; }).await }
        }
        self.wanted.store(false, SeqCst); self.working.store(false, SeqCst);
    }
    fn now_s() -> u64 { now_ms() / 1000 }
}

/// Registers the requests and the hourly check, then works out how the last update ended.
pub fn install(hub: &Arc<Hub>) {
    let u = Arc::new(Updates::default());
    hub.register_rpc(Arc::new(Rpc(u.clone()))); hub.register_ticker(Arc::new(Hourly(u.clone())));
    let h = hub.clone(); tokio::spawn(async move { u.start(&h).await; });
}

pub struct Rpc(pub Arc<Updates>);
impl RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["update.check", "update.status", "app.update"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, _p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let channel = Updates::channel(hub).await; // as stored now: a change in Settings counts at once
            let status = |u: &Updates| { let s = u.st.lock().unwrap().clone(); let (line, check) = crate::present::settings::update_line(&serde_json::to_string(&s).unwrap_or_default());
                json!({"version":s.available,"channel":channel,"current":s.current,"state":s.state,"note":s.note,"checked":s.checked,"line":line,"canCheck":check}) };
            if method != "update.status" && builds().is_some() && !off() {
                if method == "app.update" { self.0.wanted.store(true, SeqCst); }
                let (u, h, now) = (self.0.clone(), hub.clone(), Updates::now_s()); tokio::spawn(async move { u.pass(&h, now).await; });
            }
            if method == "app.update" && builds().is_none() { hub.push_all(&json!({"t":"update","error":"this install updates through its package"})); }
            vec![json!({"t":"reply","id":id,"ok":true,"result":status(&self.0)})]
        })
    }
}

pub struct Hourly(pub Arc<Updates>);
impl Ticker for Hourly {
    fn every_s(&self) -> u64 { 60 }
    fn tick<'a>(&'a self, hub: &'a Arc<Hub>, now_s: u64) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            // a wish waits for the turns to end here, not by asking the feed each minute: GitHub allows 60 unsigned calls an hour
            let last = *self.0.last.lock().unwrap();
            let go = due(now_s, last) || (self.0.wanted.load(SeqCst) && !Updates::turns_run(hub).await);
            if go { let (u, h) = (self.0.clone(), hub.clone()); tokio::spawn(async move { u.pass(&h, now_s).await; }); }
            vec![]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test] async fn a_package_install_answers_and_changes_nothing() {
        use crate::{hub::Cfg, presenter::Canned};
        let home = std::env::temp_dir().join(format!("bp-upd-pkg-{}", std::process::id())); let _ = fs::remove_dir_all(&home); fs::create_dir_all(&home).unwrap();
        let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let u = Arc::new(Updates::default()); u.start(&hub).await; let r = Rpc(u);
        let call = |m: &'static str| { let (hub, r) = (hub.clone(), &r); async move { r.call(&hub, &Caller { gen: "1".into(), client: 1 }, &json!(4), m, &json!({})).await.remove(0) } };
        let v = call("update.check").await; assert_eq!((v["result"]["version"].as_str(), v["result"]["channel"].as_str(), v["result"]["state"].as_str()), (Some(""), Some("stable"), Some("package")));
        assert_eq!((v["result"]["line"].as_str(), v["result"]["canCheck"].as_bool()), (Some("Updated by its package."), Some(false)), "the sentence every client shows");
        hub.commit(&json!({"$":"SettingSet","key":"update.channel","value":"nightly","at":1})).await.unwrap(); assert_eq!(call("update.check").await["result"]["channel"], "beta", "0.14's stored nightly");
        hub.commit(&json!({"$":"SettingSet","key":"update.channel","value":"beta","at":2})).await.unwrap(); assert_eq!(call("update.status").await["result"]["channel"], "beta");
        assert_eq!(call("app.update").await["ok"], true); assert!(!home.join("update").exists(), "a package install keeps no update files");
        let _ = fs::remove_dir_all(&home);
    }
    #[test] fn a_running_turn_anywhere_holds_an_update() {
        assert!(busy(["idle", "running", "completed"].into_iter())); assert!(!busy(["idle", "completed", "interrupted", "failed", ""].into_iter())); assert!(!busy([].into_iter()));
    }
    #[test] fn the_check_comes_hourly_and_survives_a_clock_going_back() {
        assert!(due(100, 0), "never checked"); assert!(!due(1000, 100)); assert!(due(100 + EVERY, 100)); assert!(due(50, 100), "the clock went back");
    }
    #[test] fn the_bad_list_keeps_its_reason() {
        let d = std::env::temp_dir().join(format!("bp-upd-svc-{}", std::process::id())); let _ = fs::remove_dir_all(&d); fs::create_dir_all(&d).unwrap();
        fs::write(d.join("BAD"), "v0.16.0\tit panicked\nv0.16.1\n").unwrap();
        assert_eq!(bad_lines(&d), vec![("v0.16.0".to_string(), "it panicked".to_string()), ("v0.16.1".to_string(), String::new())]);
        assert_eq!(bad_lines(&d.join("none")), vec![]); let _ = fs::remove_dir_all(&d);
    }
    /// The real `tar`, on archives made to escape: each is refused before a byte of it is written, and the good one unpacks.
    #[cfg(unix)] #[test] fn tar_lists_before_it_writes() {
        let d = std::env::temp_dir().join(format!("bp-upd-tar-{}", std::process::id())); let _ = fs::remove_dir_all(&d);
        let (src, out) = (d.join("src"), d.join("out")); fs::create_dir_all(src.join("in")).unwrap(); fs::create_dir_all(&out).unwrap();
        fs::write(src.join("in/backplane-hub"), b"x").unwrap(); fs::write(src.join("evil"), b"x").unwrap(); fs::hard_link(src.join("in/backplane-hub"), src.join("in/twin")).unwrap();
        let mk = |name: &str, args: &[&str]| { let a = d.join(name); assert!(std::process::Command::new("tar").arg("-czf").arg(&a).args(args).current_dir(&src).status().unwrap().success()); a };
        let good = { let a = d.join("good.tgz"); assert!(std::process::Command::new("tar").args(["-czf"]).arg(&a).args(["backplane-hub"]).current_dir(src.join("in")).status().unwrap().success()); a };
        let up = mk("up.tgz", &["-P", "in/../evil"]);
        let abs = { let a = d.join("abs.tgz"); assert!(std::process::Command::new("tar").arg("-czPf").arg(&a).arg(src.join("evil")).status().unwrap().success()); a };
        let hard = mk("hard.tgz", &["in/backplane-hub", "in/twin"]);
        for (a, why) in [(&up, "outside"), (&abs, "outside"), (&hard, "hard link")] { let e = tar(a, &out).unwrap_err(); assert!(e.contains(why), "{a:?}: {e}"); assert_eq!(fs::read_dir(&out).unwrap().count(), 0, "nothing written"); }
        tar(&good, &out).unwrap(); assert!(out.join("backplane-hub").is_file());
        let _ = fs::remove_dir_all(&d);
    }
}

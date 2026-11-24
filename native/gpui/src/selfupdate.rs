//! This device's own app, kept current where no guard does it: an installed build (`<prefix>/builds/<tag>/`) whose app uses a hub on another
//! machine (`launch.json`'s `url`, or a paired remote hub in `hubs.json`). The rules and files are appkeep.rs's (shared with the guard); this is when
//! they run (docs/deploy.md, "A desktop app on another machine's hub"):
//! - at launch, before any window: the staged build is tried, or a dead trial that never presented is marked bad (`at_launch`);
//! - once the first frame has drawn and presented: this build is proven here, and a trial becomes CURRENT (`opened`);
//! - then, and every six hours while the app runs, and on Check now: the build's own guard stages what is newer (`backplane builds keep`, detached).
//! Whatever updates it, Settings shows this app's version beside the hub's (`row`), and the sidebar says when the two differ (`skew`), with Restart to
//! update once a newer build waits here (`restart`). The channel is the hub's unless this device chose its own (`channel`): the hub's `update.status`
//! is asked at every connection and hourly (`hub_said`), and kept per hub in `app-update/hub.json` for checks run before connecting.
//! A local hub's guard owns the builds, so with one (or with `BACKPLANE_NO_UPDATE=1`, a package install, a build tree) none of this runs.
use crate::appkeep::{self, Start};
use serde_json::{json, Value};
use std::{
    fs::File,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

/// How often a running app looks for a newer build (the hub looks hourly; the release feed allows 60 unsigned calls an hour per address).
pub const EVERY: Duration = Duration::from_secs(6 * 3600);
/* i18n-ok: an environment variable */ pub const TRIAL_ENV: &str = "BACKPLANE_APP_TRIAL";
/* i18n-ok: an environment variable */ const AFTER: &str = "BACKPLANE_APP_AFTER";

#[derive(Clone, Debug, PartialEq)]
pub struct Here { pub prefix: PathBuf, pub me: String }
static INSTALLED: OnceLock<Option<Here>> = OnceLock::new();
static REMOTE_ACTIVE: AtomicBool = AtomicBool::new(false);
static PRIMARY_HUB_KEY: Mutex<String> = Mutex::new(String::new());
static FAILOVER_HUB_KEY: Mutex<Option<String>> = Mutex::new(None);
static TRIAL_NONCE: OnceLock<String> = OnceLock::new();
/// The shared hold on `clients.lock` for this process's life: a launch moves builds only when no app holds it.
static HELD: OnceLock<File> = OnceLock::new();
/// The hub's last `update.status` answer ({current, channel, state, canCheck, ...}); Null before the first.
static HUB: Mutex<Value> = Mutex::new(Value::Null);
/// Cache for `restart_to()` so sidebar render doesn't stat the filesystem every frame.
static RESTART_CACHE: Mutex<Option<(Instant, Option<String>)>> = Mutex::new(None);
/// Serializes `keep` runs from this window so an early channel change or Check now queues a follow-up instead of hitting `app-update/lock`.
static CHECK_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
static CHECK_AGAIN: AtomicBool = AtomicBool::new(false);

fn active_hub_key() -> (String, bool) {
    if let Ok(Some(k)) = FAILOVER_HUB_KEY.lock().map(|g| g.clone()) {
        return (k, true);
    }
    (PRIMARY_HUB_KEY.lock().map(|k| k.clone()).unwrap_or_default(), false)
}
fn set_primary_hub_key(k: &str) {
    if let Ok(mut g) = PRIMARY_HUB_KEY.lock() { *g = k.to_string(); }
    if let Ok(mut g) = FAILOVER_HUB_KEY.lock() { *g = None; }
}

/// The guarded install this binary runs from, when eligible (not an AppImage, test, or special mode), regardless of active hub.
pub fn installed() -> Option<Here> { INSTALLED.get().and_then(|o| o.clone()) }
/// Where this app looks after its own build while connected to a remote hub; None when on a local hub or non-install.
pub fn here() -> Option<Here> { if REMOTE_ACTIVE.load(Ordering::SeqCst) { installed() } else { None } }
fn off() -> bool { std::env::var("BACKPLANE_NO_UPDATE").map_or(false, |v| v == "1") }

/// Whether this launch uses a hub on another machine: either through `launch.json` (no local hub, saved remote `url`) or through a paired
/// non-loopback hub in `hubs.json` (`Start::Paired`). Special test/swatch/script modes and explicit `BACKPLANE_HOME` never self-update.
pub fn remote(pending: bool, paired: bool, hub_url: &str, env_home: bool, local_hub: bool, saved_url: &str, special_mode: bool) -> bool {
    if special_mode || env_home { return false }
    let via_launch = pending && !local_hub && !saved_url.is_empty();
    let via_hubs = !pending && paired && !hub_url.is_empty() && !crate::hubs::is_loopback(hub_url);
    via_launch || via_hubs
}
/// The build this program is, when it runs from a guarded install's builds folder: (prefix, tag).
pub fn find(me: &Path) -> Option<Here> {
    let (_, prefix) = crate::launch::guarded_install(me)?;
    let me = me.parent()?.file_name()?.to_string_lossy().into_owned();
    appkeep::name_ok(&me).then_some(Here { prefix, me })
}

fn lock_file(prefix: &Path) -> Option<File> { std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(prefix.join("clients.lock")).ok() }
/// Held shared from here on, by this process. A launch deciding meanwhile is a moment; this waits it out.
fn hold(f: Option<File>, prefix: &Path) {
    let Some(f) = f.or_else(|| lock_file(prefix)) else { return };
    let _ = f.unlock();
    if f.lock_shared().is_ok() { let _ = HELD.set(f); }
}

#[cfg(target_os = "macos")]
fn apply_mac_bundle_env(c: &mut Command) {
    if let Some(app_bundle) = std::env::var_os("BACKPLANE_APP") { // i18n-ok: environment variable name
        let launcher = Path::new(&app_bundle).join("Contents/MacOS/Backplane"); // i18n-ok: macOS bundle executable path
        if launcher.is_file() { c.env("CFProcessPath", launcher); } // i18n-ok: environment variable name
    }
}
#[cfg(not(target_os = "macos"))]
fn apply_mac_bundle_env(_c: &mut Command) {}

#[allow(dead_code)]
enum TrialLaunch { Exit, Failed(std::io::Error, Option<File>) }
fn launch_trial(app: &Path, prefix: &Path, tag: &str, nonce: &str, f: Option<File>) -> TrialLaunch {
    let mut c = Command::new(app);
    c.args(std::env::args_os().skip(1))
        .env(TRIAL_ENV, nonce)
        .env_remove("BACKPLANE_GPUI_SCRIPT"); // i18n-ok: environment variable name
    apply_mac_bundle_env(&mut c);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let _ = (prefix, tag);
        // Keep `f` (opened with O_CLOEXEC by std) locked exclusively until `execve` atomically replaces this process, so no competitor steals
        // `clients.lock` ahead of the new trial process bearing the same PID.
        let e = c.exec();
        TrialLaunch::Failed(e, f)
    }
    #[cfg(not(unix))]
    {
        c.stdin(Stdio::null());
        crate::launch::detached(&mut c);
        match c.spawn() {
            Ok(child) => {
                appkeep::update_trial_pid(prefix, tag, nonce, child.id());
                drop(f);
                TrialLaunch::Exit
            }
            Err(e) => TrialLaunch::Failed(e, f),
        }
    }
}

/// Before any window: decide this launch's build. A dead trial that never opened its window is marked bad here; a staged newer build is tried
/// (this process becomes its app). Also records guarded install info so switching to a remote hub later in-process can enable self-updating.
pub fn at_launch(eligible_install: bool, is_remote: bool, hub_key: &str) {
    let trial_nonce = std::env::var(TRIAL_ENV).ok().filter(|v| !v.is_empty());
    if trial_nonce.is_some() {
        // SAFETY: before any other thread of this process starts (at the start of main).
        unsafe { std::env::remove_var(TRIAL_ENV) }
    }
    if let Some(ref n) = trial_nonce { let _ = TRIAL_NONCE.set(n.clone()); }

    let inst = if eligible_install && std::env::var_os("APPIMAGE").is_none() && std::env::var_os("BACKPLANE_HUB_BIN").is_none() {
        std::env::current_exe().ok().and_then(|me| find(&me))
    } else {
        None
    };
    let _ = INSTALLED.set(inst.clone());
    REMOTE_ACTIVE.store(is_remote, Ordering::SeqCst);
    set_primary_hub_key(hub_key);

    let Some(h) = inst else { return };
    let p = &h.prefix;
    let f = lock_file(p);
    // An explicitly launched trial process (from Start::Try or Windows maybe_install) takes no competing trial decision; it holds clients.lock shared
    // once the parent releases it.
    if trial_nonce.is_some() { return hold(f, p); }
    if !is_remote { return hold(f, p); }

    let mut sole = f.as_ref().map_or(false, |f| f.try_lock().is_ok());
    // Started by Restart to update: wait until the asking process has exited and released clients.lock (or briefly if another window holds it).
    if let Some(after) = std::env::var_os(AFTER) {
        // SAFETY: before any other thread of this process starts (main, ahead of the window).
        unsafe { std::env::remove_var(AFTER) }
        let pid: u32 = after.to_string_lossy().trim().parse().unwrap_or(0);
        let mut checks = 0u32;
        let mut dead_wait = 0u32;
        while !sole {
            if pid != 0 && !appkeep::pid_alive(pid) {
                dead_wait += 1;
                if dead_wait > 5 { break }
            } else if checks >= 150 {
                break;
            }
            checks += 1;
            std::thread::sleep(Duration::from_millis(100));
            sole = f.as_ref().map_or(false, |f| f.try_lock().is_ok());
        }
    }
    if off() { return hold(f, p) }
    let eff_ch = effective_channel(p);
    let nonce = format!("{}-{}", std::process::id(), appkeep::now());
    match appkeep::decide_and_begin(p, &h.me, eff_ch.as_str(), sole, std::process::id(), &nonce) {
        Start::Stay | Start::Forget => {}
        Start::Failed { tag, why } => { /* i18n-ok: a log line */ eprintln!("backplane-gpui: {tag} {why}: marked bad, {} stays", h.me); }
        Start::Try(tag) => {
            /* i18n-ok: a log line */ eprintln!("backplane-gpui: trying {tag} (from {})", h.me);
            match launch_trial(&appkeep::app_of(p, &tag), p, &tag, &nonce, f) {
                TrialLaunch::Exit => std::process::exit(0),
                TrialLaunch::Failed(e, retained_lock) => {
                    /* i18n-ok: the reason BAD keeps, never drawn */ appkeep::failed(p, &tag, &format!("its app would not start ({e})"));
                    return hold(retained_lock, p);
                }
            }
        }
    }
    hold(f, p)
}

/// Update active remote self-update eligibility and hub identity when switching hubs in-process.
/// Returns true if periodic remote app update checks should run now.
pub fn switch_hub(is_remote: bool, key: &str) -> bool {
    REMOTE_ACTIVE.store(is_remote, Ordering::SeqCst);
    set_primary_hub_key(key);
    clear_hub();
    invalidate_restart_cache();
    if is_remote && HELD.get().is_none() {
        if let Some(h) = installed() { hold(None, &h.prefix); }
    }
    here().is_some() && !off()
}
/// Update the active hub identity when automatic failover switches to or returns from the home hub, without clobbering the primary hub key.
pub fn set_using(on_home: Option<&str>) {
    if let Ok(mut g) = FAILOVER_HUB_KEY.lock() { *g = on_home.filter(|s| !s.is_empty()).map(str::to_string); }
    clear_hub();
    invalidate_restart_cache();
}

/// This build is a launch's trial whose window has not presented yet: CURRENT still names the build it came from until `opened`, which is no
/// reason for `main.rs::to_moved_build` to go back to that build.
pub fn on_trial() -> bool {
    installed().is_some_and(|h| {
        appkeep::trial(&h.prefix).is_some_and(|t| {
            t.tag == h.me && TRIAL_NONCE.get().map(String::as_str) == Some(t.nonce.as_str())
        })
    })
}

/// The first frame has drawn and presented: this build is proven here. A trial becomes CURRENT; answers true when this app is managing its own
/// remote builds and should start checking for newer releases.
pub fn opened() -> bool {
    let Some(h) = installed() else { return false };
    let has_trial = TRIAL_NONCE.get().is_some();
    let is_remote = REMOTE_ACTIVE.load(Ordering::SeqCst);
    if !is_remote && !has_trial { return false }
    if !off() && !appkeep::guard_runs(&h.prefix) {
        if let Some(t) = appkeep::opened(&h.prefix, &h.me, TRIAL_NONCE.get().map(String::as_str)) {
            /* i18n-ok: a log line */ eprintln!("backplane-gpui: {t} opened its window: it is CURRENT now");
            invalidate_restart_cache();
        }
        return is_remote;
    }
    false
}

/// Begin a serialized check if none is in flight; otherwise mark `CHECK_AGAIN` so a follow-up check starts right after the current one ends.
pub fn request_check() -> bool {
    if CHECK_IN_FLIGHT.swap(true, Ordering::SeqCst) {
        CHECK_AGAIN.store(true, Ordering::SeqCst);
        return false;
    }
    true
}
/// Mark the current check finished, invalidate the restart cache, and return whether another check was requested meanwhile.
pub fn finish_check() -> bool {
    invalidate_restart_cache();
    CHECK_IN_FLIGHT.store(false, Ordering::SeqCst);
    CHECK_AGAIN.swap(false, Ordering::SeqCst)
}
pub fn status_state() -> String {
    here().map(|h| appkeep::status(&h.prefix)["state"].as_str().unwrap_or("").to_string()).unwrap_or_default()
}
pub fn invalidate_restart_cache() {
    if let Ok(mut g) = RESTART_CACHE.lock() { *g = None; }
}
/// Recover from an unexpected keeper launch failure or crash that left `checking` or `downloading` behind with no live lock holder.
pub fn mark_helper_failed() {
    if let Some(h) = here() { appkeep::fail_stale_busy(&h.prefix); }
    invalidate_restart_cache();
}

/// `backplane builds keep` of this build's own guard, detached: it outlives the window, and a second one while it runs ends at once (its lock).
/// None where this app does not look after its build, or the build has no guard.
pub fn check() -> Option<Child> {
    let h = here()?; if off() { return None }
    let g = appkeep::guard_of(&h.prefix, &h.me); if !g.is_file() { return None }
    let d = appkeep::dir(&h.prefix); let _ = std::fs::create_dir_all(&d);
    let log = std::fs::OpenOptions::new().create(true).append(true).open(d.join("keep.log")).ok();
    let mut c = Command::new(g);
    c.args(["builds", "keep", "--builds"]).arg(&h.prefix).args(["--running", &h.me, "--channel", &effective_channel(&h.prefix)]);
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(log.map_or(Stdio::null(), Stdio::from));
    crate::launch::detached(&mut c);
    c.spawn().ok()
}

fn effective_channel(prefix: &Path) -> String {
    let own = crate::local::path().map(|p| crate::local::Local::read(&p)).unwrap_or_default().app_channel;
    channel(&own, &hub_channel(prefix)).to_string()
}

/// The channel this app updates on: its own when this device chose one, else the hub's, so the two move to the same releases; stable when neither says.
pub fn channel<'a>(own: &'a str, hub: &'a str) -> &'a str {
    if crate::local::APP_CHANNELS.contains(&own) { own } else if crate::local::APP_CHANNELS.contains(&hub) { hub } else { "stable" }
}
fn hub_file(prefix: &Path) -> PathBuf { appkeep::dir(prefix).join("hub.json") }
fn read_hub_json(prefix: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(hub_file(prefix)).unwrap_or_default()).unwrap_or_else(|_| json!({}))
}
fn hub_channel_for(v: &Value, key: &str) -> String {
    if !key.is_empty() {
        if let Some(ch) = v["hubs"][key]["channel"].as_str().filter(|s| !s.is_empty()) {
            return ch.to_string();
        }
        if v.get("hubs").is_none() {
            return v["channel"].as_str().unwrap_or("").to_string();
        }
        return String::new();
    }
    let last = v["last"].as_str().unwrap_or("");
    if !last.is_empty() {
        if let Some(ch) = v["hubs"][last]["channel"].as_str().filter(|s| !s.is_empty()) {
            return ch.to_string();
        }
    }
    v["channel"].as_str().unwrap_or("").to_string()
}
/// The hub's channel as last heard: this run's answer, else the one kept per hub from an earlier run.
fn hub_channel(prefix: &Path) -> String {
    let now = HUB.lock().map(|v| v["channel"].as_str().unwrap_or("").to_string()).unwrap_or_default();
    if !now.is_empty() { return now }
    let (key, _) = active_hub_key();
    hub_channel_for(&read_hub_json(prefix), &key)
}
/// Clear cached hub update info when switching hubs in-process so stale skew/channel from the old hub disappears until the new hub replies.
pub fn clear_hub() {
    if let Ok(mut g) = HUB.lock() { *g = Value::Null; }
}
fn record_hub_entry(root: &mut Value, key: &str, ch: &str, ver: &str, is_failover: bool) -> String {
    if !root.is_object() { *root = json!({}); }
    let prev = hub_channel_for(root, key);
    let entry_key = if key.is_empty() { "_default" } else { key };
    if !root["hubs"].is_object() { root["hubs"] = json!({}); }
    root["hubs"][entry_key] = json!({"channel": ch, "version": ver});
    if !is_failover { root["last"] = json!(entry_key); }
    prev
}
/// The hub's `update.status` came: kept per hub in `app-update/hub.json` (failover never overwrites `last`). Answers true if this hub's channel
/// changed while this device follows it.
pub fn hub_said(v: &Value) -> bool {
    if let Ok(mut g) = HUB.lock() { *g = v.clone(); }
    let Some(h) = here() else { return false };
    let (ch, ver) = (v["channel"].as_str().unwrap_or(""), v["current"].as_str().unwrap_or(""));
    if ch.is_empty() { return false }
    let (key, is_failover) = active_hub_key();
    let Ok(before) = appkeep::try_with_state_lock(&h.prefix, || {
        let mut root = read_hub_json(&h.prefix);
        let prev = record_hub_entry(&mut root, &key, ch, ver, is_failover);
        let _ = appkeep::put(&hub_file(&h.prefix), &root.to_string());
        prev
    }) else {
        return false;
    };
    let own = crate::local::path().map(|p| crate::local::Local::read(&p)).unwrap_or_default().app_channel;
    own.is_empty() && before != ch
}
/// The hub's version as it last said it ("" before it has).
pub fn hub_version() -> String { HUB.lock().map(|v| v["current"].as_str().unwrap_or("").to_string()).unwrap_or_default() }
pub fn hub_can_check() -> bool { HUB.lock().map(|v| v["canCheck"] == true).unwrap_or(false) }

/// The build a Restart to update would bring up, when there is one: where this app keeps its own build, the eligible staged one; where a guard here
/// runs the hub, CURRENT once the guard moved it past this app.
pub fn restart_to() -> Option<String> {
    if let Some(h) = here() {
        let n = appkeep::next(&h.prefix);
        let eligible = appkeep::is_manual_next(&h.prefix, &n) || appkeep::channel_takes(effective_channel(&h.prefix).as_str(), appkeep::tag_channel(&n));
        return (!off() && appkeep::name_ok(&n) && eligible && appkeep::newer(&n, &h.me) && appkeep::app_of(&h.prefix, &n).is_file() && !appkeep::bad(&h.prefix).contains(&n)).then_some(n);
    }
    let me = std::env::current_exe().ok()?; let (_, prefix) = crate::launch::guarded_install(&me)?;
    let mine = me.parent()?.file_name()?.to_string_lossy().into_owned(); let cur = appkeep::current(&prefix);
    (appkeep::name_ok(&cur) && appkeep::newer(&cur, &mine) && appkeep::app_of(&prefix, &cur).is_file()).then_some(cur)
}
/// Whether this window holds `clients.lock` alone (no other Backplane windows of this prefix open), so Restart can replace the build immediately.
pub fn can_restart_now() -> bool {
    let Some(f) = HELD.get() else { return true };
    let _ = f.unlock();
    let alone = f.try_lock().is_ok();
    let _ = f.lock_shared();
    alone
}
/// Restart to update: requires no other open windows of this prefix (`WouldBlock` otherwise). On Unix, replaces this process via `exec` (preserving
/// macOS `Backplane.app` bundle identity); on Windows, spawns CURRENT's app with `AFTER=<pid>` so the caller can exit immediately.
pub fn restart() -> std::io::Result<()> {
    let prefix = here()
        .map(|h| h.prefix)
        .or_else(|| std::env::current_exe().ok().and_then(|me| crate::launch::guarded_install(&me)).map(|(_, p)| p))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "not an installed build"))?; // i18n-ok: internal io error detail
    let app = appkeep::app_of(&prefix, &appkeep::current(&prefix));
    if !app.is_file() { return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "missing app binary")); } // i18n-ok: internal io error detail
    if let Some(f) = HELD.get() {
        let _ = f.unlock();
        if f.try_lock().is_err() {
            let _ = f.lock_shared();
            return Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, "close other windows first")); // i18n-ok: callers map WouldBlock to gpui.appupdate.closeothers
        }
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut c = Command::new(app);
    c.args(crate::hubs::relaunch_with(&args, &crate::hubs::load()))
        .env_remove("BACKPLANE_GPUI_SCRIPT") // i18n-ok: environment variable name
        .stdin(Stdio::null());
    apply_mac_bundle_env(&mut c);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let e = c.exec();
        if let Some(f) = HELD.get() { let _ = f.lock_shared(); }
        Err(e)
    }
    #[cfg(not(unix))]
    {
        c.env(AFTER, std::process::id().to_string());
        crate::launch::detached(&mut c);
        match c.spawn() {
            Ok(_) => Ok(()),
            Err(e) => {
                if let Some(f) = HELD.get() { let _ = f.lock_shared(); }
                Err(e)
            }
        }
    }
}

/// What the sidebar says of this app beside the hub it uses.
#[derive(Clone, Debug, PartialEq)]
pub enum Skew {
    /// nothing to say
    Same,
    /// a newer build waits here: Restart to update
    Restart(String),
    /// this app is older than the hub, and nothing newer waits here yet
    AppBehind { app: String, hub: String },
    /// the hub is older than this app
    HubBehind { app: String, hub: String },
}
pub fn skew(app: &str, hub: &str, waiting: Option<&str>) -> Skew {
    if let Some(t) = waiting { return Skew::Restart(t.to_string()) }
    let (a, h) = (app.trim_start_matches('v'), hub.trim_start_matches('v'));
    if a.is_empty() || h.is_empty() || a == h { return Skew::Same }
    if appkeep::newer(h, a) { Skew::AppBehind { app: a.into(), hub: h.into() } } else { Skew::HubBehind { app: a.into(), hub: h.into() } }
}

/// A time of day as Settings shows the last check: the local clock's hours and minutes, `checked` seconds since the epoch.
pub fn hhmm(checked: u64, now: u64, local: (i32, i32, i32)) -> String {
    if checked == 0 || checked > now { return String::new() }
    let sod = (local.0 * 3600 + local.1 * 60 + local.2) as i64 - (now - checked) as i64;
    let sod = sod.rem_euclid(86400);
    format!("{:02}:{:02}", sod / 3600, sod % 3600 / 60)
}

/// The build this app is: its folder in a guarded install, else the version it was built as. Worked out once.
pub fn version() -> String {
    static V: OnceLock<String> = OnceLock::new();
    V.get_or_init(|| {
        if let Some(h) = installed() { return h.me }
        crate::install::tag()
    }).clone()
}

/// `restart_to`, looked at no more than every five seconds: the sidebar asks at every frame it draws.
fn restart_soon() -> Option<String> {
    let mut g = RESTART_CACHE.lock().ok()?;
    if let Some((at, v)) = g.as_ref() { if at.elapsed() < Duration::from_secs(5) { return v.clone() } }
    let v = restart_to(); *g = Some((Instant::now(), v.clone())); v
}
/// The sidebar's line above the connection status when this app and its hub differ, or a newer build waits here, with the one thing to do about it.
pub fn notice(cx: &mut gpui::Context<crate::Backplane>, light: bool) -> Option<gpui::AnyElement> {
    use crate::i18n::{tf, trs};
    use gpui::{div, prelude::*, px};
    let c = |n: &str| crate::theme::color(light, n);
    let (text, btn): (String, Option<(String, &'static str)>) = match skew(&version(), &hub_version(), restart_soon().as_deref()) {
        Skew::Same => return None,
        Skew::Restart(t) => (tf("gpui.appnotice.ready", &[("tag", t.trim_start_matches('v').to_string().into())]), Some((trs("Restart to update"), "app-restart"))),
        Skew::AppBehind { app, hub } => (tf("gpui.appnotice.appbehind", &[("app", app.into()), ("hub", hub.into())]), (here().is_some() && !off()).then(|| (trs("Check now"), "app-check"))),
        Skew::HubBehind { app, hub } => (tf("gpui.appnotice.hubbehind", &[("app", app.into()), ("hub", hub.into())]), hub_can_check().then(|| (trs("Check the hub"), "hub-check"))),
    };
    let mut d = div().id("app-notice").px_3().py_2().border_t_1().border_color(c("line")).flex().flex_col().gap_1().text_size(crate::scale::tpx(11.)).text_color(c("text")).child(text);
    if let Some((label, name)) = btn {
        d = d.child(div().id("app-notice-act").flex().child(crate::ui::with_rect(div().id("app-notice-btn"), move || format!("ui:{name}")).cursor_pointer().px_2().py(px(2.)).bg(c("raise")).hover(|s| s.bg(c("panel"))).child(label)
            .on_click(cx.listener(move |this, _, _, cx| this.local_press(name, "", cx)))));
    }
    Some(d.into_any_element())
}

#[derive(Clone, Debug, PartialEq)]
pub enum RowMode<'a> {
    Remote { status: &'a Value, when: &'a str, off: bool, restart_to: Option<&'a str>, hub_channel: &'a str },
    Local { guarded: bool, restart_to: Option<&'a str> },
}

/// Pure construction of this computer's app row for Settings > Network and Settings > About, including version difference warning.
pub fn format_row(page: &str, is_mac: bool, app_ver: &str, hub_ver: &str, mode: RowMode<'_>, local_channel: &str) -> Option<crate::screens::settings::Row> {
    use crate::{i18n::{tf, trs}, screens::settings::{Btn, Row}};
    let app_v = app_ver.trim_start_matches('v');
    let hub_v = hub_ver.trim_start_matches('v');
    let differ = !hub_v.is_empty() && !app_v.is_empty() && hub_v != app_v;
    let label = tf(if is_mac { "gpui.appupdate.label.mac" } else { "gpui.appupdate.label" }, &[("tag", app_v.into())]);
    let mk = |note: String, buttons: Vec<Btn>, tone: String| Some(Row { label: label.clone(), note, buttons, local: true, tone, ..Default::default() });
    match (page, mode) {
        ("about", _) => {
            let (note, tone) = if differ {
                (format!("{} {}", tf("gpui.appupdate.differs", &[("hub", hub_v.into())]), trs("Updates are under Network.")), "warn".into()) // i18n-ok: both parts come from the catalog
            } else {
                (trs("Updates are under Network."), String::new())
            };
            mk(note, vec![], tone)
        }
        ("network", RowMode::Remote { status, when, off, restart_to, hub_channel }) => {
            let mut s = status.clone();
            if off { s["state"] = "off".into(); }
            let state = s["state"].as_str().unwrap_or("");
            let (key, args, check) = appkeep::line(&s, when);
            let args: Vec<(&str, crate::i18n::Arg)> = args.into_iter().map(|(k, v)| (k, v.into())).collect();
            let mut note = tf(key, &args);
            let mut tone = String::new();
            if differ && !matches!(state, "ready" | "checking" | "downloading") {
                note = format!("{note} {}", tf("gpui.appupdate.differs", &[("hub", hub_v.into())])); // i18n-ok: both parts come from the catalog
                tone = "warn".into();
            }
            let b = |lbl: String, name: &str, value: &str, on: bool| Btn { label: lbl, action: format!("local:{name}"), value: value.into(), on };
            let mut buttons: Vec<Btn> = if restart_to.is_some() { vec![b(trs("Restart to update"), "app-restart", "", true)] } else { vec![] };
            if check { buttons.push(b(trs("Check now"), "app-check", "", false)) }
            buttons.push(b(tf("gpui.appupdate.samehub", &[("channel", channel("", hub_channel).to_string().into())]), "app-channel", "", local_channel.is_empty()));
            buttons.extend(crate::local::APP_CHANNELS.iter().map(|c| b(c.to_string(), "app-channel", c, local_channel == *c)));
            mk(note, buttons, tone)
        }
        ("network", RowMode::Local { guarded, restart_to }) => {
            let mut note = tf(if guarded { "gpui.appupdate.local" } else { "gpui.appupdate.package" }, &[]);
            let mut tone = String::new();
            if differ && restart_to.is_none() {
                note = format!("{note} {}", tf("gpui.appupdate.differs", &[("hub", hub_v.into())])); // i18n-ok: both parts come from the catalog
                tone = "warn".into();
            }
            let b = Btn { label: trs("Restart to update"), action: "local:app-restart".into(), value: String::new(), on: true };
            mk(note, if restart_to.is_some() { vec![b] } else { vec![] }, tone)
        }
        _ => None,
    }
}

/// This computer's app beside the hub's version, on the hub's Network and About pages, whenever this client is connected to a hub: the two can
/// differ (a hub on another machine, or one updated while this window stayed open). On Network it also says what updates it: where it keeps its
/// own build (a hub on another machine), what `keep` last did, with Check now and its channel; else this computer's hub (its guard), or its package.
pub fn row(l: &crate::local::Local, page: &str) -> Option<crate::screens::settings::Row> {
    let hub_v = hub_version();
    let app_v = version();
    let rto = restart_to();
    match here() {
        Some(h) if page == "network" || page == "about" => {
            let mut s = appkeep::status(&h.prefix);
            if matches!(s["state"].as_str(), Some("checking" | "downloading"))
                && !CHECK_IN_FLIGHT.load(Ordering::SeqCst)
                && !appkeep::keep_running(&h.prefix)
            {
                s["state"] = json!("failed");
                s["note"] = json!("");
            }
            let (_, _, _, hh, mm, ss) = crate::plat::local_now();
            let when = hhmm(s["checked"].as_u64().unwrap_or(0), appkeep::now(), (hh, mm, ss));
            let hub_ch = hub_channel(&h.prefix);
            format_row(
                page,
                cfg!(target_os = "macos"),
                &app_v,
                &hub_v,
                RowMode::Remote { status: &s, when: &when, off: off(), restart_to: rto.as_deref(), hub_channel: &hub_ch },
                &l.app_channel,
            )
        }
        None if page == "network" || page == "about" => {
            let guarded = std::env::current_exe().ok().and_then(|me| crate::launch::guarded_install(&me)).is_some();
            format_row(page, cfg!(target_os = "macos"), &app_v, &hub_v, RowMode::Local { guarded, restart_to: rto.as_deref() }, &l.app_channel)
        }
        _ => None,
    }
}
/// Where the row goes: after the hub's own update row on Network (it follows "Update channel"), after the hub's version on About; else at the end.
pub fn row_at(labels: &[&str]) -> usize {
    if let Some(i) = labels.iter().position(|l| *l == "Update channel") { return (i + 2).min(labels.len()) }
    /* i18n-ok: hub row labels */ labels.iter().position(|l| l.starts_with("Hub: Backplane ") || l.starts_with("Backplane ")).map_or(labels.len(), |i| i + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn only_a_client_on_another_machine_s_hub_looks_after_its_build() {
        assert!(remote(true, false, "", false, false, "http://192.0.2.7:3787", false), "launch.json remote URL");
        assert!(remote(false, true, "wss://desktop.ts.net", false, false, "", false), "hubs.json Start::Paired remote hub");
        assert!(!remote(false, true, "ws://127.0.0.1:3800", false, false, "", false), "paired loopback hub does not self-update");
        assert!(!remote(false, false, "http://192.0.2.7:3787", false, false, "http://192.0.2.7:3787", false), "--url: a hub was named on the command line");
        assert!(!remote(true, false, "", true, false, "http://192.0.2.7:3787", false), "BACKPLANE_HOME: this device's hub");
        assert!(!remote(true, false, "", false, true, "http://192.0.2.7:3787", false), "a local hub this client was started on");
        assert!(!remote(true, false, "", false, false, "", false), "launch.json names a home");
        assert!(!remote(false, true, "wss://desktop.ts.net", false, false, "", true), "special/script modes never self-update");
    }
    #[test] fn the_build_is_the_folder_this_app_runs_from() {
        let d = crate::tmp::uncreated("selfupdate-find");
        let me = d.join("builds/v0.16.0").join(if cfg!(windows) { "backplane-gpui.exe" } else { "backplane-gpui" }); std::fs::create_dir_all(me.parent().unwrap()).unwrap();
        assert_eq!(find(&me), None, "no CURRENT, no guard: not a guarded install");
        std::fs::write(d.join("CURRENT"), "v0.16.0\n").unwrap(); std::fs::write(d.join(if cfg!(windows) { "backplane.exe" } else { "backplane" }), b"x").unwrap();
        assert_eq!(find(&me), Some(Here { prefix: d.to_path_buf(), me: "v0.16.0".into() }));
        assert_eq!(find(&d.join("target/release/backplane-gpui")), None, "a build tree");
    }
    #[test] fn the_channel_is_the_hub_s_unless_this_device_chose_one() {
        assert_eq!(channel("", "nightly"), "nightly", "follows the hub, so both move to the same releases");
        assert_eq!(channel("beta", "nightly"), "beta", "this device's own choice");
        assert_eq!(channel("", ""), "stable", "nothing heard from the hub yet"); assert_eq!(channel("", "canary"), "stable"); assert_eq!(channel("x", "beta"), "beta");
    }
    #[test] fn per_hub_channel_storage_and_failover_isolate_different_hubs() {
        let v = json!({
            "hubs": {
                "h1": {"channel": "nightly", "version": "0.17.0"},
                "h2": {"channel": "stable", "version": "0.16.0"}
            },
            "last": "h2"
        });
        assert_eq!(hub_channel_for(&v, "h1"), "nightly");
        assert_eq!(hub_channel_for(&v, "h2"), "stable");
        assert_eq!(hub_channel_for(&v, "h3"), "");

        let mut store = json!({});
        record_hub_entry(&mut store, "work", "nightly", "0.17.0", false);
        record_hub_entry(&mut store, "home", "stable", "0.16.0", true);
        assert_eq!(store["last"], "work", "failover does not clobber primary last hub");
        assert_eq!(hub_channel_for(&store, "work"), "nightly");
        assert_eq!(hub_channel_for(&store, "home"), "stable");

        set_primary_hub_key("work");
        assert_eq!(active_hub_key(), ("work".to_string(), false));
        set_using(Some("home"));
        assert_eq!(active_hub_key(), ("home".to_string(), true));
        set_using(None);
        assert_eq!(active_hub_key(), ("work".to_string(), false));
    }
    #[test] fn the_sidebar_says_when_the_app_and_the_hub_differ() {
        assert_eq!(skew("v0.16.0", "0.16.0", None), Skew::Same);
        assert_eq!(skew("0.16.0", "", None), Skew::Same, "the hub has not said");
        assert_eq!(skew("v0.15.2", "0.16.0", None), Skew::AppBehind { app: "0.15.2".into(), hub: "0.16.0".into() });
        assert_eq!(skew("v0.16.1", "0.16.0", None), Skew::HubBehind { app: "0.16.1".into(), hub: "0.16.0".into() });
        assert_eq!(skew("v0.15.2", "0.16.0", Some("v0.16.0")), Skew::Restart("v0.16.0".into()), "a newer build waits: the one thing to do is restart");
        assert_eq!(skew("v0.16.0-nightly.20261114.1", "0.16.0", None), Skew::AppBehind { app: "0.16.0-nightly.20261114.1".into(), hub: "0.16.0".into() });
    }
    #[test] fn the_last_check_is_a_time_of_day_on_this_clock() {
        assert_eq!(hhmm(1000, 1000 + 125 * 60, (14, 10, 0)), "12:05");
        assert_eq!(hhmm(1000, 1000 + 3600, (0, 30, 0)), "23:30", "before midnight");
        assert_eq!(hhmm(0, 5, (1, 0, 0)), "", "never"); assert_eq!(hhmm(9, 5, (1, 0, 0)), "", "a clock that went back");
    }
    #[test] fn the_row_sits_after_the_hub_s_own_update_row() {
        assert_eq!(row_at(&["Network", "Update channel", "Hub: Backplane 0.16.0", "Tailscale"]), 3);
        assert_eq!(row_at(&["Network", "Update channel", "Backplane v0.16.0", "Tailscale"]), 3);
        assert_eq!(row_at(&["Network", "Update channel"]), 2); assert_eq!(row_at(&["Network", "Tailscale"]), 2, "an older hub: at the end");
        assert_eq!(row_at(&["About", "Hub: Backplane 0.16.0", "Browser"]), 2, "About: under the hub's version");
        assert_eq!(row_at(&["About", "Backplane v0.15.2", "Browser"]), 2, "an older hub's label");
    }
    #[test] fn settings_rows_show_this_computers_app_and_warn_when_versions_differ() {
        crate::i18n::with_lang("en", || {
            let idle = json!({"state": "idle"});
            let same = format_row("network", false, "v0.16.0", "0.16.0", RowMode::Remote { status: &idle, when: "14:05", off: false, restart_to: None, hub_channel: "nightly" }, "").unwrap();
            assert_eq!(same.label, "This computer's app: 0.16.0");
            assert_eq!(same.note, "Up to date, checked 14:05.");
            assert!(same.tone.is_empty());
            assert_eq!(same.buttons[0].label, "Check now");
            assert_eq!(same.buttons[1].label, "Same as the hub (nightly)");
            assert!(same.buttons[1].on);

            let skewed = format_row("network", true, "v0.15.2", "0.16.0", RowMode::Remote { status: &idle, when: "14:05", off: false, restart_to: None, hub_channel: "stable" }, "beta").unwrap();
            assert_eq!(skewed.label, "This Mac's app: 0.15.2");
            assert_eq!(skewed.tone, "warn");
            assert!(skewed.note.contains("Differs from the hub (0.16.0)."), "{}", skewed.note);
            assert!(skewed.buttons.iter().any(|b| b.action == "local:app-channel" && b.value == "beta" && b.on));

            let ready = json!({"state": "ready", "available": "v0.16.0"});
            let rdy = format_row("network", false, "v0.15.2", "0.16.0", RowMode::Remote { status: &ready, when: "14:05", off: false, restart_to: Some("v0.16.0"), hub_channel: "stable" }, "").unwrap();
            assert_eq!(rdy.buttons[0].label, "Restart to update");
            assert_eq!(rdy.buttons[0].action, "local:app-restart");
            assert!(rdy.buttons[0].on);
            assert!(rdy.tone.is_empty(), "ready already offers Restart to update without extra mismatch warning");

            let about_diff = format_row("about", false, "v0.15.2", "0.16.0", RowMode::Local { guarded: false, restart_to: None }, "").unwrap();
            assert_eq!(about_diff.tone, "warn");
            assert!(about_diff.note.contains("Differs from the hub (0.16.0)."), "{}", about_diff.note);
            let about_same = format_row("about", false, "v0.16.0", "0.16.0", RowMode::Local { guarded: true, restart_to: None }, "").unwrap();
            assert!(about_same.tone.is_empty());
            assert_eq!(about_same.note, "Updates are under Network.");

            let local_skew = format_row("network", false, "v0.15.2", "0.16.0", RowMode::Local { guarded: true, restart_to: Some("v0.16.0") }, "").unwrap();
            assert_eq!(local_skew.buttons[0].action, "local:app-restart");
        });
    }
    #[test] fn settings_rows_and_error_notes_render_cleanly_in_japanese_and_spanish() {
        let failed = json!({"state": "failed", "note": "could not reach the release feed (curl 7)"});
        let ready = json!({"state": "ready", "available": "v0.16.1"});
        for (lang, hub_lbl, label_sub, err_sub, btn_sub) in [
            ("ja", "ハブ: Backplane 0.16.0", "このコンピューターのアプリ: 0.15.2", "リリースフィードに接続できませんでした", "今すぐ確認"),
            ("es", "Hub: Backplane 0.16.0", "La app de este ordenador: 0.15.2", "No se pudo conectar al feed de versiones", "Comprobar ahora"),
        ] {
            crate::i18n::with_lang(lang, || {
                assert_eq!(crate::i18n::trs("Hub: Backplane 0.16.0"), hub_lbl);
                let r_err = format_row("network", false, "v0.15.2", "0.16.0", RowMode::Remote { status: &failed, when: "14:05", off: false, restart_to: None, hub_channel: "stable" }, "").unwrap();
                assert_eq!(r_err.label, label_sub);
                assert!(r_err.note.contains(err_sub), "{lang}: {}", r_err.note);
                assert!(!r_err.note.contains("curl 7"), "{lang} must not leak raw English errors: {}", r_err.note);
                assert_eq!(r_err.buttons[0].label, btn_sub);

                let r_ready = format_row("network", false, "v0.15.2", "0.16.0", RowMode::Remote { status: &ready, when: "14:05", off: false, restart_to: Some("v0.16.1"), hub_channel: "stable" }, "").unwrap();
                assert!(!r_ready.buttons[0].label.contains("Restart to update"), "{lang} button: {}", r_ready.buttons[0].label);

                let r_about = format_row("about", false, "v0.15.2", "0.16.0", RowMode::Local { guarded: false, restart_to: None }, "").unwrap();
                assert!(!r_about.note.contains("Updates are under Network"), "{lang} about: {}", r_about.note);
                assert_eq!(r_about.tone, "warn");
            });
        }
    }
    #[test] fn concurrent_check_requests_serialize_and_queue_one_followup() {
        // Reset flags cleanly in case prior tests ran in same process
        CHECK_IN_FLIGHT.store(false, Ordering::SeqCst);
        CHECK_AGAIN.store(false, Ordering::SeqCst);
        assert!(request_check());
        assert!(!request_check());
        assert!(!request_check());
        assert!(finish_check(), "one follow-up was queued");
        assert!(!finish_check(), "queue cleared after drain");
    }
}

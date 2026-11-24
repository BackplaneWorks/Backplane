//! The desktop app's own updates where no guard runs: a client that uses a hub on another machine (`launch.json` names a `url`, or a paired
//! remote hub in `hubs.json`) starts no hub here, so no guard looks at the release feed and nothing moves `CURRENT` (docs/deploy.md, "A desktop app
//! on another machine's hub").
//!
//! Two programs share this file (by path, like scopeprobe.rs): the guard's `backplane builds keep` (keep.rs) finds, checks and stages a release
//! with the hub's own updater (`update::rel`, `update::run`), and the desktop app (`native/gpui`, selfupdate.rs) switches to it at a launch and
//! proves it. `CURRENT` only ever names a build whose window opened here: a launch tries the staged build while `CURRENT` still names the one it
//! came from, so a new build that cannot start leaves the launchers (Backplane.app, `backplane-app`, the Windows guard's `backplane`) on the old one,
//! and the next launch marks it bad and stays.
//!
//! Plain files beside the builds, in `<prefix>/app-update/`:
//!   next         the build `keep` staged for the next launch
//!   manual       marker naming a hand-installed Windows build in `next` (not discarded on channel checks)
//!   trial        {"tag","from","at","pid","nonce"}: the build a launch is trying; CURRENT still names `from` until the new app's window is up
//!   good         builds whose app opened its window here, oldest first
//!   status.json  what `keep` last did: {state, available, channel, note, checked}, and `said`: how the last switch ended ({"updated": tag} or {"stayed": tag})
//!   state.lock   short-lived exclusive lock serializing pointer, trial, good, bad, guard-lock ownership, and status mutations across app and keeper
//!   lock         one `keep` at a time
//!   keep.log     what `keep` said
//! `<prefix>/clients.lock`: every app of this prefix holds it shared while it runs; a launch moves to another build only when it can hold it alone,
//! so no build ever changes under a running app. `<prefix>/.guard.lock`: held shared by any running guard. `<prefix>/BAD` (id, tab, why) is the
//! guard's list: a build that failed here, under a guard or here, is never offered or tried again.
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};

static PUT_SEQ: AtomicU64 = AtomicU64::new(0);

pub fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) }
pub fn dir(prefix: &Path) -> PathBuf { prefix.join("app-update") }
/// The app of a build: what every launcher runs.
pub fn app_of(prefix: &Path, tag: &str) -> PathBuf { prefix.join("builds").join(tag).join(if cfg!(windows) { "backplane-gpui.exe" } else { "backplane-gpui" }) }
/// The guard of a build: the app runs its own build's `backplane builds keep`, so the command is always the one this file was built into.
pub fn guard_of(prefix: &Path, tag: &str) -> PathBuf { prefix.join("builds").join(tag).join(if cfg!(windows) { "backplane.exe" } else { "backplane" }) }

/// Execute `f` while holding `<prefix>/app-update/state.lock` exclusively (fail closed: returns Err if the lock cannot be opened or acquired).
/// Callers inside `f` must not re-enter `try_with_state_lock`.
pub fn try_with_state_lock<T>(prefix: &Path, f: impl FnOnce() -> T) -> std::io::Result<T> {
    let d = dir(prefix);
    fs::create_dir_all(&d)?;
    let lock = fs::OpenOptions::new().create(true).truncate(false).write(true).open(d.join("state.lock"))?;
    lock.lock()?;
    let out = f();
    drop(lock);
    Ok(out)
}

fn rename_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let mut n = 0;
        loop {
            match fs::rename(from, to) {
                Err(e) if n < 100 && matches!(e.raw_os_error(), Some(5) | Some(32)) => {
                    n += 1;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                r => return r,
            }
        }
    }
    #[cfg(not(windows))]
    {
        fs::rename(from, to)
    }
}

/// Written beside and renamed over: a reader never sees half, and a full disk loses the write, never the file that was there.
pub fn put(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(d) = path.parent() { fs::create_dir_all(d)?; }
    let seq = PUT_SEQ.fetch_add(1, Relaxed);
    let base = path.file_name().map_or_else(|| "x".to_string(), |n| n.to_string_lossy().into_owned());
    let tmp = path.with_file_name(format!(".{base}.{}.{seq}", std::process::id()));
    let r = fs::write(&tmp, text).and_then(|_| rename_replace(&tmp, path));
    if r.is_err() { let _ = fs::remove_file(&tmp); }
    r
}
pub fn lines(p: &Path) -> Vec<String> { fs::read_to_string(p).unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect() }
fn first(p: &Path) -> String { lines(p).into_iter().next().unwrap_or_default() }

/// Strict single-line folder name allowed in pointer files (`CURRENT`, `next`): strips at most one trailing `\n`/`\r` and requires `name_ok`.
pub fn valid_current(raw: &str) -> Option<String> {
    let s = raw.strip_suffix('\n').unwrap_or(raw);
    let s = s.strip_suffix('\r').unwrap_or(s);
    name_ok(s).then(|| s.to_string())
}
/// Strict tag-list parser (`good`): accepts empty file or lines separated by `\n`/`\r\n` with every line satisfying `name_ok`.
pub fn valid_tag_list(raw: &str) -> Option<Vec<String>> {
    if raw.is_empty() { return Some(Vec::new()); }
    let body = raw.strip_suffix('\n').unwrap_or(raw);
    let mut out = Vec::new();
    for line in body.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if !name_ok(line) { return None }
        out.push(line.to_string());
    }
    Some(out)
}
/// Read `<prefix>/app-update/good` fail-closed: `Ok(Vec::new())` only when absent, `Err(())` on I/O errors or malformed entries.
pub fn try_read_good_unlocked(prefix: &Path) -> Result<Vec<String>, ()> {
    match fs::read_to_string(dir(prefix).join("good")) {
        Ok(raw) => valid_tag_list(&raw).ok_or(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(_) => Err(()),
    }
}
/// Read `<prefix>/app-update/next` fail-closed: `Ok(None)` only when absent, `Err(())` on non-NotFound errors or malformed contents.
pub fn try_read_next_unlocked(prefix: &Path) -> Result<Option<String>, ()> {
    let raw = match fs::read_to_string(dir(prefix).join("next")) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    valid_current(&raw).map(Some).ok_or(())
}

/// `CURRENT`: the build the launchers run.
pub fn current(prefix: &Path) -> String { first(&prefix.join("CURRENT")) }
fn publish_unlocked(prefix: &Path, tag: &str) -> std::io::Result<()> { put(&prefix.join("CURRENT"), &format!("{tag}\n")) }
/// The one writer of CURRENT where no guard runs (serialized under `state.lock`).
pub fn publish(prefix: &Path, tag: &str) -> std::io::Result<()> { try_with_state_lock(prefix, || publish_unlocked(prefix, tag))? }
/// `BAD`'s ids.
pub fn bad(prefix: &Path) -> Vec<String> { bad_lines(prefix).into_iter().map(|x| x.0).collect() }
/// `BAD`, the guard's list: id, tab, why.
pub fn bad_lines(prefix: &Path) -> Vec<(String, String)> {
    fs::read_to_string(prefix.join("BAD")).unwrap_or_default().lines().filter_map(|l| { let (i, w) = l.split_once('\t').unwrap_or((l, "")); (!i.is_empty()).then(|| (i.to_string(), w.to_string())) }).collect()
}
fn mark_bad_unlocked(prefix: &Path, id: &str, why: &str) {
    if bad(prefix).iter().any(|b| b == id) { return }
    let why: String = why.chars().filter(|c| !c.is_control()).take(200).collect();
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(prefix.join("BAD")) { let _ = writeln!(f, "{id}\t{why}"); }
    let _ = fs::write(prefix.join(".fresh-bad"), format!("{id}\n"));
}
fn clear_bad_unlocked(prefix: &Path, id: &str) {
    let kept: Vec<String> = fs::read_to_string(prefix.join("BAD")).unwrap_or_default().lines().filter(|l| l.split('\t').next() != Some(id)).map(str::to_string).collect();
    let body = if kept.is_empty() { String::new() } else { kept.join("\n") + "\n" };
    let _ = put(&prefix.join("BAD"), &body);
    if fs::read_to_string(prefix.join(".fresh-bad")).unwrap_or_default().trim() == id { let _ = fs::remove_file(prefix.join(".fresh-bad")); }
}
/// One line more in BAD, as the guard writes it (`store::Builds::mark_bad` is this); `.fresh-bad` holds the newest alone.
pub fn mark_bad(prefix: &Path, id: &str, why: &str) { let _ = try_with_state_lock(prefix, || mark_bad_unlocked(prefix, id, why)); }
/// Take the bad mark off (`store::Builds::clear_bad`).
pub fn clear_bad(prefix: &Path, id: &str) { let _ = try_with_state_lock(prefix, || clear_bad_unlocked(prefix, id)); }

/// A name that can only be a folder in `builds/`: it reaches `exec`, so nothing that could leave the folder gets through.
pub fn name_ok(tag: &str) -> bool { !tag.is_empty() && tag.len() <= 64 && !tag.starts_with('.') && tag.chars().all(|c| c.is_ascii_alphanumeric() || "-._".contains(c)) }

/// The channel a tag belongs to (`update::rel::tag_channel`): `-nightly.` is nightly, any other prerelease beta.
pub fn tag_channel(tag: &str) -> &'static str {
    if tag.contains("-nightly.") { "nightly" } else if tag.contains('-') { "beta" } else { "stable" }
}
/// Strict channel policy matching `update::rel::takes`: nightly tracks nightly builds only; beta takes stable and beta; stable takes stable only.
pub fn channel_takes(channel: &str, release_channel: &str) -> bool {
    match channel.trim() {
        "nightly" => release_channel == "nightly",
        "beta" => matches!(release_channel, "stable" | "beta"),
        _ => release_channel == "stable",
    }
}

/// Whether process `pid` is currently alive on this OS.
pub fn pid_alive(pid: u32) -> bool {
    if pid == 0 || pid > i32::MAX as u32 { return false }
    #[cfg(unix)]
    {
        unsafe extern "C" { fn kill(pid: i32, sig: i32) -> i32; }
        unsafe { kill(pid as i32, 0) == 0 || std::io::Error::last_os_error().raw_os_error() == Some(1) }
    }
    #[cfg(windows)]
    {
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        const STILL_ACTIVE: u32 = 259;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut core::ffi::c_void;
            fn GetExitCodeProcess(handle: *mut core::ffi::c_void, code: *mut u32) -> i32;
            fn CloseHandle(handle: *mut core::ffi::c_void) -> i32;
        }
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() { return false }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(h, &mut code) != 0 && code == STILL_ACTIVE;
            CloseHandle(h);
            ok
        }
    }
    #[cfg(not(any(unix, windows)))]
    { false }
}

/// Whether `backplane builds keep` currently holds `<prefix>/app-update/lock`.
pub fn keep_running(prefix: &Path) -> bool {
    let p = dir(prefix).join("lock");
    if !p.is_file() { return false }
    fs::OpenOptions::new().write(true).open(&p).ok().is_some_and(|f| f.try_lock().is_err())
}

/// A release tag's order (`update::rel::Ver`'s rule, which a test in keep.rs holds this to): a prerelease before its release, prereleases by their numbers.
pub fn ver_key(tag: &str) -> (u64, u64, u64, bool, u64, u64) {
    let s = tag.trim(); let s = s.strip_prefix('v').unwrap_or(s);
    let (core, pre) = match s.split_once('-') { Some((c, p)) => (c, Some(p)), None => (s, None) };
    let mut c = core.split('.').map(|x| x.parse::<u64>().unwrap_or(0));
    let (a, b, d) = (c.next().unwrap_or(0), c.next().unwrap_or(0), c.next().unwrap_or(0));
    let nums: Vec<u64> = pre.map(|p| p.split('.').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    (a, b, d, pre.is_none(), nums.first().copied().unwrap_or(0), nums.get(1).copied().unwrap_or(0))
}
pub fn newer(a: &str, than: &str) -> bool { ver_key(a) > ver_key(than) }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trial { pub tag: String, pub from: String, pub at: u64, pub pid: u32, pub nonce: String }
/// Read `<prefix>/app-update/trial` fail-closed: `Ok(None)` only when absent, `Err(())` on non-NotFound I/O errors or invalid/incomplete JSON.
pub fn try_read_trial_unlocked(prefix: &Path) -> Result<Option<Trial>, ()> {
    let raw = match fs::read_to_string(dir(prefix).join("trial")) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    let v: Value = serde_json::from_str(&raw).map_err(|_| ())?;
    let tag = v.get("tag").and_then(Value::as_str).filter(|s| name_ok(s)).ok_or(())?.to_string();
    let from = v.get("from").and_then(Value::as_str).filter(|s| name_ok(s)).ok_or(())?.to_string();
    let at = v.get("at").and_then(Value::as_u64).ok_or(())?;
    let pid = v.get("pid").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()).filter(|&n| n > 0 && n <= i32::MAX as u32).ok_or(())?;
    let nonce = v.get("nonce").and_then(Value::as_str).filter(|s| !s.is_empty()).ok_or(())?.to_string();
    Ok(Some(Trial { tag, from, at, pid, nonce }))
}
fn trial_unlocked(prefix: &Path) -> Option<Trial> { try_read_trial_unlocked(prefix).ok().flatten() }
pub fn trial(prefix: &Path) -> Option<Trial> { trial_unlocked(prefix) }
fn next_unlocked(prefix: &Path) -> String { try_read_next_unlocked(prefix).ok().flatten().unwrap_or_default() }
pub fn next(prefix: &Path) -> String { next_unlocked(prefix) }
fn good_unlocked(prefix: &Path) -> Vec<String> { try_read_good_unlocked(prefix).unwrap_or_default() }
pub fn good(prefix: &Path) -> Vec<String> { good_unlocked(prefix) }
fn add_good_unlocked(prefix: &Path, tag: &str) {
    if !name_ok(tag) { return }
    let Ok(mut g) = try_read_good_unlocked(prefix) else { return };
    g.retain(|b| b != tag);
    g.push(tag.to_string());
    let from = g.len().saturating_sub(16);
    let _ = put(&dir(prefix).join("good"), &(g[from..].join("\n") + "\n"));
}
/// `tag` goes last; the newest 16 are kept.
pub fn add_good(prefix: &Path, tag: &str) { let _ = try_with_state_lock(prefix, || add_good_unlocked(prefix, tag)); }

pub fn set_manual_next(prefix: &Path, tag: &str) -> std::io::Result<()> {
    try_with_state_lock(prefix, || {
        if guard_runs(prefix) { return Ok(()); }
        let Ok(raw_cur) = fs::read_to_string(prefix.join("CURRENT")) else {
            return Err(std::io::Error::other("CURRENT is unreadable"));
        };
        let Some(cur) = valid_current(&raw_cur) else {
            return Err(std::io::Error::other("CURRENT is malformed"));
        };
        if !newer(tag, &cur) { return Ok(()); }
        if try_read_next_unlocked(prefix).is_err() {
            return Err(std::io::Error::other("staged build pointer is unreadable"));
        }
        put(&dir(prefix).join("next"), &format!("{tag}\n"))?;
        put(&dir(prefix).join("manual"), &format!("{tag}\n"))
    })?
}
pub fn is_manual_next(prefix: &Path, tag: &str) -> bool {
    !tag.is_empty() && fs::read_to_string(dir(prefix).join("manual")).ok().and_then(|s| valid_current(&s)).as_deref() == Some(tag)
}
/// Commit `next` assuming the caller already holds `<prefix>/app-update/state.lock`.
pub fn commit_next_unlocked(prefix: &Path, tag: &str) -> Result<(), String> {
    if guard_runs(prefix) { return Err("a guard owns this prefix".into()); }
    if bad(prefix).iter().any(|b| b == tag) { return Err("build was marked bad".into()); }
    if try_read_next_unlocked(prefix).is_err() { return Err("staged build pointer is unreadable".into()); }
    if !app_of(prefix, tag).is_file() { return Err("staged build is missing".into()); }
    put(&dir(prefix).join("next"), &format!("{tag}\n")).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(dir(prefix).join("manual"));
    Ok(())
}
pub fn commit_next(prefix: &Path, tag: &str) -> Result<(), String> {
    try_with_state_lock(prefix, || commit_next_unlocked(prefix, tag)).map_err(|e| e.to_string())?
}
pub fn clear_next_if_matching(prefix: &Path, expected: &str) {
    let _ = try_with_state_lock(prefix, || {
        if matches!(try_read_next_unlocked(prefix), Ok(Some(ref n)) if n == expected) && !is_manual_next(prefix, expected) {
            let _ = fs::remove_file(dir(prefix).join("next"));
            let _ = fs::remove_file(dir(prefix).join("manual"));
        }
    });
}

/// What a launch does, before any window. `me`: the build this app is; `next`: the build `keep` staged; `sole`: no other app of this prefix runs;
/// `trial_alive`: true when an existing `trial.pid` is a different live process; `channel_ok`: whether `next` matches the effective channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Start {
    /// run as this build
    Stay,
    /// try this build: write the trial, then run its app in this one's place
    Try(String),
    /// the build last tried never opened its window: mark it bad and run as this build
    Failed { tag: String, why: String },
    /// a trial this launch cannot judge (CURRENT was moved by hand meanwhile: install.sh run again): forget it, run as this build
    Forget,
}
pub fn start(me: &str, next: &str, trial: Option<&Trial>, bad: &[String], exists: &dyn Fn(&str) -> bool, sole: bool, trial_alive: bool, channel_ok: bool) -> Start {
    // another app of this prefix runs: nothing is judged or moved under it
    if !sole { return Start::Stay }
    if let Some(t) = trial {
        // the trial itself, just started by the build it came from
        if t.tag == me { return Start::Stay }
        // a live trial process is still starting: never mark it bad or forget it while it runs
        if trial_alive { return Start::Stay }
        if t.from == me {
            return /* i18n-ok: stored in BAD as reason; never drawn verbatim */ Start::Failed { tag: t.tag.clone(), why: "its app never opened its window".into() };
        }
        return Start::Forget;
    }
    if !name_ok(next) || !channel_ok || next == me || bad.iter().any(|b| b == next) || !exists(next) || !newer(next, me) { return Start::Stay }
    Start::Try(next.to_string())
}

/// `Start::Try`: write the trial before the new app runs, so a crash before its window leaves the way back written down.
#[cfg(test)]
fn begin(prefix: &Path, tag: &str, from: &str, pid: u32, nonce: &str) -> std::io::Result<()> {
    try_with_state_lock(prefix, || {
        if guard_runs(prefix) { return Err(std::io::Error::other("a guard owns this prefix")); }
        put(&dir(prefix).join("trial"), &json!({"tag": tag, "from": from, "at": now(), "pid": pid, "nonce": nonce}).to_string())
    })?
}
/// Atomically evaluate launch trial/promotion preconditions and commit `trial` under `state.lock` so a keeper cannot clear `next` or
/// prune the target build between the check and trial record write.
pub fn decide_and_begin(prefix: &Path, me: &str, channel: &str, sole: bool, pid: u32, nonce: &str) -> Start {
    if !sole { return Start::Stay }
    try_with_state_lock(prefix, || {
        if guard_runs(prefix) { return Start::Stay }
        let Ok(raw_cur) = fs::read_to_string(prefix.join("CURRENT")) else { return Start::Stay };
        let Some(cur) = valid_current(&raw_cur) else { return Start::Stay };
        if cur != me { return Start::Stay }
        let Ok(cur_trial) = try_read_trial_unlocked(prefix) else { return Start::Stay };
        let trial_alive = cur_trial.as_ref().is_some_and(|t| t.pid != 0 && t.pid != pid && pid_alive(t.pid));
        let Ok(nxt_opt) = try_read_next_unlocked(prefix) else { return Start::Stay };
        let nxt = nxt_opt.unwrap_or_default();
        let channel_ok = is_manual_next(prefix, &nxt) || channel_takes(channel, tag_channel(&nxt));
        let b = bad(prefix);
        match start(me, &nxt, cur_trial.as_ref(), &b, &|t| app_of(prefix, t).is_file(), sole, trial_alive, channel_ok) {
            Start::Stay => Start::Stay,
            Start::Forget => {
                let _ = fs::remove_file(dir(prefix).join("trial"));
                Start::Forget
            }
            Start::Failed { tag, why } => {
                failed_unlocked(prefix, &tag, &why);
                Start::Failed { tag, why }
            }
            Start::Try(tag) => {
                if !name_ok(&tag) || !name_ok(me) || pid == 0 || pid > i32::MAX as u32 || nonce.is_empty() { return Start::Stay }
                let body = json!({"tag": tag, "from": me, "at": now(), "pid": pid, "nonce": nonce}).to_string();
                if put(&dir(prefix).join("trial"), &body).is_ok() { Start::Try(tag) } else { Start::Stay }
            }
        }
    }).unwrap_or(Start::Stay)
}
/// Write a manual-upgrade trial under `state.lock`, verifying no guard owns `prefix`, `CURRENT == from`, and no live trial is currently running.
pub fn begin_if_no_live_trial(prefix: &Path, tag: &str, from: &str, pid: u32, nonce: &str) -> Result<(), String> {
    if !name_ok(tag) || !name_ok(from) || pid == 0 || pid > i32::MAX as u32 || nonce.is_empty() {
        return Err("invalid trial arguments".into());
    }
    try_with_state_lock(prefix, || {
        if guard_runs(prefix) { return Err("a guard owns this prefix".into()); }
        let Ok(raw_cur) = fs::read_to_string(prefix.join("CURRENT")) else { return Err("CURRENT is unreadable".into()); };
        let Some(cur) = valid_current(&raw_cur) else { return Err("CURRENT is malformed".into()); };
        if cur != from { return Err("CURRENT changed".into()); }
        let cur_trial = try_read_trial_unlocked(prefix).map_err(|()| "trial record is unreadable".to_string())?;
        if let Some(t) = cur_trial {
            if t.pid != 0 && t.pid != std::process::id() && pid_alive(t.pid) {
                return Err("another trial is already starting".into());
            }
        }
        put(&dir(prefix).join("trial"), &json!({"tag": tag, "from": from, "at": now(), "pid": pid, "nonce": nonce}).to_string()).map_err(|e| e.to_string())
    }).map_err(|e| e.to_string())?
}
pub fn update_trial_pid(prefix: &Path, tag: &str, nonce: &str, pid: u32) {
    let _ = try_with_state_lock(prefix, || {
        if let Some(t) = trial_unlocked(prefix) {
            if t.tag == tag && t.nonce == nonce {
                let _ = put(&dir(prefix).join("trial"), &json!({"tag": t.tag, "from": t.from, "at": t.at, "pid": pid, "nonce": t.nonce}).to_string());
            }
        }
    });
}
fn failed_unlocked(prefix: &Path, tag: &str, why: &str) {
    mark_bad_unlocked(prefix, tag, why);
    let d = dir(prefix); let _ = fs::remove_file(d.join("trial"));
    if matches!(try_read_next_unlocked(prefix), Ok(Some(ref n)) if n == tag) {
        let _ = fs::remove_file(d.join("next"));
        let _ = fs::remove_file(d.join("manual"));
    }
    set_status_unlocked(prefix, |s| { s["available"] = json!(""); s["said"] = json!({"stayed": tag}); if s["state"] == "ready" { s["state"] = json!("idle") } });
}
/// `Start::Failed`, or a trial that could not even be started: marked bad, never offered or tried again; the staged pointer goes with it.
pub fn failed(prefix: &Path, tag: &str, why: &str) {
    let _ = try_with_state_lock(prefix, || {
        if guard_runs(prefix) { return; }
        failed_unlocked(prefix, tag, why);
    });
}
/// Remove `trial` only if its nonce matches `nonce` (used on child spawn failure so another process's trial is never erased).
pub fn forget_if_nonce(prefix: &Path, nonce: &str) {
    let _ = try_with_state_lock(prefix, || {
        if trial_unlocked(prefix).is_some_and(|t| t.nonce == nonce) {
            let _ = fs::remove_file(dir(prefix).join("trial"));
        }
    });
}
/// The app's window has presented its first frame: this build is proven here. A trial becomes CURRENT; a newer `next` staged while it opened stays intact.
/// Answers the build a trial moved to.
pub fn opened(prefix: &Path, me: &str, my_nonce: Option<&str>) -> Option<String> {
    try_with_state_lock(prefix, || {
        if guard_runs(prefix) { return None }
        let Ok(cur_trial) = try_read_trial_unlocked(prefix) else { return None };
        let cur = fs::read_to_string(prefix.join("CURRENT")).ok().and_then(|s| valid_current(&s));
        match cur_trial {
            Some(t) if t.tag == me => {
                if my_nonce != Some(t.nonce.as_str()) { return None }
                let Some(ref cur_tag) = cur else { return None };
                if cur_tag != &t.from || bad(prefix).iter().any(|b| b == me) {
                    let _ = fs::remove_file(dir(prefix).join("trial"));
                    return None;
                }
                add_good_unlocked(prefix, me);
                if publish_unlocked(prefix, me).is_err() { return None }
                let d = dir(prefix); let _ = fs::remove_file(d.join("trial"));
                if matches!(try_read_next_unlocked(prefix), Ok(Some(ref n)) if n == me) {
                    let _ = fs::remove_file(d.join("next"));
                    let _ = fs::remove_file(d.join("manual"));
                    set_status_unlocked(prefix, |s| { s["state"] = json!("idle"); s["available"] = json!(""); s["said"] = json!({"updated": me}); });
                } else {
                    set_status_unlocked(prefix, |s| { s["said"] = json!({"updated": me}); });
                }
                Some(me.to_string())
            }
            _ => { if cur.as_deref() == Some(me) { add_good_unlocked(prefix, me); } None }
        }
    }).ok().flatten()
}

/// Clear a stale `checking`/`downloading` status only when still busy and no keeper holds `app-update/lock` inside `state.lock`.
pub fn fail_stale_busy(prefix: &Path) {
    let _ = try_with_state_lock(prefix, || {
        if matches!(status(prefix)["state"].as_str(), Some("checking" | "downloading")) && !keep_running(prefix) {
            set_status_unlocked(prefix, |s| { s["state"] = json!("failed"); s["note"] = json!(""); });
        }
    });
}

/// Builds pruning keeps whatever the guard's rule says (`run::prunable`): CURRENT, the newest good build, and these: the two good builds before it
/// (a second app opened before a switch still runs the one CURRENT named then), the staged build, both sides of a trial, and the build this is.
pub fn protected(good: &[String], next: &str, trial: Option<&Trial>, running: &str) -> Vec<String> {
    let mut p: Vec<String> = good[good.len().saturating_sub(3)..].to_vec();
    for x in [next, running] { if !x.is_empty() { p.push(x.to_string()) } }
    if let Some(t) = trial { p.push(t.tag.clone()); p.push(t.from.clone()); }
    p
}

pub fn status(prefix: &Path) -> Value { serde_json::from_str(&fs::read_to_string(dir(prefix).join("status.json")).unwrap_or_default()).unwrap_or_else(|_| json!({})) }
fn set_status_unlocked(prefix: &Path, f: impl FnOnce(&mut Value)) {
    let mut s = status(prefix); if !s.is_object() { s = json!({}); } f(&mut s); let _ = put(&dir(prefix).join("status.json"), &s.to_string());
}
pub fn set_status(prefix: &Path, f: impl FnOnce(&mut Value)) { let _ = try_with_state_lock(prefix, || set_status_unlocked(prefix, f)); }

fn is_backplane_health_port(port: u16) -> bool {
    let Ok(mut sock) = std::net::TcpStream::connect_timeout(&std::net::SocketAddr::from(([127, 0, 0, 1], port)), std::time::Duration::from_millis(300)) else {
        return false;
    };
    let _ = sock.set_read_timeout(Some(std::time::Duration::from_millis(300)));
    let _ = sock.set_write_timeout(Some(std::time::Duration::from_millis(300)));
    if sock.write_all(b"GET /healthz HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").is_err() {
        return false;
    }
    let mut buf = [0u8; 512];
    let mut total = 0;
    while total < buf.len() {
        match sock.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(_) => break,
        }
    }
    if total == 0 { return false }
    let text = String::from_utf8_lossy(&buf[..total]);
    text.starts_with("HTTP/1.") && (text.contains("\"control\"") || text.contains("\"status\"") || text.contains("\"version\"") || text.contains("\"down\"") || text.contains("\"backplane\""))
}

/// Whether a guard runs a hub from this prefix: then it owns CURRENT, and the app leaves its builds alone. Modern guards hold `<prefix>/.guard.lock`
/// shared throughout their lifetime; legacy guards pin ports under `.pins/<port>` and answer Backplane `/healthz`.
pub fn guard_runs(prefix: &Path) -> bool {
    let gl = prefix.join(".guard.lock");
    match fs::symlink_metadata(&gl) {
        Ok(_) => {
            let Ok(f) = fs::OpenOptions::new().write(true).open(&gl) else { return true };
            if f.try_lock().is_err() { return true }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return true,
    }
    let mut ports = Vec::new();
    match fs::read_dir(prefix.join(".pins")) {
        Ok(d) => {
            for e in d {
                let Ok(e) = e else { return true };
                if let Ok(port) = e.file_name().to_string_lossy().parse::<u16>() {
                    ports.push(port);
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return true,
    }
    ports.into_iter().any(is_backplane_health_port)
}

/// Map internal updater errors in `status.json` to localized catalog keys so Settings never renders raw English text under non-English locales.
pub fn error_key(note: &str) -> &'static str {
    let low = note.to_ascii_lowercase();
    if low.contains("no build is made for this machine") || low.contains("no asset for") {
        "gpui.appupdate.err.platform"
    } else if low.contains("curl") || low.contains("release feed") || low.contains("release list") || low.contains("refusing release url") || low.contains("refusing unsafe release url") {
        "gpui.appupdate.err.network"
    } else if low.contains("signature") || low.contains("checksum") || low.contains("wrong size") || low.contains("update key") || low.contains("manifest") || low.contains("another release") {
        "gpui.appupdate.err.verify"
    } else if low.contains("desktop app") || low.contains("archive") || low.contains("unpack") || low.contains("hard link") || low.contains("staged build") {
        "gpui.appupdate.err.package"
    } else {
        "gpui.appupdate.failed"
    }
}

/// The words for this device's row in Settings > Network, beside the hub's own, from `status.json`, and whether Check now is offered. The app
/// draws them through its catalog by the key (`gpui.appupdate.*`) with these arguments. `when`: the last check's time as the app shows a time.
pub fn line(s: &Value, when: &str) -> (&'static str, Vec<(&'static str, String)>, bool) {
    let g = |k: &str| s[k].as_str().unwrap_or("").to_string();
    let (avail, note, when) = (g("available"), g("note"), when.to_string());
    // how the last switch ended, said until a check finds something new
    let said = [("updated", "gpui.appupdate.updated"), ("stayed", "gpui.appupdate.stayed")].into_iter()
        .find_map(|(k, key)| s["said"][k].as_str().filter(|t| !t.is_empty()).map(|t| (key, t.to_string())));
    match g("state").as_str() {
        "off" => ("gpui.appupdate.off", vec![], false),
        "guard" => ("gpui.appupdate.guard", vec![], false),
        "checking" => ("gpui.appupdate.checking", vec![], false),
        "downloading" => ("gpui.appupdate.downloading", vec![("tag", avail)], false),
        "ready" => ("gpui.appupdate.ready", vec![("tag", avail)], true),
        "failed" => (error_key(&note), vec![], true),
        _ => match (said, when.is_empty()) {
            (Some((key, tag)), true) => (key, vec![("tag", tag)], true),
            (Some((key, tag)), false) => (if key == "gpui.appupdate.updated" { "gpui.appupdate.updated.when" } else { "gpui.appupdate.stayed.when" }, vec![("tag", tag), ("when", when)], true),
            (None, false) => ("gpui.appupdate.idle", vec![("when", when)], true),
            (None, true) => ("gpui.appupdate.never", vec![], true),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> crate::tmp::TempDir { crate::tmp::dir(&format!("appkeep-{n}")) }
    fn all(_: &str) -> bool { true }
    fn none(_: &str) -> bool { false }
    fn t(tag: &str, from: &str) -> Trial { Trial { tag: tag.into(), from: from.into(), at: 1, pid: 0, nonce: String::new() } }

    #[test] fn a_staged_newer_build_is_tried_only_when_this_app_is_alone() {
        assert_eq!(start("v0.16.0", "v0.16.1", None, &[], &all, true, false, true), Start::Try("v0.16.1".into()));
        assert_eq!(start("v0.16.0", "v0.16.1", None, &[], &all, false, false, true), Start::Stay, "another app of this prefix runs: nothing moves under it");
        assert_eq!(start("v0.16.0", "v0.16.1", None, &[], &all, true, false, false), Start::Stay, "staged build does not match active channel");
        assert_eq!(start("v0.16.0", "", None, &[], &all, true, false, true), Start::Stay, "nothing staged");
        assert_eq!(start("v0.16.0", "v0.16.0", None, &[], &all, true, false, true), Start::Stay, "staged is what runs");
        assert_eq!(start("v0.16.1", "v0.16.0", None, &[], &all, true, false, true), Start::Stay, "older than this (an install by hand moved past it)");
        assert_eq!(start("v0.16.0", "v0.16.1", None, &["v0.16.1".into()], &all, true, false, true), Start::Stay, "bad here");
        assert_eq!(start("v0.16.0", "v0.16.1", None, &[], &none, true, false, true), Start::Stay, "pruned, or never whole");
        assert_eq!(start("v0.16.0", "../../x", None, &[], &all, true, false, true), Start::Stay, "no name that leaves builds/");
        assert_eq!(start("v0.15.4-nightly.20261113.1", "v0.15.4", None, &[], &all, true, false, true), Start::Try("v0.15.4".into()), "the release follows its nightlies");
    }
    #[test] fn a_trial_that_never_opened_its_window_is_failed_by_the_build_it_came_from() {
        let tr = t("v0.16.1", "v0.16.0");
        assert_eq!(start("v0.16.1", "v0.16.1", Some(&tr), &[], &all, true, false, true), Start::Stay, "the trial itself, just started");
        assert_eq!(start("v0.16.0", "v0.16.1", Some(&tr), &[], &all, true, true, true), Start::Stay, "live trial process must not be failed");
        assert_eq!(start("v0.16.0", "v0.16.1", Some(&tr), &[], &all, true, false, true), Start::Failed { tag: "v0.16.1".into(), why: "its app never opened its window".into() });
        assert_eq!(start("v0.16.0", "v0.16.1", Some(&tr), &[], &all, false, false, true), Start::Stay, "the trial may still be starting in another app: not judged");
        assert_eq!(start("v0.16.2", "v0.16.1", Some(&tr), &[], &all, true, false, true), Start::Forget, "CURRENT moved by hand meanwhile: nothing is known of the trial, nothing is marked");
    }
    #[test] fn the_order_is_the_updaters() {
        assert!(newer("v0.16.0", "v0.15.9") && newer("0.10.0", "0.9.9") && newer("v0.16.0", "v0.16.0-beta.9"));
        assert!(newer("v0.3.1-nightly.20260923.2", "v0.3.1-nightly.20260923.1") && newer("0.15.1-beta.10", "0.15.1-beta.2"));
        assert!(!newer("v0.16.0", "v0.16.0") && !newer("v0.16.0-beta.1", "v0.16.0"));
    }
    #[test] fn names_that_could_leave_the_builds_folder_are_refused() {
        for n in ["v0.16.0", "v0.16.0-nightly.20261114.1"] { assert!(name_ok(n), "{n}"); }
        for n in ["", ".stage-x", "../x", "a/b", r"a\b", "v0.16.0 ", &"v".repeat(65)] { assert!(!name_ok(n), "{n:?}"); }
    }
    #[test] fn a_trial_becomes_current_only_once_its_window_is_up() {
        let d = tmp("open"); let p: &Path = &d;
        fs::create_dir_all(p.join("builds")).unwrap(); publish(p, "v1").unwrap(); put(&dir(p).join("next"), "v2\n").unwrap();
        assert_eq!(opened(p, "v1", None), None); assert_eq!(good(p), vec!["v1"], "the build CURRENT names is good once its window opens");
        begin(p, "v2", "v1", 99, "n1").unwrap();
        assert_eq!(current(p), "v1", "a trial leaves CURRENT alone: the launchers still run the build it came from");
        assert_eq!(opened(p, "v2", Some("wrong")), None, "mismatched nonce is refused");
        assert_eq!(opened(p, "v2", Some("n1")), Some("v2".into()));
        assert_eq!((current(p), good(p), next(p), trial(p)), ("v2".to_string(), vec!["v1".to_string(), "v2".to_string()], String::new(), None));
        assert_eq!(status(p)["said"]["updated"], "v2");

        // A newer `next = v3` staged before v2 opens must be preserved
        begin(p, "v2.5", "v2", 100, "n2").unwrap();
        put(&dir(p).join("next"), "v3\n").unwrap();
        set_status(p, |s| { s["state"] = json!("ready"); s["available"] = json!("v3"); });
        assert_eq!(opened(p, "v2.5", Some("n2")), Some("v2.5".into()));
        assert_eq!(current(p), "v2.5");
        assert_eq!(next(p), "v3");
        assert_eq!(status(p)["state"], "ready");
        assert_eq!(status(p)["available"], "v3");
        assert_eq!(status(p)["said"]["updated"], "v2.5");
    }
    #[test] fn opened_rejects_stale_current_or_bad_build_and_nonce_forget_is_scoped() {
        let d = tmp("stale"); let p: &Path = &d;
        publish(p, "v2").unwrap();
        begin(p, "v3", "v1", 10, "n1").unwrap();
        forget_if_nonce(p, "other");
        assert!(trial(p).is_some(), "wrong nonce does not clear trial");
        assert_eq!(opened(p, "v3", Some("n1")), None, "CURRENT no longer matches trial.from");
        assert!(trial(p).is_none());
        assert_eq!(current(p), "v2");

        mark_bad(p, "v4", "broken");
        begin(p, "v4", "v2", 11, "n2").unwrap();
        assert_eq!(opened(p, "v4", Some("n2")), None, "build marked bad cannot promote");
        assert_eq!(current(p), "v2");
        forget_if_nonce(p, "n2");
        assert!(trial(p).is_none());

        set_status(p, |s| s["state"] = json!("downloading"));
        fail_stale_busy(p);
        assert_eq!(status(p)["state"], "failed");

        // Malformed CURRENT/trial/next/good fail closed without mutating state
        let bad_good = "v1\r\r\n";
        put(&dir(p).join("good"), bad_good).unwrap();
        add_good(p, "v2");
        assert_eq!(fs::read_to_string(dir(p).join("good")).unwrap(), bad_good, "add_good preserves malformed good");
        assert_eq!(opened(p, "v2", None), None);
        assert_eq!(fs::read_to_string(dir(p).join("good")).unwrap(), bad_good, "opened preserves malformed good");

        put(&dir(p).join("next"), "v2\nv3\n").unwrap();
        assert!(set_manual_next(p, "v3").is_err(), "set_manual_next refuses overwriting malformed next");
        assert_eq!(fs::read_to_string(dir(p).join("next")).unwrap(), "v2\nv3\n");
        assert!(!dir(p).join("manual").exists());
        let _ = fs::remove_file(dir(p).join("next"));

        put(&p.join("CURRENT"), "v2\n\n").unwrap();
        assert!(begin_if_no_live_trial(p, "v3", "v2", 12, "n3").is_err(), "malformed CURRENT refuses manual trial");
        begin(p, "v3", "v2", 12, "n3").unwrap();
        assert_eq!(opened(p, "v3", Some("n3")), None);
        assert!(trial(p).is_some(), "malformed CURRENT must not drop active trial");
        publish(p, "v2").unwrap();
        put(&dir(p).join("trial"), r#"{"tag":"v3"}"#).unwrap();
        assert!(begin_if_no_live_trial(p, "v3", "v2", 12, "n4").is_err(), "malformed trial blocks overwriting");
        for partial in [r#"{"tag":"v3","from":"v2"}"#, r#"{"tag":"v3","from":"v2","at":1,"pid":0,"nonce":"n"}"#, r#"{"tag":"v3","from":"v2","at":1,"pid":12,"nonce":""}"#] {
            put(&dir(p).join("trial"), partial).unwrap();
            assert!(try_read_trial_unlocked(p).is_err(), "{partial} must fail strict trial validation");
            assert!(begin_if_no_live_trial(p, "v3", "v2", 12, "n4").is_err(), "{partial} must block overwriting");
            assert_eq!(opened(p, "v3", None), None, "{partial} must not promote even with None nonce");
            assert_eq!(opened(p, "v3", Some("")), None, "{partial} must not promote even with empty nonce");
            assert!(fs::read_to_string(dir(p).join("trial")).is_ok(), "{partial} must not be deleted by opened");
        }
    }
    #[test] fn a_failed_trial_is_marked_bad_and_never_offered_again() {
        let d = tmp("fail"); let p: &Path = &d;
        publish(p, "v1").unwrap(); put(&dir(p).join("next"), "v2\n").unwrap(); begin(p, "v2", "v1", 1, "n").unwrap();
        failed(p, "v2", "its app never opened its window");
        assert_eq!(bad(p), vec!["v2"]); assert_eq!(bad_lines(p)[0].1, "its app never opened its window");
        assert_eq!((trial(p), next(p), current(p)), (None, String::new(), "v1".to_string()));
        assert_eq!(status(p)["said"]["stayed"], "v2");
        failed(p, "v2", "again"); assert_eq!(bad_lines(p).len(), 1, "one line a build");
        assert_eq!(start("v1", "v2", None, &bad(p), &all, true, false, true), Start::Stay);
    }
    #[test] fn pruning_keeps_both_sides_of_a_switch() {
        let g: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| s.to_string()).collect();
        let p = protected(&g, "n", Some(&t("t", "f")), "r");
        for x in ["b", "c", "d", "n", "t", "f", "r"] { assert!(p.contains(&x.to_string()), "{x}"); } assert!(!p.contains(&"a".to_string()));
        assert_eq!(protected(&[], "", None, ""), Vec::<String>::new());
    }
    #[test] fn the_line_says_what_this_device_s_app_is_doing() {
        let a = |v: &[(&'static str, &str)]| v.iter().map(|(k, x)| (*k, x.to_string())).collect::<Vec<_>>();
        assert_eq!(line(&json!({}), ""), ("gpui.appupdate.never", vec![], true));
        assert_eq!(line(&json!({"state": "idle"}), "14:05"), ("gpui.appupdate.idle", a(&[("when", "14:05")]), true));
        assert_eq!(line(&json!({"state": "ready", "available": "v0.16.1"}), "14:05"), ("gpui.appupdate.ready", a(&[("tag", "v0.16.1")]), true));
        assert!(!line(&json!({"state": "checking"}), "").2); assert_eq!(line(&json!({"state": "downloading", "available": "v2"}), "").1, a(&[("tag", "v2")]));
        assert_eq!(line(&json!({"state": "failed", "note": "could not reach the release feed"}), ""), ("gpui.appupdate.err.network", vec![], true));
        assert_eq!(line(&json!({"state": "failed", "note": "manifest signature invalid"}), ""), ("gpui.appupdate.err.verify", vec![], true));
        assert_eq!(line(&json!({"state": "failed", "note": "no build is made for this machine"}), ""), ("gpui.appupdate.err.platform", vec![], true));
        assert_eq!(line(&json!({"state": "failed", "note": "the release has no desktop app"}), ""), ("gpui.appupdate.err.package", vec![], true));
        assert_eq!(line(&json!({"state": "failed"}), ""), ("gpui.appupdate.failed", vec![], true));
        assert_eq!(line(&json!({"state": "idle", "note": "an old error", "said": {"updated": "v2"}}), "14:05"), ("gpui.appupdate.updated.when", a(&[("tag", "v2"), ("when", "14:05")]), true),
            "how the last switch ended stays beside the check; the keeper's old error does not");
        assert_eq!(line(&json!({"said": {"updated": "v2"}}), ""), ("gpui.appupdate.updated", a(&[("tag", "v2")]), true));
        assert_eq!(line(&json!({"said": {"stayed": "v3"}}), ""), ("gpui.appupdate.stayed", a(&[("tag", "v3")]), true));
        assert_eq!(line(&json!({"said": {"stayed": "v3"}}), "14:05"), ("gpui.appupdate.stayed.when", a(&[("tag", "v3"), ("when", "14:05")]), true));
        assert_eq!(line(&json!({"state": "off"}), ""), ("gpui.appupdate.off", vec![], false));
        assert!(!line(&json!({"state": "guard"}), "").2, "a guard here updates it");
    }
    #[test] fn guard_runs_detects_lock_and_verifies_split_backplane_healthz_for_legacy_pins() {
        let d = tmp("pins"); let p: &Path = &d; assert!(!guard_runs(p));
        let f = fs::OpenOptions::new().create(true).write(true).truncate(false).open(p.join(".guard.lock")).unwrap();
        f.lock_shared().unwrap();
        assert!(guard_runs(p), ".guard.lock detects running guard even with no port");
        drop(f);
        assert!(!guard_runs(p));
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            let gl = p.join(".guard.lock");
            fs::set_permissions(&gl, fs::Permissions::from_mode(0o444)).unwrap();
            if fs::OpenOptions::new().write(true).open(&gl).is_err() {
                assert!(guard_runs(p), "unopenable .guard.lock fails closed");
            }
            fs::set_permissions(&gl, fs::Permissions::from_mode(0o644)).unwrap();
            let _ = fs::remove_file(&gl);
        }

        // Non-Backplane listener on a stale .pins port must not count as a guard
        let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || { if let Ok((mut s, _)) = l.accept() { let _ = s.write_all(b"SSH-2.0-OpenSSH\r\n"); } });
        fs::create_dir_all(p.join(".pins")).unwrap();
        fs::write(p.join(".pins").join(port.to_string()), "v1\n").unwrap();
        assert!(!guard_runs(p));
        let _ = h.join();

        // Split Backplane /healthz response across multiple chunks still counts as a guard
        let l2 = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port2 = l2.local_addr().unwrap().port();
        let h2 = std::thread::spawn(move || {
            if let Ok((mut s, _)) = l2.accept() {
                let mut req = [0u8; 256]; let _ = s.read(&mut req);
                let _ = s.write_all(b"HTTP/1.0 200 OK\r\n");
                std::thread::sleep(std::time::Duration::from_millis(20));
                let _ = s.write_all(b"\r\n{\"control\":1}");
            }
        });
        fs::write(p.join(".pins").join(port2.to_string()), "v1\n").unwrap();
        assert!(guard_runs(p));
        let _ = h2.join();
    }
    #[test] fn decide_and_begin_atomically_checks_next_guard_and_commits_trial() {
        let d = tmp("decide"); let p: &Path = &d;
        publish(p, "v1").unwrap();
        fs::create_dir_all(p.join("builds/v2")).unwrap();
        let app = app_of(p, "v2");
        fs::write(&app, b"x").unwrap();
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(&app, fs::Permissions::from_mode(0o755)).unwrap(); }

        // No `next` yet -> Stay
        assert_eq!(decide_and_begin(p, "v1", "stable", true, 123, "n1"), Start::Stay);
        assert!(trial(p).is_none());

        // `next` present, but guard owns prefix -> Stay
        commit_next(p, "v2").unwrap();
        let gl = fs::OpenOptions::new().create(true).write(true).truncate(false).open(p.join(".guard.lock")).unwrap();
        gl.lock_shared().unwrap();
        assert_eq!(decide_and_begin(p, "v1", "stable", true, 123, "n1"), Start::Stay);
        assert!(trial(p).is_none());
        drop(gl);

        let dead_pid = 2_000_000_001u32;
        assert!(!pid_alive(dead_pid));

        // `CURRENT` mismatch -> Stay even when `trial` exists (neither Forget nor Failed mutates state!)
        publish(p, "v1.5").unwrap();
        assert_eq!(decide_and_begin(p, "v1", "stable", true, 123, "n1"), Start::Stay);
        assert!(trial(p).is_none());
        begin(p, "v2", "v1", dead_pid, "stale").unwrap();
        assert_eq!(decide_and_begin(p, "v1", "stable", true, 123, "n1"), Start::Stay);
        assert!(trial(p).is_some(), "stale launch must not mutate trial");
        assert!(bad(p).is_empty(), "stale launch must not mark BAD");
        let _ = fs::remove_file(dir(p).join("trial"));
        // Malformed or missing `CURRENT` -> Stay (including trailing blank lines)
        put(&p.join("CURRENT"), "v1\n\n").unwrap();
        assert_eq!(decide_and_begin(p, "v1", "stable", true, 123, "n1"), Start::Stay);
        put(&p.join("CURRENT"), "v1\nv2\n").unwrap();
        assert_eq!(decide_and_begin(p, "v1", "stable", true, 123, "n1"), Start::Stay);
        put(&p.join("CURRENT"), "../bad\n").unwrap();
        assert_eq!(decide_and_begin(p, "v1", "stable", true, 123, "n1"), Start::Stay);
        let _ = fs::remove_file(p.join("CURRENT"));
        assert_eq!(decide_and_begin(p, "v1", "stable", true, 123, "n1"), Start::Stay);
        publish(p, "v1").unwrap();
        // Malformed `next` (two tags or trailing blank line) -> fail closed without mutating existing trial or BAD
        put(&dir(p).join("next"), "v2\nv3\n").unwrap();
        begin(p, "v2", "v1", dead_pid, "keep").unwrap();
        assert_eq!(decide_and_begin(p, "v1", "stable", true, 123, "n1"), Start::Stay);
        assert!(trial(p).is_some() && bad(p).is_empty(), "malformed next must fail closed before judging trial");
        put(&dir(p).join("next"), "v2\n\n").unwrap();
        assert_eq!(decide_and_begin(p, "v1", "stable", true, 123, "n1"), Start::Stay);
        let _ = fs::remove_file(dir(p).join("trial"));
        assert!(commit_next(p, "v2").is_err(), "commit_next must refuse overwriting malformed next");
        let _ = fs::remove_file(dir(p).join("next"));
        commit_next(p, "v2").unwrap();
        // Incomplete or malformed `trial` -> fail closed (Err(()) / Stay)
        for partial in [r#"{"tag":"v2"}"#, r#"{"tag":"v2","from":"v1"}"#, r#"{"tag":"v2","from":"v1","at":1,"pid":0,"nonce":"n"}"#] {
            put(&dir(p).join("trial"), partial).unwrap();
            assert!(try_read_trial_unlocked(p).is_err(), "{partial}");
            assert_eq!(decide_and_begin(p, "v1", "stable", true, 123, "n1"), Start::Stay);
        }
        let _ = fs::remove_file(dir(p).join("trial"));

        // Valid preconditions -> atomically writes trial and returns Try("v2")
        assert_eq!(decide_and_begin(p, "v1", "stable", true, 123, "n1"), Start::Try("v2".into()));
        let t = trial(p).unwrap();
        assert_eq!((t.tag.as_str(), t.from.as_str(), t.pid, t.nonce.as_str()), ("v2", "v1", 123, "n1"));

        // Dead trial (dead_pid) from "v1" is marked bad atomically and returns Failed
        begin(p, "v2", "v1", dead_pid, "n2").unwrap();
        let res = decide_and_begin(p, "v1", "stable", true, 123, "n3");
        assert!(matches!(res, Start::Failed { ref tag, .. } if tag == "v2"), "{res:?}");
        assert!(bad(p).contains(&"v2".to_string()));
        assert!(trial(p).is_none());
        assert_eq!(next(p), "");
    }
}

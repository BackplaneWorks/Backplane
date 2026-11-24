//! Memory scopes (Linux): every agent process and every `work_run` command runs in a transient systemd scope of its own in `bpagent.slice`,
//! capped (`MemoryMax`, `BACKPLANE_SCOPE_MEM_MAX`, 40% of RAM by default; the slice holds them all under `BACKPLANE_SLICE_MEM_MAX`, 75%), with
//! `OOMPolicy=continue`: when something an agent started eats memory, the kernel's cgroup OOM killer kills that one process inside its scope, and
//! the agent, its other commands, the hub and every other thread go on. Before this, agents ran in the hub's own scope (OOMPolicy=stop), so the
//! kernel killing one runaway test client made systemd kill the hub and every agent with it (Nov 13 04:42: a 45 GB client). What they start also
//! gets a higher `oom_score_adj` than the hub's, so a machine-wide OOM picks it first. The thread is told when one of its processes is killed.
//!
//! Off when there is no user systemd answering (no `systemd-run`, no user bus, a container or WSL without one), on a copy, in tests, or with
//! `BACKPLANE_NO_SCOPE`: then commands start as before, with one log line. A scope gets only the properties a probe scope here took
//! (`scopeprobe.rs`: a scope takes OOMPolicy from 253 or a late 252 point release, ManagedOOMPreference from 248), and a scoped launch that still fails is
//! started again unscoped, and scopes stay off for the session (`spawn`). macOS has no equivalent (no cgroups; a memory cap per process tree would be a sampler that kills, not done); Windows' is a Job
//! Object memory limit with the same caps (`cap_job`; allocations past it fail, nothing is killed or told). KiCad and the browser helper have
//! scopes of their own (`wrap_helper`).
use std::sync::OnceLock;

/// The slice every scope goes in: `bpagent.slice`, or BACKPLANE_SCOPE_SLICE (a test hub's own, so it never sets the person's slice's cap).
pub fn slice() -> String { std::env::var("BACKPLANE_SCOPE_SLICE").ok().filter(|v| v.ends_with(".slice") && v.len() > 6 && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')).unwrap_or_else(|| "bpagent.slice".into()) }
static ON: OnceLock<bool> = OnceLock::new();

/// A scope's cap and the slice's (systemd's syntax: a percentage of RAM or a size).
pub fn scope_max() -> String {
    let o = AGENT_CAP.read().map(|g| g.clone()).unwrap_or_default(); if !o.is_empty() { return o }
    std::env::var("BACKPLANE_SCOPE_MEM_MAX").ok().filter(|v| valid_size(v)).unwrap_or_else(|| "40%".into())
}
/// The person's own cap for a thread's agent and commands (the setting `memory.agent.max`, set by a memory card's Raise limit): over
/// BACKPLANE_SCOPE_MEM_MAX and the 40% default; "" goes back to them. Takes effect for the next process started.
static AGENT_CAP: std::sync::RwLock<String> = std::sync::RwLock::new(String::new());
pub fn set_agent_cap(v: &str) { let v = v.trim(); if v.is_empty() || valid_size(v) { if let Ok(mut g) = AGENT_CAP.write() { *g = v.to_string() } } }
/// The machine's RAM in bytes (Linux: /proc/meminfo; elsewhere none).
pub fn total_ram() -> Option<u64> {
    #[cfg(windows)] { return crate::plat::proc::total_ram() }
    #[cfg(not(windows))] { std::fs::read_to_string("/proc/meminfo").ok()?.lines().find_map(|l| l.strip_prefix("MemTotal:")).and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok()).map(|k| k << 10) }
}
/// Raise limit: the next cap after `cur` (systemd's syntax), never past the slice's (`top`), or None when it is already there. A percentage
/// grows by 15 points, a size by half again (rounded to a whole GB, or MB under 2 GB), so two presses reach the slice's cap from the default.
pub fn raised(cur: &str, top: &str, total: u64) -> Option<String> {
    let (c, t) = (cap_bytes(cur, total)?, cap_bytes(top, total)?);
    if c >= t { return None }
    if cur.ends_with('%') && top.ends_with('%') {
        let (cp, tp): (u64, u64) = (cur.trim_end_matches('%').parse().ok()?, top.trim_end_matches('%').parse().ok()?);
        return Some(format!("{}%", (cp + 15).min(tp)))
    }
    let n = (c + c / 2).min(t);
    Some(if n == t && !top.ends_with('%') { top.to_string() } else if n >= 2 << 30 { format!("{}G", (n + (1 << 29)) >> 30) } else { format!("{}M", n >> 20) })
}
/// A size for people: "38.4 GB", "400 MB".
pub fn human(bytes: u64) -> String { crate::agents::halt_view::human(bytes) }
pub fn slice_max() -> String { std::env::var("BACKPLANE_SLICE_MEM_MAX").ok().filter(|v| valid_size(v)).unwrap_or_else(|| "75%".into()) }
/// "40%", "24G", "512M", "1073741824": what MemoryMax takes (never anything that could be another argument).
pub fn valid_size(v: &str) -> bool {
    let d = v.trim_end_matches(['%', 'K', 'M', 'G', 'T']);
    !d.is_empty() && d.len() + 1 >= v.len() && d.chars().all(|c| c.is_ascii_digit())
}

/// Whether scopes are used: decided once, by `init` (false until then), and off for the rest of the session once a scoped launch failed (`give_up`).
pub fn on() -> bool { *ON.get().unwrap_or(&false) && !OFF.load(std::sync::atomic::Ordering::Relaxed) }
static OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// The properties this machine's systemd took in a scope (`scopeprobe::probe`): a subset of `WANTED`.
static PROPS: OnceLock<Vec<String>> = OnceLock::new();
/// Every scope's properties but its cap, each used only where systemd takes it (a scope's OOMPolicy from 253 or a late 252 point release, ManagedOOMPreference from 248).
const WANTED: [&str; 3] = ["MemorySwapMax=1G", "OOMPolicy=continue", "ManagedOOMPreference=none"];
fn wanted() -> Vec<String> { std::iter::once(format!("MemoryMax={}", scope_max())).chain(WANTED.iter().map(|s| s.to_string())).collect() }
/// The properties a scope gets here: those the probe found (or every one, before it ran: only tests).
fn props() -> Vec<String> { PROPS.get().cloned().unwrap_or_else(wanted) }
/// A scoped launch failed for the scope (systemd-run ended before it started the command): every later one starts unscoped, said once.
pub fn give_up(why: &str) {
    if !OFF.swap(true, std::sync::atomic::Ordering::Relaxed) { eprintln!("memscope: a scoped launch failed ({why}): agents and jobs start without scopes for the rest of this session") }
}

/// At the hub's start (blocking: run it off the runtime): probe a scope once, with the properties it should have, then cap the slice.
pub fn init() {
    let live = cfg!(target_os = "linux") && !cfg!(test);
    let s = if !live { None } else if crate::copy::on() { None } else { Some(crate::scopeprobe::probe(&|p| crate::proc::std_cmd(crate::proc::Kind::Local, p), &[format!("--slice={}", slice())], &wanted())) };
    let on = s.as_ref().map_or(false, |s| s.ok);
    if let Some(s) = &s { let _ = PROPS.set(s.props.iter().filter(|p| !p.starts_with("MemoryMax=")).cloned().collect()); }
    let capped = s.as_ref().map_or(false, |s| s.props.iter().any(|p| p.starts_with("MemoryMax=")));
    if on {
        let mut sp = vec!["--user".to_string(), "set-property".into(), "--runtime".into(), slice(), format!("MemoryMax={}", slice_max())];
        if props().iter().any(|p| p.starts_with("MemorySwapMax=")) { sp.push("MemorySwapMax=2G".into()) }
        let st = crate::proc::std_cmd(crate::proc::Kind::Local, "systemctl").args(&sp).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        eprintln!("memscope: agents and jobs run in scopes of their own in {} (each up to {}, all up to {}{}; {})", slice(), scope_max(), slice_max(),
            if st.map_or(false, |s| s.success()) { "" } else { "; the slice's cap was not set" }, crate::scopeprobe::describe(s.as_ref().unwrap(), &wanted()));
    } else if live {
        let why = match &s { Some(s) => crate::scopeprobe::describe(s, &wanted()), None => "a copy".into() };
        eprintln!("memscope: off ({why}): agents and jobs start without scopes of their own");
    }
    let _ = CAPPED.set(capped);
    let _ = ON.set(on);
}
/// Whether a scope can be made here: the probe found it so (and no scoped launch has failed since), or the probe has not run (a command-line
/// use of the hub's code: try it, a failure falls back).
pub fn usable() -> bool { ON.get().is_none() || on() }
/// Whether this machine's systemd takes `prop` ("ManagedOOMPreference=none") in a scope; true before the probe ran.
pub fn takes(prop: &str) -> bool {
    let key = prop.split('=').next().unwrap_or(prop);
    if key == "MemoryMax" { return capped() }
    PROPS.get().map_or(true, |v| v.iter().any(|p| p.split('=').next() == Some(key)))
}
/// Whether systemd took MemoryMax in a scope here (else scopes go on, uncapped).
static CAPPED: OnceLock<bool> = OnceLock::new();
fn capped() -> bool { *CAPPED.get().unwrap_or(&true) }

/// Where a scoped launch's systemd-run would have said why it failed: its stderr pipe (read, and put back when it was not that), the file its
/// stderr went to (a job's output), or nowhere (`Assume`: a long-lived helper ending at once with systemd-run's exit code 1 is taken as one).
pub enum Evidence<'a> { Stderr, File(&'a std::path::Path), Assume }
/// Starts `make(wrap)`: in a scope when `wrap` is Some, and when systemd-run ended before it started the command saying it could not make the
/// scope (an unknown property, the user manager gone: `scopeprobe::scope_failure`), the same launch again unscoped, once, and scopes off for
/// the session (`give_up`). A command that ran and ended at once is never started twice. `make` builds the whole command each time (fresh
/// pipes). Blocks until the scope is made (a few ms; at most 3 s).
pub fn spawn(wrap: Option<Vec<String>>, ev: Evidence, mut make: impl FnMut(Option<Vec<String>>) -> tokio::process::Command) -> std::io::Result<tokio::process::Child> {
    let scoped = wrap.is_some();
    let mut child = make(wrap).spawn()?;
    if !scoped { return Ok(child) }
    let pid = child.id().unwrap_or(0);
    let mut code = None;
    let went = crate::scopeprobe::wait_exec(pid, || match child.try_wait() { Ok(Some(s)) => { code = s.code(); true } Ok(None) => false, Err(_) => true }, std::time::Duration::from_secs(3));
    if went != crate::scopeprobe::Launch::Failed || code != Some(1) { return Ok(child) }
    let said = match ev {
        Evidence::Stderr => match child.stderr.take() { Some(e) => { let (b, back) = take_back(e); child.stderr = back; String::from_utf8_lossy(&b).into_owned() } None => String::new() },
        Evidence::File(p) => std::fs::read_to_string(p).unwrap_or_default(),
        Evidence::Assume => "Failed to start transient scope (systemd-run exited 1)".into(),
    };
    if !crate::scopeprobe::scope_failure(&said) { return Ok(child) }
    give_up(said.lines().find(|l| crate::scopeprobe::scope_failure(l)).unwrap_or("systemd-run exited 1").trim());
    make(None).spawn()
}
/// Elsewhere (macOS and Windows: no systemd scopes, so no systemd-run to read) the stderr is handed back untouched. `init` turns scopes on
/// only on Linux, so `spawn` returns before it reads any evidence there; even if it got here, nothing is read and nothing waits.
#[cfg(not(target_os = "linux"))]
fn take_back(e: tokio::process::ChildStderr) -> (Vec<u8>, Option<tokio::process::ChildStderr>) { (Vec::new(), Some(e)) }
/// What an exited child wrote to its stderr pipe, and a pipe that gives the same bytes again to whoever reads it next.
#[cfg(target_os = "linux")]
fn take_back(e: tokio::process::ChildStderr) -> (Vec<u8>, Option<tokio::process::ChildStderr>) {
    use std::io::{Read, Write};
    let Ok(fd) = e.into_owned_fd() else { return (vec![], None) };
    let mut f = std::fs::File::from(fd); let mut b = Vec::new(); let mut buf = [0u8; 8192];
    loop { match f.read(&mut buf) { Ok(0) => break, Ok(n) => b.extend_from_slice(&buf[..n]), Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue, Err(_) => break } } // non-blocking: what is there
    let mut fds = [0i32; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 { return (b, None) }
    use std::os::fd::FromRawFd;
    let (r, mut w) = unsafe { (std::os::fd::OwnedFd::from_raw_fd(fds[0]), std::fs::File::from_raw_fd(fds[1])) };
    let bytes = b.clone(); std::thread::spawn(move || { let _ = w.write_all(&bytes); });
    let back = tokio::process::ChildStderr::from_std(std::process::ChildStderr::from(r)).ok();
    (b, back)
}

/// systemd-run's arguments that start what follows them in a new scope (`what`: "agent" or "work", in its unit's name), or None when scopes are off.
pub fn wrap(what: &str) -> Option<Vec<String>> {
    if !on() { return None }
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    Some(args_with(what, std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed), capped().then(scope_max).as_deref(), &slice(), &props()))
}
/// The hub's own long-lived helpers, each in a scope of its own outside the hub's (a kill there never stops the hub, and never takes the hub's
/// other children with it): the embedded KiCad (`kicad`: in bpkicad.slice, BACKPLANE_KICAD_MEM_MAX, 12G by default: lane A's real boards peak at
/// 4.1-4.5 GB per KiCad, so about 2.7 times that) and the browser helper (`browser`: in bpagent.slice, BACKPLANE_BROWSER_MEM_MAX, 8G).
pub fn wrap_helper(what: &str) -> Option<Vec<String>> {
    if !on() { return None }
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let (slice, max) = helper_cap(what);
    Some(args_with(what, std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed), capped().then_some(max.as_str()), &slice, &props()))
}
pub fn helper_cap(what: &str) -> (String, String) {
    let env = |k: &str, d: &str| std::env::var(k).ok().filter(|v| valid_size(v)).unwrap_or_else(|| d.into());
    match what { "kicad" => ("bpkicad.slice".into(), env("BACKPLANE_KICAD_MEM_MAX", "12G")), _ => (slice(), env("BACKPLANE_BROWSER_MEM_MAX", "8G")) }
}
/// Windows: the same caps as a Job Object memory limit (bytes), from systemd's syntax and the machine's RAM: "40%" of 64 GiB, "12G", "512M".
pub fn cap_bytes(size: &str, total: u64) -> Option<u64> {
    if !valid_size(size) { return None }
    let (n, unit) = size.split_at(size.len() - size.chars().last().map_or(0, |c| if c.is_ascii_digit() { 0 } else { 1 }));
    let n: u64 = n.parse().ok()?;
    Some(match unit { "%" => ((total as u128 * n.min(100) as u128) / 100) as u64, "K" => n << 10, "M" => n << 20, "G" => n << 30, "T" => n << 40, _ => n })
}
/// The cap for an agent or job ("agent", "work") or a helper ("kicad", "browser"), in bytes for a machine with `total` bytes of RAM.
pub fn cap_for(what: &str, total: u64) -> Option<u64> {
    let size = match what { "kicad" | "browser" => helper_cap(what).1, _ => scope_max() };
    cap_bytes(&size, total)
}
/// Windows: what an agent, its job or a helper started is held to its cap by its Job Object (JOB_OBJECT_LIMIT_JOB_MEMORY): past it, the job's
/// allocations fail, so the runaway fails alone (Windows does not kill on a job's memory limit; the process usually ends on its failed allocation)
/// and the hub and the other agents go on. Off with BACKPLANE_NO_SCOPE.
pub fn cap_job(pid: u32, what: &str) {
    #[cfg(windows)] {
        if pid == 0 || std::env::var_os("BACKPLANE_NO_SCOPE").is_some() || crate::copy::on() { return }
        if let Some(b) = crate::plat::proc::total_ram().and_then(|t| cap_for(what, t)) { crate::plat::proc::cap_job_memory(pid, b) }
    }
    #[cfg(not(windows))] { let _ = (pid, what); }
}
/// A helper's scope logged when the kernel kills a process in it for memory (the hub log; the KiCad pane's own "KiCad closed" handling follows).
pub fn watch_helper(pid: u32, what: &'static str) { watch_as(pid, Some(what), move |w| eprintln!("memscope: {w}")) }
/// systemd-run's arguments for a scope with every property (a current systemd).
pub fn args(what: &str, hub: u32, n: u64, max: &str, slice: &str) -> Vec<String> { args_with(what, hub, n, Some(max), slice, &WANTED.map(String::from)) }
/// ... with only `props` (those this systemd takes) and the cap `max` when it takes MemoryMax.
pub fn args_with(what: &str, hub: u32, n: u64, max: Option<&str>, slice: &str, props: &[String]) -> Vec<String> {
    let mut a: Vec<String> = ["--user", "--scope", "-q", "--collect"].iter().map(|s| s.to_string()).collect();
    a.push(format!("--slice={slice}")); a.push(format!("--unit=bp-{what}-{hub}-{n}"));
    if let Some(m) = max { a.push("-p".into()); a.push(format!("MemoryMax={m}")); }
    a.extend(crate::scopeprobe::p_args(props)); a.push("--".into()); a
}

/// What runs in a scope gets the hub's own `oom_score_adj` plus 300 (at most 1000): a machine-wide OOM takes it before the hub or the window.
/// Raising it needs no privilege. Linux only.
pub fn raise_oom(c: &mut tokio::process::Command) {
    #[cfg(target_os = "linux")] {
        let own: i32 = std::fs::read_to_string("/proc/self/oom_score_adj").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        let v = format!("{}\0", (own + 300).min(1000)); let path = b"/proc/self/oom_score_adj\0";
        unsafe { c.pre_exec(move || {
            let fd = libc::open(path.as_ptr() as *const libc::c_char, libc::O_WRONLY);
            if fd >= 0 { let b = v.as_bytes(); let _ = libc::write(fd, b.as_ptr() as *const libc::c_void, b.len() - 1); libc::close(fd); }
            Ok(())
        }); }
    }
    #[cfg(not(target_os = "linux"))] { let _ = c; }
}

/// What a killed process was: its name, pid and size at the last look.
#[derive(Clone, Debug, PartialEq)]
pub struct Big { pub name: String, pub pid: u32, pub mb: u64 }
/// Why the kernel killed it: the scope's own cap (`MemoryMax`), the cap every agent shares (the slice's), or the whole machine out of memory.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Why { Cap, Shared, Machine }
impl Why { pub fn key(self) -> &'static str { match self { Why::Cap => "cap", Why::Shared => "shared", Why::Machine => "machine" } } }
/// A kill seen in a watched scope: the process (the largest at the last look), the scope's cap ("400 MB"; its bytes), why, and whether the
/// watched process itself died (`whole`: the agent, not something it started).
#[derive(Clone, Debug, PartialEq)]
pub struct Kill { pub big: Option<Big>, pub cap: String, pub cap_bytes: u64, pub why: Why, pub whole: bool }
impl Kill { pub fn told(&self) -> String { told(self.big.as_ref(), &self.cap) } }
/// The kills of watched processes that died themselves, by pid, until the process's end asks (`kill_of`).
static KILLS: std::sync::Mutex<Vec<(u32, Kill)>> = std::sync::Mutex::new(Vec::new());
pub fn note_kill(pid: u32, k: Kill) { if let Ok(mut v) = KILLS.lock() { v.retain(|(p, _)| *p != pid); v.push((pid, k)); let n = v.len(); if n > 64 { v.drain(..n - 64); } } }
/// The kill that ended `pid`, taken (once).
pub fn kill_of(pid: u32) -> Option<Kill> { let mut v = KILLS.lock().ok()?; let i = v.iter().position(|(p, _)| *p == pid)?; Some(v.remove(i).1) }
/// `oom N` from memory.events: how often the cgroup's own limit was hit (not `oom_kill`, which counts a kill of any OOM).
pub fn ooms(events: &str) -> Option<u64> { events.lines().find_map(|l| l.strip_prefix("oom ")).and_then(|n| n.trim().parse().ok()) }
/// Why, from the scope's and the slice's `oom` counts before and after.
pub fn why(scope: (u64, u64), slice: (u64, u64)) -> Why { if scope.1 > scope.0 { Why::Cap } else if slice.1 > slice.0 { Why::Shared } else { Why::Machine } }
/// The words for the thread: what was killed, the cap it hit, and that nothing else was.
pub fn told(big: Option<&Big>, cap: &str) -> String { told_as(None, big, cap) }
/// The same for a helper of the hub's (`helper`: "KiCad", "the browser helper"): what it was, its cap, that the hub goes on.
pub fn told_as(helper: Option<&str>, big: Option<&Big>, cap: &str) -> String {
    let what = match big { Some(b) => format!("{} (pid {}, about {} MB)", b.name, b.pid, b.mb), None => "a process".into() };
    if let Some(h) = helper { return format!("{h} killed for memory: {what} reached {h}'s memory cap ({cap}) and the kernel killed it. Only {h} was killed: the hub and every thread go on.") }
    format!("Killed for memory: {what}, started by this thread, reached the memory cap of this thread's commands ({cap}) and the kernel killed it. \
             Only that process was killed: this thread's agent, the hub and other threads go on. Look for a leak or an unbounded queue in it before running it again.")
}
/// `oom_kill N` from a cgroup's memory.events.
pub fn kills(events: &str) -> Option<u64> { events.lines().find_map(|l| l.strip_prefix("oom_kill ")).and_then(|n| n.trim().parse().ok()) }

/// Watches the scope `pid` runs in: each time the kernel kills a process there for memory, `tell` gets the words (`told`). Ends with the scope.
pub fn watch(pid: u32, tell: impl Fn(Kill) + Send + 'static) { watch_k(pid, tell) }
fn watch_as(pid: u32, helper: Option<&'static str>, tell: impl Fn(String) + Send + 'static) { watch_k(pid, move |k| tell(told_as(helper, k.big.as_ref(), &k.cap))) }
fn watch_k(pid: u32, tell: impl Fn(Kill) + Send + 'static) {
    if !on() || pid == 0 { return }
    // a scope this hub made (bp-<what>-<hub pid>-<n>): until systemd-run has moved itself, the process is still in its parent's cgroup, which
    // may be another hub's scope (a test hub started from an agent's job)
    let me = format!("-{}-", std::process::id());
    tokio::spawn(async move {
        let mut dir = None;
        for _ in 0..250 { // systemd-run moves itself into the scope before it starts the command
            if let Ok(cg) = std::fs::read_to_string(format!("/proc/{pid}/cgroup")) { if let Some(p) = cg.lines().find_map(|l| l.strip_prefix("0::")).filter(|p| p.rsplit('/').next().map_or(false, |s| s.starts_with("bp-") && s.ends_with(".scope") && s.contains(&me))) { dir = Some(std::path::PathBuf::from(format!("/sys/fs/cgroup{}", p.trim()))); break } }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let dbg = std::env::var_os("BACKPLANE_MEMSCOPE_DEBUG").is_some();
        let Some(dir) = dir else { if dbg { eprintln!("memscope: pid {pid}: no scope found") } return };
        if dbg { eprintln!("memscope: pid {pid} in {}", dir.display()) }
        let ev = dir.join("memory.events");
        let mut seen = 0; // a new scope: any kill counted there already is one to tell (a fast eater is killed before this first look)
        let mut big: Option<Big> = None;
        // the slice's count too: a scope whose last process the kernel killed is gone before its own count can be read (a lone KiCad), and the
        // slice's (hierarchical) count went up by that kill
        let pev = dir.parent().map(|p| p.join("memory.events"));
        let slice_ev = || pev.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        // whether the slice hit its own cap: its local count (memory.events counts every scope in it too, a scope's own cap included)
        let plocal = dir.parent().map(|p| p.join("memory.events.local"));
        let slice_oom = || plocal.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| ooms(&s)).unwrap_or(0);
        let s0 = slice_ev(); let (slice0, slice_oom0) = (kills(&s0), slice_oom());
        let (mut cap, mut cap_b, mut oom_seen) = (String::new(), 0u64, 0u64);
        loop {
            let evs = std::fs::read_to_string(&ev).ok();
            let Some(k) = evs.as_deref().and_then(kills) else {
                let s1 = slice_ev();
                if dbg { eprintln!("memscope: pid {pid}: scope gone (kills seen {seen}, slice {:?} -> {:?}, last biggest {:?})", slice0, kills(&s1), big) }
                // its last process killed: the scope went with it (so did the watched process); its own counts are gone, the slice's say why
                if seen == 0 && kills(&s1).zip(slice0).map_or(false, |(now, then)| now > then) && big.is_some() {
                    // its own counts went with it: the slice's own cap, or near the scope's cap at the last look (sampled every 100 ms, so 80%), or the machine
                    let w = if slice_oom() > slice_oom0 { Why::Shared } else if cap_b > 0 && big.as_ref().map_or(false, |b| (b.mb << 20) * 10 >= cap_b * 8) { Why::Cap } else { Why::Machine };
                    tell(Kill { big: big.clone(), cap: cap.clone(), cap_bytes: cap_b, why: w, whole: true });
                }
                break // the scope is gone
            };
            if k > seen {
                seen = k;
                // the watched process itself, or something it started: the largest one was it, or it is gone a moment later
                let mut whole = big.as_ref().map_or(false, |b| b.pid == pid);
                for _ in 0..6 { if whole || !alive(pid) { whole = true; break } tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
                let o = evs.as_deref().and_then(ooms).unwrap_or(0); let so = slice_oom();
                let w = why((oom_seen, o), (slice_oom0, so)); oom_seen = o;
                tell(Kill { big: big.clone(), cap: cap.clone(), cap_bytes: cap_b, why: w, whole });
            }
            if let Some(b) = std::fs::read_to_string(dir.join("memory.max")).ok().and_then(|s| s.trim().parse::<u64>().ok()) { cap = human(b); cap_b = b } else if cap.is_empty() { cap = scope_max() }
            big = biggest(&dir).or(big);
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });
}
/// Whether `pid` still runs (a zombie waiting to be reaped does not).
fn alive(pid: u32) -> bool { std::fs::read_to_string(format!("/proc/{pid}/stat")).ok().and_then(|s| s.rsplit_once(')').map(|(_, r)| r.trim_start().chars().next() != Some('Z'))).unwrap_or(false) }
/// The largest process in a cgroup now.
fn biggest(dir: &std::path::Path) -> Option<Big> {
    std::fs::read_to_string(dir.join("cgroup.procs")).ok()?.lines().filter_map(|l| l.trim().parse::<u32>().ok()).filter_map(|p| {
        let st = std::fs::read_to_string(format!("/proc/{p}/status")).ok()?;
        let mb = st.lines().find_map(|l| l.strip_prefix("VmRSS:")).and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())? >> 10;
        let name = st.lines().find_map(|l| l.strip_prefix("Name:")).unwrap_or("").trim().to_string();
        Some(Big { name, pid: p, mb })
    }).max_by_key(|b| b.mb)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_scope_is_capped_and_outlives_a_kill_inside_it() {
        let a = args("work", 7, 3, "40%", "bpagent.slice");
        assert_eq!(a.last().map(String::as_str), Some("--"), "the command follows");
        for want in ["--scope", "--collect", "--slice=bpagent.slice", "--unit=bp-work-7-3", "MemoryMax=40%", "OOMPolicy=continue", "ManagedOOMPreference=none"] { assert!(a.iter().any(|x| x == want), "{want} in {a:?}") }
    }
    #[test] fn only_sizes_systemd_takes_are_used() {
        for ok in ["40%", "24G", "512M", "1073741824"] { assert!(valid_size(ok), "{ok}") }
        for bad in ["", "%", "40%%", "4 0G", "-p", "1G OOMPolicy=stop", "x"] { assert!(!valid_size(bad), "{bad}") }
    }
    #[test] fn the_kill_count_and_the_words_for_the_thread() {
        assert_eq!(kills("low 0\nhigh 0\nmax 12\noom 2\noom_kill 1\noom_group_kill 0\n"), Some(1));
        assert_eq!(kills("low 0\n"), None);
        let t = told(Some(&Big { name: "backplane-gpui".into(), pid: 42, mb: 23000 }), "24000 MB");
        assert!(t.contains("backplane-gpui (pid 42, about 23000 MB)") && t.contains("24000 MB") && t.contains("the hub and other threads go on"), "{t}");
        assert!(told(None, "40%").contains("a process"));
        let k = told_as(Some("KiCad"), Some(&Big { name: "ld-linux-x86-64".into(), pid: 7, mb: 3951 }), "12288 MB");
        assert!(k.starts_with("KiCad killed for memory: ld-linux-x86-64 (pid 7, about 3951 MB)") && k.contains("the hub and every thread go on") && !k.contains("this thread"), "{k}");
    }
    #[test] fn kicad_and_the_browser_helper_have_caps_of_their_own() {
        assert_eq!(helper_cap("kicad"), ("bpkicad.slice".to_string(), "12G".to_string()));
        assert_eq!(helper_cap("browser").1, "8G");
        assert!(wrap_helper("kicad").is_none(), "off in tests");
    }
    #[test] fn the_caps_in_bytes_for_a_job_object() {
        let g = 1u64 << 30;
        assert_eq!(cap_bytes("40%", 64 * g), Some(64 * g * 40 / 100));
        assert_eq!(cap_bytes("12G", 64 * g), Some(12 * g));
        assert_eq!(cap_bytes("512M", 0), Some(512 << 20));
        assert_eq!(cap_bytes("1073741824", 0), Some(g));
        assert_eq!(cap_bytes("150%", 10 * g), Some(10 * g), "never more than the machine");
        assert_eq!(cap_bytes("x", g), None);
        assert_eq!(cap_for("kicad", 64 * g), Some(12 * g)); assert_eq!(cap_for("browser", 64 * g), Some(8 * g)); assert_eq!(cap_for("agent", 100 * g), Some(40 * g));
    }
    /// A scope systemd would not make (an older systemd's "Unknown assignment: OOMPolicy=continue"): the same launch again unscoped, and the
    /// command runs; one that systemd made runs scoped.
    #[cfg(target_os = "linux")]
    #[tokio::test] async fn a_launch_whose_scope_fails_runs_unscoped() {
        let (_d, f) = super::probe_tests::fake("memscope", 249, &["OOMPolicy"], false); // Ubuntu 22.04's answer (#1312)
        let tries = std::sync::Arc::new(std::sync::Mutex::new(Vec::<bool>::new()));
        let run = |wrap: Option<Vec<String>>| {
            let t = tries.clone(); let f = f.clone();
            let child = spawn(wrap, Evidence::Stderr, move |w: Option<Vec<String>>| {
                t.lock().unwrap().push(w.is_some());
                let mut c = match w { Some(w) => { let mut c = tokio::process::Command::new(&f); c.args(w).arg("sh"); c } None => tokio::process::Command::new("sh") };
                c.args(["-c", "echo ran"]).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()); c
            }).unwrap();
            child
        };
        let o = run(Some(args("agent", 1, 0, "40%", "t.slice"))).wait_with_output().await.unwrap();
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "ran"); assert!(o.status.success());
        assert_eq!(*tries.lock().unwrap(), vec![true, false], "scoped, then unscoped once");
        assert!(OFF.load(std::sync::atomic::Ordering::Relaxed), "scopes off for the session");
        tries.lock().unwrap().clear();
        let o = run(Some(args_with("agent", 1, 1, Some("40%"), "t.slice", &["MemorySwapMax=1G".into()]))).wait_with_output().await.unwrap();
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "ran"); assert_eq!(*tries.lock().unwrap(), vec![true], "a scope it takes: no retry");
        // a command that fails at once by itself (exit 1, its own words) is never run twice, and its stderr is still there for the caller
        tries.lock().unwrap().clear();
        let ff = f.clone(); let t = tries.clone();
        let o = spawn(Some(args_with("agent", 1, 2, None, "t.slice", &[])), Evidence::Stderr, move |w| { t.lock().unwrap().push(w.is_some());
            let mut c = match w { Some(w) => { let mut c = tokio::process::Command::new(&ff); c.args(w).arg("sh"); c } None => tokio::process::Command::new("sh") };
            c.args(["-c", "echo not logged in >&2; exit 1"]).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()); c }).unwrap().wait_with_output().await.unwrap();
        assert_eq!(*tries.lock().unwrap(), vec![true], "run once"); assert_eq!(o.status.code(), Some(1)); assert_eq!(String::from_utf8_lossy(&o.stderr).trim(), "not logged in");
        let a = args_with("work", 7, 3, None, "s.slice", &["MemorySwapMax=1G".into()]);
        assert!(!a.iter().any(|x| x.starts_with("MemoryMax") || x.starts_with("OOMPolicy")) && a.iter().any(|x| x == "MemorySwapMax=1G"), "{a:?}");
    }
    #[test] fn scopes_are_off_in_tests() { assert!(!on() && wrap("agent").is_none()) }
}

#[cfg(all(test, target_os = "linux"))]
pub mod probe_tests {
    use crate::scopeprobe::*;
    use std::process::{Command, Stdio};
    use std::time::Duration;
    /// A stand-in systemd-run: `version` for --version, rejecting every property in `rejects` the way systemd does ("Unknown assignment"),
    /// else running what follows `--`; `no_manager`: one that cannot make any scope (no user manager). Each call's arguments are appended
    /// to `<its path>.calls` (`scopes_made`).
    /// (its folder, removed when dropped; the stand-in's path)
    pub fn fake(name: &str, version: u32, rejects: &[&str], no_manager: bool) -> (crate::plat::tmp::TempDir, std::path::PathBuf) {
        let d = crate::plat::tmp::dir(&format!("scopeprobe-{name}"));
        let p = d.join("systemd-run");
        let rej: String = rejects.iter().map(|r| format!("{r}=*) echo \"Unknown assignment: $a\" >&2; exit 1;; ")).collect();
        let body = format!("#!/bin/sh\necho \"$*\" >> \"$0.calls\"\ncase \"$1\" in --version) echo \"systemd {version} (test)\"; echo +PAM; exit 0;; esac\n{}for a; do case \"$a\" in {rej}esac; done\nwhile [ $# -gt 0 ] && [ \"$1\" != -- ]; do shift; done; shift; exec \"$@\"\n",
            if no_manager { "echo 'Failed to create bus connection: No such file or directory' >&2; exit 1\n" } else { "" });
        std::fs::write(&p, body).unwrap();
        use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        // a fork elsewhere in this test process may still hold the file open for writing a moment (ETXTBSY): run it until it runs
        for _ in 0..200 { match Command::new(&p).arg("--version").stdout(Stdio::null()).status() { Err(e) if e.raw_os_error() == Some(libc::ETXTBSY) => std::thread::sleep(Duration::from_millis(10)), _ => break } }
        (d, p)
    }
    fn w(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }
    /// The scopes a stand-in was asked to make (each call's arguments, `--version` left out).
    fn scopes_made(f: &std::path::Path) -> Vec<String> {
        std::fs::read_to_string(f.with_extension("calls")).unwrap_or_default().lines().filter(|l| !l.starts_with("--version")).map(String::from).collect()
    }
    /// #1312: systemd 249 (Ubuntu 22.04) refuses OOMPolicy for a scope, 253 takes it, and 252 takes it in some point releases only. Each gets
    /// a scope with what it takes, 249 without ever being asked for OOMPolicy, and a systemd that refuses one anyway ("Unknown assignment"
    /// where its version says it should not) is asked again without it. A current one is probed once.
    #[test] fn each_systemd_version_gets_a_scope_with_what_it_takes() {
        let want = w(&["MemoryMax=40%", "MemorySwapMax=1G", "OOMPolicy=continue", "ManagedOOMPreference=none"]);
        let no_oom = w(&["MemoryMax=40%", "MemorySwapMax=1G", "ManagedOOMPreference=none"]);
        let probe = |name: &str, v: u32, rejects: &[&str]| { let (d, f) = fake(name, v, rejects, false); let s = probe_in(&|p| Command::new(p), f.to_str().unwrap(), true, &[], &want); (s, scopes_made(&f), d) };

        let (s, made, _d) = probe("v249", 249, &["OOMPolicy"]);
        assert!(s.ok, "{s:?}"); assert_eq!(s.props, no_oom); assert_eq!(made.len(), 1, "one probe scope: {made:?}");
        assert!(!made.iter().any(|m| m.contains("OOMPolicy")), "249 is never asked for OOMPolicy: {made:?}");
        assert_eq!(describe(&s, &want), "systemd 249; without OOMPolicy=continue (not supported here)");

        let (s, made, _d) = probe("v252-0", 252, &["OOMPolicy"]); // 252.0: refuses it, so the retry leaves it out
        assert!(s.ok); assert_eq!(s.props, no_oom); assert!(made[0].contains("OOMPolicy=continue"), "{made:?}");
        assert!(!made[1].contains("-p "), "then the scope alone: {made:?}");
        assert_eq!(made.len(), 6, "all four at once, the scope alone, then each of the four alone: {made:?}");

        let (s, made, _d) = probe("v252-39", 252, &[]); // Debian 12's 252.39 takes it
        assert_eq!(s.props, want); assert_eq!(made.len(), 1);

        for v in [253, 255] {
            let (s, made, _d) = probe(&format!("v{v}"), v, &[]);
            assert!(s.ok); assert_eq!(s.props, want, "systemd {v}"); assert_eq!(made.len(), 1, "systemd {v}: one probe scope");
            assert_eq!(describe(&s, &want), format!("systemd {v}"));
        }
        let (s, _, _d) = probe("v253-odd", 253, &["OOMPolicy"]); // a build that refuses it anyway: the probe, not the version, decides
        assert!(s.ok); assert_eq!(s.props, no_oom);
    }
    #[test] fn an_older_systemd_gets_a_scope_without_the_properties_it_rejects() {
        let (_d, f) = fake("old", 239, &["OOMPolicy", "ManagedOOMPreference"], false);
        let want = w(&["MemoryMax=40%", "MemorySwapMax=1G", "OOMPolicy=continue", "ManagedOOMPreference=none"]);
        let s = probe_in(&|p| Command::new(p), f.to_str().unwrap(), true, &w(&["--slice=t.slice"]), &want);
        assert!(s.ok, "{s:?}"); assert_eq!(s.version, Some(239));
        assert_eq!(s.props, w(&["MemoryMax=40%", "MemorySwapMax=1G"]));
        assert_eq!(describe(&s, &want), "systemd 239; without OOMPolicy=continue, ManagedOOMPreference=none (not supported here)");

        let (_d, f) = fake("mid", 250, &["OOMPolicy"], false); // knows OOMPolicy for services only: never tried for a scope
        assert_eq!(probe_in(&|p| Command::new(p), f.to_str().unwrap(), true, &[], &want).props, w(&["MemoryMax=40%", "MemorySwapMax=1G", "ManagedOOMPreference=none"]));
        let (_d, f) = fake("new", 255, &[], false);
        let s = probe_in(&|p| Command::new(p), f.to_str().unwrap(), true, &[], &want); assert_eq!(s.props, want); assert_eq!(describe(&s, &want), "systemd 255");
    }
    #[test] fn no_systemd_run_no_bus_or_no_manager_is_no_scope_never_an_error() {
        let want = w(&["OOMPolicy=continue"]);
        let s = probe_in(&|p| Command::new(p), "/nonexistent/systemd-run", true, &[], &want); assert!(!s.ok && s.why == "no systemd-run", "{s:?}");
        let (_d, f) = fake("nobus", 255, &[], false);
        let s = probe_in(&|p| Command::new(p), f.to_str().unwrap(), false, &[], &want); assert!(!s.ok && s.why == "no user session bus", "{s:?}");
        let (_d, f) = fake("nomgr", 252, &[], true);
        let s = probe_in(&|p| Command::new(p), f.to_str().unwrap(), true, &[], &want); assert!(!s.ok && s.why.contains("Failed to create bus connection"), "{s:?}");
        assert!(describe(&s, &want).starts_with("systemd 252: no scopes ("), "{}", describe(&s, &want));
    }
    #[test] fn a_launch_is_seen_to_start_or_to_fail_for_its_scope() {
        let (_d, f) = fake("exec", 239, &["OOMPolicy"], false); let prog = f.to_str().unwrap();
        let mut ok = Command::new(prog).args(["--user", "--scope", "--", "sleep", "2"]).spawn().unwrap();
        assert_eq!(wait_exec_of(prog, ok.id(), || ok.try_wait().map_or(true, |s| s.is_some()), Duration::from_secs(3)), Launch::Started);
        let _ = ok.kill(); let _ = ok.wait();
        let mut bad = Command::new(prog).args(["--user", "--scope", "-p", "OOMPolicy=continue", "--", "sleep", "2"]).stderr(Stdio::null()).spawn().unwrap();
        assert_eq!(wait_exec_of(prog, bad.id(), || bad.try_wait().map_or(true, |s| s.is_some()), Duration::from_secs(3)), Launch::Failed);
    }
}

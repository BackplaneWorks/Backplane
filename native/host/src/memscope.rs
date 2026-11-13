//! Memory scopes (Linux): every agent process and every `work_run` command runs in a transient systemd scope of its own in `bpagent.slice`,
//! capped (`MemoryMax`, `BACKPLANE_SCOPE_MEM_MAX`, 40% of RAM by default; the slice holds them all under `BACKPLANE_SLICE_MEM_MAX`, 75%), with
//! `OOMPolicy=continue`: when something an agent started eats memory, the kernel's cgroup OOM killer kills that one process inside its scope, and
//! the agent, its other commands, the hub and every other thread go on. Before this, agents ran in the hub's own scope (OOMPolicy=stop), so the
//! kernel killing one runaway test client made systemd kill the hub and every agent with it (Nov 13 04:42: a 45 GB client). What they start also
//! gets a higher `oom_score_adj` than the hub's, so a machine-wide OOM picks it first. The thread is told when one of its processes is killed.
//!
//! Off when there is no user systemd answering (no `systemd-run`, no user bus), on a copy, in tests, or with `BACKPLANE_NO_SCOPE`: then commands
//! start as before. macOS has no equivalent (no cgroups; a memory cap per process tree would be a sampler that kills, not done); Windows' is a Job
//! Object with `JOB_OBJECT_LIMIT_JOB_MEMORY` per agent (not done: agents already run in a Job Object there, without a memory limit).
use std::sync::OnceLock;

/// The slice every scope goes in: `bpagent.slice`, or BACKPLANE_SCOPE_SLICE (a test hub's own, so it never sets the person's slice's cap).
pub fn slice() -> String { std::env::var("BACKPLANE_SCOPE_SLICE").ok().filter(|v| v.ends_with(".slice") && v.len() > 6 && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')).unwrap_or_else(|| "bpagent.slice".into()) }
static ON: OnceLock<bool> = OnceLock::new();

/// A scope's cap and the slice's (systemd's syntax: a percentage of RAM or a size).
pub fn scope_max() -> String { std::env::var("BACKPLANE_SCOPE_MEM_MAX").ok().filter(|v| valid_size(v)).unwrap_or_else(|| "40%".into()) }
pub fn slice_max() -> String { std::env::var("BACKPLANE_SLICE_MEM_MAX").ok().filter(|v| valid_size(v)).unwrap_or_else(|| "75%".into()) }
/// "40%", "24G", "512M", "1073741824": what MemoryMax takes (never anything that could be another argument).
pub fn valid_size(v: &str) -> bool {
    let d = v.trim_end_matches(['%', 'K', 'M', 'G', 'T']);
    !d.is_empty() && d.len() + 1 >= v.len() && d.chars().all(|c| c.is_ascii_digit())
}

/// Whether scopes are used. Decided once, by `init`; false until then.
pub fn on() -> bool { *ON.get().unwrap_or(&false) }

/// At the hub's start (blocking: run it off the runtime): probe a scope once, then cap the slice.
pub fn init() {
    let on = cfg!(target_os = "linux") && !cfg!(test) && std::env::var_os("BACKPLANE_NO_SCOPE").is_none() && !crate::copy::on() && bus() && probe();
    if on {
        let s = crate::proc::std_cmd(crate::proc::Kind::Local, "systemctl").args(["--user", "set-property", "--runtime", &slice(), &format!("MemoryMax={}", slice_max()), "MemorySwapMax=2G"])
            .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        eprintln!("memscope: agents and jobs run in scopes of their own in {} (each up to {}, all up to {}{})", slice(), scope_max(), slice_max(), if s.map_or(false, |s| s.success()) { "" } else { "; the slice's cap was not set" });
    } else if cfg!(target_os = "linux") && !cfg!(test) {
        eprintln!("memscope: off (no user systemd, a copy, or BACKPLANE_NO_SCOPE): agents and jobs run in the hub's own scope");
    }
    let _ = ON.set(on);
}
fn bus() -> bool { std::env::var_os("XDG_RUNTIME_DIR").map_or(false, |d| std::path::Path::new(&d).join("bus").exists()) }
fn probe() -> bool {
    let Ok(mut c) = crate::proc::std_cmd(crate::proc::Kind::Local, "systemd-run").args(["--user", "--scope", "-q", "--collect", &format!("--slice={}", slice()), "--", "true"])
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn() else { return false };
    let t = std::time::Instant::now();
    while t.elapsed() < std::time::Duration::from_secs(5) {
        if let Ok(Some(s)) = c.try_wait() { return s.success() }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let _ = c.kill(); let _ = c.wait(); false
}

/// systemd-run's arguments that start what follows them in a new scope (`what`: "agent" or "work", in its unit's name), or None when scopes are off.
pub fn wrap(what: &str) -> Option<Vec<String>> {
    if !on() { return None }
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    Some(args(what, std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed), &scope_max(), &slice()))
}
pub fn args(what: &str, hub: u32, n: u64, max: &str, slice: &str) -> Vec<String> {
    ["--user", "--scope", "-q", "--collect", &format!("--slice={slice}"), &format!("--unit=bp-{what}-{hub}-{n}"), "-p", &format!("MemoryMax={max}"), "-p", "MemorySwapMax=1G",
     "-p", "OOMPolicy=continue", "-p", "ManagedOOMPreference=none", "--"].iter().map(|s| s.to_string()).collect()
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
/// The words for the thread: what was killed, the cap it hit, and that nothing else was.
pub fn told(big: Option<&Big>, cap: &str) -> String {
    let what = match big { Some(b) => format!("{} (pid {}, about {} MB)", b.name, b.pid, b.mb), None => "a process".into() };
    format!("Killed for memory: {what}, started by this thread, reached the memory cap of this thread's commands ({cap}) and the kernel killed it. \
             Only that process was killed: this thread's agent, the hub and other threads go on. Look for a leak or an unbounded queue in it before running it again.")
}
/// `oom_kill N` from a cgroup's memory.events.
pub fn kills(events: &str) -> Option<u64> { events.lines().find_map(|l| l.strip_prefix("oom_kill ")).and_then(|n| n.trim().parse().ok()) }

/// Watches the scope `pid` runs in: each time the kernel kills a process there for memory, `tell` gets the words (`told`). Ends with the scope.
pub fn watch(pid: u32, tell: impl Fn(String) + Send + 'static) {
    if !on() || pid == 0 { return }
    let slice = slice();
    tokio::spawn(async move {
        let mut dir = None;
        for _ in 0..250 { // systemd-run moves itself into the scope before it starts the command
            if let Ok(cg) = std::fs::read_to_string(format!("/proc/{pid}/cgroup")) { if let Some(p) = cg.lines().find_map(|l| l.strip_prefix("0::")).filter(|p| p.contains(&slice)) { dir = Some(std::path::PathBuf::from(format!("/sys/fs/cgroup{}", p.trim()))); break } }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let Some(dir) = dir else { return };
        let ev = dir.join("memory.events");
        let mut seen = 0; // a new scope: any kill counted there already is one to tell (a fast eater is killed before this first look)
        let mut big: Option<Big> = None;
        loop {
            let Some(k) = std::fs::read_to_string(&ev).ok().and_then(|s| kills(&s)) else { break }; // the scope is gone
            if k > seen { seen = k; let cap = std::fs::read_to_string(dir.join("memory.max")).ok().and_then(|s| s.trim().parse::<u64>().ok()).map_or_else(scope_max, |b| format!("{} MB", b >> 20)); tell(told(big.as_ref(), &cap)); }
            big = biggest(&dir);
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });
}
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
    }
    #[test] fn scopes_are_off_in_tests() { assert!(!on() && wrap("agent").is_none()) }
}

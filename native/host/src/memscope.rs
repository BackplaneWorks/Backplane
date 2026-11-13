//! Memory scopes (Linux): every agent process and every `work_run` command runs in a transient systemd scope of its own in `bpagent.slice`,
//! capped (`MemoryMax`, `BACKPLANE_SCOPE_MEM_MAX`, 40% of RAM by default; the slice holds them all under `BACKPLANE_SLICE_MEM_MAX`, 75%), with
//! `OOMPolicy=continue`: when something an agent started eats memory, the kernel's cgroup OOM killer kills that one process inside its scope, and
//! the agent, its other commands, the hub and every other thread go on. Before this, agents ran in the hub's own scope (OOMPolicy=stop), so the
//! kernel killing one runaway test client made systemd kill the hub and every agent with it (Nov 13 04:42: a 45 GB client). What they start also
//! gets a higher `oom_score_adj` than the hub's, so a machine-wide OOM picks it first. The thread is told when one of its processes is killed.
//!
//! Off when there is no user systemd answering (no `systemd-run`, no user bus), on a copy, in tests, or with `BACKPLANE_NO_SCOPE`: then commands
//! start as before. macOS has no equivalent (no cgroups; a memory cap per process tree would be a sampler that kills, not done); Windows' is a Job
//! Object memory limit with the same caps (`cap_job`; allocations past it fail, nothing is killed or told). KiCad and the browser helper have
//! scopes of their own (`wrap_helper`).
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
/// The hub's own long-lived helpers, each in a scope of its own outside the hub's (a kill there never stops the hub, and never takes the hub's
/// other children with it): the embedded KiCad (`kicad`: in bpkicad.slice, BACKPLANE_KICAD_MEM_MAX, 12G by default: lane A's real boards peak at
/// 4.1-4.5 GB per KiCad, so about 2.7 times that) and the browser helper (`browser`: in bpagent.slice, BACKPLANE_BROWSER_MEM_MAX, 8G).
pub fn wrap_helper(what: &str) -> Option<Vec<String>> {
    if !on() { return None }
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let (slice, max) = helper_cap(what);
    Some(args(what, std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed), &max, &slice))
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
pub fn watch(pid: u32, tell: impl Fn(String) + Send + 'static) { watch_as(pid, None, tell) }
fn watch_as(pid: u32, helper: Option<&'static str>, tell: impl Fn(String) + Send + 'static) {
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
        let slice_kills = || pev.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| kills(&s));
        let slice0 = slice_kills();
        let mut cap = String::new();
        loop {
            let Some(k) = std::fs::read_to_string(&ev).ok().and_then(|s| kills(&s)) else {
                if dbg { eprintln!("memscope: pid {pid}: scope gone (kills seen {seen}, slice {:?} -> {:?}, last biggest {:?})", slice0, slice_kills(), big) }
                if seen == 0 && slice_kills().zip(slice0).map_or(false, |(now, then)| now > then) && big.is_some() { tell(told_as(helper, big.as_ref(), &cap)); }
                break // the scope is gone
            };
            if k > seen { seen = k; tell(told_as(helper, big.as_ref(), &cap)); }
            if let Some(b) = std::fs::read_to_string(dir.join("memory.max")).ok().and_then(|s| s.trim().parse::<u64>().ok()) { cap = format!("{} MB", b >> 20) } else if cap.is_empty() { cap = scope_max() }
            big = biggest(&dir).or(big);
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
    #[test] fn scopes_are_off_in_tests() { assert!(!on() && wrap("agent").is_none()) }
}

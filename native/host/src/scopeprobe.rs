//! What `systemd-run --user --scope` takes on this machine, probed once (std only: the hub's memscope.rs and kicad checks, the window's
//! launcher and the guard include this file). Older systemd rejects properties it does not know ("Unknown assignment: OOMPolicy=continue":
//! OOMPolicy came in 243, ManagedOOMPreference in 248), and some machines have no user manager at all (a container, WSL, a non-systemd
//! distro, a login without a user bus). A scope is made only with the properties a real scope here accepted; where none can be made, things
//! start unscoped, with one log line, never an error. `BACKPLANE_NO_SCOPE` turns scopes off by hand. `BACKPLANE_SYSTEMD_RUN` names the
//! systemd-run to use (tests: a stand-in that rejects a property, or one that is missing).
#![allow(dead_code)] // each includer uses a part
use std::process::{Command, Stdio};
/// How the includer makes a process: the hub through its gate (`crate::proc::std_cmd`, so a copy refuses what it must), the window and the
/// guard directly.
pub type Mk<'a> = &'a dyn Fn(&str) -> Command;
use std::time::{Duration, Instant};

/// The systemd-run to run: BACKPLANE_SYSTEMD_RUN, or the one on PATH.
pub fn program() -> String { std::env::var("BACKPLANE_SYSTEMD_RUN").ok().filter(|v| !v.is_empty()).unwrap_or_else(|| "systemd-run".into()) }

/// What the probe found: whether a scope can be made at all (and why not), systemd's version, and which of the wanted properties it took.
#[derive(Clone, Debug, PartialEq)]
pub struct Support { pub ok: bool, pub why: String, pub version: Option<u32>, pub props: Vec<String> }

/// The first systemd version that could take a property in a transient scope (None: any we would run on): only to skip a probe that cannot
/// pass; the probe decides. OOMPolicy came in 243 for services, and a scope takes it later (249 refuses it, 252 takes it: measured).
pub fn since(prop: &str) -> Option<u32> {
    match prop.split('=').next().unwrap_or("") {
        "OOMPolicy" => Some(243), "ManagedOOMPreference" => Some(248), "MemorySwapMax" => Some(232), "MemoryMax" => Some(231), _ => None,
    }
}
/// "systemd 249 (249.11-0ubuntu3.12)" -> 249.
pub fn parse_version(out: &str) -> Option<u32> {
    out.lines().next()?.split_whitespace().skip_while(|w| *w != "systemd").nth(1)?.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}
/// Whether a scoped launch failed for the scope, not the command (systemd-run's own words): an unknown property or no user manager.
pub fn scope_failure(stderr: &str) -> bool {
    ["Unknown assignment", "Unknown property", "Failed to start transient scope", "Failed to create bus connection", "Failed to connect to bus",
     "transient scope"].iter().any(|w| stderr.contains(w))
}
fn user_bus() -> bool {
    std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() || std::env::var_os("XDG_RUNTIME_DIR").map_or(false, |d| std::path::Path::new(&d).join("bus").exists())
}

/// One scope that runs `true`, with `props` (each passed as `-p`), within 5 s: Ok, or systemd-run's words (stderr) when it failed.
fn try_scope(mk: Mk, prog: &str, base: &[String], props: &[String]) -> Result<(), String> {
    let mut c = mk(prog);
    c.args(["--user", "--scope", "-q", "--collect"]).args(base);
    for p in props { c.arg("-p").arg(p); }
    let mut ch = c.args(["--", "true"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| format!("{prog} could not start: {e}"))?;
    let t = Instant::now();
    loop {
        match ch.try_wait() {
            Ok(Some(s)) => {
                let mut err = String::new(); if let Some(mut e) = ch.stderr.take() { use std::io::Read; let _ = e.read_to_string(&mut err); }
                return if s.success() { Ok(()) } else { Err(err.lines().find(|l| !l.trim().is_empty()).unwrap_or("systemd-run failed").trim().to_string()) }
            }
            Ok(None) if t.elapsed() < Duration::from_secs(5) => std::thread::sleep(Duration::from_millis(10)),
            _ => { let _ = ch.kill(); let _ = ch.wait(); return Err("systemd-run did not answer in 5 s".into()) }
        }
    }
}

/// Probe once: can a scope be made (with `base`, e.g. `--slice=...`), and which of `wanted` does it take. All of them at once first (one run
/// where systemd is current); when that fails, the scope alone, then each property the version should know, alone.
pub fn probe(mk: Mk, base: &[String], wanted: &[String]) -> Support {
    if !cfg!(target_os = "linux") { return Support { ok: false, why: "not Linux".into(), version: None, props: vec![] } }
    if std::env::var_os("BACKPLANE_NO_SCOPE").is_some() { return Support { ok: false, why: "BACKPLANE_NO_SCOPE is set".into(), version: None, props: vec![] } }
    probe_in(mk, &program(), user_bus(), base, wanted)
}
/// `probe` with a given systemd-run and whether a user bus is there (tests).
pub fn probe_in(mk: Mk, prog: &str, bus: bool, base: &[String], wanted: &[String]) -> Support {
    let no = |why: &str, version| Support { ok: false, why: why.into(), version, props: vec![] };
    let version = mk(prog).arg("--version").stdin(Stdio::null()).stderr(Stdio::null()).output().ok().filter(|o| o.status.success()).and_then(|o| parse_version(&String::from_utf8_lossy(&o.stdout)));
    if version.is_none() && mk(prog).arg("--version").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_err() { return no("no systemd-run", None) }
    if !bus { return no("no user session bus", version) }
    if try_scope(mk, prog, base, wanted).is_ok() { return Support { ok: true, why: String::new(), version, props: wanted.to_vec() } }
    if let Err(e) = try_scope(mk, prog, base, &[]) { return no(&format!("no user systemd answers ({e})"), version) }
    let props = wanted.iter().filter(|p| version.zip(since(p)).map_or(true, |(v, s)| v >= s)).filter(|p| try_scope(mk, prog, base, std::slice::from_ref(p)).is_ok()).cloned().collect();
    Support { ok: true, why: String::new(), version, props }
}
/// `probe`'s answer for the log: "systemd 245; without OOMPolicy=continue".
pub fn describe(s: &Support, wanted: &[String]) -> String {
    let v = s.version.map_or("systemd (version unknown)".to_string(), |v| format!("systemd {v}"));
    if !s.ok { return match s.version { Some(_) => format!("{v}: no scopes ({})", s.why), None => s.why.clone() } }
    let left: Vec<&str> = wanted.iter().filter(|w| !s.props.contains(w)).map(String::as_str).collect();
    if left.is_empty() { v } else { format!("{v}; without {} (not supported here)", left.join(", ")) }
}
/// `-p` before each property, for systemd-run's arguments.
pub fn p_args(props: &[String]) -> Vec<String> { props.iter().flat_map(|p| ["-p".to_string(), p.clone()]).collect() }

/// How a scoped launch went, seen from its pid: systemd-run made the scope and became the command (`Started`), or ended first (`Failed`:
/// it could not make the scope, so the command never ran), or neither within `limit` (`Unknown`: left to run).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Launch { Started, Failed, Unknown }
/// Waits for a scoped launch's pid to exec its command (it is no longer systemd-run) or to end (`exited`: the caller's try_wait).
pub fn wait_exec(pid: u32, exited: impl FnMut() -> bool, limit: Duration) -> Launch { wait_exec_of(&program(), pid, exited, limit) }
/// `wait_exec` for a launch through `prog` (tests: a stand-in).
pub fn wait_exec_of(prog: &str, pid: u32, mut exited: impl FnMut() -> bool, limit: Duration) -> Launch {
    let t = Instant::now();
    let me: String = prog.rsplit('/').next().unwrap_or(&prog).chars().take(15).collect(); // the kernel's comm: 15 bytes
    // A forked child wears the forking thread's comm until its exec sets the new one (the exe link changes a moment earlier, so it cannot tell).
    let ours = std::fs::read_to_string("/proc/thread-self/comm").map(|c| c.trim().to_string()).unwrap_or_default();
    loop {
        if exited() { return Launch::Failed }
        match std::fs::read_to_string(format!("/proc/{pid}/comm")) {
            Ok(c) if c.trim() != me && !c.trim().is_empty() && c.trim() != ours => return Launch::Started,
            Err(_) => if exited() { return Launch::Failed },
            _ => {}
        }
        if t.elapsed() >= limit { return Launch::Unknown }
        std::thread::sleep(Duration::from_millis(3));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn versions_and_systemd_runs_own_failures() {
        assert_eq!(parse_version("systemd 249 (249.11-0ubuntu3.12)\n+PAM +AUDIT"), Some(249));
        assert_eq!(parse_version("systemd 239 (239-58.el8)"), Some(239));
        assert_eq!(parse_version("garbage"), None);
        assert_eq!(since("OOMPolicy=continue"), Some(243)); assert_eq!(since("ManagedOOMPreference=none"), Some(248));
        assert!(scope_failure("Unknown assignment: OOMPolicy=continue\nFailed to start transient scope unit: Invalid argument"));
        assert!(scope_failure("Failed to create bus connection: No such file or directory"));
        assert!(!scope_failure("Error: claude: not logged in")); assert!(!scope_failure("Failed to parse board: line 3"), "a kicad-cli error is not a scope's");
    }
}

//! The ONE place a child process is made. Every lane builds its commands with `tokio_cmd` / `std_cmd` and says what the process is
//! for (`Kind`); on a copy of a real log (`BACKPLANE_COPY=1`, copy.rs) a command that acts outside the home is not run: the program
//! becomes a path that does not exist, so the caller's own "could not start" handling answers, with the reason in the error.
//! A source scan (tests below) fails the build of any other `Command::new` in non-test code.
use crate::copy;
use std::ffi::OsStr;

/// What a process is for. On a copy: Net, Terminal, Browser and Shell are refused outright, Agent only from the stand-in folder
/// (`BACKPLANE_AGENTS`), Local (git reads, kicad-cli, our own helpers) unless the program is a network command by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// our own helper children and local tools: the policy, the window program, plot, kicad-cli, git (its arguments are checked where known)
    Local,
    /// reaches another machine: curl, tailscale, ssh...
    Net,
    /// a user's terminal (a shell on a pty)
    Terminal,
    /// the browser helper
    Browser,
    /// a shell running a command line an agent or the user supplied (secret_run, hooks)
    Shell,
    /// an agent CLI (claude, codex, ...)
    Agent,
    /// a REAL agent CLI for a turn the person started (side-by-side mode only: `copy::person_turn_allowed()`); the only other kind that runs a real agent on a copy
    PersonTurn,
}

/// Set once in safe mode (guard.rs): no agent starts, for this reason.
pub static PAUSED: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Why a copy does not run this, or None. `on` is the copy switch (a parameter so the rule is testable).
pub fn refusal_with(on: bool, kind: Kind, program: &str, agents_dir: Option<&str>) -> Option<String> {
    if !on { return None }
    let base = program.rsplit('/').next().unwrap_or(program);
    let no = || Some(copy::refusal(base));
    match kind {
        Kind::Net | Kind::Terminal | Kind::Browser | Kind::Shell | Kind::PersonTurn => no(), // PersonTurn: refused here; the global `refusal` lets it through in side-by-side mode
        Kind::Agent => match agents_dir { Some(d) if !d.is_empty() && in_agents_dir(program, d) => None, _ => no() },
        Kind::Local => if copy::refused_with(true, program, &[]) { no() } else { None },
    }
}
/// Whether `program` lies inside the stand-in folder `dir`, both by their real paths where they exist: the agent is resolved to its real path
/// (macOS's /var is /private/var; Windows' RUNNER~1 is runneradmin, and its separator is "\\"), and BACKPLANE_AGENTS is given as the caller wrote it.
/// Compared as given, a copy refused its own stand-ins there, the person's turn too ("could not start claude: No such file or directory").
fn in_agents_dir(program: &str, dir: &str) -> bool {
    let real = |x: &str| crate::plat::fs::canonicalize(std::path::Path::new(x)).unwrap_or_else(|_| std::path::PathBuf::from(x));
    let (p, d) = (real(program), real(dir)); p != d && p.starts_with(&d)
}
fn refusal(kind: Kind, program: &str) -> Option<String> {
    if let (Some(why), Kind::Agent | Kind::PersonTurn) = (PAUSED.get(), kind) { return Some(why.clone()) }
    if cfg!(test) { if let Some(why) = test_refusal(program, &std::env::temp_dir()) { return Some(why) } }
    if !copy::on() { return None }
    if kind == Kind::PersonTurn && copy::person_turn_allowed() { return None }
    refusal_with(true, kind, program, std::env::var("BACKPLANE_AGENTS").ok().as_deref())
}
/// A test never runs the machine's own Tailscale CLI: its tailnet is the person's (the desktop's gate, a Boat) or a VM's (the Windows gate),
/// and a real `tailscale` reached from a test acts on it. A program named tailscale (`tailscale`, `tailscale.exe`, any folder, any case) is
/// refused under test unless it is a stand-in under the temporary folder `tmp`; code that asks Tailscale then sees it unavailable, as on a
/// machine without it. (bp-winbuild's Tailscale was logged out in the middle of a Windows gate run on 2026-11-20; no hub path to a local
/// logout was found, and this closes the door to all of them.)
fn test_refusal(program: &str, tmp: &std::path::Path) -> Option<String> {
    let base = program.rsplit(['/', '\\']).next().unwrap_or(program).to_ascii_lowercase();
    if base != "tailscale" && base != "tailscale.exe" { return None }
    let norm = |x: &str| { let x = x.replace('\\', "/"); if cfg!(windows) { x.to_ascii_lowercase() } else { x } };
    let t = norm(&tmp.to_string_lossy()); let t = t.trim_end_matches('/');
    if !t.is_empty() && norm(program).starts_with(&format!("{t}/")) { return None }
    Some(format!("{program}: a test never runs the machine's real tailscale (proc.rs test_refusal); give it a stand-in under {}", tmp.display()))
}
/// The hub's own variables from its guard and updater: which builds folder it was started from, its commit, its guard, safe mode, injected faults, the update feed.
/// An agent, a shell an agent or the person runs, or a terminal never gets them: a test hub started from one would take the person's builds folder for its own,
/// and could update itself against it (or a `backplane hub` there would act on the person's hub). Its listener and its address and name on the network go too
/// (a test hub would bind the person's tailnet port, or claim the hub's URL), and the push key (it would notify the person's phones). Nothing an agent runs
/// needs them to reach the hub: the MCP bridge gets the hub's URL as an argument, and `backplane hub` finds it by --home or its pointer file.
/// BACKPLANE_NO_UPDATE stays: it only ever holds an update back.
pub const HUB_ONLY: &[&str] = &["BACKPLANE_BUILDS", "BACKPLANE_BUILD", "BACKPLANE_COMMIT", "BACKPLANE_GUARD", "BACKPLANE_HOME", "BACKPLANE_SAFE", "BACKPLANE_SAFE_SKIP",
    "BACKPLANE_CARRY_WAIT", "BACKPLANE_FAULT", "BACKPLANE_STEP_BUDGET", "BACKPLANE_UPDATE_FEED", "BACKPLANE_RELEASE_KEY", "BACKPLANE_TEST_BUILDS", "BACKPLANE_TEST_NO_UPDATE",
    "BACKPLANE_HTTPS_PORT", "BACKPLANE_URL", "BACKPLANE_NAME", "BACKPLANE_HOOK_URL", "BACKPLANE_PEERS"];
pub fn hub_only(name: &str) -> bool { HUB_ONLY.contains(&name) || name.starts_with("BACKPLANE_GUARD_") || name.starts_with("BACKPLANE_APNS_") }
/// macOS: the documents folder every KiCad the hub starts gets (KICAD_DOCUMENTS_HOME), set once at startup (main: `<home>/kicad/documents`).
/// KiCad's default, ~/Documents/KiCad, made macOS ask Backplane (KiCad's responsible process) for the Documents folder on the first Edit,
/// with KiCad's UI thread waiting on the answer (the Mac mini gate, Nov 16). Given to every child but a person's own terminal (agents' and
/// the hub's kicad-cli, the embedded and headless KiCad, Open in KiCad), and never over a value the hub itself was started with.
pub static KICAD_DOCUMENTS: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
pub fn kicad_documents(macos: bool, kind: Kind, inherited: bool, dir: Option<&std::path::Path>) -> Option<&std::path::Path> {
    if !macos || kind == Kind::Terminal || inherited { return None }
    dir
}
fn documents_for(kind: Kind) -> Option<&'static std::path::Path> {
    kicad_documents(cfg!(target_os = "macos"), kind, std::env::var_os("KICAD_DOCUMENTS_HOME").is_some_and(|v| !v.is_empty()), KICAD_DOCUMENTS.get().map(|p| p.as_path()))
}
/// Python's bytecode caches for every child but a person's own terminal (`PYTHONPYCACHEPREFIX`, set once at startup: `<home>/cache/pycache`),
/// and never over a value the hub was started with. FreeCAD's Python wrote `__pycache__` inside FreeCAD.app when the hub (or an agent of it)
/// ran freecadcmd, and macOS said "'backplane-hub' was prevented from modifying apps on your Mac" (0.16.0 release check). Every system: an
/// installed FreeCAD's folder is not the place for them anywhere (Program Files is read-only to a user), and the caches still speed runs up.
pub static PYCACHE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
pub fn pycache(kind: Kind, inherited: bool, dir: Option<&std::path::Path>) -> Option<&std::path::Path> {
    if kind == Kind::Terminal || inherited { return None }
    dir
}
fn pycache_for(kind: Kind) -> Option<&'static std::path::Path> {
    pycache(kind, std::env::var_os("PYTHONPYCACHEPREFIX").is_some_and(|v| !v.is_empty()), PYCACHE.get().map(|p| p.as_path()))
}
/// macOS: a user base (`PYTHONUSERBASE`) whose `usercustomize` sets `sys.pycache_prefix` from PYTHONPYCACHEPREFIX. FreeCAD starts its Python
/// isolated (Base/Interpreter.cpp: PyConfig_InitIsolatedConfig), so it reads no PYTHON* variable, but it keeps the user site on, and site.py takes
/// the user base from PYTHONUSERBASE whatever the flags say: this is the one way in before FreeCAD imports its modules. FreeCAD.app ships with
/// almost no bytecode (9878 of 10197 .py files on the Mac mini), so its first runs wrote __pycache__ into the bundle, and macOS said
/// "'backplane-hub' was prevented from modifying apps on your Mac". The same rule as PYCACHE; set at startup with `write_userbase`.
pub static PYUSERBASE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
fn userbase_for(kind: Kind) -> Option<&'static std::path::Path> {
    if !cfg!(target_os = "macos") { return None }
    pycache(kind, std::env::var_os("PYTHONUSERBASE").is_some_and(|v| !v.is_empty()), PYUSERBASE.get().map(|p| p.as_path()))
}
/// The usercustomize every Python started with PYUSERBASE runs: the cache prefix, then the person's own user site packages (their default
/// user base's), which the moved user base would otherwise hide.
pub const USERCUSTOMIZE: &str = r#"# Backplane (proc.rs PYUSERBASE): Python's bytecode caches go to PYTHONPYCACHEPREFIX, also in a Python that ignores the environment
# (FreeCAD's), so nothing is written inside an installed app; the person's own user site packages stay importable.
import os, sys
_p = os.environ.get("PYTHONPYCACHEPREFIX")
if _p and not sys.pycache_prefix:
    sys.pycache_prefix = _p
try:
    import site
    _v = "%d.%d" % sys.version_info[:2]
    if sys.platform == "darwin" and getattr(sys, "_framework", None):
        _u = os.path.expanduser("~/Library/Python/" + _v + "/lib/python/site-packages")
    else:
        _u = os.path.expanduser("~/.local/lib/python" + _v + "/site-packages")
    if os.path.isdir(_u) and _u not in sys.path:
        site.addsitedir(_u)
except Exception:
    pass
"#;
/// Where site.py looks for usercustomize under a user base, for the layouts it uses: a framework build's `lib/python/site-packages`, else
/// `lib/pythonX.Y/site-packages` (FreeCAD's conda Python 3.11 among them).
pub fn userbase_sites(base: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut v = vec![base.join("lib").join("python").join("site-packages")];
    v.extend((8..=16).map(|m| base.join("lib").join(format!("python3.{m}")).join("site-packages")));
    v
}
pub fn write_userbase(base: &std::path::Path) -> std::io::Result<()> {
    for d in userbase_sites(base) { std::fs::create_dir_all(&d)?; let f = d.join("usercustomize.py"); if std::fs::read_to_string(&f).ok().as_deref() != Some(USERCUSTOMIZE) { std::fs::write(&f, USERCUSTOMIZE)?; } }
    Ok(())
}
/// The kinds that run what an agent or a person supplies.
fn scrubbed(kind: Kind) -> bool { matches!(kind, Kind::Agent | Kind::PersonTurn | Kind::Shell | Kind::Terminal) }
fn hub_only_present() -> Vec<std::ffi::OsString> { std::env::vars_os().map(|(k, _)| k).filter(|k| k.to_str().map_or(false, hub_only)).collect() }

fn dead(program: &str, why: &str) -> String { eprintln!("{why}"); format!("/nonexistent/backplane-copy-refused/{}", program.rsplit('/').next().unwrap_or(program)) }

/// `tokio::process::Command::new`, gated.
pub fn tokio_cmd<S: AsRef<OsStr>>(kind: Kind, program: S) -> tokio::process::Command {
    let p = program.as_ref().to_string_lossy().into_owned();
    let mut c = match refusal(kind, &p) { None => { let (prog, pre) = crate::plat::hub::launch(program.as_ref()); let mut c = tokio::process::Command::new(prog); c.args(pre); c } Some(why) => tokio::process::Command::new(dead(&p, &why)) };
    crate::plat::proc::default_signals_tokio(&mut c);
    // Windows: no console window for what the hub starts, whether or not the hub has a console to share (a guard's, a task's); own_group keeps it
    #[cfg(windows)] c.creation_flags(crate::plat::proc::NO_WINDOW);
    if scrubbed(kind) { for k in hub_only_present() { c.env_remove(k); } }
    if let Some(d) = documents_for(kind) { c.env("KICAD_DOCUMENTS_HOME", d); }
    if let Some(d) = pycache_for(kind) { c.env("PYTHONPYCACHEPREFIX", d); }
    if let Some(d) = userbase_for(kind) { c.env("PYTHONUSERBASE", d); }
    c
}
/// `tokio_cmd`, in a memory scope of its own when `wrap` is given (memscope.rs: systemd-run's arguments, the program after them). A refused
/// program is never wrapped. The caller raises its oom_score_adj (memscope::raise_oom).
#[allow(dead_code)] // bp-plot includes this file and starts no agent
pub fn tokio_cmd_in<S: AsRef<OsStr>>(kind: Kind, program: S, wrap: Option<Vec<String>>) -> tokio::process::Command {
    let Some(w) = wrap.filter(|_| refusal(kind, &program.as_ref().to_string_lossy()).is_none()) else { return tokio_cmd(kind, program) };
    let (prog, pre) = crate::plat::hub::launch(program.as_ref());
    // BACKPLANE_SYSTEMD_RUN: a stand-in (a test hub's; scopeprobe.rs reads the same)
    let sr = std::env::var("BACKPLANE_SYSTEMD_RUN").ok().filter(|v| !v.is_empty()).unwrap_or_else(|| "systemd-run".into());
    let mut c = tokio::process::Command::new(sr); c.args(w).arg(prog).args(pre);
    crate::plat::proc::default_signals_tokio(&mut c);
    if scrubbed(kind) { for k in hub_only_present() { c.env_remove(k); } }
    if let Some(d) = documents_for(kind) { c.env("KICAD_DOCUMENTS_HOME", d); }
    if let Some(d) = pycache_for(kind) { c.env("PYTHONPYCACHEPREFIX", d); }
    if let Some(d) = userbase_for(kind) { c.env("PYTHONUSERBASE", d); }
    c
}
/// A probe of a KiCad (`--version`, `api-server --describe`): its settings go to a throwaway folder (`KICAD_CONFIG_HOME`), never the
/// person's. KiCad loads (and migrates, or creates) its settings even for `--version`: measured Nov 16 on stock 10.0.8, Linux created
/// ~/.config/kicad/10.0 when absent and rewrote an older kicad_common.json, macOS rewrote a current one on every call; the isolated
/// `KICAD_CONFIG_HOME` / `KICAD_CACHE_HOME` / `KICAD_DOCUMENTS_HOME` folders protect the person's settings across all releases (including
/// stock 9.0/10.0, whose CLI exit path saves settings without checking `KICAD_INHIBIT_SETTINGS_WRITES`). The throwaway folder goes when
/// the guard is dropped after the probe finishes.
///
/// `ProbeHome::new` also pre-seeds `<ver>/kicad_common.json` with a non-empty `api.interpreter_path` and `api.enable_server: false`,
/// and `env()` sets `KICAD_INHIBIT_SETTINGS_WRITES=1`: Backplane `bp.6`-`bp.13.2` and stock KiCad skip `PYTHON_MANAGER::FindPythonInterpreter()`
/// (which runs `wxExecute("which -a python3", wxEXEC_SYNC)` and hung probes on macOS, #1160) and `ReloadPlugins()`, and builds honoring
/// `KICAD_INHIBIT_SETTINGS_WRITES` skip settings writeback too.
pub struct ProbeHome(std::path::PathBuf);
impl ProbeHome {
    pub fn new() -> Self {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("bp-kicad-probe-{}-{n}", std::process::id()));
        if let Err(e) = std::fs::create_dir_all(&d) {
            eprintln!("kicad: could not create probe home {}: {e}", d.display());
            return ProbeHome(d);
        }
        for (ver, schema) in [("10.99", 7u32), ("10.0", 6), ("9.0", 3), ("8.0", 3)] {
            let sub = d.join(ver);
            if let Err(e) = std::fs::create_dir_all(&sub) {
                eprintln!("kicad: could not create probe settings folder {}: {e}", sub.display());
                continue;
            }
            let file = sub.join("kicad_common.json");
            let body = format!(r#"{{"api":{{"enable_server":false,"interpreter_path":"/nonexistent/bp-probe-python"}},"meta":{{"filename":"kicad_common.json","version":{schema}}}}}"#);
            if let Err(e) = std::fs::write(&file, body) {
                eprintln!("kicad: could not seed probe settings {}: {e}", file.display());
            }
        }
        ProbeHome(d)
    }
    pub fn path(&self) -> &std::path::Path { &self.0 }
    /// What a probed KiCad is given: its settings (`KICAD_CONFIG_HOME`), and the cache and documents folders KiCad makes at every start
    /// (PATHS::EnsureUserPathsExist: `~/.cache/kicad/<ver>`, `~/.local/share/kicad/<ver>`, macOS and Windows `Documents/KiCad/<ver>`),
    /// all in this throwaway folder, plus `KICAD_INHIBIT_SETTINGS_WRITES=1`: a probe leaves nothing in the person's home (#1204).
    pub fn env(&self) -> [(&'static str, std::ffi::OsString); 4] {
        [
            ("KICAD_CONFIG_HOME", self.0.as_os_str().to_os_string()),
            ("KICAD_CACHE_HOME", self.0.join("cache").into_os_string()),
            ("KICAD_DOCUMENTS_HOME", self.0.join("documents").into_os_string()),
            ("KICAD_INHIBIT_SETTINGS_WRITES", std::ffi::OsString::from("1")),
        ]
    }
}
impl Drop for ProbeHome { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
/// `std_cmd` for a probe of the KiCad at `cli`, with its throwaway settings folder (keep the guard until the probe has run).
pub fn kicad_probe<S: AsRef<OsStr>>(cli: S) -> (std::process::Command, ProbeHome) {
    let h = ProbeHome::new(); let mut c = std_cmd(Kind::Local, cli); c.envs(h.env()); (c, h)
}
/// How long a KiCad probe may run. A `--version` takes a few seconds; on macOS (#1160) some never exit: KiCad's start-up runs Python to
/// find its interpreter (PYTHON_MANAGER::FindPythonInterpreter → wxExecute) and misses that child's exit, so the probe waits forever,
/// ignoring SIGTERM, and they piled up under the hub (42 in two hours). Past this its process group is killed (SIGKILL) and reaped.
#[allow(dead_code)] // bp-plot includes this file
pub const PROBE_LIMIT: std::time::Duration = std::time::Duration::from_secs(15);

#[allow(dead_code)] // bp-plot includes this file
#[derive(Debug)]
pub enum ProbeError {
    Spawn(std::io::Error),
    Timeout,
}

#[allow(dead_code)] // bp-plot includes this file
fn drain_pipe<R: std::io::Read + Send + 'static>(pipe: Option<R>) -> std::sync::mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut b = vec![];
        if let Some(mut r) = pipe { let _ = r.read_to_end(&mut b); }
        let _ = tx.send(b);
    });
    rx
}

#[allow(dead_code)] // bp-plot includes this file
fn poll_pipe(slot: &mut Option<Vec<u8>>, rx: &std::sync::mpsc::Receiver<Vec<u8>>) {
    if slot.is_none() {
        match rx.try_recv() {
            Ok(b) => *slot = Some(b),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => *slot = Some(vec![]),
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
}

#[allow(dead_code)] // bp-plot includes this file
#[cfg(unix)]
fn kill_and_reap_group(
    child: &mut std::process::Child,
    pid: u32,
    stdout_done: &mut Option<Vec<u8>>,
    stderr_done: &mut Option<Vec<u8>>,
    rx_out: &std::sync::mpsc::Receiver<Vec<u8>>,
    rx_err: &std::sync::mpsc::Receiver<Vec<u8>>,
    cleanup_deadline: Option<std::time::Instant>,
) {
    use crate::plat::proc::ChildWait;
    if crate::plat::proc::poll_child_nowait(pid) == ChildWait::NotChild {
        let _ = child.wait();
        return;
    }
    crate::plat::proc::kill_group(pid, crate::plat::proc::TERM);
    crate::plat::proc::kill(pid, crate::plat::proc::TERM);
    let mut grace = std::time::Instant::now() + std::time::Duration::from_millis(200);
    if let Some(dl) = cleanup_deadline {
        grace = grace.min(dl);
    }
    while std::time::Instant::now() < grace {
        poll_pipe(stdout_done, rx_out);
        poll_pipe(stderr_done, rx_err);
        match crate::plat::proc::poll_child_nowait(pid) {
            ChildWait::Exited if stdout_done.is_some() && stderr_done.is_some() => break,
            ChildWait::NotChild => break,
            _ => {
                let step = std::time::Duration::from_millis(20).min(grace.saturating_duration_since(std::time::Instant::now()));
                if !step.is_zero() {
                    std::thread::sleep(step);
                }
            }
        }
    }
    if crate::plat::proc::poll_child_nowait(pid) != ChildWait::NotChild {
        crate::plat::proc::kill_group(pid, crate::plat::proc::KILL);
        let _ = child.kill();
    }
    let _ = child.wait();
}

#[allow(dead_code)] // bp-plot includes this file
#[cfg(not(unix))]
fn kill_and_reap_group(
    child: &mut std::process::Child,
    pid: u32,
    _stdout_done: &mut Option<Vec<u8>>,
    _stderr_done: &mut Option<Vec<u8>>,
    _rx_out: &std::sync::mpsc::Receiver<Vec<u8>>,
    _rx_err: &std::sync::mpsc::Receiver<Vec<u8>>,
    _cleanup_deadline: Option<std::time::Instant>,
) {
    crate::plat::proc::kill_group(pid, crate::plat::proc::KILL);
    let _ = child.kill();
    let _ = child.wait();
}

/// Run a probe `c` in its own process group/job (stdin on /dev/null, stdout and stderr captured) until `end`, capping any post-timeout
/// `SIGTERM` grace to `cleanup_deadline` when provided: returns `Ok(Output)` when BOTH the child exited and stdout/stderr reached EOF by
/// `end`, `Err(ProbeError::Spawn(_))` when it could not start or poll failed, or `Err(ProbeError::Timeout)` when it or a descendant ran past
/// `end` (then the entire process group/job is killed and the child is reaped before returning). On Unix, the child is polled non-reapingly
/// via `WNOWAIT` until both pipes close or cleanup finishes, keeping its PID/PGID reserved so `kill_group` never signals a recycled PID.
#[allow(dead_code)] // bp-plot includes this file
pub fn probe_output_until(
    mut c: std::process::Command,
    end: std::time::Instant,
    cleanup_deadline: Option<std::time::Instant>,
    log_limit: std::time::Duration,
) -> Result<std::process::Output, ProbeError> {
    if std::time::Instant::now() >= end {
        return Err(ProbeError::Timeout);
    }
    crate::plat::proc::own_group(&mut c);
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut tries = 0u32;
    let mut child = loop {
        if std::time::Instant::now() >= end {
            return Err(ProbeError::Timeout);
        }
        match c.spawn() {
            Err(e) if e.raw_os_error() == Some(crate::plat::errno::ETXTBSY) && tries < 40 => {
                let Some(rem) = end.checked_duration_since(std::time::Instant::now()).filter(|d| !d.is_zero()) else {
                    return Err(ProbeError::Timeout);
                };
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(25).min(rem));
            }
            Err(e) => return Err(ProbeError::Spawn(e)),
            Ok(ch) => break ch,
        }
    };
    let pid = child.id();
    // Best-effort Job Object assignment on Windows (std::process::Command does not expose the primary thread handle for CREATE_SUSPENDED,
    // matching the rest of plat/windows.rs); timeout cleanup also falls back to child.kill().
    crate::plat::proc::adopt(pid, false);
    let rx_out = drain_pipe(child.stdout.take());
    let rx_err = drain_pipe(child.stderr.take());
    let mut stdout_done: Option<Vec<u8>> = None;
    let mut stderr_done: Option<Vec<u8>> = None;
    let timeout_err = |child: &mut std::process::Child, stdout_done: &mut Option<Vec<u8>>, stderr_done: &mut Option<Vec<u8>>| -> Result<std::process::Output, ProbeError> {
        kill_and_reap_group(child, pid, stdout_done, stderr_done, &rx_out, &rx_err, cleanup_deadline);
        eprintln!("kicad: a probe ({:?} {:?}, pid {pid}) ran over {} s and was killed", c.get_program(), c.get_args().collect::<Vec<_>>(), log_limit.as_secs());
        Err(ProbeError::Timeout)
    };

    #[cfg(unix)]
    {
        use crate::plat::proc::ChildWait;
        loop {
            if std::time::Instant::now() >= end {
                return timeout_err(&mut child, &mut stdout_done, &mut stderr_done);
            }
            poll_pipe(&mut stdout_done, &rx_out);
            poll_pipe(&mut stderr_done, &rx_err);
            match crate::plat::proc::poll_child_nowait(pid) {
                ChildWait::Exited if stdout_done.is_some() && stderr_done.is_some() => {
                    if std::time::Instant::now() >= end {
                        return timeout_err(&mut child, &mut stdout_done, &mut stderr_done);
                    }
                    return match child.wait() {
                        Ok(status) => Ok(std::process::Output {
                            status,
                            stdout: stdout_done.unwrap_or_default(),
                            stderr: stderr_done.unwrap_or_default(),
                        }),
                        Err(e) => Err(ProbeError::Spawn(e)),
                    };
                }
                ChildWait::NotChild => {
                    if std::time::Instant::now() >= end {
                        return timeout_err(&mut child, &mut stdout_done, &mut stderr_done);
                    }
                    return match child.wait() {
                        Ok(status) => Ok(std::process::Output {
                            status,
                            stdout: stdout_done.unwrap_or_default(),
                            stderr: stderr_done.unwrap_or_default(),
                        }),
                        Err(e) => Err(ProbeError::Spawn(e)),
                    };
                }
                ChildWait::Running | ChildWait::Exited => {
                    if let Some(rem) = end.checked_duration_since(std::time::Instant::now()).filter(|d| !d.is_zero()) {
                        std::thread::sleep(std::time::Duration::from_millis(10).min(rem));
                    } else {
                        return timeout_err(&mut child, &mut stdout_done, &mut stderr_done);
                    }
                }
            }
        }
    }

    #[cfg(not(unix))]
    {
        let mut exited: Option<std::process::ExitStatus> = None;
        loop {
            if std::time::Instant::now() >= end {
                return timeout_err(&mut child, &mut stdout_done, &mut stderr_done);
            }
            poll_pipe(&mut stdout_done, &rx_out);
            poll_pipe(&mut stderr_done, &rx_err);
            if exited.is_none() {
                match child.try_wait() {
                    Ok(Some(st)) => exited = Some(st),
                    Ok(None) => {}
                    Err(e) => {
                        kill_and_reap_group(&mut child, pid, &mut stdout_done, &mut stderr_done, &rx_out, &rx_err, cleanup_deadline);
                        return Err(ProbeError::Spawn(e));
                    }
                }
            }
            if let (Some(status), Some(stdout), Some(stderr)) = (exited, stdout_done.as_ref(), stderr_done.as_ref()) {
                if std::time::Instant::now() >= end {
                    return timeout_err(&mut child, &mut stdout_done, &mut stderr_done);
                }
                return Ok(std::process::Output { status, stdout: stdout.clone(), stderr: stderr.clone() });
            }
            if let Some(rem) = end.checked_duration_since(std::time::Instant::now()).filter(|d| !d.is_zero()) {
                std::thread::sleep(std::time::Duration::from_millis(10).min(rem));
            } else {
                return timeout_err(&mut child, &mut stdout_done, &mut stderr_done);
            }
        }
    }
}

/// Run a probe `c` in its own process group/job (stdin on /dev/null, stdout and stderr captured) for at most `limit`:
/// returns `Ok(Output)` when BOTH the child exited and stdout/stderr reached EOF within `limit`, `Err(ProbeError::Spawn(_))` when it
/// could not start or poll failed, or `Err(ProbeError::Timeout)` when either the child or any pipe-holding descendant ran past `limit`
/// (then the entire process group/job is killed, the child is always reaped before returning, and the hub's log says so).
#[allow(dead_code)] // bp-plot includes this file
pub fn probe_output_result(c: std::process::Command, limit: std::time::Duration) -> Result<std::process::Output, ProbeError> {
    probe_output_until(c, std::time::Instant::now() + limit, None, limit)
}

#[cfg(all(test, unix))]
#[allow(dead_code)] // bp-plot includes this file
pub(crate) fn is_live_non_zombie(pid: &str) -> bool { tests::is_live_non_zombie(pid) }

/// Run a probe `c` (stdin on /dev/null, stdout/stderr captured) for at most `limit`: `Some(output)` when it exited in time, None when
/// it could not start or ran over (then its process group is killed and the child is reaped).
#[allow(dead_code)] // bp-plot includes this file
pub fn probe_output(c: std::process::Command, limit: std::time::Duration) -> Option<std::process::Output> {
    probe_output_result(c, limit).ok()
}
/// `kicad_probe` as a tokio command.
#[allow(dead_code)] // bp-plot includes this file
pub fn kicad_probe_tokio<S: AsRef<OsStr>>(cli: S) -> (tokio::process::Command, ProbeHome) {
    let h = ProbeHome::new(); let mut c = tokio_cmd(Kind::Local, cli); c.envs(h.env()); (c, h)
}
/// How often [`wait`] asks a child whether it has exited, at most.
const WAIT_POLL_MAX: std::time::Duration = std::time::Duration::from_millis(250);
/// A tokio child's exit, without depending on SIGCHLD alone (h04x3d, #1108/#1110). tokio's own `wait()` asks once and then waits for SIGCHLD; on Linux it has
/// a pidfd for each child, but on macOS that signal is all it has, and a hub there that missed it (a signal mask inherited from the desktop app's thread,
/// fixed in e16ae77c6) never saw a child exit that came after the first ask. So the exit is also asked for (`try_wait`, a `waitpid` on this child's pid
/// through tokio, which cannot take another child's status) every few ms at first, then every 250 ms; whichever comes first wins. tokio's `wait()` is
/// cancel-safe, so it is dropped and asked again on each round.
pub async fn wait(child: &mut tokio::process::Child) -> std::io::Result<std::process::ExitStatus> {
    let mut gap = std::time::Duration::from_millis(5);
    loop {
        if let Some(s) = child.try_wait()? { return Ok(s) }
        tokio::select! {
            s = child.wait() => return s,
            _ = tokio::time::sleep(gap) => {}
        }
        gap = (gap * 2).min(WAIT_POLL_MAX);
    }
}
/// `Command::output`, with the exit taken by [`wait`]: stdout and stderr read to their ends (both at once, so neither pipe fills), then the exit. As tokio's
/// `output()`, the child's stdin is null: nothing is ever written to it here, so a piped one would hold a child that reads it until the wait.
pub async fn output(c: &mut tokio::process::Command) -> std::io::Result<std::process::Output> {
    use tokio::io::AsyncReadExt;
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = c.spawn()?;
    let (mut o, mut e) = (child.stdout.take(), child.stderr.take());
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let read_o = async { if let Some(p) = o.as_mut() { p.read_to_end(&mut stdout).await.map(|_| ()) } else { Ok(()) } };
    let read_e = async { if let Some(p) = e.as_mut() { p.read_to_end(&mut stderr).await.map(|_| ()) } else { Ok(()) } };
    let (ro, re) = tokio::join!(read_o, read_e); ro?; re?;
    let status = wait(&mut child).await?;
    Ok(std::process::Output { status, stdout, stderr })
}
/// A child that was killed and is no longer needed: kill it, then reap it in the background with [`wait`] (for at most a minute), instead of dropping
/// it to tokio's orphan queue, which is only drained on a SIGCHLD. Needs a tokio runtime (every caller is async).
pub fn kill_and_reap(mut child: tokio::process::Child) {
    let _ = child.start_kill();
    tokio::spawn(async move { let _ = tokio::time::timeout(std::time::Duration::from_secs(60), wait(&mut child)).await; });
}
/// Kill a child and wait up to `limit` for its exit; one that is not seen to exit by then is reaped in the background ([`kill_and_reap`]), so the
/// caller is never held past `limit` and no zombie is left.
pub async fn kill_wait(mut child: tokio::process::Child, limit: std::time::Duration) {
    let _ = child.start_kill();
    if tokio::time::timeout(limit, wait(&mut child)).await.is_err() { kill_and_reap(child) }
}
/// `std::process::Command::new`, gated.
pub fn std_cmd<S: AsRef<OsStr>>(kind: Kind, program: S) -> std::process::Command {
    let p = program.as_ref().to_string_lossy().into_owned();
    let mut c = match refusal(kind, &p) { None => { let (prog, pre) = crate::plat::hub::launch(program.as_ref()); let mut c = std::process::Command::new(prog); c.args(pre); c } Some(why) => std::process::Command::new(dead(&p, &why)) };
    crate::plat::proc::default_signals(&mut c);
    #[cfg(windows)] { use std::os::windows::process::CommandExt; c.creation_flags(crate::plat::proc::NO_WINDOW); }
    if scrubbed(kind) { for k in hub_only_present() { c.env_remove(k); } }
    if let Some(d) = documents_for(kind) { c.env("KICAD_DOCUMENTS_HOME", d); }
    if let Some(d) = pycache_for(kind) { c.env("PYTHONPYCACHEPREFIX", d); }
    if let Some(d) = userbase_for(kind) { c.env("PYTHONUSERBASE", d); }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)] #[tokio::test] async fn a_childs_exit_is_taken_without_sigchld_and_a_killed_child_is_reaped() {
        let mut c = tokio::process::Command::new("sh"); c.args(["-c", "printf out; printf err >&2; exit 3"]);
        let o = output(&mut c).await.unwrap();
        assert_eq!((o.status.code(), o.stdout.as_slice(), o.stderr.as_slice()), (Some(3), &b"out"[..], &b"err"[..]));
        // a child that would read its stdin: output() gives it a null one, so it ends at once instead of waiting
        let mut r = tokio::process::Command::new("sh"); r.args(["-c", "cat; echo done"]).stdin(std::process::Stdio::piped());
        let o = tokio::time::timeout(std::time::Duration::from_secs(5), output(&mut r)).await.expect("stdin is null").unwrap();
        assert_eq!(o.stdout, b"done\n");
        // a child still running when it is waited for: the exit comes by asking, as it must where SIGCHLD is missed
        let mut ch = tokio::process::Command::new("sh").args(["-c", "sleep 0.3"]).spawn().unwrap();
        assert!(tokio::time::timeout(std::time::Duration::from_secs(5), wait(&mut ch)).await.unwrap().unwrap().success());
        // killed and handed over: reaped in the background, not left as a zombie
        let ch = tokio::process::Command::new("sleep").arg("30").spawn().unwrap(); let pid = ch.id().unwrap() as i32;
        kill_and_reap(ch);
        let mut gone = false;
        for _ in 0..100 { tokio::time::sleep(std::time::Duration::from_millis(20)).await; if unsafe { libc::kill(pid, 0) } != 0 { gone = true; break } }
        assert!(gone, "the killed child was reaped");
        // kill_wait: back within its limit, the child gone
        let ch = tokio::process::Command::new("sleep").arg("30").spawn().unwrap(); let pid = ch.id().unwrap() as i32;
        let t = std::time::Instant::now(); kill_wait(ch, std::time::Duration::from_secs(2)).await;
        assert!(t.elapsed() < std::time::Duration::from_secs(3) && unsafe { libc::kill(pid, 0) } != 0, "killed and reaped within the limit");
    }

    #[test] fn a_test_never_runs_the_real_tailscale() {
        let tmp = std::path::Path::new(if cfg!(windows) { "C:\\Users\\x\\AppData\\Local\\Temp" } else { "/tmp" });
        for p in ["tailscale", "tailscale.exe", "TailScale.EXE", "/usr/bin/tailscale", "/Applications/Tailscale.app/Contents/MacOS/Tailscale", "C:\\Program Files\\Tailscale\\tailscale.exe", "C:/Program Files/Tailscale/tailscale.exe", "/tmp-evil/tailscale"] {
            assert!(test_refusal(p, tmp).is_some(), "{p} would run the machine's tailscale");
        }
        let standin = tmp.join("bp-ts-standin").join("tailscale"); let standin = standin.to_str().unwrap();
        assert!(test_refusal(standin, tmp).is_none(), "a stand-in under the temporary folder runs");
        for p in ["curl", "tailscaled-helper", "git", "/usr/bin/tail"] { assert!(test_refusal(p, tmp).is_none(), "{p} is not tailscale") }
        // through the gate the hub uses: the real one is never started, and a caller sees an error (Tailscale unavailable)
        assert!(std_cmd(Kind::Net, "tailscale").args(["status", "--json"]).output().is_err(), "std_cmd ran the real tailscale");
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        assert!(rt.block_on(tokio_cmd(Kind::Net, "tailscale").args(["status", "--json"]).output()).is_err(), "tokio_cmd ran the real tailscale");
    }
    #[test] fn on_macos_every_child_but_a_terminal_gets_kicads_documents_folder_unless_the_hub_was_given_one() {
        let d = std::path::Path::new("/h/kicad/documents");
        for k in [Kind::Local, Kind::Agent, Kind::Shell, Kind::Net, Kind::Browser, Kind::PersonTurn] {
            assert_eq!(kicad_documents(true, k, false, Some(d)), Some(d), "{k:?}");
        }
        assert_eq!(kicad_documents(true, Kind::Terminal, false, Some(d)), None, "a person's terminal: their own KiCad keeps its own folder");
        assert_eq!(kicad_documents(true, Kind::Local, true, Some(d)), None, "the hub was started with one: it stays");
        assert_eq!(kicad_documents(false, Kind::Local, false, Some(d)), None, "macOS only");
        assert_eq!(kicad_documents(true, Kind::Local, false, None), None, "not set up (tests, bp-plot)");
    }

    #[test] fn every_child_but_a_terminal_keeps_pythons_caches_in_the_hubs_folder_unless_the_hub_was_given_one() {
        let d = std::path::Path::new("/h/cache/pycache");
        for k in [Kind::Local, Kind::Agent, Kind::Shell, Kind::Net, Kind::Browser, Kind::PersonTurn] { assert_eq!(pycache(k, false, Some(d)), Some(d), "{k:?}"); }
        assert_eq!(pycache(Kind::Terminal, false, Some(d)), None, "a person's terminal keeps their own Python's ways");
        assert_eq!(pycache(Kind::Local, true, Some(d)), None, "the hub was started with one: it stays");
        assert_eq!(pycache(Kind::Local, false, None), None, "not set up (tests, bp-plot)");
        // the real builders carry it
        let _ = PYCACHE.set(d.to_path_buf());
        if std::env::var_os("PYTHONPYCACHEPREFIX").is_none() {
            let c = std_cmd(Kind::Local, "freecadcmd");
            assert!(c.get_envs().any(|(k, v)| k == "PYTHONPYCACHEPREFIX" && v == Some(d.as_os_str())));
            assert!(!std_cmd(Kind::Terminal, "sh").get_envs().any(|(k, _)| k == "PYTHONPYCACHEPREFIX"));
        }
    }

    #[test] fn the_user_base_sets_the_cache_prefix_in_every_layout_site_py_reads() {
        let d = crate::plat::tmp::dir("pyub"); write_userbase(&d).unwrap();
        for s in userbase_sites(&d) { assert_eq!(std::fs::read_to_string(s.join("usercustomize.py")).unwrap(), USERCUSTOMIZE); }
        assert!(d.join("lib/python3.11/site-packages/usercustomize.py").is_file(), "FreeCAD's Python 3.11");
        assert!(USERCUSTOMIZE.contains("sys.pycache_prefix = _p") && USERCUSTOMIZE.contains("site.addsitedir"));
        // a real Python, isolated as FreeCAD starts its own (-I ignores PYTHON*), still takes the prefix through the user base: -E -s are
        // FreeCAD's flags but for the user site, which FreeCAD keeps on, so run with -E alone
        // (a Unix Python's layout, lib/python3.N: the hub sets the user base on macOS only; Windows' Python reads Python3NN\site-packages)
        if cfg!(unix) { if let Ok(o) = std::process::Command::new("python3").args(["-E", "-c", "import sys; print(sys.pycache_prefix)"]).env("PYTHONUSERBASE", &*d).env("PYTHONPYCACHEPREFIX", "/x/pyc").output() {
            if o.status.success() { assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "/x/pyc", "{}", String::from_utf8_lossy(&o.stderr)); }
        } }
        let _ = std::fs::remove_dir_all(&*d);
    }

    // side-by-side: the person's turn is the one real agent on a copy, and nothing else opens up with it
    #[test] fn a_person_turn_is_refused_on_a_copy_by_the_pure_rule_and_free_off_a_copy() {
        assert!(refusal_with(true, Kind::PersonTurn, "/home/u/.local/bin/claude", None).is_some(), "refused unless side-by-side lets it through (the global `refusal`)");
        assert!(refusal_with(false, Kind::PersonTurn, "claude", None).is_none(), "not a copy: no gate");
    }
    #[test] fn hub_only_names() {
        for k in ["BACKPLANE_BUILDS", "BACKPLANE_COMMIT", "BACKPLANE_GUARD", "BACKPLANE_GUARD_STABLE", "BACKPLANE_SAFE", "BACKPLANE_UPDATE_FEED", "BACKPLANE_HTTPS_PORT", "BACKPLANE_URL", "BACKPLANE_APNS_KEY_ID"] { assert!(hub_only(k), "{k}"); }
        for k in ["BACKPLANE_NO_UPDATE", "BACKPLANE_AGENT_OWNER", "BACKPLANE_AGENTS", "PATH", "BACKPLANE_GUARDIAN"] { assert!(!hub_only(k), "{k}"); }
    }
    /// What prints the environment: /usr/bin/env, or on Windows Git's shell's `env`.
    const ENV: (&str, &[&str]) = if cfg!(windows) { ("sh", &["-c", "env"]) } else { ("/usr/bin/env", &[]) };
    fn env_cmd(mut c: tokio::process::Command) -> tokio::process::Command { c.args(ENV.1); c }
    #[tokio::test] async fn hub_env_probe_child() { // run by the test below with the hub's variables set, as a guarded dev hub has them
        if std::env::var("BP_HUB_ENV_PROBE").is_err() { return }
        let leaked = |out: &[u8]| String::from_utf8_lossy(out).lines().filter(|l| l.split('=').next().map_or(false, hub_only)).map(str::to_string).collect::<Vec<_>>();
        for k in [Kind::Agent, Kind::PersonTurn, Kind::Shell, Kind::Terminal] {
            let o = env_cmd(tokio_cmd(k, ENV.0)).output().await.unwrap(); assert!(o.status.success(), "{k:?}");
            assert!(leaked(&o.stdout).is_empty(), "{k:?} saw {:?}", leaked(&o.stdout));
            assert!(String::from_utf8_lossy(&o.stdout).lines().any(|l| l == "BACKPLANE_NO_UPDATE=1"), "{k:?}: the hold on updates stays");
            let o = { let mut c = std_cmd(k, ENV.0); c.args(ENV.1); c }.output().unwrap(); assert!(leaked(&o.stdout).is_empty(), "std {k:?} saw {:?}", leaked(&o.stdout));
        }
        let o = { let mut c = std_cmd(Kind::Local, ENV.0); c.args(ENV.1); c }.output().unwrap(); assert!(!leaked(&o.stdout).is_empty(), "our own helpers still get them");
        println!("hub_env_probe_ok");
    }
    #[test] fn an_agent_shell_or_terminal_never_sees_the_guards_or_updaters_variables() {
        let me = std::env::current_exe().unwrap();
        let mut c = std::process::Command::new(me); c.args(["proc::tests::hub_env_probe_child", "--exact", "--nocapture", "--test-threads=1"]).env("BP_HUB_ENV_PROBE", "1").env_remove("BACKPLANE_COPY");
        for (k, v) in [("BACKPLANE_BUILDS", "/home/x/.backplane-gpui-dev"), ("BACKPLANE_BUILD", "20261106-1"), ("BACKPLANE_COMMIT", "abc1234"), ("BACKPLANE_GUARD", "1"), ("BACKPLANE_GUARD_STABLE", "60"),
            ("BACKPLANE_SAFE", "1"), ("BACKPLANE_SAFE_SKIP", "bots"), ("BACKPLANE_CARRY_WAIT", "60"), ("BACKPLANE_FAULT", "panic:bots"), ("BACKPLANE_UPDATE_FEED", "http://x"), ("BACKPLANE_NO_UPDATE", "1"),
            ("BACKPLANE_HTTPS_PORT", "8451"), ("BACKPLANE_URL", "https://desktop.example:8451"), ("BACKPLANE_NAME", "desktop"), ("BACKPLANE_HOOK_URL", "http://127.0.0.1:3797"), ("BACKPLANE_PEERS", "a,b"),
            ("BACKPLANE_APNS_KEY_ID", "K"), ("BACKPLANE_APNS_PRIVATE_KEY_PATH", "/home/x/AuthKey.p8")] { c.env(k, v); }
        let o = c.output().unwrap();
        assert!(o.status.success() && String::from_utf8_lossy(&o.stdout).contains("hub_env_probe_ok"), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    }
    #[test] fn side_by_side_probe_child() { // run by the tests below with BACKPLANE_COPY=1 and the flag as given
        let Ok(mode) = std::env::var("BP_SBS_PROBE") else { return };
        assert!(copy::on());
        let person = refusal(Kind::PersonTurn, "/home/u/.local/bin/claude").is_none();
        assert_eq!(person, mode == "on", "PersonTurn runs only with the flag");
        assert_eq!(copy::person_turn_allowed(), mode == "on");
        for (k, p) in [(Kind::Agent, "/home/u/.local/bin/claude"), (Kind::Net, "curl"), (Kind::Shell, "sh"), (Kind::Terminal, "/bin/bash"), (Kind::Browser, "b"), (Kind::Local, "gh")] {
            assert!(refusal(k, p).is_some(), "{k:?} {p} stays refused in side-by-side mode");
        }
        println!("sbs_probe_ok {mode}");
    }
    fn sbs_child(mode: &str, flag: Option<&str>) -> String {
        let mut c = std::process::Command::new(std::env::current_exe().unwrap()); c.args(["proc::tests::side_by_side_probe_child", "--exact", "--nocapture", "--test-threads=1"]).env("BACKPLANE_COPY", "1").env("BP_SBS_PROBE", mode).env_remove("BACKPLANE_AGENTS");
        match flag { Some(f) => { c.env("BACKPLANE_SIDE_BY_SIDE", f); } None => { c.env_remove("BACKPLANE_SIDE_BY_SIDE"); } }
        let o = c.output().unwrap(); assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)); String::from_utf8_lossy(&o.stdout).into_owned()
    }
    #[test] fn with_the_side_by_side_flag_only_a_person_turn_may_run_a_real_agent_and_every_other_gate_holds() { assert!(sbs_child("on", Some("1")).contains(&format!("sbs_probe_ok on"))) }
    #[test] fn without_the_flag_or_with_a_wrong_value_a_person_turn_is_refused_too() { assert!(sbs_child("off", None).contains("sbs_probe_ok off")); assert!(sbs_child("off", Some("yes")).contains("sbs_probe_ok off")); assert!(sbs_child("off", Some("0")).contains("sbs_probe_ok off")) }

    // a copy runs nothing that acts outside its home
    #[test] fn a_copy_refuses_network_terminal_browser_shell_and_foreign_agents() {
        for (k, p) in [(Kind::Net, "curl"), (Kind::Net, "/usr/bin/tailscale"), (Kind::Terminal, "/bin/bash"), (Kind::Browser, "/x/backplane-browser"), (Kind::Shell, "/bin/sh"),
            (Kind::Local, "curl"), (Kind::Local, "/usr/bin/ssh"), (Kind::Local, "gh"), (Kind::Agent, "/usr/bin/claude"), (Kind::Agent, "/home/u/.local/bin/codex")] {
            assert!(refusal_with(true, k, p, None).is_some(), "{k:?} {p}");
            assert!(refusal_with(true, k, p, Some("/tmp/standins")).is_some() || k == Kind::Agent && p.starts_with("/tmp/standins/"), "{k:?} {p}");
        }
        assert!(refusal_with(true, Kind::Agent, "/tmp/standins/claude", Some("/tmp/standins")).is_none(), "a stand-in agent runs");
        assert!(refusal_with(true, Kind::Agent, "/tmp/standins-evil/claude", Some("/tmp/standins")).is_some(), "a sibling folder is not the stand-in folder");
        // the folder named by another path to it (macOS: /var is /private/var), the agent by its real path: still the stand-in folder; a link out of it is not
        #[cfg(unix)] { let d = crate::plat::tmp::dir("standin-alias"); let real = d.join("real"); std::fs::create_dir_all(&real).unwrap(); std::fs::write(real.join("claude"), "").unwrap();
            let alias = d.join("alias"); std::os::unix::fs::symlink(&real, &alias).unwrap(); let prog = crate::plat::fs::canonicalize(&real.join("claude")).unwrap();
            assert!(refusal_with(true, Kind::Agent, &prog.to_string_lossy(), Some(&alias.to_string_lossy())).is_none(), "a stand-in by its real path, its folder by an alias");
            std::os::unix::fs::symlink("/usr/bin/env", real.join("out")).unwrap();
            assert!(refusal_with(true, Kind::Agent, &alias.join("out").to_string_lossy(), Some(&alias.to_string_lossy())).is_some(), "a link out of the folder is no stand-in"); }
        for (k, p) in [(Kind::Local, "git"), (Kind::Local, "/usr/bin/kicad-cli"), (Kind::Local, "sh"), (Kind::Local, "/x/backplane-window"), (Kind::Local, "nice")] { assert_eq!(refusal_with(true, k, p, None), None, "{k:?} {p}") }
    }
    #[test] fn a_hub_that_is_no_copy_refuses_nothing() {
        for (k, p) in [(Kind::Net, "curl"), (Kind::Terminal, "bash"), (Kind::Browser, "b"), (Kind::Shell, "sh"), (Kind::Agent, "claude"), (Kind::Local, "ssh")] { assert_eq!(refusal_with(false, k, p, None), None) }
    }
    #[test] fn a_refused_command_cannot_start_and_says_why() {
        let p = dead("curl", "x"); assert!(p.starts_with("/nonexistent/backplane-copy-refused/"));
        assert!(std::process::Command::new(&p).output().is_err());
    }

    /// The switch end to end: a child test process started with BACKPLANE_COPY=1 (copy::on reads it once) cannot make any of the
    /// outside-reaching children; a normal process can still start local ones.
    #[test] fn copy_child_probe() {
        if std::env::var("BP_COPY_PROBE").ok().as_deref() != Some("1") { return }
        assert!(copy::on(), "BACKPLANE_COPY=1 reached the child");
        for (k, p) in [(Kind::Net, "curl"), (Kind::Net, "tailscale"), (Kind::Terminal, "/bin/sh"), (Kind::Browser, "/bin/true"), (Kind::Shell, "/bin/sh"), (Kind::Agent, "/bin/true"), (Kind::Local, "ssh")] {
            assert!(std_cmd(k, p).output().is_err(), "{k:?} {p} ran on a copy");
            assert!(tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(tokio_cmd(k, p).output()).is_err(), "{k:?} {p} ran on a copy (tokio)");
        }
        assert!(std_cmd(Kind::Local, "true").output().map_or(false, |o| o.status.success()), "a local tool still runs");
    }
    #[test] fn a_copy_run_does_nothing_outside_its_home() {
        let me = std::env::current_exe().unwrap();
        let o = std::process::Command::new(me).args(["proc::tests::copy_child_probe", "--exact", "--nocapture", "--test-threads=1"])
            .env("BACKPLANE_COPY", "1").env("BP_COPY_PROBE", "1").env_remove("BACKPLANE_AGENTS").output().unwrap();
        assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    }

    /// What a child reports about its signals: SIGCHLD/SIGPIPE/SIGXFSZ dispositions, whether its mask is empty, and whether its own fork+waitpid sees the exit.
    #[cfg(unix)] #[test] fn sigchld_probe_inner() {
        if std::env::var_os("BP_SIG_INNER").is_none() { return }
        unsafe {
            let disp = |sig: i32| { let mut o: libc::sigaction = std::mem::zeroed(); libc::sigaction(sig, std::ptr::null(), &mut o); if o.sa_sigaction == libc::SIG_IGN { "IGN" } else if o.sa_sigaction == libc::SIG_DFL { "DFL" } else { "HANDLER" } };
            let mut m: libc::sigset_t = std::mem::zeroed(); libc::sigprocmask(libc::SIG_SETMASK, std::ptr::null(), &mut m);
            let blocked = (1..32).filter(|s| libc::sigismember(&m, *s) == 1).count();
            let pid = libc::fork(); if pid == 0 { libc::_exit(7) }
            let mut st = 0; let r = libc::waitpid(pid, &mut st, 0);
            let waited = r == pid && libc::WIFEXITED(st) && libc::WEXITSTATUS(st) == 7;
            println!("BPSIG chld={} xfsz={} blocked={} waited={}", disp(libc::SIGCHLD), disp(libc::SIGXFSZ), blocked, waited);
        }
    }
    /// The hub as it can be started (SIGCHLD, SIGPIPE, SIGXFSZ ignored; a signal blocked): the children it makes through the gate start with defaults.
    #[cfg(unix)] #[test] fn sigchld_probe_outer() {
        if std::env::var_os("BP_SIG_OUTER").is_none() { return }
        unsafe {
            for s in [libc::SIGCHLD, libc::SIGPIPE, libc::SIGXFSZ] { libc::signal(s, libc::SIG_IGN); }
            let mut m: libc::sigset_t = std::mem::zeroed(); libc::sigemptyset(&mut m); libc::sigaddset(&mut m, libc::SIGUSR1); libc::pthread_sigmask(libc::SIG_BLOCK, &m, std::ptr::null_mut());
        }
        let me = std::env::current_exe().unwrap();
        let args = ["proc::tests::sigchld_probe_inner", "--exact", "--nocapture", "--test-threads=1"];
        let mut c = std_cmd(Kind::Local, &me); c.args(args).env("BP_SIG_INNER", "1").env_remove("BP_SIG_OUTER");
        // SIGCHLD is ignored here, so wait() would fail with ECHILD (the hub's own symptom, #1108): read the pipe to its end instead
        c.stdout(std::process::Stdio::piped()); let mut ch = c.spawn().unwrap(); let mut out = String::new(); std::io::Read::read_to_string(ch.stdout.as_mut().unwrap(), &mut out).unwrap(); println!("STD {out}");
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let _g = rt.enter(); unsafe { libc::signal(libc::SIGCHLD, libc::SIG_DFL); } // tokio's own reaping needs it; SIGCHLD itself is covered by the std child above
         let mut t = tokio_cmd(Kind::Local, &me); t.args(args).env("BP_SIG_INNER", "1").env_remove("BP_SIG_OUTER");
        let o = rt.block_on(async move { t.output().await }).unwrap(); println!("TOKIO {}", String::from_utf8_lossy(&o.stdout));
    }
    #[cfg(unix)] #[test] fn a_child_starts_with_default_signals_and_an_empty_mask_even_when_the_hub_ignores_sigchld() {
        let mut c = std::process::Command::new(std::env::current_exe().unwrap());
        c.args(["proc::tests::sigchld_probe_outer", "--exact", "--nocapture", "--test-threads=1"]).env("BP_SIG_OUTER", "1").env_remove("BACKPLANE_COPY");
        let o = c.output().unwrap(); let out = String::from_utf8_lossy(&o.stdout).into_owned();
        assert!(o.status.success(), "{out}{}", String::from_utf8_lossy(&o.stderr));
        let want = "BPSIG chld=DFL xfsz=DFL blocked=0 waited=true";
                assert!(out.contains(want), "std child (SIGCHLD ignored, so every ignored signal is reset): {out}");
        // the tokio child runs with SIGCHLD at its default (tokio reaps by it), so only its mask and SIGCHLD are checked
        assert!(out.matches("chld=DFL").count() == 2 && out.matches("blocked=0 waited=true").count() == 2, "std and tokio children: {out}");
    }

    /// Non-test source lines up to the first inline `#[cfg(test)] mod <name> {` module (forward declarations such as `#[cfg(test)] mod tests;` stay).
    fn non_test_code(text: &str) -> String {
        fn is_cfg_test(t: &str) -> bool {
            let Some(rest) = t.strip_prefix("#[cfg(") else { return false };
            let Some((args, _)) = rest.split_once(")]") else { return false };
            // Strip balanced `not(...)` clauses so `#[cfg(not(test))]` and `#[cfg(all(not(test), unix))]` stay production code.
            let mut filtered = String::with_capacity(args.len());
            let b = args.as_bytes();
            let mut i = 0;
            while i < b.len() {
                if b[i..].starts_with(b"not(") {
                    let mut depth = 1usize;
                    let mut j = i + 4;
                    while j < b.len() && depth > 0 {
                        if b[j] == b'(' { depth += 1; } else if b[j] == b')' { depth -= 1; }
                        j += 1;
                    }
                    i = j;
                } else {
                    let ch = args[i..].chars().next().unwrap();
                    filtered.push(ch);
                    i += ch.len_utf8();
                }
            }
            filtered.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').any(|w| w == "test")
        }
        fn starts_mod(rest: &str) -> Option<&str> {
            let mut item = rest.trim_start();
            while item.starts_with("#[") {
                let Some((_, after)) = item.split_once(']') else { return None };
                item = after.trim_start();
            }
            let s = item.strip_prefix("pub").map(|r| {
                let r = r.trim_start();
                if r.starts_with('(') { r.split_once(')').map_or(r, |x| x.1).trim_start() } else { r }
            }).unwrap_or(item).trim_start();
            s.strip_prefix("mod ")
        }
        fn inline_mod_at(lines: &[&str], idx: usize, same_line: &str) -> bool {
            let first = if same_line.is_empty() {
                let mut k = idx + 1;
                while k < lines.len() {
                    let s = lines[k].trim();
                    if s.is_empty() || s.starts_with("//") || s.starts_with("#[") { k += 1; } else { break; }
                }
                if k >= lines.len() { return false }
                (k, lines[k].trim())
            } else {
                (idx, same_line)
            };
            let Some(after) = starts_mod(first.1) else { return false };
            if let Some((name, _)) = after.split_once('{') {
                return !name.contains(';') && !name.trim().is_empty();
            }
            if after.contains(';') { return false }
            let mut k = first.0 + 1;
            while k < lines.len() {
                let s = lines[k].trim();
                if s.is_empty() || s.starts_with("//") { k += 1; continue }
                return s.starts_with('{');
            }
            false
        }
        let lines: Vec<&str> = text.lines().collect();
        let mut out = String::new();
        for (i, &l) in lines.iter().enumerate() {
            let t = l.trim();
            if is_cfg_test(t) {
                if let Some((_, after)) = l.split_once(")]") {
                    if inline_mod_at(&lines, i, after.trim()) { break }
                }
            }
            out.push_str(l);
            out.push('\n');
        }
        out
    }

    #[test] fn non_test_code_keeps_code_after_forward_test_mod_declarations() {
        let s = "fn a() {}\n#[cfg(test)] mod tests;\n#[cfg(test)] mod laws;\n#[cfg(not(test))] mod prod { Command::new(\"git\"); }\n#[cfg(all(not(test), unix))] mod prod2 { Command::new(\"sh\"); }\nfn b() { Command::new(\"git\"); }\n#[cfg(test)]\n#[allow(dead_code)]\nmod inline_tests\n{\n    fn t() { Command::new(\"x\"); }\n}\n";
        let got = non_test_code(s);
        assert!(got.contains("fn b()"), "must keep code after `#[cfg(test)] mod tests;`: {got}");
        assert!(got.contains("mod prod"), "must keep #[cfg(not(test))] production module: {got}");
        assert!(got.contains("mod prod2"), "must keep #[cfg(all(not(test), unix))] production module: {got}");
        assert!(!got.contains("inline_tests"), "must stop at inline test module with intervening attrs / next-line brace: {got}");
        assert!(non_test_code("#[cfg(test)] pub(crate) mod t { fn x() {} }\n").is_empty());
    }

    /// The gate is the only way to make a process: no other non-test `Command::new` anywhere under src/ (src/bin has its own crates' helpers).
    #[test] fn every_child_process_is_made_through_proc() {
        fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) { for e in std::fs::read_dir(d).unwrap().flatten() { let p = e.path(); if p.is_dir() { if p.file_name().map_or(false, |n| n == "bin") { continue } walk(&p, out) } else if p.extension().map_or(false, |x| x == "rs") { out.push(p) } } }
        let mut files = vec![]; walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let mut bad = vec![];
        for f in files {
            if f.ends_with("proc.rs") { continue }
            let text = std::fs::read_to_string(&f).unwrap();
            let name = f.to_string_lossy().replace('\\', "/");
            if name.ends_with("tests.rs") || name.ends_with("copy_gate.rs") || name.ends_with("copy_outs.rs") || name.contains("/tests/") { continue }
            let code = non_test_code(&text);
            for (i, l) in code.lines().enumerate() { if l.contains("Command::new(") && !l.trim_start().starts_with("//") { bad.push(format!("{}:{}", name, i + 1)) } }
        }
        assert!(bad.is_empty(), "build children with crate::proc::{{tokio_cmd, std_cmd}}: {bad:?}");
    }

    /// A raw socket to another machine is not a child process, so the gate above does not see it: every non-test connect is on this list,
    /// and each entry says why it is fine on a copy. A new one must be added here by a reviewer, not by accident.
    #[test] fn every_outbound_socket_is_on_the_allowlist() {
        // presenter.rs: loopback to our own policy child; kicad/nng.rs: the KiCad unix socket; net/tailnet.rs: `hello_with` returns at once on a copy;
        // agents/mcp_stdio.rs: the `--mcp-stdio` bridge an agent starts, loopback only (`endpoint` refuses any other host) to the hub's own MCP server;
        // net/alwayson/tunnel.rs: `run_local`, 127.0.0.1 only, to this hub's own untrusted door for calls that came over the reverse link (never started on a copy);
        // net/webprovision.rs: `system_proxy_identity` reads peer_cred() on /run/tailscale/tailscaled.sock (sends no bytes; returns PermissionDenied immediately on a copy);
        // net/peerws.rs: `start` and `connect_outbound` return at once on a copy, never opening an outbound socket
        const ALLOWED: [&str; 7] = ["presenter.rs", "kicad/nng.rs", "net/tailnet.rs", "agents/mcp_stdio.rs", "net/alwayson/tunnel.rs", "net/webprovision.rs", "net/peerws.rs"];
        fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) { for e in std::fs::read_dir(d).unwrap().flatten() { let p = e.path(); if p.is_dir() { if p.file_name().map_or(false, |n| n == "bin") { continue } walk(&p, out) } else if p.extension().map_or(false, |x| x == "rs") { out.push(p) } } }
        let mut files = vec![]; walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let mut bad = vec![];
        for f in files {
            let name = f.to_string_lossy().replace('\\', "/");
            if name.ends_with("tests.rs") || name.ends_with("e2e.rs") || name.ends_with("copy_gate.rs") || name.ends_with("copy_outs.rs") || name.contains("/tests/") || ALLOWED.iter().any(|a| name.ends_with(a)) { continue }
            let text = std::fs::read_to_string(&f).unwrap();
            let code = non_test_code(&text);
            for (i, l) in code.lines().enumerate() {
                let l = l.trim_start(); if l.starts_with("//") { continue }
                if ["TcpStream::connect", "UdpSocket", "TcpSocket", "connect_async", "UnixStream::connect"].iter().any(|k| l.contains(k)) { bad.push(format!("{name}:{}", i + 1)) }
            }
        }
        assert!(bad.is_empty(), "an outbound socket outside the allowlist (add it to proc.rs's list with a copy-guard): {bad:?}");
    }

    #[test]
    fn probe_home_seeds_python_interpreter_and_inhibits_writes() {
        let h = ProbeHome::new();
        let root = h.path().to_path_buf();
        for (ver, schema) in [("10.99", 7u64), ("10.0", 6), ("9.0", 3), ("8.0", 3)] {
            let raw = std::fs::read_to_string(root.join(ver).join("kicad_common.json")).unwrap();
            let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(v["api"]["enable_server"], serde_json::json!(false), "{ver}");
            assert!(v["api"]["interpreter_path"].as_str().is_some_and(|s| !s.is_empty()), "{ver}");
            assert_eq!(v["meta"]["filename"], "kicad_common.json", "{ver}");
            assert_eq!(v["meta"]["version"], serde_json::json!(schema), "{ver}");
        }
        let env = h.env();
        assert!(env.iter().any(|(k, v)| *k == "KICAD_CONFIG_HOME" && *v == root.as_os_str()));
        assert!(env.iter().any(|(k, v)| *k == "KICAD_CACHE_HOME" && *v == root.join("cache").as_os_str()));
        assert!(env.iter().any(|(k, v)| *k == "KICAD_DOCUMENTS_HOME" && *v == root.join("documents").as_os_str()));
        assert!(env.iter().any(|(k, v)| *k == "KICAD_INHIBIT_SETTINGS_WRITES" && v == "1"));
        drop(h);
        assert!(!root.exists(), "ProbeHome drop removes throwaway folder");
    }

    #[cfg(unix)]
    fn warm_script(cli: &std::path::Path) {
        let t = std::time::Instant::now();
        loop {
            match std::process::Command::new(cli).arg("--bp-test-noop").status() {
                Err(e) if e.raw_os_error() == Some(crate::plat::errno::ETXTBSY) && t.elapsed() < std::time::Duration::from_secs(30) => {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                _ => break,
            }
        }
    }

    #[cfg(unix)]
    fn pid_signal_ok(pid: &str) -> bool {
        std::process::Command::new("kill")
            .args(["-0", pid])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    #[cfg(unix)]
    pub(super) fn is_live_non_zombie(pid: &str) -> bool {
        if !pid_signal_ok(pid) { return false; }
        #[cfg(target_os = "linux")]
        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            if let Some((_, rest)) = stat.rsplit_once(')') {
                if rest.trim_start().starts_with('Z') { return false; }
            }
        }
        #[cfg(target_os = "macos")]
        if let Ok(o) = std::process::Command::new("ps").args(["-o", "state=", "-p", pid]).output() {
            if String::from_utf8_lossy(&o.stdout).trim().starts_with('Z') { return false; }
        }
        true
    }

    #[cfg(unix)]
    #[test]
    fn probe_output_result_captures_stdout_and_stderr() {
        use std::os::unix::fs::PermissionsExt;
        let d = crate::plat::tmp::dir("probe-out");
        let cli = d.join("kicad-cli");
        std::fs::write(&cli, "#!/bin/sh\necho out-line\necho err-line >&2\n").unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Do not pre-warm here so probe_output_result's built-in ETXTBSY retry is exercised directly.
        let mut c = std_cmd(Kind::Local, &cli);
        c.arg("--version");
        let out = probe_output_result(c, std::time::Duration::from_secs(5)).expect("ok");
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "out-line");
        assert_eq!(String::from_utf8_lossy(&out.stderr).trim(), "err-line");

        let missing = std_cmd(Kind::Local, d.join("missing-kicad-cli"));
        assert!(matches!(probe_output_result(missing, std::time::Duration::from_secs(2)), Err(ProbeError::Spawn(_))));
    }

    #[cfg(unix)]
    #[test]
    fn probe_output_kills_sigterm_ignoring_process_group_and_reaps_child() {
        use std::os::unix::fs::PermissionsExt;
        let d = crate::plat::tmp::dir("probe-kill-grp");
        let (cli, parent_pid_file, gc_pid_file) = (d.join("kicad-cli"), d.join("parent.pid"), d.join("gc.pid"));
        std::fs::write(
            &cli,
            format!(
                "#!/bin/sh\n[ \"$1\" = \"--bp-test-noop\" ] && exit 0\ntrap '' TERM\nsh -c \"trap '' TERM; echo \\$\\$ > '{}'; exec sleep 60\" &\nwhile [ ! -s '{}' ]; do sleep 0.01; done\necho $$ > '{}'\nexec sleep 60\n",
                gc_pid_file.display(),
                gc_pid_file.display(),
                parent_pid_file.display(),
            ),
        )
        .unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        warm_script(&cli);

        let mut c = std_cmd(Kind::Local, &cli);
        c.arg("--version");
        let at = std::time::Instant::now();
        let res = probe_output_result(c, std::time::Duration::from_secs(2));
        assert!(matches!(res, Err(ProbeError::Timeout)), "{res:?}");
        assert!(at.elapsed() < std::time::Duration::from_secs(10), "bounded: {:?}", at.elapsed());

        let ppid = std::fs::read_to_string(&parent_pid_file).unwrap().trim().to_string();
        let gpid = std::fs::read_to_string(&gc_pid_file).unwrap().trim().to_string();
        assert!(!ppid.is_empty() && !gpid.is_empty(), "ppid={ppid:?} gpid={gpid:?}");
        assert!(!pid_signal_ok(&ppid), "direct child pid {ppid} was synchronously killed and reaped");
        let gc_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while is_live_non_zombie(&gpid) && std::time::Instant::now() < gc_deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!is_live_non_zombie(&gpid), "grandchild pid {gpid} in the probe's process group was terminated");
    }

    #[cfg(unix)]
    #[test]
    fn probe_output_kills_descendant_holding_pipes_after_leader_exits() {
        use std::os::unix::fs::PermissionsExt;
        let d = crate::plat::tmp::dir("probe-pipe-hold");
        let (cli, parent_pid_file, gc_pid_file) = (d.join("kicad-cli"), d.join("parent.pid"), d.join("gc.pid"));
        std::fs::write(
            &cli,
            format!(
                "#!/bin/sh\n[ \"$1\" = \"--bp-test-noop\" ] && exit 0\nsh -c \"trap '' TERM; echo \\$\\$ > '{}'; exec sleep 60\" &\nwhile [ ! -s '{}' ]; do sleep 0.01; done\necho $$ > '{}'\necho 10.99.0\nexit 0\n",
                gc_pid_file.display(),
                gc_pid_file.display(),
                parent_pid_file.display(),
            ),
        )
        .unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        warm_script(&cli);

        let mut c = std_cmd(Kind::Local, &cli);
        c.arg("--version");
        let at = std::time::Instant::now();
        let res = probe_output_result(c, std::time::Duration::from_secs(2));
        assert!(matches!(res, Err(ProbeError::Timeout)), "descendant holding stdout pipe must time out cleanly: {res:?}");
        assert!(at.elapsed() < std::time::Duration::from_secs(10), "bounded: {:?}", at.elapsed());

        let ppid = std::fs::read_to_string(&parent_pid_file).unwrap().trim().to_string();
        let gpid = std::fs::read_to_string(&gc_pid_file).unwrap().trim().to_string();
        assert!(!ppid.is_empty() && !gpid.is_empty(), "ppid={ppid:?} gpid={gpid:?}");
        assert!(!pid_signal_ok(&ppid), "direct leader pid {ppid} was reaped after cleanup");
        let gc_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while is_live_non_zombie(&gpid) && std::time::Instant::now() < gc_deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!is_live_non_zombie(&gpid), "pipe-holding descendant pid {gpid} was killed");
    }

    #[cfg(unix)]
    #[test]
    fn probe_output_kills_stubborn_grandchild_when_leader_exits_on_sigterm() {
        use std::os::unix::fs::PermissionsExt;
        let d = crate::plat::tmp::dir("probe-leader-term");
        let (cli, parent_pid_file, gc_pid_file) = (d.join("kicad-cli"), d.join("parent.pid"), d.join("gc.pid"));
        std::fs::write(
            &cli,
            format!(
                "#!/bin/sh\n[ \"$1\" = \"--bp-test-noop\" ] && exit 0\nsh -c \"trap '' TERM; echo \\$\\$ > '{}'; exec sleep 60\" &\nwhile [ ! -s '{}' ]; do sleep 0.01; done\necho $$ > '{}'\nexec sleep 60\n",
                gc_pid_file.display(),
                gc_pid_file.display(),
                parent_pid_file.display(),
            ),
        )
        .unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        warm_script(&cli);

        let mut c = std_cmd(Kind::Local, &cli);
        c.arg("--version");
        let res = probe_output_result(c, std::time::Duration::from_secs(2));
        assert!(matches!(res, Err(ProbeError::Timeout)), "{res:?}");

        let ppid = std::fs::read_to_string(&parent_pid_file).unwrap().trim().to_string();
        let gpid = std::fs::read_to_string(&gc_pid_file).unwrap().trim().to_string();
        assert!(!ppid.is_empty() && !gpid.is_empty(), "ppid={ppid:?} gpid={gpid:?}");
        assert!(!pid_signal_ok(&ppid), "direct leader pid {ppid} was reaped");
        let gc_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while is_live_non_zombie(&gpid) && std::time::Instant::now() < gc_deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!is_live_non_zombie(&gpid), "SIGKILL still swept grandchild pid {gpid} after leader exited on SIGTERM");
    }

    #[cfg(unix)]
    #[test]
    fn probe_output_until_enforces_deadline_before_spawn_and_completion() {
        use std::os::unix::fs::PermissionsExt;
        let d = crate::plat::tmp::dir("probe-dl");
        let (early_cli, marker, late_cli) = (d.join("early"), d.join("ran"), d.join("late"));
        std::fs::write(&early_cli, format!("#!/bin/sh\n: > '{}'\necho ok\n", marker.display())).unwrap();
        std::fs::set_permissions(&early_cli, std::fs::Permissions::from_mode(0o755)).unwrap();

        let expired = std::time::Instant::now();
        let res_expired = probe_output_until(std_cmd(Kind::Local, &early_cli), expired, Some(expired), std::time::Duration::from_secs(1));
        assert!(matches!(res_expired, Err(ProbeError::Timeout)), "{res_expired:?}");
        assert!(!marker.exists(), "probe must not execute after deadline");

        std::fs::write(&late_cli, "#!/bin/sh\n[ \"$1\" = \"--bp-test-noop\" ] && exit 0\nsleep 0.12\necho late\n").unwrap();
        std::fs::set_permissions(&late_cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        warm_script(&late_cli);

        let mut c = std_cmd(Kind::Local, &late_cli);
        c.arg("--version");
        let end = std::time::Instant::now() + std::time::Duration::from_millis(25);
        let res_late = probe_output_until(c, end, Some(end), std::time::Duration::from_millis(25));
        assert!(matches!(res_late, Err(ProbeError::Timeout)), "late completion must time out: {res_late:?}");
    }
}

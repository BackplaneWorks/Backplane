//! Agent processes: where the CLI is found (stand-in folder only in test mode), spawn in a process group of its
//! own, bounded stdin/stdout queues, a waiter that always reaps, group kill. No process is left behind:
//! the child gets SIGKILL when the hub dies (PDEATHSIG) and when its handle drops.
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};

pub const LINE_CAP: usize = 32 << 20;   // one stream-json line, at most
pub const IN_QUEUE: usize = 256;         // lines waiting for a process's stdin
pub const OUT_QUEUE: usize = 1024;       // lines read and not yet handled: the pipe backs up behind it
pub const STDERR_TAIL: usize = 2048;

/// Where agent CLIs may come from. `BACKPLANE_AGENTS=<dir>` is test mode: only `<dir>/<cli>` runs, a missing
/// or non-executable file fails the turn ("<dir> has no runnable <cli>", as the JS hub's stand-in block does), and a hub
/// on a copy of a real log (`BACKPLANE_COPY=1`) without a stand-in folder runs no agent at all.
#[derive(Clone, Debug)]
pub struct Cli { dir: Option<PathBuf>, copy: bool }

impl Cli {
    pub fn from_env() -> Cli {
        Cli { dir: std::env::var("BACKPLANE_AGENTS").ok().filter(|d| !d.is_empty()).map(PathBuf::from), copy: std::env::var("BACKPLANE_COPY").map_or(false, |v| v == "1") }
    }
    pub fn standin(dir: &Path) -> Cli { Cli { dir: Some(dir.to_path_buf()), copy: false } }
    pub fn test_mode(&self) -> bool { self.dir.is_some() }
    pub fn dir(&self) -> Option<&Path> { self.dir.as_deref() }

    pub fn resolve(&self, cli: &str) -> Result<PathBuf, String> {
        if cli.is_empty() || cli.contains('/') { return Err(format!("not an agent: {cli}")) }
        match &self.dir {
            Some(d) => {
                let p = d.join(cli);
                let (cd, cp) = (crate::plat::fs::canonicalize(d).map_err(|_| format!("{} has no runnable {cli}", d.display()))?, crate::plat::fs::canonicalize(&p).map_err(|_| format!("{} has no runnable {cli}", d.display()))?);
                let exec = std::fs::metadata(&cp).map(|m| { use crate::plat::fs::PermissionsExt; m.is_file() && m.permissions().mode() & 0o111 != 0 }).unwrap_or(false);
                if exec && cp.starts_with(&cd) { Ok(cp) } else { Err(format!("{} has no runnable {cli}", d.display())) }
            }
            None if self.copy => Err(format!("a copy of a log runs agents only as stand-ins: {cli} refused")),
            None => find_in(search_dirs(), cli, cfg!(windows)).ok_or_else(|| format!("{cli} is not installed (looked in the hub's PATH, the login shell's and the usual install directories)")),
        }
    }
    /// The PATH a probe of an agent CLI runs with (`--version`, its model list), the one a turn gets: the merged search directories, or in test mode the
    /// stand-in folder before the hub's own PATH. A CLI that is a launcher script (pi's `#!/bin/sh` that execs a `#!/usr/bin/env node` one) needs it to find
    /// its interpreter; with the hub's bare PATH (a desktop launcher's or a service's) `pi --version` failed and pi was shown as not installed.
    pub fn child_path(&self) -> Option<std::ffi::OsString> {
        match &self.dir {
            Some(d) => std::env::join_paths(std::iter::once(d.clone()).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()))).ok(),
            None if self.copy => None,
            None => std::env::join_paths(search_dirs()).ok(),
        }
    }
}
/// The first of `dirs` that has `cli` as a file (a symlink to one counts), as named there (not canonicalized: a launcher script finds its own install from `$0`).
/// Windows: also `cli.exe` and npm's `cli.cmd`.
pub fn find_in(dirs: &[PathBuf], cli: &str, windows: bool) -> Option<PathBuf> {
    let names: Vec<String> = if windows { vec![cli.into(), format!("{cli}.exe"), format!("{cli}.cmd")] } else { vec![cli.into()] };
    dirs.iter().flat_map(|d| names.iter().map(move |n| d.join(n))).find(|c| c.is_file())
}

// ---- where the provider CLIs are: a hub started from a launcher or a service has a PATH without the login shell's additions (nvm, volta, ~/.local/bin...) ----
/// CliPath.merge: each directory once, the first of each, in order, none empty.
pub fn merge_dirs(parts: &[Vec<PathBuf>]) -> Vec<PathBuf> { let mut out: Vec<PathBuf> = vec![]; for d in parts.iter().flatten() { if !d.as_os_str().is_empty() && !out.contains(d) { out.push(d.clone()) } } out }
fn semver_key(s: &str) -> (u64, u64, u64) { let mut p = s.trim_start_matches('v').split('.').map(|x| x.parse::<u64>().unwrap_or(0)); (p.next().unwrap_or(0), p.next().unwrap_or(0), p.next().unwrap_or(0)) }
/// CliPath.usual: the usual install directories that exist, for this platform (see [`usual_dirs_for`]).
pub fn usual_dirs(home: &Path) -> Vec<PathBuf> { usual_dirs_for(home, cfg!(windows), cfg!(target_os = "macos"), &|k| std::env::var_os(k).filter(|v| !v.is_empty())) }
/// Each node version folder under `root` (`v24.13.1`, `24.13.1`...), newest first, joined with `tail`.
fn node_versions(root: &Path, tail: &str) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(root) else { return vec![] };
    let mut vs: Vec<String> = rd.flatten().filter_map(|e| e.file_name().into_string().ok()).collect(); vs.sort_by(|a, b| semver_key(b).cmp(&semver_key(a)));
    vs.into_iter().map(|n| root.join(n).join(tail)).collect()
}
/// Where agent CLIs and the node they run on are installed when no PATH says so: a hub started by a desktop launcher, a service or macOS's
/// Finder has a PATH without the login shell's additions, and an installer that only edits `~/.zshrc` (grok's, opencode's) is not in the login
/// shell's either when the shell reads no rc file. In order: node version managers (nvm, fnm; newest version first), JS tool folders (volta, bun,
/// deno, pnpm, yarn, npm's global prefix), version-manager shims (asdf, mise, nodenv), each agent's own installer folder (claude, grok, opencode,
/// pi and the node pi installs, antigravity), then the system's package folders. Windows has its own: npm's `%APPDATA%\npm`, nvm-windows'
/// `%NVM_SYMLINK%`, Program Files' node, scoop, winget, and the same per-user agent folders under `%USERPROFILE%`. Only those that exist.
/// `env` reads an environment variable (empty is unset).
pub fn usual_dirs_for(home: &Path, windows: bool, mac: bool, env: &dyn Fn(&str) -> Option<std::ffi::OsString>) -> Vec<PathBuf> {
    let h = |d: &str| home.join(d);
    let var = |k: &str| env(k).map(PathBuf::from);
    let agents = [".claude/local", ".local/bin", ".grok/bin", ".opencode/bin", ".pi/agent/bin", ".gemini/antigravity-cli/bin", ".omp/bin", ".muse/bin"];
    let mut v: Vec<PathBuf> = vec![];
    if windows {
        let appdata = var("APPDATA").unwrap_or_else(|| h("AppData/Roaming")); let local = var("LOCALAPPDATA").unwrap_or_else(|| h("AppData/Local"));
        if let Some(s) = var("NVM_SYMLINK") { v.push(s) }
        v.extend(node_versions(&var("NVM_HOME").unwrap_or_else(|| appdata.join("nvm")), ""));
        v.push(appdata.join("npm"));
        v.extend(node_versions(&var("FNM_DIR").unwrap_or_else(|| appdata.join("fnm")).join("node-versions"), "installation"));
        for d in [local.join("Volta/bin"), h(".bun/bin"), h(".deno/bin"), local.join("pnpm"), local.join("Yarn/bin")] { v.push(d) }
        for d in agents { v.push(h(d)) }
        for d in [h("scoop/shims"), local.join("Microsoft/WinGet/Links"), h(".cargo/bin")] { v.push(d) }
        for k in ["ProgramFiles", "ProgramW6432"] { if let Some(p) = var(k) { v.push(p.join("nodejs")); v.push(p.join("Volta")) } }
        return v.into_iter().filter(|d| d.is_dir()).collect();
    }
    let data = var("XDG_DATA_HOME").unwrap_or_else(|| h(".local/share"));
    v.extend(node_versions(&var("NVM_DIR").unwrap_or_else(|| h(".nvm")).join("versions/node"), "bin"));
    let fnm = var("FNM_DIR").into_iter().chain([data.join("fnm"), h(".fnm")]).chain(mac.then(|| h("Library/Application Support/fnm")));
    for f in fnm { v.extend(node_versions(&f.join("node-versions"), "installation/bin")) }
    for d in [h(".volta/bin"), h(".bun/bin"), h(".deno/bin"), var("PNPM_HOME").unwrap_or_else(|| if mac { h("Library/pnpm") } else { data.join("pnpm") }), h(".yarn/bin"), h(".config/yarn/global/node_modules/.bin"), h(".npm-global/bin"), h(".npm/bin")] { v.push(d) }
    for d in [h(".asdf/shims"), data.join("mise/shims"), h(".nodenv/shims")] { v.push(d) }
    for d in agents { v.push(h(d)) }
    v.push(data.join("pi-node/current/bin")); // the node pi's installer puts there without adding it to any profile
    v.push(h(".cargo/bin"));
    for d in ["/opt/homebrew/bin", "/home/linuxbrew/.linuxbrew/bin", "/usr/local/bin", "/opt/local/bin", "/snap/bin"] { v.push(PathBuf::from(d)) }
    v.into_iter().filter(|d| d.is_dir()).collect()
}
/// The login shell's PATH ($SHELL, five seconds at most for each way of asking), as directories; empty when it cannot be read.
fn login_dirs() -> Vec<PathBuf> { login_dirs_with(None) }
const MARK: &str = "__BP_PATH__"; const MARK_END: &str = "__BP_END__";
/// The PATH between the marks a probe shell printed, past whatever its rc files printed before or after it.
pub fn marked_path(out: &str) -> Option<&str> { let a = out.rfind(MARK)? + MARK.len(); let b = out[a..].find(MARK_END)? + a; Some(&out[a..b]).filter(|p| !p.trim().is_empty()) }
/// The login shell's PATH; `home`: run it with that HOME (the person's real one in side-by-side mode, where the hub's own is a fake).
/// Asked of an interactive login shell (`-ilc`) first, as an editor reads it: zsh reads `~/.zshrc` and bash `~/.bashrc` only when interactive, and that is
/// where installers put their folders (grok's `~/.grok/bin`, opencode's `~/.opencode/bin`, nvm). It runs in a session of its own (no terminal to take) with
/// nothing on stdin, and its PATH is read between marks, so a greeting an rc file prints is not taken for it. A shell that cannot (an rc that fails or hangs)
/// is asked again as a plain login shell (`-lc`).
pub fn login_dirs_with(home: Option<&Path>) -> Vec<PathBuf> {
    if !probes_login_shell(cfg!(windows)) { return vec![] }
    let sh = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/sh".into());
    shell_path_dirs(&sh, home, 5)
}
/// login_dirs_with, of the shell `sh`, each try given `secs`.
pub fn shell_path_dirs(sh: &str, home: Option<&Path>, secs: u64) -> Vec<PathBuf> {
    let script = format!("printf '\\n{MARK}%s{MARK_END}\\n' \"$PATH\"");
    for flags in ["-ilc", "-lc"] {
        let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, sh); c.args([flags, &script]).stdin(Stdio::null()).stderr(Stdio::null()); if let Some(h) = home { c.env("HOME", h); }
        crate::plat::proc::own_group(&mut c);
        let Ok(mut child) = c.stdout(Stdio::piped()).spawn() else { return vec![] };
        // read until the end mark, not to the end of output: something an rc file started in the background may hold stdout open
        let (tx, rx) = std::sync::mpsc::channel::<Option<String>>(); let so = child.stdout.take();
        std::thread::spawn(move || { use std::io::Read; let Some(mut so) = so else { let _ = tx.send(None); return }; let (mut out, mut b) = (String::new(), [0u8; 4096]);
            loop { match so.read(&mut b) { Ok(0) | Err(_) => { let _ = tx.send(None); return } Ok(n) => { out.push_str(&String::from_utf8_lossy(&b[..n])); if let Some(p) = marked_path(&out) { let _ = tx.send(Some(p.to_string())); return } if out.len() > 1 << 20 { let _ = tx.send(None); return } } } } });
        let got = rx.recv_timeout(std::time::Duration::from_secs(secs)).ok().flatten();
        // the shell is not reaped yet, so its group is still its own: end it with whatever it left running, then reap it
        crate::plat::proc::kill_group(child.id(), crate::plat::proc::KILL); let _ = child.kill(); let _ = child.wait();
        if let Some(p) = got { return std::env::split_paths(p.trim()).collect() }
    }
    vec![]
}
/// Whether the login shell's PATH is asked for. Not on Windows: there is no login shell whose profile adds to PATH (the hub inherits the
/// person's PATH from the registry), `$SHELL` there can name PowerShell (it was handed `-lc "printf %s \"$PATH\""` at every hub start on
/// bp-win), and Git's sh would print an MSYS PATH (`/c/...:...`) that the Windows path list does not read.
pub fn probes_login_shell(windows: bool) -> bool { !windows }
static SEARCH: std::sync::OnceLock<Vec<PathBuf>> = std::sync::OnceLock::new();
/// Where an agent CLI is looked for (and the PATH an agent runs with): the hub's own PATH first, then the login shell's, then the usual install directories. Worked out once.
pub fn search_dirs() -> &'static [PathBuf] {
    SEARCH.get_or_init(|| {
        let cur: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default();
        // what setup installs itself comes last, and is listed whether or not it exists yet: the list is worked out once, before the first install
        merge_dirs(&[cur, login_dirs(), usual_dirs(&home), vec![crate::settings::tools_install::tools_dir(), home.join(".local/bin")]])
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct Exit { pub code: Option<i32>, pub signal: Option<i32>, pub stderr: String }

pub struct Proc {
    pub pid: u32,
    pub input: mpsc::Sender<String>,
    pub lines: mpsc::Receiver<String>,
    pub exit: oneshot::Receiver<Exit>,
}

/// One line, at most `cap` bytes kept (the rest of an over-long line is read and dropped). None at EOF.
pub async fn read_capped<R: AsyncBufReadExt + Unpin>(r: &mut R, cap: usize) -> std::io::Result<Option<String>> {
    let mut buf: Vec<u8> = Vec::new(); let mut over = false; let mut any = false;
    loop {
        let chunk = r.fill_buf().await?;
        if chunk.is_empty() { return Ok(if any { Some(String::from_utf8_lossy(&buf).into_owned()) } else { None }) }
        any = true;
        let (take, done) = match chunk.iter().position(|b| *b == b'\n') { Some(i) => (i + 1, true), None => (chunk.len(), false) };
        if !over { let room = cap.saturating_sub(buf.len()); buf.extend_from_slice(&chunk[..take.min(room + 1).min(take)]); if buf.len() > cap { over = true; buf.truncate(cap) } }
        r.consume(take);
        if done { while buf.last().map_or(false, |b| *b == b'\n' || *b == b'\r') { buf.pop(); } return Ok(Some(String::from_utf8_lossy(&buf).into_owned())) }
    }
}

static OWNER: std::sync::OnceLock<String> = std::sync::OnceLock::new();
/// Every agent process carries BACKPLANE_AGENT_OWNER=<home>: after a hub is SIGKILLed (the kernel kills the agent itself, PDEATHSIG, but not what it
/// started), the next hub on that home ends what is left. One hub per home, so nothing live is taken.
pub fn set_owner(home: &str) { let _ = OWNER.set(home.to_string()); }
/// Whether `me` is an ancestor of `pid` (parent chain from /proc/<pid>/stat, bounded).
#[cfg(target_os = "linux")]
fn descends_from(pid: u32, me: u32) -> bool {
    let mut cur = pid;
    for _ in 0..16 {
        let Ok(s) = std::fs::read_to_string(format!("/proc/{cur}/stat")) else { return false };
        let Some(rest) = s.rsplit_once(')').map(|x| x.1) else { return false };
        let ppid: u32 = rest.split_whitespace().nth(1).and_then(|x| x.parse().ok()).unwrap_or(0);
        if ppid == me { return true } if ppid <= 1 { return false } cur = ppid;
    }
    false
}
/// Linux: the process table is /proc, each environment is /proc/<pid>/environ.
#[cfg(target_os = "linux")]
pub fn reap_orphans(home: &str) -> usize {
    let mark = format!("BACKPLANE_AGENT_OWNER={home}").into_bytes(); let me = std::process::id(); let mut n = 0;
    let Ok(rd) = std::fs::read_dir("/proc") else { return 0 };
    for e in rd.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else { continue }; if pid == me { continue }
        let Ok(env) = std::fs::read(format!("/proc/{pid}/environ")) else { continue };
        if !env.split(|b| *b == 0).any(|kv| kv == mark.as_slice()) { continue }
        if descends_from(pid, me) { continue } // this process's own agents are not orphans (only a previous hub's, reparented, are)
        crate::plat::proc::kill(pid, crate::plat::proc::KILL); n += 1
    }
    n
}
/// macOS, other Unix and Windows: no /proc, and no way to read another process's environment that is exact (`ps eww` joins argv and environment with spaces, so a
/// text match could end a process that only quotes the mark, or one whose home has a space). Until it reads KERN_PROCARGS2 entry by entry, or keeps a pid file with
/// start times, this ends nothing: an orphan left running is better than killing the wrong process.
#[cfg(not(target_os = "linux"))]
pub fn reap_orphans(_home: &str) -> usize { 0 }

/// Off Linux nothing ends an agent when the hub is killed (no PDEATHSIG), so one small watcher process per hub does: the hub keeps the write end of a pipe to it
/// (close-on-exec, so no agent holds it), registers each agent's process group, and when the pipe closes because the hub is gone the watcher kills every group still registered.
/// The watcher is this binary with `--agent-reaper`; it is its own session, so the signal that killed the hub does not reach it.
#[cfg(all(unix, not(target_os = "linux")))]
mod reaper {
    use std::io::Write;
    static PIPE: std::sync::Mutex<Option<std::process::ChildStdin>> = std::sync::Mutex::new(None);
    fn send(line: &str) -> bool {
        let mut g = PIPE.lock().unwrap();
        if g.is_none() {
            let Ok(exe) = std::env::current_exe() else { return false };
            let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, exe);
            #[cfg(not(test))] c.arg("--agent-reaper");
            #[cfg(test)] c.args(["agents::tests::reaper_child", "--exact", "--nocapture", "--test-threads=1"]).env("BP_REAPER_CHILD", "1"); // a test binary takes no such flag: its watcher is a test that only runs as that child
            c.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
            crate::plat::proc::own_group(&mut c);
            match c.spawn() { Ok(mut ch) => { *g = ch.stdin.take(); std::thread::spawn(move || { let _ = ch.wait(); }); } Err(_) => return false }
        }
        let ok = g.as_mut().map_or(false, |w| w.write_all(line.as_bytes()).and_then(|_| w.flush()).is_ok());
        if !ok { *g = None }
        ok
    }
    pub fn add(pid: u32) { if pid > 0 && !send(&format!("+{pid}\n")) { send(&format!("+{pid}\n")); } }
    pub fn remove(pid: u32) { if pid > 0 { let mut g = PIPE.lock().unwrap(); if let Some(w) = g.as_mut() { let _ = w.write_all(format!("-{pid}\n").as_bytes()).and_then(|_| w.flush()); } } }
}
/// Linux: PDEATHSIG; Windows: the agent's Job Object dies with the hub (plat::proc::adopt).
#[cfg(any(target_os = "linux", windows))]
mod reaper { pub fn add(_pid: u32) {} pub fn remove(_pid: u32) {} }

pub(crate) fn watch_group(pid: u32) { reaper::add(pid); }
pub(crate) fn unwatch_group(pid: u32) { reaper::remove(pid); }

/// How long the reaper lets each agent group end on SIGTERM before SIGKILL (#1109: a Claude CLI killed outright in the middle of its OAuth refresh leaves its refresh lock behind,
/// and every later start in that account folder refuses to refresh).
pub const REAPER_GRACE: std::time::Duration = std::time::Duration::from_millis(2500);
/// End each listed process group: SIGTERM to all at once, then wait up to `grace` for them to go, then SIGKILL to each group (and its leader) still there.
/// Returns how many groups needed the SIGKILL.
pub fn end_groups(pids: &[i32], grace: std::time::Duration) -> usize {
    use crate::plat::proc::{group_alive, kill, kill_group, KILL, TERM};
    let pids: Vec<u32> = pids.iter().filter(|p| **p > 1).map(|p| *p as u32).collect();
    for &p in &pids { kill_group(p, TERM); kill(p, TERM); }
    let gone = |p: u32| !group_alive(p);
    let end = std::time::Instant::now() + grace;
    while std::time::Instant::now() < end && !pids.iter().all(|p| gone(*p)) { std::thread::sleep(std::time::Duration::from_millis(50)) }
    let mut n = 0;
    for &p in &pids { if !gone(p) { n += 1; kill_group(p, KILL); kill(p, KILL); } }
    n
}

/// `backplane-hub --agent-reaper`: read "+pid" / "-pid" lines from stdin; at end of input (the hub is gone) end each process group still listed (`end_groups`: TERM, a short grace, KILL). Returns the exit code.
pub fn reaper_main() -> i32 {
    use std::io::BufRead;
    let mut live = std::collections::BTreeSet::<i32>::new();
    for l in std::io::stdin().lock().lines() {
        let Ok(l) = l else { break };
        let (sign, n) = l.split_at(l.len().min(1));
        let Ok(pid) = n.trim().parse::<i32>() else { continue };
        match sign { "+" => { live.insert(pid); } "-" => { live.remove(&pid); } _ => {} }
    }
    end_groups(&live.into_iter().collect::<Vec<_>>(), REAPER_GRACE);
    0
}

/// What a person's real agent runs with (SIDE-BY-SIDE.md check 6): the real HOME and the PATH worked out from it; every `BACKPLANE_*` of this hub is removed from its environment.
pub struct PersonChild { pub home: PathBuf, pub path: std::ffi::OsString }
pub fn spawn(program: &Path, args: &[String], cwd: &Path, extra_path: Option<&Path>, env: &[(String, String)]) -> std::io::Result<Proc> { spawn_with(crate::proc::Kind::Agent, None, program, args, cwd, extra_path, env) }
/// The only place a REAL agent CLI is made on a copy (`Kind::PersonTurn`, refused unless `copy::person_turn_allowed()`); the actor calls it only for a turn the person started.
pub fn spawn_person(person: &PersonChild, program: &Path, args: &[String], cwd: &Path, env: &[(String, String)]) -> std::io::Result<Proc> { spawn_with(crate::proc::Kind::PersonTurn, Some(person), program, args, cwd, None, env) }
fn spawn_with(kind: crate::proc::Kind, person: Option<&PersonChild>, program: &Path, args: &[String], cwd: &Path, extra_path: Option<&Path>, env: &[(String, String)]) -> std::io::Result<Proc> {
    // its own memory scope (memscope.rs): a runaway it starts is killed alone, never the hub's scope with every agent in it
    let mut c = crate::proc::tokio_cmd_in(kind, program, crate::memscope::wrap("agent")); crate::memscope::raise_oom(&mut c);
    let args: Vec<String> = args.iter().map(|a| crate::settings::mask::mask(a)).collect(); // settings lane: Spawn.agent masks every launch argument
    c.args(&args).current_dir(cwd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    // production: the agent runs with the merged PATH (its own children find node, git...); a test's stand-in folder comes first on the hub's own
    let mut path: Option<Vec<PathBuf>> = if person.is_none() && extra_path.is_none() { Some(search_dirs().to_vec()) } else { None };
    if let Some(d) = extra_path { let mut v = vec![d.to_path_buf()]; v.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())); path = Some(v) }
    // the selected KiCad's shims (kicadsel.rs) come before everything, so a `kicad-cli` on the agent's PATH is always the one Backplane selected
    if let (Some(v), Some((_, d))) = (path.as_mut(), env.iter().find(|(k, _)| k == super::kicadsel::SHIMS_KEY)) { v.insert(0, PathBuf::from(d)) }
    if let Some(j) = path.and_then(|v| std::env::join_paths(v).ok()) { c.env("PATH", j); }
    if person.is_none() { for k in crate::settings::accounts_io::STORE_ENV { c.env_remove(k); } } // the default login is ~/.claude, never a profile store the hub itself inherited; a named account puts its own back below
    for k in crate::settings::accounts_io::scrub_for(env) { c.env_remove(k); } // a named account's child inherits no provider credential (a sync account's token is its own, below)
    for (k, v) in env.iter().filter(|(k, _)| k != super::kicadsel::SHIMS_KEY) { c.env(k, v); }
    match person {
        None => { if let Some(o) = OWNER.get() { c.env("BACKPLANE_AGENT_OWNER", o).env("BACKPLANE_LOCK_HOME", o); } } // scripts/bp-lock.sh finds its hub by it (never BACKPLANE_HOME: proc::HUB_ONLY)
        Some(p) => { let (remove, set) = super::person::child_env(&p.home, &p.path, std::env::vars_os()); for k in remove { c.env_remove(k); } for (k, v) in set { c.env(k, v); } } // no BACKPLANE_* (so no owner mark either: PDEATHSIG ends it with the hub)
    }
    // Linux: the kernel kills the agent when the hub dies (PDEATHSIG); Windows: its Job Object does. Elsewhere the `reaper` watcher does (the whole group, grandchildren included).
    crate::plat::proc::agent_group(&mut c);
    let mut child = c.spawn()?;
    let pid = child.id().unwrap_or(0);
    crate::plat::proc::adopt(pid, true); crate::memscope::cap_job(pid, "agent"); // Windows: its Job Object's memory limit (Linux: its scope)
    reaper::add(pid);
    let mut stdin = child.stdin.take().expect("piped"); let stdout = child.stdout.take().expect("piped"); let mut stderr = child.stderr.take().expect("piped");
    let (in_tx, mut in_rx) = mpsc::channel::<String>(IN_QUEUE);
    let (out_tx, out_rx) = mpsc::channel::<String>(OUT_QUEUE);
    let (exit_tx, exit_rx) = oneshot::channel();
    tokio::spawn(async move { while let Some(l) = in_rx.recv().await { let l = crate::settings::mask::mask_owned(l); /* Agent.write */ if stdin.write_all(l.as_bytes()).await.is_err() || stdin.write_all(b"\n").await.is_err() || stdin.flush().await.is_err() { break } } });
    tokio::spawn(async move { let mut r = BufReader::with_capacity(64 << 10, stdout); while let Ok(Some(l)) = read_capped(&mut r, LINE_CAP).await { let l = crate::settings::mask::mask_owned(l); /* Hub.tagged */ if out_tx.send(l).await.is_err() { break } } });
    let tail = tokio::spawn(async move { let mut keep: Vec<u8> = Vec::new(); let mut b = [0u8; 4096]; loop { match stderr.read(&mut b).await { Ok(0) | Err(_) => break, Ok(n) => { keep.extend_from_slice(&b[..n]); if keep.len() > STDERR_TAIL { let cut = keep.len() - STDERR_TAIL; keep.drain(..cut); } } } } crate::settings::mask::mask(&String::from_utf8_lossy(&keep)) });
    tokio::spawn(async move {
        let st = child.wait().await; // always reaped
        let err = tail.await.unwrap_or_default();
        let e = match st { Ok(s) => Exit { code: s.code(), signal: crate::plat::proc::signal(&s), stderr: err }, Err(e) => Exit { code: None, signal: None, stderr: e.to_string() } };
        if pid > 0 { crate::plat::proc::kill_group(pid, crate::plat::proc::KILL); } // the group: anything the agent left behind
        reaper::remove(pid);
        let _ = exit_tx.send(e);
    });
    Ok(Proc { pid, input: in_tx, lines: out_rx, exit: exit_rx })
}

/// Ask the process group to end; SIGKILL follows after `grace` if it is still there.
pub fn terminate(pid: u32, grace: std::time::Duration) {
    if pid == 0 { return }
    crate::plat::proc::kill_group(pid, crate::plat::proc::TERM);
    tokio::spawn(async move { tokio::time::sleep(grace).await; crate::plat::proc::kill_group(pid, crate::plat::proc::KILL); });
}

/// Whether a process is still there (kill 0): for tests that check nothing is left.
pub fn alive(pid: u32) -> bool { crate::plat::proc::alive(pid) }

#[cfg(test)]
mod tests {
    /// Windows has no login shell's PATH to read: nothing is started for it (PowerShell was handed sh's -lc at every hub start).
    #[test] fn the_login_shell_is_asked_for_its_path_only_where_there_is_one() {
        assert!(probes_login_shell(false) && !probes_login_shell(true));
        #[cfg(windows)] { std::env::set_var("SHELL", "powershell.exe"); assert!(super::login_dirs_with(None).is_empty()); }
    }
    #[test] fn the_usual_directories_are_merged_once_with_the_newest_node_first() {
        let h = dir("usual"); for v in ["v20.1.0", "v24.13.1", "v9.0.0"] { std::fs::create_dir_all(h.join(".nvm/versions/node").join(v).join("bin")).unwrap() } std::fs::create_dir_all(h.join(".local/bin")).unwrap();
        let u = usual_dirs(&h); assert_eq!(u[0], h.join(".nvm/versions/node/v24.13.1/bin"), "{u:?}"); assert_eq!(u[1], h.join(".nvm/versions/node/v20.1.0/bin")); assert_eq!(u[2], h.join(".nvm/versions/node/v9.0.0/bin")); assert!(u.contains(&h.join(".local/bin")) && !u.contains(&h.join(".volta/bin")), "only those that exist");
        let m = merge_dirs(&[vec![PathBuf::from("/a"), PathBuf::from("/b")], vec![PathBuf::from("/b"), PathBuf::from("/c"), PathBuf::new()], vec![PathBuf::from("/a"), PathBuf::from("/d")]]); assert_eq!(m, vec![PathBuf::from("/a"), PathBuf::from("/b"), PathBuf::from("/c"), PathBuf::from("/d")]);
    }
    #[cfg(target_os = "linux")]
    #[test] fn reaping_leftovers_never_takes_this_processs_own_agents() { // two hubs' tests in one binary: a marked child of ours is not a dead hub's orphan
        // the mark names this process: another test binary running at once reaps by its own mark, never ours
        let owner = format!("reap-own-test-home-{}", std::process::id());
        let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, "sleep"); c.arg("30").env("BACKPLANE_AGENT_OWNER", &owner);
        let mut child = c.spawn().unwrap(); let pid = child.id();
        assert_eq!(reap_orphans(&owner), 0, "our own child is left alone"); assert!(alive(pid));
        let _ = child.kill(); let _ = child.wait();
    }
    use super::*;
    fn dir(name: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::dir(&format!("agents-{name}")) }
    fn script(d: &Path, name: &str, body: &str) -> PathBuf { use crate::plat::fs::PermissionsExt; let p = d.join(name); std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap(); std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); p }

    #[test] fn test_mode_runs_only_the_standin_folder() {
        let d = dir("res"); script(&d, "claude", "exit 0"); let a = Cli::standin(&d);
        assert!(a.resolve("claude").is_ok());
        assert_eq!(a.resolve("codex").unwrap_err(), format!("{} has no runnable codex", d.display()));
        assert!(a.resolve("../claude").is_err()); assert!(a.resolve("/bin/sh").is_err());
        std::fs::write(d.join("grok"), "x").unwrap(); assert!(a.resolve("grok").is_err()); // not executable
        let outside = dir("res-out"); script(&outside, "muse", "exit 0"); crate::plat::fs::symlink(outside.join("muse"), d.join("muse")).unwrap();
        assert!(a.resolve("muse").is_err()); // a link out of the folder is refused
    }
    fn mk(d: &Path) -> PathBuf { std::fs::create_dir_all(d).unwrap(); d.to_path_buf() }
    fn none(_: &str) -> Option<std::ffi::OsString> { None }
    /// Each agent's installer folder is looked in without any PATH naming it: grok's and opencode's (their installers only edit ~/.zshrc), pi's own
    /// and the node pi installs, bun's, nvm's and fnm's node bins.
    #[test] fn agents_in_their_installers_folders_are_found_without_a_path() {
        let h = dir("inst");
        for d in [".grok/bin", ".opencode/bin", ".pi/agent/bin", ".bun/bin", ".nvm/versions/node/v22.0.0/bin", ".local/share/fnm/node-versions/v23.1.0/installation/bin", ".local/share/pi-node/current/bin", ".local/bin", ".gemini/antigravity-cli/bin", ".claude/local"] { mk(&h.join(d)); }
        let u = usual_dirs_for(&h, false, false, &none);
        for d in [".grok/bin", ".opencode/bin", ".pi/agent/bin", ".bun/bin", ".nvm/versions/node/v22.0.0/bin", ".local/share/fnm/node-versions/v23.1.0/installation/bin", ".local/share/pi-node/current/bin", ".gemini/antigravity-cli/bin", ".claude/local"] { assert!(u.contains(&h.join(d)), "{d} in {u:?}") }
        assert!(u.iter().position(|d| d.ends_with("v22.0.0/bin")) < u.iter().position(|d| d.ends_with(".grok/bin")), "node before the agents");
        script(&h.join(".grok/bin"), "grok", "echo grok 1.0.77"); script(&h.join(".opencode/bin"), "opencode", "echo 2.3.0"); script(&h.join(".bun/bin"), "omp", "echo omp");
        // pi as its installer leaves it: ~/.local/bin/pi a relative symlink to ~/.pi/agent/bin/pi
        script(&h.join(".pi/agent/bin"), "pi", "echo 1.4.0"); crate::plat::fs::symlink("../../.pi/agent/bin/pi", h.join(".local/bin/pi")).unwrap();
        assert_eq!(find_in(&u, "grok", false), Some(h.join(".grok/bin/grok"))); assert_eq!(find_in(&u, "opencode", false), Some(h.join(".opencode/bin/opencode"))); assert_eq!(find_in(&u, "omp", false), Some(h.join(".bun/bin/omp")));
        let pi = find_in(&u, "pi", false).unwrap(); assert!(pi == h.join(".local/bin/pi") || pi == h.join(".pi/agent/bin/pi"), "{pi:?}");
        assert_eq!(find_in(&u, "codex", false), None);
        // a version manager's own variables move its folder
        let alt = dir("inst-alt"); mk(&alt.join("versions/node/v20.0.0/bin")); mk(&alt.join("pnpm"));
        let env = |k: &str| match k { "NVM_DIR" => Some(alt.as_os_str().to_owned()), "PNPM_HOME" => Some(alt.join("pnpm").into_os_string()), _ => None };
        let u = usual_dirs_for(&h, false, false, &env); assert_eq!(u[0], alt.join("versions/node/v20.0.0/bin")); assert!(u.contains(&alt.join("pnpm")));
    }
    /// macOS: an app started from the Finder or the Dock has PATH /usr/bin:/bin:/usr/sbin:/sbin; homebrew's and the Library folders are looked in.
    #[test] fn macos_folders_are_looked_in() {
        let h = dir("mac"); for d in ["Library/pnpm", "Library/Application Support/fnm/node-versions/v24.1.0/installation/bin", ".grok/bin"] { mk(&h.join(d)); }
        let u = usual_dirs_for(&h, false, true, &none);
        for d in ["Library/pnpm", "Library/Application Support/fnm/node-versions/v24.1.0/installation/bin", ".grok/bin"] { assert!(u.contains(&h.join(d)), "{d} in {u:?}") }
        assert!(!usual_dirs_for(&h, false, false, &none).contains(&h.join("Library/pnpm")), "Library is macOS's");
    }
    /// Windows: npm's %APPDATA%\npm (its .cmd shims), nvm-windows' symlink, Program Files' node, scoop, winget, and the per-user agent folders.
    #[test] fn windows_folders_and_cmd_shims_are_found() {
        let h = dir("win"); let (roam, local, pf, nvm) = (h.join("AppData/Roaming"), h.join("AppData/Local"), h.join("PF"), h.join("nvm4w/nodejs"));
        for d in [roam.join("npm"), local.join("Microsoft/WinGet/Links"), pf.join("nodejs"), nvm.clone(), h.join("scoop/shims"), h.join(".grok/bin"), h.join(".opencode/bin"), h.join(".pi/agent/bin"), h.join(".bun/bin"), h.join(".local/bin")] { mk(&d); }
        let env = |k: &str| match k { "APPDATA" => Some(roam.as_os_str().to_owned()), "LOCALAPPDATA" => Some(local.as_os_str().to_owned()), "ProgramFiles" => Some(pf.as_os_str().to_owned()), "NVM_SYMLINK" => Some(nvm.as_os_str().to_owned()), _ => None };
        let u = usual_dirs_for(&h, true, false, &env);
        for d in [roam.join("npm"), local.join("Microsoft/WinGet/Links"), pf.join("nodejs"), nvm.clone(), h.join("scoop/shims"), h.join(".grok/bin"), h.join(".opencode/bin"), h.join(".pi/agent/bin"), h.join(".bun/bin")] { assert!(u.contains(&d), "{d:?} in {u:?}") }
        assert!(!u.iter().any(|d| d.starts_with("/usr") || d.starts_with("/opt")), "no Unix folders: {u:?}");
        std::fs::write(roam.join("npm/opencode.cmd"), "@echo off").unwrap(); std::fs::write(h.join(".grok/bin/grok.exe"), "MZ").unwrap(); std::fs::write(h.join(".pi/agent/bin/pi.cmd"), "@echo off").unwrap();
        assert_eq!(find_in(&u, "opencode", true), Some(roam.join("npm/opencode.cmd"))); assert_eq!(find_in(&u, "grok", true), Some(h.join(".grok/bin/grok.exe"))); assert_eq!(find_in(&u, "pi", true), Some(h.join(".pi/agent/bin/pi.cmd")));
        assert_eq!(find_in(&u, "opencode", false), None, "a .cmd is a program only on Windows");
    }
    #[test] fn the_path_is_read_between_its_marks() {
        assert_eq!(marked_path("Welcome!\n__BP_PATH__/a:/b__BP_END__\nbye\n"), Some("/a:/b"));
        assert_eq!(marked_path("__BP_PATH__/a"), None, "not finished yet"); assert_eq!(marked_path("__BP_PATH____BP_END__"), None); assert_eq!(marked_path("nothing"), None);
    }
    /// The PATH an installer adds in the interactive rc file (zsh's ~/.zshrc) is read; a shell whose interactive start hangs is asked again as a plain login shell.
    #[cfg(unix)]
    #[test] fn the_login_shells_path_includes_what_its_interactive_rc_adds() {
        let h = dir("shell");
        // a stand-in shell: interactive (-ilc) it reads "rc" (prints a greeting, adds .grok/bin, or hangs when told to), then runs the command
        let sh = script(&h, "fakesh", "case \"$1\" in -ilc) echo hello from rc; [ -f \"$HOME/hang\" ] && sleep 30; PATH=\"$HOME/.grok/bin:$PATH\";; esac\nPATH=\"$HOME/.login/bin:$PATH\"\nexport PATH\neval \"$2\"");
        let got = shell_path_dirs(sh.to_str().unwrap(), Some(&h), 5);
        assert_eq!(got[..2], [h.join(".login/bin"), h.join(".grok/bin")], "{got:?}");
        std::fs::write(h.join("hang"), "").unwrap(); let t = std::time::Instant::now();
        let got = shell_path_dirs(sh.to_str().unwrap(), Some(&h), 1);
        assert_eq!(got[0], h.join(".login/bin"), "{got:?}"); assert!(!got.contains(&h.join(".grok/bin"))); assert!(t.elapsed() < std::time::Duration::from_secs(5), "the hung shell was ended: {:?}", t.elapsed());
        assert!(shell_path_dirs(h.join("no-such-shell").to_str().unwrap(), None, 1).is_empty());
    }
    /// A probe runs with the turn's PATH: pi's launcher (a sh script that execs a `#!/usr/bin/env node` one) answers --version only when node is on it.
    #[cfg(unix)]
    #[test] fn a_launcher_that_needs_node_answers_with_the_agents_path() {
        let d = dir("pi-node"); script(&d, "fakenode-bp", "echo 1.4.0"); let real = d.join("pi-real"); std::fs::write(&real, "#!/usr/bin/env fakenode-bp\n").unwrap();
        { use crate::plat::fs::PermissionsExt; std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o755)).unwrap(); }
        let pi = script(&d, "pi", &format!("exec {} \"$@\"", real.display()));
        let run = |path: Option<std::ffi::OsString>| { let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, &pi); c.arg("--version").env_clear(); if let Some(p) = path { c.env("PATH", p); } else { c.env("PATH", "/usr/bin:/bin"); }
            c.output().ok().map(|o| (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())) };
        let bare = run(None); assert_ne!(bare.as_ref().unwrap().0, 0, "without node: {bare:?}");
        assert!(crate::settings::models_probe::version_or_why(&pi, bare).contains("--version failed: exit 127"), "found but failing is said, not taken for not installed");
        let with = run(Cli::standin(&d).child_path()); assert_eq!(crate::settings::models_probe::version_or_why(&pi, with), "1.4.0");
    }
    #[test] fn a_version_is_its_first_line_or_why_there_is_none() {
        use crate::settings::models_probe::version_or_why; let p = Path::new("/x/pi");
        assert_eq!(version_or_why(p, Some((0, "\npi 1.4.0\nmore".into(), String::new()))), "pi 1.4.0");
        assert_eq!(version_or_why(p, Some((0, String::new(), "1.4.0\n".into()))), "1.4.0", "on stderr");
        assert_eq!(version_or_why(p, Some((127, String::new(), "env: 'node': No such file or directory\n".into()))), "/x/pi (--version failed: exit 127: env: 'node': No such file or directory)");
        assert_eq!(version_or_why(p, None), "/x/pi (--version failed: it did not start or answer in time)");
    }
    #[test] fn a_copy_of_a_log_without_standins_runs_nothing() { let a = Cli { dir: None, copy: true }; assert!(a.resolve("claude").unwrap_err().contains("refused")); }

    #[tokio::test] async fn lines_in_lines_out_and_the_exit_is_reaped() {
        let d = dir("io"); script(&d, "echoer", "while read l; do echo \"got:$l\"; done; exit 3");
        let mut p = spawn(&d.join("echoer"), &[], &d, None, &[]).unwrap();
        p.input.send("a".into()).await.unwrap(); p.input.send("b".into()).await.unwrap();
        assert_eq!(p.lines.recv().await.unwrap(), "got:a"); assert_eq!(p.lines.recv().await.unwrap(), "got:b");
        drop(p.input);
        let e = tokio::time::timeout(std::time::Duration::from_secs(5), p.exit).await.unwrap().unwrap();
        assert_eq!(e.code, Some(3)); assert!(p.lines.recv().await.is_none());
        assert!(!alive(p.pid));
    }
    #[tokio::test] async fn terminate_kills_the_whole_group_and_leaves_no_zombie() {
        let d = dir("grp"); script(&d, "tree", "sleep 60 &\necho $!\nwait");
        let mut p = spawn(&d.join("tree"), &[], &d, None, &[]).unwrap();
        let kid: u32 = p.lines.recv().await.unwrap().trim().parse().unwrap(); assert!(alive(kid));
        terminate(p.pid, std::time::Duration::from_secs(1));
        let e = tokio::time::timeout(std::time::Duration::from_secs(5), p.exit).await.unwrap().unwrap();
        assert!(e.signal.is_some() || e.code.is_some());
        for _ in 0..50 { if !alive(kid) { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await }
        assert!(!alive(kid), "the child's own child outlived it"); assert!(!alive(p.pid));
    }
    /// A group whose leader prints "ready" once its TERM trap is set; a thread reaps it (in the hub the reaper is never the agents' parent, so it never holds a zombie).
    #[cfg(unix)]
    fn group(d: &Path, name: &str, body: &str) -> u32 {
        use std::io::BufRead;
        let p = script(d, name, body);
        let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, &p); c.stdout(Stdio::piped()).stdin(Stdio::null()); crate::plat::proc::own_group(&mut c);
        let mut ch = c.spawn().unwrap(); let pid = ch.id();
        let mut l = String::new(); std::io::BufReader::new(ch.stdout.take().unwrap()).read_line(&mut l).unwrap(); assert_eq!(l.trim(), "ready");
        std::thread::spawn(move || { let _ = ch.wait(); });
        pid
    }
    #[cfg(unix)]
    #[test] fn the_reaper_asks_with_term_first_so_an_agent_can_clean_up() { // #1109: a CLI mid-refresh gets to remove its lock
        let d = dir("reap-term"); let out = d.join("cleaned");
        let pid = group(&d, "polite", &format!("trap 'echo done > \"{}\"; exit 0' TERM\necho ready\nwhile :; do sleep 0.05; done", out.display()));
        let t = std::time::Instant::now();
        assert_eq!(end_groups(&[pid as i32], std::time::Duration::from_secs(5)), 0, "it ended on TERM, no KILL was needed");
        assert!(t.elapsed() < std::time::Duration::from_secs(4), "the reaper does not sit out the whole grace once the group is gone");
        assert_eq!(std::fs::read_to_string(&out).unwrap_or_default().trim(), "done", "its TERM handler ran");
        assert!(!crate::plat::proc::group_alive(pid));
    }
    #[cfg(unix)]
    #[test] fn the_reaper_kills_what_is_left_after_the_grace() {
        let d = dir("reap-kill");
        let pid = group(&d, "stubborn", "trap '' TERM\necho ready\nwhile :; do sleep 0.05; done");
        let t = std::time::Instant::now();
        assert_eq!(end_groups(&[pid as i32, 0, 1, -5], std::time::Duration::from_millis(300)), 1, "one group needed the KILL; 0, 1 and negatives are never signalled");
        assert!(t.elapsed() >= std::time::Duration::from_millis(300), "it waited the grace");
        for _ in 0..50 { if !crate::plat::proc::group_alive(pid) { break } std::thread::sleep(std::time::Duration::from_millis(50)) }
        assert!(!crate::plat::proc::group_alive(pid), "nothing of the group is left");
    }
    #[tokio::test] async fn an_overlong_line_is_cut_not_buffered_whole() {
        let data = format!("{}\nshort\n", "x".repeat(5000)); let mut r = BufReader::new(data.as_bytes());
        assert_eq!(read_capped(&mut r, 100).await.unwrap().unwrap().len(), 100); assert_eq!(read_capped(&mut r, 100).await.unwrap().unwrap(), "short"); assert!(read_capped(&mut r, 100).await.unwrap().is_none());
    }
    #[tokio::test] async fn a_full_stdin_queue_is_a_refusal_not_a_stall() {
        let d = dir("full"); script(&d, "deaf", "sleep 30"); let p = spawn(&d.join("deaf"), &[], &d, None, &[]).unwrap();
        let mut refused = false; for i in 0..(IN_QUEUE * 4) { if p.input.try_send("y".repeat(60_000) + &i.to_string()).is_err() { refused = true; break } }
        assert!(refused); terminate(p.pid, std::time::Duration::from_millis(100));
    }
}

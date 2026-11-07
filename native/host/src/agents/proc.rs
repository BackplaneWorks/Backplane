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
                let (cd, cp) = (std::fs::canonicalize(d).map_err(|_| format!("{} has no runnable {cli}", d.display()))?, std::fs::canonicalize(&p).map_err(|_| format!("{} has no runnable {cli}", d.display()))?);
                let exec = std::fs::metadata(&cp).map(|m| { use std::os::unix::fs::PermissionsExt; m.is_file() && m.permissions().mode() & 0o111 != 0 }).unwrap_or(false);
                if exec && cp.starts_with(&cd) { Ok(cp) } else { Err(format!("{} has no runnable {cli}", d.display())) }
            }
            None if self.copy => Err(format!("a copy of a log runs agents only as stand-ins: {cli} refused")),
            None => search_dirs().iter().map(|d| d.join(cli)).find(|c| c.is_file()).ok_or_else(|| format!("{cli} is not installed (looked in the hub's PATH, the login shell's and the usual install directories)")),
        }
    }
}

// ---- where the provider CLIs are: a hub started from a launcher or a service has a PATH without the login shell's additions (nvm, volta, ~/.local/bin...) ----
/// CliPath.merge: each directory once, the first of each, in order, none empty.
pub fn merge_dirs(parts: &[Vec<PathBuf>]) -> Vec<PathBuf> { let mut out: Vec<PathBuf> = vec![]; for d in parts.iter().flatten() { if !d.as_os_str().is_empty() && !out.contains(d) { out.push(d.clone()) } } out }
fn semver_key(s: &str) -> (u64, u64, u64) { let mut p = s.trim_start_matches('v').split('.').map(|x| x.parse::<u64>().unwrap_or(0)); (p.next().unwrap_or(0), p.next().unwrap_or(0), p.next().unwrap_or(0)) }
/// CliPath.usual: the usual install directories that exist (nvm's node versions newest first, volta, bun, ~/.local/bin, npm's global bin, homebrew, /usr/local/bin).
pub fn usual_dirs(home: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = vec![];
    if let Ok(rd) = std::fs::read_dir(home.join(".nvm/versions/node")) { let mut vs: Vec<String> = rd.flatten().filter_map(|e| e.file_name().into_string().ok()).collect(); vs.sort_by(|a, b| semver_key(b).cmp(&semver_key(a))); for n in vs { v.push(home.join(".nvm/versions/node").join(n).join("bin")) } }
    for d in [".volta/bin", ".bun/bin", ".local/bin", ".npm-global/bin", ".cargo/bin"] { v.push(home.join(d)) }
    for d in ["/opt/homebrew/bin", "/usr/local/bin"] { v.push(PathBuf::from(d)) }
    v.into_iter().filter(|d| d.is_dir()).collect()
}
/// The login shell's PATH ($SHELL -lc, five seconds at most), as directories; empty when it cannot be read.
fn login_dirs() -> Vec<PathBuf> { login_dirs_with(None) }
/// The login shell's PATH; `home`: run it with that HOME (the person's real one in side-by-side mode, where the hub's own is a fake).
pub fn login_dirs_with(home: Option<&Path>) -> Vec<PathBuf> {
    let sh = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/sh".into());
    let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, &sh); c.args(["-lc", "printf %s \"$PATH\""]).stdin(Stdio::null()).stderr(Stdio::null()); if let Some(h) = home { c.env("HOME", h); }
    let Ok(mut child) = c.stdout(Stdio::piped()).spawn() else { return vec![] };
    let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop { match child.try_wait() { Ok(Some(_)) => break, Ok(None) if std::time::Instant::now() < end => std::thread::sleep(std::time::Duration::from_millis(20)), _ => { let _ = child.kill(); let _ = child.wait(); return vec![] } } }
    let mut out = String::new(); if let Some(mut so) = child.stdout.take() { use std::io::Read; let _ = so.read_to_string(&mut out); }
    std::env::split_paths(out.trim()).collect()
}
static SEARCH: std::sync::OnceLock<Vec<PathBuf>> = std::sync::OnceLock::new();
/// Where an agent CLI is looked for (and the PATH an agent runs with): the hub's own PATH first, then the login shell's, then the usual install directories. Worked out once.
pub fn search_dirs() -> &'static [PathBuf] {
    SEARCH.get_or_init(|| {
        let cur: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        merge_dirs(&[cur, login_dirs(), usual_dirs(&home)])
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
        unsafe { libc::kill(pid as i32, libc::SIGKILL); } n += 1
    }
    n
}
/// macOS and other Unix: no /proc, and no way to read another process's environment that is exact (`ps eww` joins argv and environment with spaces, so a
/// text match could end a process that only quotes the mark, or one whose home has a space). Until it reads KERN_PROCARGS2 entry by entry, or keeps a pid file with
/// start times, this ends nothing: an orphan left running is better than killing the wrong process.
#[cfg(not(target_os = "linux"))]
pub fn reap_orphans(_home: &str) -> usize { 0 }

/// Off Linux nothing ends an agent when the hub is killed (no PDEATHSIG), so one small watcher process per hub does: the hub keeps the write end of a pipe to it
/// (close-on-exec, so no agent holds it), registers each agent's process group, and when the pipe closes because the hub is gone the watcher kills every group still registered.
/// The watcher is this binary with `--agent-reaper`; it is its own session, so the signal that killed the hub does not reach it.
#[cfg(not(target_os = "linux"))]
mod reaper {
    use std::io::Write;
    static PIPE: std::sync::Mutex<Option<std::process::ChildStdin>> = std::sync::Mutex::new(None);
    fn send(line: &str) -> bool {
        let mut g = PIPE.lock().unwrap();
        if g.is_none() {
            let Ok(exe) = std::env::current_exe() else { return false };
            use std::os::unix::process::CommandExt;
            let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, exe);
            #[cfg(not(test))] c.arg("--agent-reaper");
            #[cfg(test)] c.args(["agents::tests::reaper_child", "--exact", "--nocapture", "--test-threads=1"]).env("BP_REAPER_CHILD", "1"); // a test binary takes no such flag: its watcher is a test that only runs as that child
            c.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
            unsafe { c.pre_exec(|| { libc::setsid(); Ok(()) }); }
            match c.spawn() { Ok(mut ch) => { *g = ch.stdin.take(); std::thread::spawn(move || { let _ = ch.wait(); }); } Err(_) => return false }
        }
        let ok = g.as_mut().map_or(false, |w| w.write_all(line.as_bytes()).and_then(|_| w.flush()).is_ok());
        if !ok { *g = None }
        ok
    }
    pub fn add(pid: u32) { if pid > 0 && !send(&format!("+{pid}\n")) { send(&format!("+{pid}\n")); } }
    pub fn remove(pid: u32) { if pid > 0 { let mut g = PIPE.lock().unwrap(); if let Some(w) = g.as_mut() { let _ = w.write_all(format!("-{pid}\n").as_bytes()).and_then(|_| w.flush()); } } }
}
#[cfg(target_os = "linux")]
mod reaper { pub fn add(_pid: u32) {} pub fn remove(_pid: u32) {} }

/// `backplane-hub --agent-reaper`: read "+pid" / "-pid" lines from stdin; at end of input (the hub is gone) kill each process group still listed. Returns the exit code.
pub fn reaper_main() -> i32 {
    use std::io::BufRead;
    let mut live = std::collections::BTreeSet::<i32>::new();
    for l in std::io::stdin().lock().lines() {
        let Ok(l) = l else { break };
        let (sign, n) = l.split_at(l.len().min(1));
        let Ok(pid) = n.trim().parse::<i32>() else { continue };
        match sign { "+" => { live.insert(pid); } "-" => { live.remove(&pid); } _ => {} }
    }
    for pid in live { if pid > 1 { unsafe { libc::kill(-pid, libc::SIGKILL); libc::kill(pid, libc::SIGKILL); } } }
    0
}

/// What a person's real agent runs with (SIDE-BY-SIDE.md check 6): the real HOME and the PATH worked out from it; every `BACKPLANE_*` of this hub is removed from its environment.
pub struct PersonChild { pub home: PathBuf, pub path: std::ffi::OsString }
pub fn spawn(program: &Path, args: &[String], cwd: &Path, extra_path: Option<&Path>, env: &[(String, String)]) -> std::io::Result<Proc> { spawn_with(crate::proc::Kind::Agent, None, program, args, cwd, extra_path, env) }
/// The only place a REAL agent CLI is made on a copy (`Kind::PersonTurn`, refused unless `copy::person_turn_allowed()`); the actor calls it only for a turn the person started.
pub fn spawn_person(person: &PersonChild, program: &Path, args: &[String], cwd: &Path, env: &[(String, String)]) -> std::io::Result<Proc> { spawn_with(crate::proc::Kind::PersonTurn, Some(person), program, args, cwd, None, env) }
fn spawn_with(kind: crate::proc::Kind, person: Option<&PersonChild>, program: &Path, args: &[String], cwd: &Path, extra_path: Option<&Path>, env: &[(String, String)]) -> std::io::Result<Proc> {
    let mut c = crate::proc::tokio_cmd(kind, program);
    let args: Vec<String> = args.iter().map(|a| crate::settings::mask::mask(a)).collect(); // settings lane: Spawn.agent masks every launch argument
    c.args(&args).current_dir(cwd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    if person.is_none() && extra_path.is_none() { if let Ok(j) = std::env::join_paths(search_dirs()) { c.env("PATH", j); } } // production: the agent runs with the merged PATH (its own children find node, git...)
    if let Some(d) = extra_path { let old = std::env::var_os("PATH").unwrap_or_default(); let mut v = vec![d.to_path_buf()]; v.extend(std::env::split_paths(&old)); if let Ok(j) = std::env::join_paths(v) { c.env("PATH", j); } }
    if person.is_none() { for k in crate::settings::accounts_io::STORE_ENV { c.env_remove(k); } } // the default login is ~/.claude, never a profile store the hub itself inherited; a named account puts its own back below
    for (k, v) in env { c.env(k, v); }
    match person {
        None => { if let Some(o) = OWNER.get() { c.env("BACKPLANE_AGENT_OWNER", o); } }
        Some(p) => { let (remove, set) = super::person::child_env(&p.home, &p.path, std::env::vars_os()); for k in remove { c.env_remove(k); } for (k, v) in set { c.env(k, v); } } // no BACKPLANE_* (so no owner mark either: PDEATHSIG ends it with the hub)
    }
    // Linux: the kernel kills the agent when the hub dies (PDEATHSIG). Elsewhere the `reaper` watcher does (the whole group, grandchildren included).
    unsafe { c.pre_exec(|| { libc::setsid(); #[cfg(target_os = "linux")] libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL); Ok(()) }); }
    let mut child = c.spawn()?;
    let pid = child.id().unwrap_or(0);
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
        use std::os::unix::process::ExitStatusExt;
        let e = match st { Ok(s) => Exit { code: s.code(), signal: s.signal(), stderr: err }, Err(e) => Exit { code: None, signal: None, stderr: e.to_string() } };
        if pid > 0 { unsafe { libc::kill(-(pid as i32), libc::SIGKILL); } } // the group: anything the agent left behind
        reaper::remove(pid);
        let _ = exit_tx.send(e);
    });
    Ok(Proc { pid, input: in_tx, lines: out_rx, exit: exit_rx })
}

/// Ask the process group to end; SIGKILL follows after `grace` if it is still there.
pub fn terminate(pid: u32, grace: std::time::Duration) {
    if pid == 0 { return }
    unsafe { libc::kill(-(pid as i32), libc::SIGTERM); }
    tokio::spawn(async move { tokio::time::sleep(grace).await; unsafe { libc::kill(-(pid as i32), libc::SIGKILL); } });
}

/// Whether a process is still there (kill 0): for tests that check nothing is left.
pub fn alive(pid: u32) -> bool { pid > 0 && unsafe { libc::kill(pid as i32, 0) } == 0 }

#[cfg(test)]
mod tests {
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
    fn dir(name: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-agents-{}-{}", std::process::id(), name)); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    fn script(d: &Path, name: &str, body: &str) -> PathBuf { use std::os::unix::fs::PermissionsExt; let p = d.join(name); std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap(); std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); p }

    #[test] fn test_mode_runs_only_the_standin_folder() {
        let d = dir("res"); script(&d, "claude", "exit 0"); let a = Cli::standin(&d);
        assert!(a.resolve("claude").is_ok());
        assert_eq!(a.resolve("codex").unwrap_err(), format!("{} has no runnable codex", d.display()));
        assert!(a.resolve("../claude").is_err()); assert!(a.resolve("/bin/sh").is_err());
        std::fs::write(d.join("grok"), "x").unwrap(); assert!(a.resolve("grok").is_err()); // not executable
        let outside = dir("res-out"); script(&outside, "muse", "exit 0"); std::os::unix::fs::symlink(outside.join("muse"), d.join("muse")).unwrap();
        assert!(a.resolve("muse").is_err()); // a link out of the folder is refused
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

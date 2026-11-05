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
            None => std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(cli)).find(|c| c.is_file())).ok_or_else(|| format!("{cli} is not installed")),
        }
    }
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
/// Linux: the process table is /proc, each environment is /proc/<pid>/environ.
#[cfg(target_os = "linux")]
pub fn reap_orphans(home: &str) -> usize {
    let mark = format!("BACKPLANE_AGENT_OWNER={home}").into_bytes(); let me = std::process::id(); let mut n = 0;
    let Ok(rd) = std::fs::read_dir("/proc") else { return 0 };
    for e in rd.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else { continue }; if pid == me { continue }
        let Ok(env) = std::fs::read(format!("/proc/{pid}/environ")) else { continue };
        if env.split(|b| *b == 0).any(|kv| kv == mark.as_slice()) { unsafe { libc::kill(pid as i32, libc::SIGKILL); } n += 1 }
    }
    n
}
/// macOS and other Unix: no /proc; `ps eww` prints each process's environment after its command line.
#[cfg(not(target_os = "linux"))]
pub fn reap_orphans(home: &str) -> usize {
    let mark = format!("BACKPLANE_AGENT_OWNER={home}"); let me = std::process::id(); let mut n = 0;
    let Ok(o) = crate::proc::std_cmd(crate::proc::Kind::Local, "ps").args(["eww", "-A", "-o", "pid=,command="]).output() else { return 0 };
    for l in String::from_utf8_lossy(&o.stdout).lines() {
        let l = l.trim_start(); let Some((pid, rest)) = l.split_once(' ') else { continue };
        let Ok(pid) = pid.parse::<u32>() else { continue }; if pid == me { continue }
        // the owner mark is a whole word of the environment part: " BACKPLANE_AGENT_OWNER=<home>" followed by a space or the end
        if let Some(at) = rest.find(&format!(" {mark}")) { let after = &rest[at + 1 + mark.len()..]; if after.is_empty() || after.starts_with(' ') { unsafe { libc::kill(pid as i32, libc::SIGKILL); } n += 1 } }
    }
    n
}

pub fn spawn(program: &Path, args: &[String], cwd: &Path, extra_path: Option<&Path>, env: &[(String, String)]) -> std::io::Result<Proc> {
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Agent, program);
    let args: Vec<String> = args.iter().map(|a| crate::settings::mask::mask(a)).collect(); // settings lane: Spawn.agent masks every launch argument
    c.args(&args).current_dir(cwd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    if let Some(d) = extra_path { let old = std::env::var_os("PATH").unwrap_or_default(); let mut v = vec![d.to_path_buf()]; v.extend(std::env::split_paths(&old)); if let Ok(j) = std::env::join_paths(v) { c.env("PATH", j); } }
    for (k, v) in env { c.env(k, v); }
    if let Some(o) = OWNER.get() { c.env("BACKPLANE_AGENT_OWNER", o); }
    // Linux: the kernel kills the agent when the hub dies (PDEATHSIG). macOS has no such call: the next hub on the home reaps what is left (reap_orphans).
    unsafe { c.pre_exec(|| { libc::setsid(); #[cfg(target_os = "linux")] libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL); Ok(()) }); }
    let mut child = c.spawn()?;
    let pid = child.id().unwrap_or(0);
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

//! #1456: the hub binary, started the way macOS's desktop app started it (its launch thread is a GCD worker, which blocks SIGCHLD with most
//! other signals, and std's posix_spawn passed that mask through the guard to the hub), or with SIGCHLD ignored, still sees its children end:
//! a project turn gets past its snapshot (git makes the folder a repository) to the agent, and the hub leaves no zombies behind.
//! Without the fix, on macOS the turn never reached the agent (tokio reaps only on SIGCHLD) and the zombies piled up within seconds.
#![cfg(unix)]
use futures_util::{SinkExt, StreamExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;

const STANDIN: &str = include_str!("../../../test/agents/fixtures/standin-claude.sh");

#[derive(Clone, Copy, Debug)]
enum Start { SigchldBlocked, SigchldIgnored }

/// A folder of the test's own, removed when it drops (also when an assert fails).
struct Tmp(PathBuf);
impl Drop for Tmp { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
fn tmp(name: &str) -> Tmp {
    let d = std::env::temp_dir().join(format!("bp-inhsig-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); Tmp(d)
}
fn free_port() -> u16 { std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port() }
fn events(home: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(home.join("events.jsonl")).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}
/// The hub's children that are zombies now.
fn zombies(hub: u32) -> Vec<u32> {
    let o = Command::new("ps").args(["-A", "-o", "pid=,ppid=,stat="]).output().unwrap();
    String::from_utf8_lossy(&o.stdout).lines().filter_map(|l| {
        let f: Vec<&str> = l.split_whitespace().collect();
        (f.len() >= 3 && f[1] == hub.to_string() && f[2].starts_with('Z')).then(|| f[0].parse().ok()).flatten()
    }).collect()
}
fn wait_for<T>(secs: u64, what: &str, log: &Path, mut f: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(v) = f() { return v }
        if Instant::now() > end { panic!("{what} within {secs} s; hub.log:\n{}", std::fs::read_to_string(log).unwrap_or_default()) }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn a_project_turn_reaches_its_agent(start: Start) {
    let name = format!("{start:?}").to_lowercase();
    let _root = tmp(&name); let root = _root.0.clone(); let (home, bin, proj, user) = (root.join("home"), root.join("bin"), root.join("proj"), root.join("user"));
    for d in [&home, &bin, &user] { std::fs::create_dir_all(d).unwrap(); }
    std::fs::write(home.join("token"), "tok").unwrap();
    let agent_log = root.join("agent.log"); std::fs::write(&agent_log, "").unwrap();
    { use std::os::unix::fs::PermissionsExt; let p = bin.join("claude"); std::fs::write(&p, STANDIN).unwrap(); std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); }
    let port = free_port();
    let mut c = Command::new(env!("CARGO_BIN_EXE_backplane-hub"));
    c.args(["--home", &home.to_string_lossy(), "--port", &port.to_string(), "--host", "127.0.0.1", "--foreground", "--no-tailscale"])
        .env("PATH", format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default())).env("HOME", &user)
        .env("FAKE_LOG", &agent_log).env("FAKE_SECS", "1").env("FAKE_PER", "1").env("FAKE_CYCLE", "5").env("BACKPLANE_NO_UPDATE", "1")
        .stdin(Stdio::null()).stdout(std::fs::File::create(root.join("hub.log")).unwrap()).stderr(std::fs::File::create(root.join("hub.err")).unwrap());
    // after std's own reset of the child's signals (its fork path runs pre_exec last), so this is what the hub starts with
    unsafe { c.pre_exec(move || {
        match start {
            Start::SigchldBlocked => { let mut m: libc::sigset_t = std::mem::zeroed(); libc::sigemptyset(&mut m); for s in [libc::SIGCHLD, libc::SIGHUP, libc::SIGTERM, libc::SIGINT] { libc::sigaddset(&mut m, s); } libc::sigprocmask(libc::SIG_BLOCK, &m, std::ptr::null_mut()); }
            Start::SigchldIgnored => { libc::signal(libc::SIGCHLD, libc::SIG_IGN); }
        }
        Ok(())
    }); }
    let mut hub = c.spawn().unwrap(); let pid = hub.id();
    struct Kill(u32); impl Drop for Kill { fn drop(&mut self) { unsafe { libc::kill(self.0 as i32, libc::SIGKILL); } } }
    let _k = Kill(pid);
    let log = root.join("hub.err");
    // a runtime of its own threads: the socket is read all along (the hub's frames are never left to pile up), and the test waits on the log
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
    let ws = wait_for(30, "the hub answers", &log, || rt.block_on(tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws?token=tok"))).ok().map(|w| w.0));
    let (mut tx, mut rx) = ws.split();
    rt.spawn(async move { while let Some(Ok(_)) = rx.next().await {} });
    let mut send = |id: u32, m: &str, p: serde_json::Value| rt.block_on(tx.send(Message::Text(serde_json::json!({"id": id, "m": m, "p": p}).to_string().into()))).unwrap();
    send(1, "project.new", serde_json::json!({"path": proj, "empty": true}));
    let thread = wait_for(20, "the project's thread", &log, || events(&home).iter().find(|e| e["$"] == "ThreadCreated").and_then(|e| e["id"].as_str().map(str::to_string)));
    // the thread is ready once its provider is set (a turn sent before that is taken into the hand-over)
    wait_for(20, "the thread's provider", &log, || events(&home).iter().any(|e| (e["$"] == "ProviderSet" || e["$"] == "ThreadCreated") && e["id"] == thread.as_str() && e["provider"].as_str().is_some_and(|p| !p.is_empty())).then_some(()));
    std::thread::sleep(Duration::from_millis(500));
    send(2, "turn.start", serde_json::json!({"thread": thread, "text": "zqshort hello"}));
    wait_for(15, "the turn runs", &log, || events(&home).iter().any(|e| e["$"] == "TurnChanged" && e["thread"] == thread.as_str() && e["state"] == "running").then_some(()));
    // the turn's snapshot ran git and saw it end: the folder is a repository, and the agent has the turn
    wait_for(40, "the agent gets the turn (the snapshot's git exits seen)", &log, || std::fs::read_to_string(&agent_log).unwrap_or_default().contains("TURN").then_some(()));
    assert!(proj.join(".git").exists(), "the project folder is a repository");
    assert!(hub.try_wait().unwrap().is_none(), "the hub still runs");
    // a child that exits is reaped at once, or by the hub's reaping watch within its 2 s look: none stays a zombie across 5 s
    let first = zombies(pid); std::thread::sleep(Duration::from_secs(5));
    let left: Vec<u32> = zombies(pid).into_iter().filter(|z| first.contains(z)).collect();
    assert!(left.is_empty(), "children of the hub left unreaped for 5 s: {left:?}; hub.log:\n{}", std::fs::read_to_string(&log).unwrap_or_default());
    let said = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(said.contains("backplane-hub: started with "), "the hub says what it was started with: {said}");
    drop(_k); let _ = hub.wait();
}

#[test] fn a_hub_started_with_sigchld_blocked_reaps_and_its_project_turn_reaches_the_agent() { a_project_turn_reaches_its_agent(Start::SigchldBlocked) }
#[test] fn a_hub_started_with_sigchld_ignored_reaps_and_its_project_turn_reaches_the_agent() { a_project_turn_reaches_its_agent(Start::SigchldIgnored) }

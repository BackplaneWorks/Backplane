//! Real programs on a real pty, and the actor against a real Hub on a temp home. No agents, no model.
use super::*;
use crate::{api::Caller, hub::{Cfg, Hub, Out}, presenter::Canned};
use std::time::{Duration, Instant};

fn dir(name: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::dir(&format!("term-{name}")) }
async fn hub_in(name: &str) -> (Arc<Hub>, crate::plat::tmp::TempDir, crate::plat::tmp::TempDir) {
    let home = dir(&format!("{name}-home")); let work = dir(&format!("{name}-work"));
    std::fs::write(home.join("events.jsonl"), format!("{{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"f\",\"root\":{},\"at\":1700000000}}\n{{\"$\":\"ThreadCreated\",\"id\":\"t1\",\"project\":\"p1\",\"title\":\"T\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":1700000001}}\n", serde_json::to_string(&work.to_string_lossy()).unwrap())).unwrap();
    std::env::set_var("SHELL", "/bin/sh");
    let hub = Hub::start(Cfg { home: home.to_path_buf(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    start(hub.clone()); (hub, home, work)
}
fn caller() -> Caller { Caller { gen: "1".into(), client: 1 } }
async fn rpc(hub: &Arc<Hub>, m: &str, p: Value) -> Value { let h = hub.rpc_for(m).expect("handler"); h.call(hub, &caller(), &json!(7), m, &p).await.remove(0) }
async fn wait<F: FnMut() -> bool>(secs: u64, mut f: F) -> bool { let end = Instant::now() + Duration::from_secs(secs); while Instant::now() < end { if f() { return true } tokio::time::sleep(Duration::from_millis(25)).await } f() }
/// Everything the hub pushed to client 1 for `key`, as bytes.
/// A client that reads its outbox on a thread of its own and keeps what it got (the hub's Outbox has only an async recv).
pub(crate) struct Tap(Arc<std::sync::Mutex<Vec<Value>>>);
impl Tap {
    pub(crate) fn new(hub: &Arc<Hub>, client: u64) -> Tap {
        let mut ob = hub.attach(client); let v = Arc::new(std::sync::Mutex::new(vec![])); let v2 = v.clone(); let h = tokio::runtime::Handle::current();
        std::thread::spawn(move || h.block_on(async move { while let Some(o) = ob.recv().await { if let Out::Value(x) = o { v2.lock().unwrap().push(x) } } }));
        Tap(v)
    }
    pub(crate) fn take(&self) -> Vec<Value> { std::mem::take(&mut *self.0.lock().unwrap()) }
}
fn drain(rx: &Tap, key: &str, acc: &mut Vec<u8>, resets: &mut Vec<Value>) {
    for v in rx.take() { { if v["t"] == "term" && v["thread"] == key { acc.extend(v["b"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as u8)); if v.get("reset").is_some() { resets.push(v["reset"].clone()) } } } }
}

#[cfg(unix)] // stty, $!, pwd: a POSIX terminal
#[tokio::test] async fn a_program_on_a_pty_sees_a_terminal_and_its_exit_code() {
    let sp = pty::Pty::spawn("/bin/sh", &["-c".into(), "stty size; echo $TERM $COLORTERM; tput cols 2>/dev/null; exit 7".into()], "/tmp", 100, 30).unwrap();
    let (mut out, exit) = (sp.output, sp.exit); let mut all = vec![];
    while let Some(c) = out.recv().await { all.extend(c) }
    let s = String::from_utf8_lossy(&all).into_owned();
    assert!(s.contains("30 100"), "{s}"); assert!(s.contains("xterm-256color truecolor"), "{s}"); assert_eq!(exit.await.unwrap(), 7);
}
// Windows: the pseudo console (ConPTY) runs a program, carries its output and its exit code, takes a resize, and a hang-up ends what it started.
#[cfg(windows)]
#[tokio::test] async fn a_program_on_a_pseudo_console_prints_and_exits_with_its_code() {
    let sp = pty::Pty::spawn("cmd.exe", &["/d".into(), "/c".into(), "echo hello-conpty& exit 7".into()], &std::env::temp_dir().to_string_lossy(), 100, 30).unwrap();
    sp.pty.resize(90, 20);
    let (mut out, exit) = (sp.output, sp.exit); let mut all = vec![];
    while let Ok(Some(c)) = tokio::time::timeout(Duration::from_secs(20), out.recv()).await { all.extend(c) }
    assert!(String::from_utf8_lossy(&all).contains("hello-conpty"), "{}", String::from_utf8_lossy(&all));
    assert_eq!(tokio::time::timeout(Duration::from_secs(20), exit).await.unwrap().unwrap(), 7);
}
#[cfg(windows)]
#[tokio::test] async fn a_hangup_on_windows_ends_the_program_and_what_it_started() {
    let sp = pty::Pty::spawn("cmd.exe", &["/d".into(), "/c".into(), "ping -n 300 127.0.0.1".into()], "", 80, 24).unwrap();
    let pid = sp.pty.pid as u32; assert!(crate::plat::proc::alive(pid));
    tokio::time::sleep(Duration::from_millis(500)).await; sp.pty.hangup();
    assert!(tokio::time::timeout(Duration::from_secs(20), sp.exit).await.is_ok(), "the program ended");
    assert!(wait(10, || !crate::plat::proc::alive(pid)).await);
}
#[tokio::test] async fn an_unknown_command_is_enoent() { let e = pty::Pty::spawn("/no/such/prog", &[], "", 80, 24).err().unwrap(); assert_eq!(e.kind(), std::io::ErrorKind::NotFound) }
#[tokio::test] async fn cat_echoes_and_a_resize_reaches_the_program() {
    let sp = pty::Pty::spawn("/bin/sh", &["-c".into(), "read x; echo got:$x; stty size".into()], "", 80, 24).unwrap();
    let (mut out, exit, p) = (sp.output, sp.exit, sp.pty); p.resize(50, 10); p.write(b"hello\n".to_vec()); let mut all = vec![];
    while let Some(c) = out.recv().await { all.extend(c) }
    let s = String::from_utf8_lossy(&all).into_owned(); assert!(s.contains("got:hello"), "{s}"); assert!(s.contains("10 50"), "{s}"); exit.await.unwrap();
}
#[cfg(unix)] // stty, $!, pwd: a POSIX terminal
#[tokio::test] async fn a_hangup_ends_the_whole_process_group() {
    let sp = pty::Pty::spawn("/bin/sh", &["-c".into(), "sleep 300 & echo $!; wait".into()], "", 80, 24).unwrap();
    let (mut out, exit, p) = (sp.output, sp.exit, sp.pty); let mut all = vec![];
    while !String::from_utf8_lossy(&all).contains('\n') { all.extend(out.recv().await.unwrap()) }
    let child: i32 = String::from_utf8_lossy(&all).trim().parse().unwrap();
    p.hangup(); exit.await.unwrap();
    assert!(wait(5, || !crate::plat::proc::alive(child as u32)).await, "the background sleep is still there");
}
#[tokio::test] async fn output_is_bounded_by_the_queue_not_memory() {
    // a flood the reader does not drain: the program blocks on its pty instead of us buffering without end
    let sp = pty::Pty::spawn("/bin/sh", &["-c".into(), "yes | head -c 200000000".into()], "", 80, 24).unwrap();
    let mut out = sp.output; tokio::time::sleep(Duration::from_millis(500)).await;
    let mut n = 0usize; while let Ok(c) = out.try_recv() { n += c.len() }
    assert!(n <= (64 + 1) * pty::CHUNK, "{n} bytes queued"); sp.pty.hangup();
}

#[cfg(unix)] // stty, $!, pwd: a POSIX terminal
#[tokio::test] async fn a_threads_shell_runs_in_its_folder_streams_bytes_and_takes_keys() {
    let (hub, _home, work) = hub_in("shell").await; let rx = Tap::new(&hub, 1);
    assert_eq!(rpc(&hub, "term.open", json!({"thread":"t1","cols":90,"rows":20})).await["ok"], true);
    let (mut all, mut rs) = (vec![], vec![]);
    assert_eq!(rpc(&hub, "term.input", json!({"thread":"t1","b":"pwd\n".bytes().collect::<Vec<u8>>()})).await["ok"], true);
    let w = crate::plat::fs::canonicalize(&work).unwrap().to_string_lossy().into_owned();
    assert!(wait(10, || { drain(&rx, "t1", &mut all, &mut rs); String::from_utf8_lossy(&all).contains(&w) }).await, "{}", String::from_utf8_lossy(&all));
    // reopening only takes the new size, and does not start a second shell
    rpc(&hub, "term.open", json!({"thread":"t1","cols":100,"rows":30})).await; assert_eq!(get(&hub).unwrap().shell_count().await, 1);
    rpc(&hub, "term.input", json!({"thread":"t1","b":"stty size\n".bytes().collect::<Vec<u8>>()})).await;
    assert!(wait(10, || { drain(&rx, "t1", &mut all, &mut rs); String::from_utf8_lossy(&all).contains("30 100") }).await, "{}", String::from_utf8_lossy(&all));
    rpc(&hub, "term.input", json!({"thread":"t1","b":"exit 3\n".bytes().collect::<Vec<u8>>()})).await;
    assert!(wait(10, || { drain(&rx, "t1", &mut all, &mut rs); String::from_utf8_lossy(&all).contains("[process exited 3]") }).await);
    assert!(wait(5, || get(&hub).map_or(false, |t| t.inner.try_lock().map_or(false, |g| g.shells.is_empty()))).await, "the shell is dropped once it exits");
}

#[tokio::test] async fn a_chat_terminal_runs_its_command_keeps_the_shell_and_the_agent_reads_it() {
    let (hub, home, _work) = hub_in("emb").await; let rx = Tap::new(&hub, 1);
    let (ok, o) = tools::call(&hub, "t1", "terminal_open", &json!({"command":"echo hello-term","title":"Greeter","cols":60,"rows":10})).await.unwrap();
    assert!(ok, "{o}"); let id = o["id"].as_str().unwrap().to_string(); assert_eq!((o["cols"].as_u64(), o["rows"].as_u64(), o["state"].as_str()), (Some(60), Some(10), Some("running")));
    // the entry is logged as an Act of tone term: "title\nCOLSxROWS\ncommand"
    let ev: Value = serde_json::from_str(&hub.store.get(hub.store.len() - 1).unwrap()).unwrap();
    assert_eq!((ev["$"].as_str(), ev["tone"].as_str(), ev["kind"].as_str(), ev["act"].as_str()), (Some("ActivityLogged"), Some("term"), Some("terminal"), Some(id.as_str())));
    assert_eq!(ev["summary"], "Greeter\n60x10\necho hello-term");
    // waitMs waits for the command to end (the mark makes it "done 0" while the shell stays open)
    let (ok, r) = tools::call(&hub, "t1", "terminal_read", &json!({"id":id,"waitMs":10000})).await.unwrap();
    let r = r.as_str().unwrap().to_string(); assert!(ok); assert!(r.contains("state=\"done 0\"") && r.contains("hello-term") && r.contains("title=\"Greeter\""), "{r}");
    assert!(!r.contains("7799"), "the mark never shows");
    // the shell is still there: type into it
    let (ok, o) = tools::call(&hub, "t1", "terminal_send", &json!({"id":id,"text":"echo second\r"})).await.unwrap(); assert!(ok && o["sent"] == 12, "{o}");
    let key = emb::key("t1", &id);
    assert!(wait(10, || { let mut s = String::new(); let t = get(&hub).unwrap(); if let Ok(g) = t.inner.try_lock() { if let Some(sc) = g.screens.get(&key) { s = emb::text(&sc.vt) } } s.contains("second") }).await);
    // a client that joins late gets the screen from a snapshot, as a reset to the terminal's size
    let (mut all, mut rs) = (vec![], vec![]); rx.take();
    assert_eq!(rpc(&hub, "term.snap", json!({"key":key})).await["ok"], true); assert!(wait(5, || { drain(&rx, &key, &mut all, &mut rs); !rs.is_empty() }).await);
    assert_eq!(rs, vec![json!([60, 10])]); assert!(String::from_utf8_lossy(&all).contains("hello-term"));
    // unknown terminal; send/close into nothing
    let (ok, o) = tools::call(&hub, "t1", "terminal_send", &json!({"id":"nope","text":"x"})).await.unwrap(); assert!(!ok && o["error"] == "no terminal nope in this thread");
    let (ok, o) = tools::call(&hub, "t1", "terminal_read", &json!({"id":"nope"})).await.unwrap(); assert!(!ok && o == "There is no such terminal in this thread.");
    // close hangs it up: stopped
    let (ok, o) = tools::call(&hub, "t1", "terminal_close", &json!({"id":id})).await.unwrap(); assert!(ok && o["state"] == "stopped");
    assert!(wait(10, || std::fs::read_to_string(home.join("terms").join(&*crate::plat::fs::file_name((&key).as_ref()))).map_or(false, |f| f.starts_with("60x10\t"))).await, "the last screen is written when the program exits");
    let f = std::fs::read_to_string(home.join("terms").join(&*crate::plat::fs::file_name((&key).as_ref()))).unwrap(); assert!(f.starts_with("60x10\tstopped\n") || f.starts_with("60x10\texited"), "{}", f.lines().next().unwrap());
}

#[tokio::test] async fn the_next_message_note_lists_unseen_terminals_once_and_stops_the_unpinned() {
    let (hub, _home, _work) = hub_in("note").await;
    let (_, a) = tools::call(&hub, "t1", "terminal_open", &json!({"command":"echo AAA; sleep 60","title":"A"})).await.unwrap(); let a = a["id"].as_str().unwrap().to_string();
    let (_, b) = tools::call(&hub, "t1", "terminal_open", &json!({"command":"echo BBB; sleep 60","title":"B"})).await.unwrap(); let b = b["id"].as_str().unwrap().to_string();
    let t = get(&hub).unwrap();
    assert!(wait(10, || { let g = t.inner.try_lock(); g.map_or(false, |g| [&a, &b].iter().all(|i| g.screens.get(&emb::key("t1", i)).map_or(false, |s| { let x = emb::text(&s.vt); x.contains("AAA") || x.contains("BBB") }))) }).await);
    // the agent reads A, so only B is news
    tools::call(&hub, "t1", "terminal_read", &json!({"id":a})).await.unwrap();
    // B is pinned: it stays when the person writes
    hub.commit(&json!({"$":"SettingSet","key":emb::pin_key(&emb::key("t1", &b)),"value":"1","at":1700000100})).await.unwrap();
    let n = tools::on_person_message(&hub, "t1").await;
    assert!(n.starts_with("\n\n<terminals>\nWhat the terminals you opened in this chat show now") && n.contains(&format!("id=\"{b}\"")) && !n.contains(&format!("id=\"{a}\"")), "{n}");
    assert_eq!(tools::on_person_message(&hub, "t1").await, "", "told once");
    assert!(wait(5, || t.inner.try_lock().map_or(false, |g| g.screens[&emb::key("t1", &a)].state != "running")).await, "the unpinned one stopped");
    assert_eq!(t.inner.lock().await.screens[&emb::key("t1", &b)].state, "running", "the pinned one stays");
    t.kill(&emb::key("t1", &b)).await;
}

#[tokio::test] async fn a_restart_runs_the_command_again_under_the_same_key_and_says_so_to_every_client() {
    let (hub, _home, _work) = hub_in("restart").await; let rx = Tap::new(&hub, 1);
    let (_, o) = tools::call(&hub, "t1", "terminal_open", &json!({"command":"echo once","cols":40,"rows":5})).await.unwrap(); let id = o["id"].as_str().unwrap().to_string(); let key = emb::key("t1", &id);
    tools::call(&hub, "t1", "terminal_read", &json!({"id":id,"waitMs":10000})).await.unwrap();
    rx.take();
    let r = rpc(&hub, "term.restart", json!({"thread":"t1","id":id})).await; assert_eq!(r["ok"], true, "{r}");
    let (mut all, mut rs) = (vec![], vec![]);
    assert!(wait(10, || { drain(&rx, &key, &mut all, &mut rs); !rs.is_empty() && String::from_utf8_lossy(&all).matches("once").count() >= 1 }).await);
    assert_eq!(rs[0], json!([40, 5]));
    assert!(hub.setting(&emb::at_key(&key)).await.is_some(), "the restart makes it live from now");
    assert_eq!(rpc(&hub, "term.restart", json!({"thread":"t1","id":"missing"})).await["error"], "no such terminal");
    tools::call(&hub, "t1", "terminal_close", &json!({"id":id})).await.unwrap();
}

#[tokio::test] async fn terminals_are_bounded_per_chat_and_inputs_by_size() {
    let (hub, _home, _work) = hub_in("bounds").await; let mut ids = vec![];
    for i in 0..MAX_PER_THREAD { let (ok, o) = tools::call(&hub, "t1", "terminal_open", &json!({"command":"sleep 60","title":format!("s{i}")})).await.unwrap(); assert!(ok, "{o}"); ids.push(o["id"].as_str().unwrap().to_string()) }
    let (ok, o) = tools::call(&hub, "t1", "terminal_open", &json!({"command":"sleep 60"})).await.unwrap(); assert!(!ok && o["error"].as_str().unwrap().contains("too many"), "{o}");
    let (ok, _) = tools::call(&hub, "t1", "terminal_open", &json!({"command":"  "})).await.unwrap(); assert!(!ok);
    let (ok, _) = tools::call(&hub, "t1", "terminal_send", &json!({"id":ids[0],"text":"x".repeat(MAX_INPUT + 1)})).await.unwrap(); assert!(!ok);
    for i in &ids { tools::call(&hub, "t1", "terminal_close", &json!({"id":i})).await.unwrap(); }
}

#[tokio::test] async fn a_screen_on_disk_answers_after_the_hub_starts_again() {
    let (hub, home, _work) = hub_in("disk").await; let key = "x:t1:xdisk-1";
    let mut vt = Vt::new(30, 4); vt.feed(b"saved screen\r\nline2"); let mut b = b"30x4\tdone 0\n".to_vec(); b.extend(emb::snap(&vt)); std::fs::write(home.join("terms").join(&*crate::plat::fs::file_name((key).as_ref())), b).unwrap();
    let t = get(&hub).unwrap(); let (state, report) = t.read(key).await.unwrap();
    assert_eq!(state, "exited 0", "its command had ended; its shell went with the hub"); assert!(report.contains("saved screen\nline2"), "{report}");
    assert!(t.read("../../etc/passwd").await.is_none(), "keys never leave the terms folder");
}

#[tokio::test] async fn a_copy_hub_runs_no_chat_terminal_but_logs_the_entry() {
    let (hub, home, _work) = hub_in("copy").await; let t = get(&hub).unwrap();
    let marker = home.join("copy-must-not-exist"); let cmd = format!("touch {}", marker.display());
    t.run_with(true, "x:t1:xcopy-1", "T", &home.to_string_lossy(), &cmd, 40, 5, false).await;
    t.run_with(true, "x:t1:xcopy-1", "T", &home.to_string_lossy(), &cmd, 40, 5, true).await;
    assert_eq!(t.shell_count().await, 0); assert!(!t.read_known("x:t1:xcopy-1").await); assert!(!marker.exists());
    // the person's own panel shell still opens, as in (TermOp is allowed on a copy)
    t.open_shell("t1", 80, 24).await; assert_eq!(t.shell_count().await, 1); t.kill("t1").await;
}
#[tokio::test] async fn only_the_newest_finished_screens_stay_in_memory() {
    let mut g = Inner::default();
    for i in 0..(KEEP_FINISHED + 10) { let k = format!("x:t:{i}"); g.screens.insert(k.clone(), Screen::new("", Vt::new(80, 24), true, "exited 0", 0, "")); Terms::finish(&mut g, &k) }
    assert_eq!(g.screens.len(), KEEP_FINISHED); assert!(g.screens.contains_key(&format!("x:t:{}", KEEP_FINISHED + 9))); assert!(!g.screens.contains_key("x:t:0"));
}
#[tokio::test] async fn an_entry_is_found_through_the_threads_own_rows_with_its_whole_command() {
    let (hub, _home, _work) = hub_in("find").await; let cmd = format!("echo {}", "a".repeat(2000));
    let (_, o) = tools::call(&hub, "t1", "terminal_open", &json!({"command":cmd,"title":"Long"})).await.unwrap(); let id = o["id"].as_str().unwrap().to_string();
    // 100 later rows push it past a page
    for i in 0..100 { hub.commit(&json!({"$":"ActivityLogged","thread":"t1","act":format!("n{i}"),"tone":"info","kind":"x","summary":"s","at":1700000500u64 + i})).await.unwrap(); }
    let (sum, _) = find_act(&hub, "t1", &id).await.unwrap(); assert_eq!(emb::parse_summary(&sum).cmd, cmd);
    assert!(find_act(&hub, "t1", "nope").await.is_none()); assert!(find_act(&hub, "other", &id).await.is_none());
    tools::call(&hub, "t1", "terminal_close", &json!({"id":id})).await.unwrap();
}
#[tokio::test] async fn dropping_a_terminal_ends_its_reader_and_a_program_that_ignores_the_hangup_is_killed() {
    let (hub, _home, _work) = hub_in("force").await; let t = get(&hub).unwrap();
    let (_, o) = tools::call(&hub, "t1", "terminal_open", &json!({"command":"trap '' HUP; echo up; while true; do sleep 1; done"})).await.unwrap(); let key = emb::key("t1", o["id"].as_str().unwrap());
    let pid = { assert!(wait(10, || t.inner.try_lock().map_or(false, |g| g.shells.contains_key(&key))).await); t.inner.lock().await.shells[&key].pid };
    tokio::time::sleep(Duration::from_millis(500)).await; t.kill(&key).await;
    // the module's own pty-wait thread reaps the shell; this test must never call waitpid itself (waitpid(-1) took other tests' children: their
    // Command::output() / tokio wait got ECHILD, so a project.add's `sh` failed and the side-by-side probe's child "had no child processes")
    assert!(wait(10, || !crate::plat::proc::alive(pid as u32)).await, "the shell that ignores the hangup is killed and reaped");
    assert!(wait(10, || t.inner.try_lock().map_or(false, |g| !g.shells.contains_key(&key))).await, "the shell is gone after the escalation to SIGKILL");
}

#[tokio::test] async fn a_restart_of_a_finished_terminal_counts_against_the_limits_and_old_screens_read_back_are_capped() {
    let (hub, home, _work) = hub_in("limits").await; let t = get(&hub).unwrap();
    // 40 finished screens on disk: reading each back keeps at most KEEP_FINISHED in memory
    for i in 0..40 { let k = format!("x:t1:xold-{i}"); std::fs::write(home.join("terms").join(&*crate::plat::fs::file_name((&k).as_ref())), b"30x4\texited 0\nold\n").unwrap(); assert!(t.read(&k).await.is_some()) }
    assert!(t.inner.lock().await.screens.len() <= KEEP_FINISHED);
    // a finished terminal restarted with the chat already full is refused
    for i in 0..MAX_PER_THREAD { let (ok, _) = tools::call(&hub, "t1", "terminal_open", &json!({"command":"sleep 60","title":format!("s{i}")})).await.unwrap(); assert!(ok) }
    let before = t.shell_count().await;
    t.run_with(false, "x:t1:xold-1", "T", "", "sleep 60", 40, 5, true).await;
    assert_eq!(t.shell_count().await, before, "a finished terminal restarted past the per-chat limit does not run");
    for k in t.running_of("t1").await { t.kill(&k).await }
}

#[tokio::test] async fn a_flood_skips_a_client_that_does_not_read_keeps_it_and_snaps_it_when_it_catches_up() {
    let (hub, _home, _work) = hub_in("flood").await; hub.set_outbox_max(256 * 1024);
    let reader = Tap::new(&hub, 1);
    let mut stalled = hub.attach(2); // never read until we say so
    assert_eq!(rpc(&hub, "term.open", json!({"thread":"t1","cols":80,"rows":24})).await["ok"], true);
    rpc(&hub, "term.input", json!({"thread":"t1","b":"yes flood | head -c 20000000; echo FLOODDONE\n".bytes().collect::<Vec<u8>>()})).await;
    let t = get(&hub).unwrap();
    assert!(wait(90, || { reader.take(); t.inner.try_lock().map_or(false, |g| g.screens.get("t1").map_or(false, |sc| emb::text(&sc.vt).contains("FLOODDONE"))) }).await, "the flood ran to its end");
    assert!(hub.is_attached(2), "a skipped client is not dropped");
    assert!(hub.outbox_pending(2) <= 256 * 1024 + 4 * pty::CHUNK, "{} bytes pending", hub.outbox_pending(2));
    // it reads again: it gets a snapshot (a reset to the screen's size) and the stale mark is gone
    let got = tokio::spawn(async move { let mut resets = 0; while let Ok(Some(o)) = tokio::time::timeout(Duration::from_secs(5), stalled.recv()).await { if let Out::Value(v) = o { if v["t"] == "term" && v.get("reset").is_some() { resets += 1; break } } } resets });
    assert_eq!(got.await.unwrap(), 1, "one snapshot reached the client that caught up");
    hub.detach(2); // its reader is gone: a gone client is forgotten
    assert!(wait(5, || t.inner.try_lock().map_or(false, |g| g.skipped.is_empty())).await);
    rpc(&hub, "term.input", json!({"thread":"t1","b":"exit\n".bytes().collect::<Vec<u8>>()})).await;
}

#[test] fn on_a_copy_the_terminal_and_the_browser_answer_with_the_copy_refusal() {
    // (copy::on() is the process's switch: here the words the callers show are checked)
    assert_eq!(crate::copy::refusal("a terminal"), "backplane: a hub on a copied log (BACKPLANE_COPY=1) does not run a terminal");
    assert!(crate::proc::refusal_with(true, crate::proc::Kind::Terminal, "/bin/sh", None).is_some() && crate::proc::refusal_with(true, crate::proc::Kind::Browser, "/x/backplane-browser", None).is_some());
}

#[tokio::test] async fn the_tasks_tab_lists_a_bots_tasks_and_task_actions_reach_bots_task() {
    let (hub, _home, _work) = hub_in("tasktab").await; let d = driver::get(&hub).expect("driver");
    d.set_task("b1", "login", r#"{"v":1,"slug":"login","name":"Log in","description":"sign in to the portal","params":[{"name":"user","label":"User name","kind":"text","example":"ada"}],"steps":[]}"#, 1700000001).await;
    d.set_task("b2", "other", r#"{"v":1,"slug":"other","name":"Other bot's","description":"","params":[],"steps":[]}"#, 1700000002).await;
    let v = d.tasks_view("b1").await;
    assert_eq!(v["items"].as_array().unwrap().len(), 1, "{v}"); assert_eq!(v["items"][0]["name"], "Log in"); assert_eq!(v["items"][0]["params"][0]["example"], "ada"); assert_eq!(v["items"][0]["last"], "");
    assert_eq!(rpc(&hub, "bots.task", json!({"bot":"b1","op":"delete","slug":"login"})).await["ok"], true);
    assert!(d.tasks_view("b1").await["items"].as_array().unwrap().is_empty());
    assert_eq!(d.tasks_view("b2").await["items"].as_array().unwrap().len(), 1);
}

#[test] fn the_presenter_reads_a_terminal_entry_as_emb_does() {
    for s in ["Pick\n100x30\necho a\necho b", "t\nbad\nc", "x\n5x1\n", "", "only"] {
        let p = emb::parse_summary(s); assert_eq!(crate::present::thread::term_summary(s), (p.title, p.cols, p.rows, p.cmd), "{s:?}");
    }
    assert_eq!(crate::present::thread::term_key("t1", "x9"), emb::key("t1", "x9"));
}

/// The bytes a client holds of `key`, by the offset rule phones use (stream::merge): frames of another run, or past a gap, are not taken.
fn take_frames(rx: &Tap, key: &str, g: &mut u64, held: &mut u64, acc: &mut Vec<u8>, gaps: &mut u32, states: &mut Vec<String>) {
    for v in rx.take() { if v["t"] != "term" || v["thread"] != key { continue }
        if let Some(st) = v["state"].as_str() { if v["b"].as_array().map_or(true, |b| b.is_empty()) { states.push(st.into()); continue } }
        let b: Vec<u8> = v["b"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as u8).collect();
        let (Some(fg), Some(o)) = (v["g"].as_u64(), v["o"].as_u64()) else { continue };
        if v.get("reset").is_some() { continue } // a snapshot: a phone would draw it; here only the raw stream is compared
        if fg != *g { *gaps += 1; continue }
        match stream::merge(*held, o, b.len()) { stream::Merge::Take(i) => { acc.extend(&b[i..]); *held = o + b.len() as u64 } stream::Merge::Skip => {} stream::Merge::Gap => *gaps += 1 }
    }
}

#[cfg(unix)]
#[tokio::test] async fn a_terminal_streams_with_offsets_and_a_client_resumes_from_where_it_was() {
    let (hub, _home, _work) = hub_in("stream").await; let rx = Tap::new(&hub, 1);
    let (ok, o) = tools::call(&hub, "t1", "terminal_open", &json!({"command":"for i in 1 2 3 4 5; do echo line-$i; done","title":"S","cols":60,"rows":10})).await.unwrap(); assert!(ok);
    let key = emb::key("t1", o["id"].as_str().unwrap());
    let mut watch = std::collections::HashSet::new();
    // a phone watches from nothing: a snapshot of the run, and where it stands
    let r = crate::data::term::call(&hub, 0, &mut watch, "term.watch", &json!({"key":key})).await.unwrap().unwrap();
    assert!(watch.contains(&key)); assert!(r["reset"].is_array() && r["g"].as_u64().unwrap() > 0, "{r}");
    let g0 = r["g"].as_u64().unwrap();
    // the live frames say where their bytes start: taken in order they are the whole stream, with no gap
    let (mut g, mut held, mut acc, mut gaps, mut states) = (g0, 0u64, vec![], 0u32, vec![]);
    let t = get(&hub).unwrap();
    assert!(wait(10, || { take_frames(&rx, &key, &mut g, &mut held, &mut acc, &mut gaps, &mut states); String::from_utf8_lossy(&acc).contains("line-5") }).await, "{}", String::from_utf8_lossy(&acc));
    assert_eq!(gaps, 0); assert!(wait(5, || states.iter().any(|s| s == "done 0") || { take_frames(&rx, &key, &mut g, &mut held, &mut acc, &mut gaps, &mut states); false }).await, "the command's end is a state frame: {states:?}");
    // a phone that held up to line-2 resumes: only what came after, from its offset
    let all = String::from_utf8_lossy(&acc).into_owned(); let cut = all.find("line-3").unwrap() as u64;
    let r = crate::data::term::call(&hub, 0, &mut watch, "term.read", &json!({"key":key,"g":g0,"from":cut})).await.unwrap().unwrap();
    assert!(r.get("reset").is_none(), "{r}"); assert_eq!(r["from"].as_u64(), Some(cut));
    use base64::Engine; let tail = base64::engine::general_purpose::STANDARD.decode(r["b"].as_str().unwrap()).unwrap();
    assert!(String::from_utf8_lossy(&tail).starts_with("line-3") && !String::from_utf8_lossy(&tail).contains("line-2"));
    assert_eq!(cut + tail.len() as u64, r["o"].as_u64().unwrap(), "from + what it got = where the stream stands");
    // another run (or an offset never reached) gets a snapshot instead
    let r = crate::data::term::call(&hub, 0, &mut watch, "term.read", &json!({"key":key,"g":g0 - 1,"from":cut})).await.unwrap().unwrap(); assert!(r["reset"].is_array());
    let r = crate::data::term::call(&hub, 0, &mut watch, "term.read", &json!({"key":key,"g":g0,"from":1u64 << 40})).await.unwrap().unwrap(); assert!(r["reset"].is_array());
    // typed through the data API, echoed in the stream
    let r = crate::data::term::call(&hub, 0, &mut watch, "term.write", &json!({"key":key,"text":"echo typed-$((40+2))\r"})).await.unwrap().unwrap(); assert_eq!(r["sent"], 21);
    assert!(wait(10, || { take_frames(&rx, &key, &mut g, &mut held, &mut acc, &mut gaps, &mut states); String::from_utf8_lossy(&acc).contains("typed-42") }).await);
    // the push a watching socket gets, and none for a key it does not watch
    let f = frame(&key, Some(1), b"ab", None); assert_eq!(crate::data::term::push_of(&watch, &f).unwrap()["b"], "YWI=");
    assert!(crate::data::term::push_of(&Default::default(), &f).is_none());
    // a list names it, by thread
    let l = crate::data::term::call(&hub, 0, &mut watch, "term.list", &json!({"thread":"t1"})).await.unwrap().unwrap();
    assert!(l["terms"].as_array().unwrap().iter().any(|x| x["key"] == key.as_str() && x["kind"] == "chat" && x["running"] == true), "{l}");
    // stop: a state frame says stopped, and it stays stopped once the shell is gone
    crate::data::term::call(&hub, 0, &mut watch, "term.stop", &json!({"key":key})).await.unwrap().unwrap();
    assert!(wait(10, || { take_frames(&rx, &key, &mut g, &mut held, &mut acc, &mut gaps, &mut states); states.iter().any(|s| s == "stopped") }).await, "{states:?}");
    assert!(wait(10, || t.inner.try_lock().map_or(false, |g| !g.shells.contains_key(&key))).await);
    assert_eq!(t.stream_read(&key, 0, None).await.unwrap()["state"], "stopped");
    crate::data::term::call(&hub, 0, &mut watch, "term.unwatch", &json!({"key":key})).await.unwrap().unwrap(); assert!(watch.is_empty());
    let e = crate::data::term::call(&hub, 0, &mut watch, "term.write", &json!({"key":"../x"})).await.unwrap().unwrap_err(); assert_eq!(e, "no such terminal");
}

#[cfg(unix)]
#[tokio::test] async fn a_resize_reaches_the_program_and_every_client_gets_the_screen_at_the_new_size() {
    let (hub, _home, _work) = hub_in("resize").await; let rx = Tap::new(&hub, 1);
    let mut watch = std::collections::HashSet::new();
    // a phone's own shell in the project's folder
    let r = crate::data::term::call(&hub, 0, &mut watch, "term.new", &json!({"project":"p1","cols":70,"rows":12})).await.unwrap().unwrap();
    let key = r["key"].as_str().unwrap().to_string(); assert!(key.starts_with("s:p1:"), "{r}"); assert_eq!((r["cols"].as_u64(), r["rows"].as_u64()), (Some(70), Some(12)));
    let l = crate::data::term::call(&hub, 0, &mut watch, "term.list", &json!({"project":"p1"})).await.unwrap().unwrap();
    assert!(l["terms"].as_array().unwrap().iter().any(|x| x["key"] == key.as_str() && x["kind"] == "shell"), "{l}");
    assert!(crate::data::term::call(&hub, 0, &mut watch, "term.list", &json!({"project":"nope"})).await.unwrap().unwrap()["terms"].as_array().unwrap().is_empty());
    rx.take();
    crate::data::term::call(&hub, 0, &mut watch, "term.resize", &json!({"key":key,"cols":44,"rows":9})).await.unwrap().unwrap();
    let (mut all, mut rs) = (vec![], vec![]);
    assert!(wait(5, || { drain(&rx, &key, &mut all, &mut rs); !rs.is_empty() }).await); assert_eq!(rs[0], json!([44, 9]), "a snapshot at the new size goes to every client");
    crate::data::term::call(&hub, 0, &mut watch, "term.write", &json!({"key":key,"b":"c3R0eSBzaXplCg=="})).await.unwrap().unwrap(); // "stty size\n"
    assert!(wait(10, || { drain(&rx, &key, &mut all, &mut rs); String::from_utf8_lossy(&all).contains("9 44") }).await, "{}", String::from_utf8_lossy(&all));
    // the same size again is no news
    rx.take(); crate::data::term::call(&hub, 0, &mut watch, "term.resize", &json!({"key":key,"cols":44,"rows":9})).await.unwrap().unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await; rs.clear(); drain(&rx, &key, &mut all, &mut rs); assert!(rs.is_empty());
    crate::data::term::call(&hub, 0, &mut watch, "term.write", &json!({"key":key,"text":"exit 4\n"})).await.unwrap().unwrap();
    assert!(wait(10, || { let t = get(&hub).unwrap(); t.inner.try_lock().map_or(false, |g| g.screens.get(&key).map_or(false, |s| s.state == "exited 4")) }).await);
    assert!(crate::data::term::call(&hub, 0, &mut watch, "term.new", &json!({"project":"nope"})).await.unwrap().is_err());
}

#[cfg(unix)]
#[tokio::test] async fn a_running_chat_terminal_is_on_disk_and_reads_back_stopped_after_the_hub_stops() {
    let (hub, home, _work) = hub_in("survive").await;
    let (_, o) = tools::call(&hub, "t1", "terminal_open", &json!({"command":"echo kept-$((6*7)); sleep 60","title":"K","cols":50,"rows":8})).await.unwrap();
    let key = emb::key("t1", o["id"].as_str().unwrap()); let t = get(&hub).unwrap();
    assert!(wait(10, || t.inner.try_lock().map_or(false, |g| g.screens.get(&key).map_or(false, |s| emb::text(&s.vt).contains("kept-42")))).await);
    t.save_dirty().await;
    let f = std::fs::read_to_string(home.join("terms").join(&*crate::plat::fs::file_name(key.as_ref()))).unwrap(); assert!(f.starts_with("50x8\trunning\n"), "{}", f.lines().next().unwrap());
    // a hub that starts again holds no screen: it reads the file, and a terminal that was running is stopped, its screen kept
    t.inner.lock().await.screens.remove(&key);
    let r = t.stream_read(&key, 0, None).await.unwrap(); assert_eq!(r["state"], "stopped"); assert_eq!((r["cols"].as_u64(), r["rows"].as_u64()), (Some(50), Some(8)));
    use base64::Engine; assert!(String::from_utf8_lossy(&base64::engine::general_purpose::STANDARD.decode(r["b"].as_str().unwrap()).unwrap()).contains("kept-42"));
    t.kill(&key).await;
}

#[test] fn the_desktop_and_phones_see_a_terminal_entry_as_a_card_with_its_key() {
    let e = crate::present::thread::Ent { id: "x9".into(), msg: false, role: "term".into(), akind: "terminal".into(), text: "Build\n100x20\nmake".into(), at: 1 };
    let v = crate::present::thread::plain_act(&e); assert_eq!((v["kind"].as_str(), v["title"].as_str(), v["cols"].as_u64(), v["rows"].as_u64(), v["cmd"].as_str()), (Some("term"), Some("Build"), Some(100), Some(20), Some("make")));
    let p = crate::data::term::entry("t1", "x9", "Build\n100x20\nmake", 5); assert_eq!(p["kind"], "term"); assert_eq!(p["term"]["key"], "x:t1:x9"); assert_eq!(p["term"]["cmd"], "make");
}

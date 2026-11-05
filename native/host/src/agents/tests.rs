//! End-to-end tests of the agent module against a real Hub on a temp home with stand-in `claude` scripts.
//! Parity: the events a stand-in turn produces here are compared with the events the JS hub wrote for the same
//! stand-in (test/gpui/agents/fixtures/js-*.jsonl, recorded by test/gpui/agents/record_js.mjs), ids and clocks normalised.
use super::*;
use crate::{api::Caller, hub::{Cfg, Hub}, presenter::Canned, store::Store};
use std::path::{Path, PathBuf};

const STANDIN: &str = include_str!("../../../../test/gpui/agents/fixtures/standin-claude.sh");
const JS_TURN: &str = include_str!("../../../../test/gpui/agents/fixtures/js-turn.jsonl");
const JS_QUEUE: &str = include_str!("../../../../test/gpui/agents/fixtures/js-queue.jsonl");
const JS_INTERRUPT: &str = include_str!("../../../../test/gpui/agents/fixtures/js-interrupt.jsonl");
const T1: &str = "t1700000001-2";

fn dir(name: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-ag-{}-{}", std::process::id(), name)); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
fn script(d: &Path, name: &str, body: &str) { use std::os::unix::fs::PermissionsExt; let p = d.join(name); std::fs::write(&p, body).unwrap(); std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); }
/// The JS test fleet's stand-in (test/tools/perf_standin.ts) with the settings the recordings used.
fn perf_standin(d: &Path, secs: u32) { script(d, "claude", &STANDIN.replacen("\n", &format!("\nexport FAKE_PER=2 FAKE_CYCLE=5 FAKE_SECS={secs} FAKE_LOG=/dev/null\n"), 1)); }

fn base_log(work: &Path, n: usize, extra: &str) -> String {
    let mut s = format!("{{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"fixture\",\"root\":{},\"at\":1700000000}}\n", serde_json::to_string(&work.to_string_lossy()).unwrap());
    for i in 0..n { let id = if i == 0 { T1.to_string() } else { format!("t17000010{i:02}-{i}") };
        s += &format!("{{\"$\":\"ThreadCreated\",\"id\":\"{id}\",\"project\":\"p1\",\"title\":\"Fixture {i}\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":{}}}\n", 1700000001 + i) }
    s + extra
}
async fn start(name: &str, log: impl FnOnce(&Path) -> String, cli: impl FnOnce(&Path) -> Cli) -> (Arc<Hub>, Arc<Agents>, PathBuf, PathBuf) {
    let home = dir(&format!("{name}-home")); let work = dir(&format!("{name}-work"));
    std::fs::write(home.join("events.jsonl"), log(&work)).unwrap();
    let hub = Hub::start(Cfg { home: home.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let ag = Agents::start_with(hub.clone(), cli(&work)).await.unwrap();
    (hub, ag, home, work)
}
fn caller() -> Caller { Caller { gen: "1".into(), client: 1 } }
async fn rpc(hub: &Arc<Hub>, m: &str, p: Value) -> Value { let h = hub.rpc_for(m).expect("handler"); h.call(hub, &caller(), &json!(7), m, &p).await.remove(0) }
async fn log_from(hub: &Arc<Hub>, from: usize) -> Vec<Value> { hub.store.range(from, usize::MAX).unwrap().iter().filter_map(|l| serde_json::from_str(l).ok()).collect() }
async fn len(hub: &Arc<Hub>) -> usize { hub.store.len() }
async fn until(hub: &Arc<Hub>, from: usize, secs: u64, f: impl Fn(&[Value]) -> bool) -> Vec<Value> {
    let end = Instant::now() + Duration::from_secs(secs);
    loop { let ev = log_from(hub, from).await; if f(&ev) || Instant::now() > end { return ev } tokio::time::sleep(Duration::from_millis(50)).await }
}
fn is(e: &Value, t: &str) -> bool { e["$"] == t }
fn turn_is(e: &Value, s: &str) -> bool { is(e, "TurnChanged") && e["state"] == s }

// ---- normalisation: ids and clocks only ----
fn digits_to(s: &str, to: &str) -> String { let mut o = String::new(); let mut run = false; for c in s.chars() { if c.is_ascii_digit() { if !run { o += to; run = true } } else { run = false; o.push(c) } } o }
/// "@1793842277304@" stand-in clocks inside a streamed text, a pid or a clock inside an id.
fn norm_str(k: &str, s: &str) -> String {
    match k {
        "msg" | "act" | "session" | "key" | "basis" => {
            if s.starts_with("live:") { return format!("live:{}:G:stopped", s.split(':').nth(1).unwrap_or("")) }
            if s.starts_with("fake-") { return "fake-P".into() }
            let p: Vec<&str> = s.split(['-', ':']).collect();
            if s.starts_with('m') && p.len() == 2 && p[0][1..].chars().all(|c| c.is_ascii_digit()) && p[1].chars().all(|c| c.is_ascii_digit()) { return "M".into() }  // a message the hub numbered
            if (s.starts_with('m') || s.starts_with("tu")) && p.len() >= 3 { return format!("{}P-{}", &s[..if s.starts_with("tu") { 2 } else { 1 }], p[1..].join("-")) } // a stand-in pid
            s.into()
        }
        "text" => { let mut o = String::new(); let mut rest = s; while let Some(i) = rest.find('@') { o += &rest[..i]; let after = &rest[i + 1..]; match after.find('@') { Some(j) if after[..j].chars().all(|c| c.is_ascii_digit()) && j > 0 => { o += "@N@"; rest = &after[j + 1..] } _ => { o.push('@'); rest = after } } } o + rest }
        _ => s.into(),
    }
}
fn norm(v: &Value) -> String {
    let setting = v["$"] == "SettingSet"; let skey = if setting { v["key"].as_str().unwrap_or("") } else { "" };
    let mut o = serde_json::Map::new();
    for (k, x) in v.as_object().unwrap() {
        let nx = match (k.as_str(), x) {
            ("at", _) => json!(0),
            ("key", Value::String(s)) if setting => { let (a, _) = s.split_once('.').unwrap_or((s, "")); json!(if a == "stopped" || a == "failed" { format!("{a}.M") } else { s.clone() }) }
            ("value", Value::String(s)) if skey.starts_with("turn.from.") => { let _ = s; json!("@M") }
            (k @ ("msg" | "act" | "session" | "key" | "text"), Value::String(s)) => json!(norm_str(k, s)),
            _ => x.clone(),
        };
        o.insert(k.clone(), nx);
    }
    Value::Object(o).to_string()
}
/// The agent-core events of a thread: not the time-driven policies (liveness, autosettle, alert, told, summary), which are M4's.
fn core(ev: &[Value], th: &str) -> Vec<Value> {
    ev.iter().filter(|e| {
        let s = e.to_string();
        let key = e["key"].as_str().unwrap_or("");
        let policy = ["liveness.", "autosettle.", "alert.", "told.", "summary.at.", "star.nudge"].iter().any(|p| key.starts_with(p));
        !policy && (s.contains(th) || key.starts_with("stopped.") || key.starts_with("failed.") || key == "tok.epoch" || key == "tok.recount")
    }).cloned().collect()
}
fn lines_of(s: &str) -> Vec<Value> { s.lines().filter_map(|l| serde_json::from_str(l).ok()).collect() }
fn shape(ev: &[Value]) -> Vec<String> { ev.iter().map(norm).collect() }

// ---- parity ----
#[tokio::test(flavor = "multi_thread")] async fn a_stand_in_turn_matches_the_js_hub_event_for_event() {
    let sd = dir("par-bin"); perf_standin(&sd, 2);
    let (hub, ag, _h, _w) = start("par", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await;
    let from = len(&hub).await;
    let r = rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort hello"})).await; assert_eq!(r["ok"], true, "{r}");
    let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| turn_is(e, "completed")) && ev.iter().any(|e| is(e, "UsageSet") && e["used"] == 0)).await;
    tokio::time::sleep(Duration::from_millis(300)).await; let ev = { let mut e2 = log_from(&hub, from).await; if e2.len() < ev.len() { e2 = ev } e2 };
    let (got, want) = (shape(&core(&ev, T1)), shape(&core(&lines_of(JS_TURN), T1)));
    assert_eq!(got.len(), want.len(), "event counts: rust {} js {}\nfirst difference:\n{}", got.len(), want.len(), got.iter().zip(want.iter()).find(|(a, b)| a != b).map(|(a, b)| format!("rust {a}\njs   {b}")).unwrap_or_default());
    for (i, (a, b)) in got.iter().zip(want.iter()).enumerate() { assert_eq!(a, b, "event {i}") }
    assert_eq!(ag.stats.procs_spawned.load(Relaxed), 1);
    ag.shutdown();
}

#[tokio::test(flavor = "multi_thread")] async fn queued_messages_go_straight_to_the_process_and_each_marks_the_turn_running() {
    let sd = dir("q-bin"); perf_standin(&sd, 2);
    let (hub, ag, _h, _w) = start("q", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await;
    let from = len(&hub).await;
    for t in ["first", "second", "third"] { assert_eq!(rpc(&hub, "turn.start", json!({"thread":T1,"text":t})).await["ok"], true); tokio::time::sleep(Duration::from_millis(250)).await }
    let ev = until(&hub, from, 30, |ev| ev.iter().filter(|e| turn_is(e, "completed")).count() >= 1).await;
    let (got, want) = (shape(&core(&ev, T1)), shape(&core(&lines_of(JS_QUEUE), T1)));
    // up to and including the third message's fallback.input: one process, three messages, running each time, no queue
    let upto = |v: &[String]| { let mut n = 0; for (i, e) in v.iter().enumerate() { if e.contains("fallback.input") { n += 1; if n == 3 { return i + 1 } } } v.len() };
    // the process's own first events (session, messages) may land anywhere among them, in both hubs: compare what the hub itself writes
    let own = |v: &[String]| v.iter().filter(|e| e.contains("\"role\":\"user\"") || e.contains("\"state\":\"running\"") || e.contains("turn.from") || e.contains("restart.retry") || e.contains("provider.resume") || e.contains("fallback.input")).cloned().collect::<Vec<_>>();
    assert_eq!(own(&got[..upto(&got)]), own(&want[..upto(&want)]));
    assert_eq!(ag.stats.procs_spawned.load(Relaxed), 1, "one process per thread");
    ag.shutdown();
}

#[tokio::test(flavor = "multi_thread")] async fn an_interrupt_posts_the_streamed_text_and_stops_a_running_turn_only() {
    let sd = dir("i-bin"); perf_standin(&sd, 30);
    let (hub, ag, _h, _w) = start("i", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await;
    assert_eq!(rpc(&hub, "turn.interrupt", json!({"thread":T1})).await["ok"], true); // idle: nothing
    assert!(log_from(&hub, 0).await.iter().all(|e| !turn_is(e, "interrupted")), "interrupt_idle_nothing");
    let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqlong"})).await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(rpc(&hub, "turn.interrupt", json!({"thread":T1})).await["ok"], true);
    let ev = until(&hub, from, 5, |ev| ev.iter().any(|e| is(e, "SettingSet") && e["key"] == "provider.resume.t1700000001-2" && ev.iter().any(|x| turn_is(x, "interrupted")))).await;
    let c = core(&ev, T1); let sh = shape(&c);
    let want = shape(&core(&lines_of(JS_INTERRUPT), T1));
    let first = |v: &[String]| v.iter().position(|e| e.contains("interrupted")).unwrap();
    // the JS hub's order around a stop: the streamed text (if any), the interrupted turn, the stopped mark, the two cleared retry settings
    let (a, b) = (first(&sh), first(&want));
    assert!(sh[a - 1].contains("live:") && sh[a - 1].contains(":stopped"), "{}", sh[a - 1]); assert!(want[b - 1].contains("live:"));
    assert_eq!(sh[a - 1].replace(&norm_text(&sh[a - 1]), ""), want[b - 1].replace(&norm_text(&want[b - 1]), ""));
    assert_eq!(&sh[a..a + 4], &want[b..b + 4]);
    // the turn does not complete after it (claude_result_after_stop_quiet), but the process lives on
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert!(log_from(&hub, from).await.iter().all(|e| !turn_is(e, "completed")));
    assert_eq!(ag.stats.procs_spawned.load(Relaxed), 1);
    ag.shutdown();
}
fn norm_text(e: &str) -> String { let v: Value = serde_json::from_str(e).unwrap(); v["text"].as_str().unwrap_or("").to_string() }

// law interrupt_idle_nothing: an interrupt of a thread that is not running writes nothing
#[tokio::test(flavor = "multi_thread")] async fn interrupt_idle_nothing() {
    let sd = dir("ii-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("ii", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await; let n = len(&hub).await;
    assert_eq!(rpc(&hub, "turn.interrupt", json!({"thread":T1})).await["ok"], true); assert_eq!(len(&hub).await, n); ag.shutdown();
}
// law steer_claude_in_place: a message into a running Claude turn goes to the same process; nothing is stopped or started
#[tokio::test(flavor = "multi_thread")] async fn steer_claude_in_place() {
    let sd = dir("sc-bin"); perf_standin(&sd, 3);
    let (hub, ag, _h, _w) = start("sc", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"a"})).await; tokio::time::sleep(Duration::from_millis(400)).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"b"})).await; tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(ag.stats.procs_spawned.load(Relaxed), 1); assert_eq!(ag.stats.procs_exited.load(Relaxed), 0); ag.shutdown();
}

// ---- Rust-only guarantees ----
const EXITS: &str = "#!/bin/sh\nread l\necho '{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"fake-x\"}'\nexit 3\n";
const KILLED: &str = "#!/bin/sh\nread l\nkill -9 $$\n";
const DEAF: &str = "#!/bin/sh\nsleep 600\n";

#[tokio::test(flavor = "multi_thread")] async fn a_process_that_exits_mid_turn_fails_the_turn_and_is_reaped() {
    let sd = dir("x-bin"); script(&sd, "claude", EXITS);
    let (hub, ag, _h, _w) = start("x", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"go"})).await;
    let ev = until(&hub, from, 10, |ev| ev.iter().any(|e| turn_is(e, "failed"))).await;
    let row = ev.iter().find(|e| is(e, "ActivityLogged") && e["kind"] == "exit").expect("an exit row"); assert_eq!(row["summary"], "agent exited with code 3"); assert_eq!(row["tone"], "error");
    assert!(ev.iter().any(|e| is(e, "SessionSet")), "what it wrote before it went is kept");
    assert!(ev.iter().any(|e| e["key"].as_str().map_or(false, |k| k.starts_with("failed."))));
    assert_eq!(ag.stats.procs_exited.load(Relaxed), 1);
    // the next message starts a fresh process
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"again"})).await; tokio::time::sleep(Duration::from_millis(500)).await; assert_eq!(ag.stats.procs_spawned.load(Relaxed), 2);
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_killed_process_is_named_as_probably_out_of_memory() {
    let sd = dir("k-bin"); script(&sd, "claude", KILLED);
    let (hub, ag, _h, _w) = start("k", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"go"})).await;
    let ev = until(&hub, from, 10, |ev| ev.iter().any(|e| turn_is(e, "failed"))).await;
    assert!(ev.iter().any(|e| e["kind"] == "exit" && e["summary"].as_str().unwrap_or("").contains("signal 9")), "{ev:?}");
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn an_agent_outside_the_standin_folder_is_refused_and_the_turn_fails() {
    let sd = dir("r-bin"); // no claude in it
    let (hub, ag, _h, _w) = start("r", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    assert_eq!(rpc(&hub, "turn.start", json!({"thread":T1,"text":"go"})).await["ok"], true); // the message is taken, as in the JS hub
    let ev = until(&hub, from, 5, |ev| ev.iter().any(|e| turn_is(e, "failed"))).await;
    let row = ev.iter().find(|e| e["kind"] == "spawn").expect("spawn row"); assert_eq!(row["summary"], format!("could not start claude: {} has no runnable claude", sd.display()));
    assert_eq!(ag.stats.procs_spawned.load(Relaxed), 0); assert_eq!(ag.stats.spawn_failures.load(Relaxed), 1);
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn other_providers_are_refused_not_dropped() {
    let sd = dir("o-bin"); script(&sd, "claude", EXITS);
    let (hub, ag, _h, _w) = start("o", |w| base_log(w, 1, "").replace("\"provider\":\"claude\"", "\"provider\":\"codex\""), |_| Cli::standin(&sd)).await; let n = len(&hub).await;
    let r = rpc(&hub, "turn.start", json!({"thread":T1,"text":"go"})).await; assert_eq!(r["ok"], false); assert!(r["error"].as_str().unwrap().contains("codex"));
    assert_eq!(len(&hub).await, n, "nothing is written for a message nobody can run");
    assert_eq!(rpc(&hub, "turn.start", json!({"thread":"nope","text":"go"})).await["error"], "unknown thread");
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_resent_message_is_stored_once() {
    let sd = dir("d-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("d", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    for _ in 0..3 { assert_eq!(rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort x","msg":"mine-1"})).await["ok"], true) }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(log_from(&hub, from).await.iter().filter(|e| is(e, "MessagePosted") && e["msg"] == "mine-1").count(), 1);
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn the_first_message_names_a_new_thread() {
    let sd = dir("n-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("n", |w| base_log(w, 1, "").replace("Fixture 0", "New thread"), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort Route the 3V3 rail\nand more"})).await;
    let ev = log_from(&hub, from).await; assert_eq!(ev[0], json!({"$":"ThreadRenamed","id":T1,"title":"zqshort Route the 3V3 rail","at":ev[0]["at"]}));
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_flood_to_one_thread_is_bounded_and_answered() {
    let sd = dir("f-bin"); script(&sd, "claude", DEAF);
    let (_hub, ag, _h, _w) = start("f", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await;
    let mut hs = vec![]; for i in 0..400 { let a = ag.clone(); hs.push(tokio::spawn(async move { a.deliver(T1, &format!("m-{i}"), &"y".repeat(5000), None, vec![]).await })) }
    let mut ok = 0; let mut refused = 0; for h in hs { match h.await.unwrap() { Ok(()) => ok += 1, Err(e) => { assert!(e.contains("too many") || e.contains("not taking input"), "{e}"); refused += 1 } } }
    assert!(ok > 0 && ok + refused == 400, "ok {ok} refused {refused}");
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn shutdown_leaves_no_process_behind() {
    let sd = dir("s-bin"); script(&sd, "claude", "#!/bin/sh\nsleep 600 &\necho $! > \"$0.kid\"\necho $$ > \"$0.pid\"\nwait\n");
    let (hub, ag, _h, _w) = start("s", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"go"})).await; tokio::time::sleep(Duration::from_millis(600)).await;
    let rd = |f: &str| std::fs::read_to_string(sd.join(f)).unwrap().trim().parse::<u32>().unwrap();
    let (pid, kid) = (rd("claude.pid"), rd("claude.kid")); assert!(proc::alive(pid) && proc::alive(kid));
    ag.shutdown();
    for _ in 0..80 { if !proc::alive(pid) && !proc::alive(kid) { break } tokio::time::sleep(Duration::from_millis(100)).await }
    assert!(!proc::alive(pid), "the agent outlived shutdown"); assert!(!proc::alive(kid), "the agent's child outlived shutdown");
}

// ---- restart recovery ----
fn running_log(work: &Path, extra: &str) -> String { base_log(work, 1, &format!("{{\"$\":\"MessagePosted\",\"thread\":\"{T1}\",\"msg\":\"m1\",\"role\":\"user\",\"text\":\"build it\",\"at\":1700000100}}\n{{\"$\":\"TurnChanged\",\"thread\":\"{T1}\",\"state\":\"running\",\"at\":1700000100}}\n{extra}")) }
#[tokio::test(flavor = "multi_thread")] async fn a_restart_marks_the_cut_turn_stopped_and_carries_it_on() {
    let sd = dir("c-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("c", |w| running_log(w, ""), |_| Cli::standin(&sd)).await;
    let ev = until(&hub, 0, 10, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
    let t: Vec<&Value> = ev.iter().collect();
    let at = |f: &dyn Fn(&Value) -> bool| t.iter().position(|e| f(e)).unwrap();
    let cut = at(&|e| is(e, "ActivityLogged") && e["kind"] == "restart" && e["tone"] == "error"); let int = at(&|e| turn_is(e, "interrupted"));
    let carry = at(&|e| is(e, "MessagePosted") && e["role"] == "user" && e["text"].as_str().unwrap_or("").starts_with(world::CARRY_TEXT));
    assert!(cut < int && int < carry);
    assert!(t[cut]["summary"].as_str().unwrap().starts_with("Backplane restarted at ")); assert_eq!(t[int + 1]["key"], "stopped.m1");
    assert!(t.iter().any(|e| is(e, "MessageFrom") && e["from"] == "backplane" && e["kind"] == "carry"));
    assert!(t.iter().any(|e| is(e, "ActivityLogged") && e["summary"] == "Continuing after the restart"));
    assert_eq!(ag.stats.procs_spawned.load(Relaxed), 1); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn with_restart_continue_off_nothing_is_started() {
    let sd = dir("co-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("co", |w| running_log(w, "{\"$\":\"SettingSet\",\"key\":\"restart.continue\",\"value\":\"off\",\"at\":1700000101}\n"), |_| Cli::standin(&sd)).await;
    tokio::time::sleep(Duration::from_millis(500)).await; let ev = log_from(&hub, 0).await;
    assert!(ev.iter().any(|e| turn_is(e, "interrupted"))); assert!(!ev.iter().any(|e| e["text"].as_str().unwrap_or("").starts_with(world::CARRY_TEXT)));
    assert_eq!(ag.stats.procs_spawned.load(Relaxed), 0); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_turn_that_made_no_progress_since_the_last_carry_is_left_stopped() {
    // carry_never_loops: the newest entry past restart notes is already the continue message
    let sd = dir("cl-bin"); perf_standin(&sd, 1);
    let carry = format!("{{\"$\":\"MessagePosted\",\"thread\":\"{T1}\",\"msg\":\"c1\",\"role\":\"user\",\"text\":\"{} If a tool\",\"at\":1700000102}}\n{{\"$\":\"ActivityLogged\",\"thread\":\"{T1}\",\"act\":\"r1\",\"tone\":\"info\",\"kind\":\"restart\",\"summary\":\"Continuing after the restart\",\"at\":1700000102}}\n", world::CARRY_TEXT);
    let (hub, ag, _h, _w) = start("cl", |w| running_log(w, &carry), |_| Cli::standin(&sd)).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(log_from(&hub, 0).await.iter().any(|e| turn_is(e, "interrupted"))); assert_eq!(ag.stats.procs_spawned.load(Relaxed), 0); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_restart_carries_on_at_most_a_burst_at_once() {
    let sd = dir("cb-bin"); script(&sd, "claude", DEAF);
    let (hub, ag, _h, _w) = start("cb", |w| { let mut l = base_log(w, 6, ""); for i in 0..6 { let id = if i == 0 { T1.to_string() } else { format!("t17000010{i:02}-{i}") }; l += &format!("{{\"$\":\"MessagePosted\",\"thread\":\"{id}\",\"msg\":\"u{i}\",\"role\":\"user\",\"text\":\"x\",\"at\":1700000100}}\n{{\"$\":\"TurnChanged\",\"thread\":\"{id}\",\"state\":\"running\",\"at\":1700000100}}\n") } l }, |_| Cli::standin(&sd)).await;
    tokio::time::sleep(Duration::from_millis(800)).await; let ev = log_from(&hub, 0).await;
    assert_eq!(ev.iter().filter(|e| turn_is(e, "interrupted")).count(), 6, "every cut turn is marked");
    assert_eq!(ev.iter().filter(|e| e["text"].as_str().unwrap_or("").starts_with(world::CARRY_TEXT)).count(), BURST);
    assert_eq!(ag.stats.procs_spawned.load(Relaxed), BURST as u64); ag.shutdown();
}

// ---- load: twenty agents ----
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn twenty_agents_stream_at_once_with_every_event_written_once() {
    let sd = dir("l-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("l", |w| base_log(w, 20, ""), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    let ids: Vec<String> = (0..20).map(|i| if i == 0 { T1.to_string() } else { format!("t17000010{i:02}-{i}") }).collect();
    let t0 = Instant::now();
    for id in &ids { assert_eq!(rpc(&hub, "turn.start", json!({"thread":id,"text":"zqshort go"})).await["ok"], true) }
    let ev = until(&hub, from, 60, |ev| ev.iter().filter(|e| turn_is(e, "completed")).count() == 20).await;
    assert_eq!(ev.iter().filter(|e| turn_is(e, "completed")).count(), 20, "stats {:?}", ag.stats.events);
    for id in &ids { let mine: Vec<&Value> = ev.iter().filter(|e| e["thread"] == id.as_str() || e["id"] == id.as_str()).collect();
        assert_eq!(mine.iter().filter(|e| is(e, "MessagePosted") && e["role"] == "assistant").count(), 16, "{id}"); // 80 ticks / 5 per message
        assert_eq!(mine.iter().filter(|e| is(e, "SessionSet")).count(), 1); }
    eprintln!("20 agents: {:?} wall, {} events, {} lines, max batch {}", t0.elapsed(), ag.stats.events.load(Relaxed), ag.stats.lines.load(Relaxed), ag.stats.max_batch_lines.load(Relaxed));
    assert_eq!(ag.actors(), 20); ag.shutdown();
    tokio::time::sleep(Duration::from_millis(500)).await; assert_eq!(Store::open(&hub.cfg.home.join("events.jsonl")).unwrap().bad, 0);
}

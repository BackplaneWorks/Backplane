//! End-to-end tests of the agent module against a real Hub on a temp home with stand-in `claude` scripts.
//! Parity: the events a stand-in turn produces here are compared with the events the JS hub wrote for the same
//! stand-in (test/agents/fixtures/js-*.jsonl, recorded by test/agents/record_js.mjs), ids and clocks normalised.
use super::*;
use crate::{api::Caller, hub::{Cfg, Hub}, presenter::Canned, store::Store};
use std::path::{Path, PathBuf};

const STANDIN: &str = include_str!("../../../../test/agents/fixtures/standin-claude.sh");
const JS_TURN: &str = include_str!("../../../../test/agents/fixtures/js-turn.jsonl");
const JS_QUEUE: &str = include_str!("../../../../test/agents/fixtures/js-queue.jsonl");
const JS_INTERRUPT: &str = include_str!("../../../../test/agents/fixtures/js-interrupt.jsonl");
const T1: &str = "t1700000001-2";

/// a folder per call: helpers shared by several tests (`deliveries`) ask by the same name, and tests run side by side
fn dir(name: &str) -> PathBuf { static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0); let p = std::env::temp_dir().join(format!("bp-ag-{}-{}-{}", std::process::id(), name, N.fetch_add(1, Relaxed))); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
fn script(d: &Path, name: &str, body: &str) { use crate::plat::fs::PermissionsExt; let p = d.join(name); std::fs::write(&p, body).unwrap(); std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); }
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
    let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
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
    // every wait below is on an event or a counter, never on time: the bounds only say when to give up, and are long, because a gate runs this on a busy machine
    let ev = until(&hub, from, 60, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "assistant")).await; // a block ended; text streams after it
    assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "assistant"), "no block ended");
    let d0 = ag.stats.deltas.load(Relaxed); let end = Instant::now() + Duration::from_secs(60); while ag.stats.deltas.load(Relaxed) == d0 && Instant::now() < end { tokio::time::sleep(Duration::from_millis(10)).await } // interrupt once new text has streamed
    assert!(ag.stats.deltas.load(Relaxed) > d0, "no text streamed after the first block");
    assert_eq!(rpc(&hub, "turn.interrupt", json!({"thread":T1})).await["ok"], true);
    let want = shape(&core(&lines_of(JS_INTERRUPT), T1));
    let first = |v: &[String]| v.iter().position(|e| e.contains("interrupted"));
    // the stop is whole when the interrupted turn and the four lines after it in the JS hub's order are all in (they need not come in one write)
    let whole = |ev: &[Value]| { let sh = shape(&core(ev, T1)); first(&sh).map_or(false, |a| a >= 1 && sh[a..].iter().filter(|e| !e.contains("\"UsageSet\"")).count() >= 4) && ev.iter().any(|e| is(e, "SettingSet") && e["key"] == "provider.resume.t1700000001-2") };
    let ev = until(&hub, from, 60, whole).await; assert!(whole(&ev), "the stop did not finish: {:?}", shape(&core(&ev, T1)));
    // usage is metered from the stream as it comes, so on a busy machine a UsageSet can fall anywhere around the stop: the order below is of the turn's own lines
    let metered = |v: Vec<String>| v.into_iter().filter(|e| !e.contains("\"UsageSet\"")).collect::<Vec<_>>();
    let c = core(&ev, T1); let (sh, want) = (metered(shape(&c)), metered(want));
    let first = |v: &[String]| first(v).unwrap();
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
    let (hub, ag, _h, _w) = start("o", |w| base_log(w, 1, "").replace("\"provider\":\"claude\"", "\"provider\":\"nonesuch\""), |_| Cli::standin(&sd)).await; let n = len(&hub).await;
    let r = rpc(&hub, "turn.start", json!({"thread":T1,"text":"go"})).await; assert_eq!(r["ok"], false); assert!(r["error"].as_str().unwrap().contains("nonesuch"));
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
/// A claude that answers a title one-shot (`--no-session-persistence`) with a quoted sentence and otherwise is the perf stand-in; never a real model.
fn title_standin(d: &Path, answer: &str) {
    perf_standin(d, 1); crate::plat::fs::rename(d.join("claude"), d.join("claude.real")).unwrap();
    script(d, "claude", &format!("#!/bin/sh\ncase \"$*\" in *--no-session-persistence*) cat >/dev/null; printf '%s\\n' '{{\"type\":\"result\",\"is_error\":false,\"result\":{}}}';; *) exec \"$(dirname \"$0\")/claude.real\" \"$@\";; esac\n", serde_json::to_string(answer).unwrap().replace('\'', "'\\''")));
}
fn renames(ev: &[Value]) -> Vec<String> { ev.iter().filter(|e| is(e, "ThreadRenamed")).map(|e| e["title"].as_str().unwrap().to_string()).collect() }
#[tokio::test(flavor = "multi_thread")] async fn the_title_model_retitles_a_new_thread_after_its_first_message() {
    let sd = dir("tt-bin"); title_standin(&sd, "\"Route the 3V3 power rail.\"");
    let (hub, ag, _h, _w) = start("tt", |w| base_log(w, 1, "").replace("Fixture 0", "New thread"), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort Route the 3V3 rail and more"})).await;
    let ev = until(&hub, from, 20, |ev| renames(ev).len() >= 2).await;
    assert_eq!(renames(&ev), vec!["zqshort Route the 3V3 rail and more".to_string(), "Route the 3V3 power rail".to_string()], "the first line at once, then the model's title");
    ag.shutdown();
}
/// Real Haiku through the person's Claude login on a temp home: `cargo test --offline --bin backplane-hub real_haiku -- --ignored --nocapture`.
#[tokio::test(flavor = "multi_thread")] #[ignore] async fn real_haiku_titles_a_thread() {
    let home = dir("real-home"); let work = dir("real-work");
    std::fs::write(home.join("events.jsonl"), base_log(&work, 1, &format!("{{\"$\":\"MessagePosted\",\"thread\":\"{T1}\",\"msg\":\"m1\",\"role\":\"user\",\"text\":\"the \\\"buy boards\\\" thing shows up at inopportune times on the thread header, make it only appear once a board file has changed\",\"at\":1700000005}}\n")).replace("Fixture 0", "New thread")).unwrap();
    let hub = Hub::start(Cfg { home, version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let ag = Agents::start(hub.clone()).await.unwrap(); let from = len(&hub).await;
    let r = ag.retitle(T1, Some("New thread"), false).await; println!("REAL TITLE: {r:?}");
    assert!(r.is_ok(), "{r:?}"); assert!(renames(&log_from(&hub, from).await).len() == 1); ag.shutdown();
}
/// /btw: answered by the writing model off to the side; nothing reaches the log or a turn. A socket's caller gets the reply late, and is not held up meanwhile.
#[tokio::test(flavor = "multi_thread")] async fn a_side_question_is_answered_and_leaves_no_trace() {
    let sd = dir("btw-bin"); title_standin(&sd, "Because of the rail");
    let (hub, ag, _h, _w) = start("btw", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    let r = rpc(&hub, "thread.btw", json!({"thread":T1,"q":"why?"})).await;
    assert_eq!((r["result"]["btw"].as_str(), r["result"]["q"].as_str(), r["result"]["thread"].as_str()), (Some("Because of the rail"), Some("why?"), Some(T1)), "{r}");
    assert_eq!(rpc(&hub, "thread.btw", json!({"thread":T1,"q":"  "})).await["error"], crate::session::composer::BTW_EMPTY);
    let (mut c, _) = hub.connect(true); let mut out = hub.attach(c.client);
    assert!(hub.request(&mut c, &json!({"id":9,"m":"thread.btw","p":{"thread":T1,"q":"why?"}})).await.is_empty(), "the socket goes on; the answer comes later");
    let late = tokio::time::timeout(Duration::from_secs(20), async { loop { if let Some(crate::hub::Out::Value(v)) = out.recv().await { if v["t"] == "reply" { return v } } } }).await.expect("the answer comes");
    assert_eq!((late["id"].as_u64(), late["result"]["btw"].as_str()), (Some(9), Some("Because of the rail")));
    assert!(log_from(&hub, from).await.is_empty(), "the log never sees it");
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_failed_title_model_leaves_the_first_line() {
    let sd = dir("tf-bin"); perf_standin(&sd, 1); // its one-shot answers nothing usable (the stand-in's reply is no result line)
    let (hub, ag, _h, _w) = start("tf", |w| base_log(w, 1, "").replace("Fixture 0", "New thread"), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort Keep me"})).await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(renames(&log_from(&hub, from).await), vec!["zqshort Keep me".to_string()]);
    assert!(ag.retitle(T1, None, false).await.is_err());
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn regenerating_keeps_a_typed_title_unless_forced_and_leaves_bots_alone() {
    let sd = dir("tr-bin"); title_standin(&sd, "Fresh title from the model");
    let (hub, ag, _h, _w) = start("tr", |w| base_log(w, 2, &format!("{{\"$\":\"MessagePosted\",\"thread\":\"{T1}\",\"msg\":\"m1\",\"role\":\"user\",\"text\":\"fix the sidebar dots\",\"at\":1700000005}}\n{{\"$\":\"SettingSet\",\"key\":\"title.person.{T1}\",\"value\":\"1\",\"at\":1700000006}}\n{{\"$\":\"BotSet\",\"id\":\"b1\",\"name\":\"Bot\",\"thread\":\"t1700000010-1\",\"at\":1700000007}}\n")), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    let (ok, m) = ag.tool_update(T1, &json!({"threadId":T1,"title":""})).await; assert!(!ok, "a typed title is kept: {m}");
    assert!(renames(&log_from(&hub, from).await).is_empty());
    let (ok, m) = ag.tool_update(T1, &json!({"threadId":T1,"title":"","force":true})).await; assert!(ok && m["title"] == "Fresh title from the model", "{m}");
    assert_eq!(rpc(&hub, "thread.retitle", json!({"thread":T1})).await["ok"], true, "the menu's request");
    assert!(ag.retitle("t1700000010-1", None, true).await.is_err(), "a bot's chat keeps its title");
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
fn tid(i: usize) -> String { if i == 0 { T1.to_string() } else { format!("t17000010{i:02}-{i}") } }
/// `n` threads cut by a restart; the last by id has run the longest
fn cut_log(w: &Path, n: usize) -> String { let mut l = base_log(w, n, ""); for i in 0..n { let (id, at) = (tid(i), 1700000100 + n - i); l += &format!("{{\"$\":\"MessagePosted\",\"thread\":\"{id}\",\"msg\":\"u{i}\",\"role\":\"user\",\"text\":\"x\",\"at\":{at}}}\n{{\"$\":\"TurnChanged\",\"thread\":\"{id}\",\"state\":\"running\",\"at\":{at}}}\n") } l }
fn carried(ev: &[Value]) -> Vec<(String, u64)> { ev.iter().filter(|e| is(e, "MessagePosted") && e["role"] == "user" && e["text"].as_str().unwrap_or("").starts_with(world::CARRY_TEXT)).map(|e| (e["thread"].as_str().unwrap().to_string(), e["at"].as_u64().unwrap())).collect() }
fn waiting(ev: &[Value]) -> Vec<String> { let mut m = std::collections::BTreeMap::new(); for e in ev.iter().filter(|e| is(e, "SettingSet") && e["key"].as_str().unwrap_or("").starts_with("carry.wait.")) { m.insert(e["key"].as_str().unwrap().to_string(), e["value"] == "1"); } m.into_iter().filter(|x| x.1).map(|x| x.0).collect() }
#[tokio::test(flavor = "multi_thread")] async fn a_restart_brings_back_every_cut_thread_oldest_first_and_a_few_at_a_time() {
    let sd = dir("ca-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("ca", |w| cut_log(w, 10), |_| Cli::standin(&sd)).await;
    let ev = until(&hub, 0, 45, |ev| carried(ev).len() == 10 && ev.iter().filter(|e| turn_is(e, "completed")).count() == 10).await;
    let c = carried(&ev); let order: Vec<String> = c.iter().map(|x| x.0.clone()).collect();
    assert_eq!(order, (0..10).rev().map(tid).collect::<Vec<_>>(), "the longest running first, every one of them, once");
    for i in 0..10 { let th = tid(i);
        let mine: Vec<&Value> = ev.iter().filter(|e| e["thread"] == th.as_str()).collect();
        assert_eq!(mine.iter().filter(|e| e["summary"] == "Continuing after the restart").count(), 1, "{th}");
        let at = |s: &str| mine.iter().rposition(|e| turn_is(e, s)).unwrap_or_else(|| panic!("{th} has no {s} turn"));
        assert!(at("interrupted") < at("running") && at("running") < at("completed"), "{th} took a turn after the cut");
    }
    // read from the hub's own log, never against the wall clock (a loaded machine stretches the burst but never the gaps' lower bound):
    // the burst is carried before the first of the line is let go (a thread of the line is the only one whose wait mark is cleared, and that comes after a gap's sleep)...
    let pos = |f: &dyn Fn(&Value) -> bool| ev.iter().position(|e| f(e));
    let cleared = pos(&|e| is(e, "SettingSet") && e["key"].as_str().unwrap_or("").starts_with("carry.wait.") && e["value"] == "").expect("the line is let go");
    let burst_end = ev.iter().enumerate().filter(|(_, e)| is(e, "MessagePosted") && e["role"] == "user" && e["text"].as_str().unwrap_or("").starts_with(world::CARRY_TEXT)).nth(BURST - 1).unwrap().0;
    assert!(burst_end < cleared, "the first BURST go before anyone in line is let go");
    // ...and each of the line follows the one before it by a gap (logged whole seconds: one short)
    for k in BURST..10 { assert!(c[k].1 + 1 >= c[k - 1].1 + CARRY_GAP_S, "a gap before {}: {c:?}", c[k].0); }
    assert_eq!(waiting(&ev), Vec::<String>::new(), "nothing left in line"); assert_eq!(ag.stats.procs_spawned.load(Relaxed), 10); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn restart_continue_turned_off_stops_those_still_in_line() {
    let sd = dir("co2-bin"); script(&sd, "claude", DEAF);
    let (hub, ag, _h, _w) = start("co2", |w| cut_log(w, 6), |_| Cli::standin(&sd)).await;
    assert_eq!(waiting(&log_from(&hub, 0).await).len(), 2, "those past the burst are written down");
    hub.commit(&json!({"$":"SettingSet","key":"restart.continue","value":"off","at":1700000200})).await.unwrap();
    let ev = until(&hub, 0, 12, |ev| waiting(ev).is_empty()).await;
    assert_eq!(waiting(&ev), Vec::<String>::new()); assert_eq!(carried(&ev).len(), BURST); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_second_restart_keeps_the_line_and_forgets_what_the_person_took_over() {
    let sd = dir("c2-bin"); perf_standin(&sd, 1);
    let wait = |i: usize| format!("{{\"$\":\"SettingSet\",\"key\":\"carry.wait.{}\",\"value\":\"1\",\"at\":1700000150}}\n", tid(i));
    let state = |i: usize, s: &str| format!("{{\"$\":\"TurnChanged\",\"thread\":\"{}\",\"state\":\"{s}\",\"at\":1700000150}}\n", tid(i));
    // 0 still waited when the hub stopped; 1's own turn ended meanwhile, and a later Stop must not read as a restart's cut
    let (hub, ag, _h, _w) = start("c2", |w| { let mut l = cut_log(w, 2); l += &(state(0, "interrupted") + &wait(0) + &state(1, "completed") + &wait(1)); l }, |_| Cli::standin(&sd)).await;
    let ev = until(&hub, 0, 12, |ev| !carried(ev).is_empty() && ev.iter().any(|e| turn_is(e, "completed") && e["at"].as_u64().unwrap_or(0) > 1700000200)).await;
    assert_eq!(carried(&ev).into_iter().map(|c| c.0).collect::<Vec<_>>(), vec![tid(0)]);
    assert_eq!(waiting(&ev), Vec::<String>::new()); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_restart_carries_a_codex_thread_on_in_its_own_session() {
    let sd = dir("cx-bin"); let args = sd.join("args");
    script(&sd, "codex", &format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\necho '{{\"type\":\"turn.completed\",\"usage\":{{\"input_tokens\":1,\"cached_input_tokens\":0,\"output_tokens\":1}}}}'\n", crate::plat::fs::script_path(&args)));
    let (hub, ag, _h, _w) = start("cx", |w| { let l = cut_log(w, 1).replace("\"provider\":\"claude\"", "\"provider\":\"codex\""); l + &format!("{{\"$\":\"SessionSet\",\"id\":\"{T1}\",\"session\":\"S-cut\",\"at\":1700000120}}\n") }, |_| Cli::standin(&sd)).await;
    let ev = until(&hub, 0, 10, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
    assert_eq!(carried(&ev).len(), 1, "{ev:?}");
    let a = std::fs::read_to_string(&args).unwrap(); assert!(a.starts_with("exec\nresume\nS-cut\n") && a.contains(world::CARRY_TEXT), "{a}"); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_restart_carries_on_at_most_a_burst_at_once() {
    let sd = dir("cb-bin"); script(&sd, "claude", DEAF);
    let (hub, ag, _h, _w) = start("cb", |w| { let mut l = base_log(w, 6, ""); for i in 0..6 { let id = if i == 0 { T1.to_string() } else { format!("t17000010{i:02}-{i}") }; l += &format!("{{\"$\":\"MessagePosted\",\"thread\":\"{id}\",\"msg\":\"u{i}\",\"role\":\"user\",\"text\":\"x\",\"at\":1700000100}}\n{{\"$\":\"TurnChanged\",\"thread\":\"{id}\",\"state\":\"running\",\"at\":1700000100}}\n") } l }, |_| Cli::standin(&sd)).await;
    // waited for in the log, never a pause: the burst is there once BURST carry messages are, and whatever else comes comes a gap later
    let ev = until(&hub, 0, 60, |ev| carried(ev).len() >= BURST && ev.iter().filter(|e| turn_is(e, "interrupted")).count() >= 6).await;
    assert_eq!(ev.iter().filter(|e| turn_is(e, "interrupted")).count(), 6, "every cut turn is marked");
    let c = carried(&ev); assert!(c.len() >= BURST, "{c:?}");
    assert_eq!(waiting(&ev).len(), 6 - c.len(), "the others are written down in line, not started");
    if c.len() > BURST { assert!(c[BURST].1 + 1 >= c[BURST - 1].1 + CARRY_GAP_S, "one past the burst came a gap later: {c:?}") } // a very late look may find the first of the line
    assert!(ag.stats.procs_spawned.load(Relaxed) <= c.len() as u64); ag.shutdown();
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

// ---- the MCP server, prompt and inputs ----
const MCP_INIT: &str = include_str!("../../../../test/agents/fixtures/mcp-initialize.json");
const MCP_PING: &str = include_str!("../../../../test/agents/fixtures/mcp-ping.json");
const MCP_BADTOOL: &str = include_str!("../../../../test/agents/fixtures/mcp-badtool.json");
const MCP_THREADLIST: &str = include_str!("../../../../test/agents/fixtures/mcp-threadlist.json");

async fn post(port: u16, path: &str, body: &str) -> (u16, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    s.write_all(format!("POST {path} HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
    let mut out = String::new(); s.read_to_string(&mut out).await.unwrap();
    let status = out.split(' ').nth(1).unwrap_or("0").parse().unwrap_or(0); (status, out.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
}
fn path_of(url: &str) -> String { format!("/{}", url.splitn(4, '/').nth(3).unwrap()) }
async fn rpc_mcp(ag: &Arc<Agents>, thread: &str, gen: &str, method: &str, params: Value) -> (u16, Value) {
    let m = ag.mcp.get().unwrap(); let (st, b) = post(m.port, &path_of(&m.url(thread, gen)), &json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string()).await;
    (st, if b.is_empty() { Value::Null } else { serde_json::from_str(&b).unwrap() })
}
fn tool_text(r: &Value) -> Value { serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap() }

#[tokio::test(flavor = "multi_thread")] async fn the_mcp_server_answers_as_the_js_hub_does() {
    let sd = dir("m-bin"); perf_standin(&sd, 1);
    let (_hub, ag, _h, _w) = start("m", |w| base_log(w, 2, ""), |_| Cli::standin(&sd)).await;
    let js = |s: &str| -> Value { serde_json::from_str(s).unwrap() };
    assert_eq!(rpc_mcp(&ag, T1, "", "initialize", json!({"protocolVersion":"2025-06-18"})).await, (200, js(MCP_INIT)));
    assert_eq!(rpc_mcp(&ag, T1, "", "ping", json!({})).await, (200, js(MCP_PING)));
    assert_eq!(rpc_mcp(&ag, T1, "", "tools/call", json!({"name":"no_such_tool","arguments":{}})).await, (200, js(MCP_BADTOOL)));
    assert_eq!(rpc_mcp(&ag, T1, "", "nope", json!({})).await, (202, Value::Null)); // unknown methods and notifications: no reply
    assert_eq!(rpc_mcp(&ag, T1, "", "notifications/initialized", json!({})).await.0, 202);
    let (_, l) = rpc_mcp(&ag, T1, "", "tools/list", json!({})).await; let all: Value = serde_json::from_str(mcp::TOOLS_JSON).unwrap();
    let want: Vec<&Value> = all["result"]["tools"].as_array().unwrap().iter().filter(|t| mcp::IMPLEMENTED.contains(&t["name"].as_str().unwrap())).collect();
    let listed: Vec<&Value> = l["result"]["tools"].as_array().unwrap().iter().collect();
    assert_eq!(listed.iter().copied().filter(|t| mcp::IMPLEMENTED.contains(&t["name"].as_str().unwrap())).collect::<Vec<_>>(), want, "the definitions are the JS hub's");
    for n in ["wake_on", "wake_list", "wake_cancel"] { let d = listed.iter().find(|t| t["name"] == n).unwrap_or_else(|| panic!("{n} is listed")); assert_eq!(*d, all["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == n).unwrap(), "{n}: the JS hub's definition") } // tool sets add to the list
    // thread_list: the JS hub's fields in its order
    let (_, tl) = rpc_mcp(&ag, T1, "", "tools/call", json!({"name":"thread_list","arguments":{}})).await; let mine = tool_text(&tl); let theirs = tool_text(&js(MCP_THREADLIST));
    let keys = |v: &Value| v["threads"][0].as_object().unwrap().keys().cloned().collect::<Vec<_>>(); assert_eq!(keys(&mine), keys(&theirs)); assert_eq!(mine["threads"].as_array().unwrap().len(), 2);
    assert_eq!(mine["threads"][0]["phase"], "idle");
}
#[tokio::test(flavor = "multi_thread")] async fn the_mcp_server_refuses_what_has_no_key_and_what_is_not_a_post() {
    let sd = dir("m2-bin"); perf_standin(&sd, 1);
    let (_hub, ag, _h, _w) = start("m2", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await; let m = ag.mcp.get().unwrap(); let body = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
    assert_eq!(post(m.port, &format!("/mcp/{T1}"), body).await.0, 401);                       // no key
    assert_eq!(post(m.port, &format!("/mcp/{T1}?k={}", "0".repeat(64)), body).await.0, 401);   // a wrong one
    let other = path_of(&m.url("t1700001001-1", "")); assert_eq!(post(m.port, &other.replace("t1700001001-1", T1), body).await.0, 401); // another thread's key
    assert_eq!(post(m.port, &path_of(&m.url(T1, "")), "{not json").await.1, json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error"}}).to_string());
}
#[tokio::test(flavor = "multi_thread")] async fn a_stale_process_may_read_but_not_change() {
    let sd = dir("m3-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("m3", |w| base_log(w, 2, ""), |_| Cli::standin(&sd)).await; ag.set_gen(T1, "g2"); let n = len(&hub).await;
    let (_, r) = rpc_mcp(&ag, T1, "g1", "tools/call", json!({"name":"thread_list","arguments":{}})).await; assert_eq!(r["result"]["isError"], false);
    let (_, r) = rpc_mcp(&ag, T1, "g1", "tools/call", json!({"name":"thread_send","arguments":{"threadId":"t1700001001-1","text":"x"}})).await;
    assert_eq!(r["result"]["isError"], true); assert!(tool_text(&r)["error"].as_str().unwrap().starts_with("stale process")); assert_eq!(len(&hub).await, n);
}
#[tokio::test(flavor = "multi_thread")] async fn thread_tools_read_send_wait_update_and_interrupt() {
    let sd = dir("m4-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("m4", |w| base_log(w, 2, ""), |_| Cli::standin(&sd)).await; let other = "t1700001001-1";
    let call = |name: &'static str, a: Value| { let ag = ag.clone(); async move { let (_, r) = rpc_mcp(&ag, T1, "", "tools/call", json!({"name":name,"arguments":a})).await; (r["result"]["isError"] == false, tool_text(&r)) } };
    assert_eq!(call("thread_send", json!({"threadId":other,"text":"  "})).await, (false, json!({"error":"thread_send needs a non-empty `text` (the message); nothing was sent"})));
    assert_eq!(call("thread_send", json!({"threadId":"nope","text":"hi"})).await, (false, json!({"error":"that thread is not in this project"})));
    assert_eq!(call("thread_send", json!({"threadId":other,"message":"zqshort hello"})).await, (true, json!({"threadId":other,"state":"sent"})));
    let r = call("thread_wait", json!({"threadId":other,"timeoutMs":60000})).await.1; assert_eq!(r["done"], true);
    let ev = log_from(&hub, 0).await; let m = ev.iter().find(|e| is(e, "MessagePosted") && e["thread"] == other && e["role"] == "user").unwrap(); assert_eq!(m["msg"].as_str().unwrap().split(':').take(2).collect::<Vec<_>>(), ["t", T1]);
    assert!(ev.iter().any(|e| is(e, "MessageFrom") && e["thread"] == other && e["from"] == format!("thread:{T1}") && e["kind"] == "thread"));
    let read = call("thread_read", json!({"threadId":other,"limit":3})).await.1; assert_eq!(read["entries"].as_array().unwrap().len(), 3); assert_eq!(read["status"]["id"], other);
    assert!(read["entries"].as_array().unwrap().iter().all(|e| e["kind"] == "message" || e["kind"] == "activity"));
    assert_eq!(call("thread_update", json!({"threadId":other,"title":"Renamed"})).await, (true, json!({"threadId":other,"title":"Renamed"})));
    assert_eq!(call("thread_interrupt", json!({"threadId":other})).await, (true, json!({"threadId":other,"state":"not running"})));
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn an_approval_waits_for_the_person_and_a_session_allowance_answers_the_next() {
    let sd = dir("p-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("p", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    let m = ag.mcp.get().unwrap().clone(); let path = path_of(&m.url(T1, ""));
    let body = |tool: &str, input: Value| json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"permission_prompt","arguments":{"tool_name":tool,"input":input}}}).to_string();
    let first = { let (p, b) = (path.clone(), body("Bash", json!({"command":"rm -rf build"}))); tokio::spawn(async move { post(m.port, &p, &b).await }) };
    let ev = until(&hub, from, 5, |ev| ev.iter().any(|e| is(e, "AskOpened"))).await; let ask = ev.iter().find(|e| is(e, "AskOpened")).unwrap().clone();
    assert_eq!((ask["kind"].as_str().unwrap(), ask["tool"].as_str().unwrap(), ask["detail"].as_str().unwrap()), ("approval", "Bash", "rm -rf build")); assert!(!first.is_finished(), "the reply waits");
    let id = ask["id"].as_str().unwrap();
    let bad = rpc(&hub, "ask.answer", json!({"id":id,"answer":"allow"})).await; assert_eq!(bad["ok"], false, "an unknown word is an error, not a decline: {bad}"); assert!(bad["error"].as_str().unwrap().contains("accept"), "{bad}");
    assert!(!log_from(&hub, from).await.iter().any(|e| is(e, "AskClosed")), "the ask stays open"); assert!(!first.is_finished());
    assert_eq!(rpc(&hub, "ask.answer", json!({"id":id,"answer":"session"})).await["ok"], true);
    let (st, b) = first.await.unwrap(); assert_eq!(st, 200); let v: Value = serde_json::from_str(&b).unwrap(); assert_eq!(v["id"], 7);
    assert_eq!(serde_json::from_str::<Value>(v["result"]["content"][0]["text"].as_str().unwrap()).unwrap(), json!({"behavior":"allow","updatedInput":{"command":"rm -rf build"}}));
    let ev = log_from(&hub, from).await; assert!(ev.iter().any(|e| is(e, "AskClosed") && e["id"] == id && e["answer"] == "session"));
    assert!(ev.iter().any(|e| is(e, "ActivityLogged") && e["kind"] == "answer" && e["summary"] == "Allowed for this session: Bash rm -rf build"));
    // ask_closes_once: answering again changes nothing
    let n = len(&hub).await; assert_eq!(rpc(&hub, "ask.answer", json!({"id":id,"answer":"decline"})).await["ok"], true); assert_eq!(len(&hub).await, n);
    // the same tool in the same thread is allowed at once now
    let (_, b2) = post(ag.mcp.get().unwrap().port, &path, &body("Bash", json!({"command":"ls"}))).await; assert!(b2.contains("allow")); assert_eq!(log_from(&hub, n).await.iter().filter(|e| is(e, "AskOpened")).count(), 0);
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_question_is_answered_into_the_agents_input_and_a_plan_is_held_for_the_person() {
    let sd = dir("q-bin2"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("q2", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    let m = ag.mcp.get().unwrap().clone(); let path = path_of(&m.url(T1, ""));
    let q = json!({"questions":[{"question":"Which port?","options":[{"label":"USB-C","description":"reversible"},{"label":"Micro"}]}]});
    let body = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"permission_prompt","arguments":{"tool_name":"AskUserQuestion","input":q}}}).to_string();
    let h = { let p = path.clone(); tokio::spawn(async move { post(m.port, &p, &body).await }) };
    let ev = until(&hub, from, 5, |ev| ev.iter().any(|e| is(e, "AskOpened"))).await; let ask = ev.iter().find(|e| is(e, "AskOpened")).unwrap();
    assert_eq!((ask["kind"].as_str().unwrap(), ask["detail"].as_str().unwrap(), ask["options"].as_str().unwrap()), ("input", "Which port?\nUSB-C: reversible", "USB-C\nMicro"));
    rpc(&hub, "ask.answer", json!({"id":ask["id"],"answer":"USB-C"})).await;
    let v: Value = serde_json::from_str(&h.await.unwrap().1).unwrap(); assert_eq!(serde_json::from_str::<Value>(v["result"]["content"][0]["text"].as_str().unwrap()).unwrap()["updatedInput"]["answers"]["Which port?"], "USB-C");
    assert!(log_from(&hub, from).await.iter().any(|e| is(e, "ActivityLogged") && e["summary"] == "Which port? → USB-C"));
    // ExitPlanMode: denied at once with the note, the plan is an ask the person answers
    let plan = json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"permission_prompt","arguments":{"tool_name":"ExitPlanMode","input":{"plan":"1. do it"}}}}).to_string();
    let (_, b) = post(ag.mcp.get().unwrap().port, &path, &plan).await; assert!(b.contains("Stop here and wait for the user"));
    assert!(log_from(&hub, from).await.iter().any(|e| is(e, "AskOpened") && e["kind"] == "plan" && e["detail"] == "1. do it"));
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_restart_closes_open_asks_and_says_which_went_unanswered() {
    let sd = dir("ra-bin"); perf_standin(&sd, 1);
    let ask = format!("{{\"$\":\"AskOpened\",\"id\":\"q1\",\"thread\":\"{T1}\",\"kind\":\"approval\",\"tool\":\"Bash\",\"detail\":\"make all\",\"options\":\"\",\"at\":1700000101}}\n");
    let (hub, ag, _h, _w) = start("ra", |w| running_log(w, &ask).replace("TurnChanged", "TurnChangedX"), |_| Cli::standin(&sd)).await; // (the turn is not running: only the ask)
    let ev = log_from(&hub, 0).await; assert!(ev.iter().any(|e| is(e, "AskClosed") && e["id"] == "q1" && e["answer"] == ""));
    let note = ev.iter().find(|e| is(e, "ActivityLogged") && e["kind"] == "restart").unwrap(); assert!(note["summary"].as_str().unwrap().ends_with("Its request to use Bash was not answered: \u{201C}make all\u{201D}"));
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn the_agent_is_started_with_the_arguments_and_prompt_the_js_hub_gives_it() {
    use crate::plat::fs::PermissionsExt;
    let sd = dir("a-bin"); let argfile = sd.join("argv.json");
    // a stand-in that writes its own argv as JSON, then behaves as a Claude that answers once
    std::fs::write(sd.join("claude"), format!("#!/bin/sh\nprintf '%s\\0' \"$@\" > {a}.tmp\nmv {a}.tmp {a}\nread l\nsleep 600\n", a = crate::plat::fs::script_path(&argfile))).unwrap();
    std::fs::set_permissions(sd.join("claude"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let (hub, ag, home, work) = {
        let home = dir("a-home"); let work = dir("a-work"); std::fs::write(home.join("events.jsonl"), base_log(&work, 1, "")).unwrap();
        let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let host = host::HostInfo { home: home.clone(), fc: "/home/h/.local/bin/freecadcmd".into(), render: "/home/h/Documents/i2cjak/backplane-gpui/build/gpui/backplane-serve-current".into(), kicad: prompt::Kicad { status: "stock".into(), cli: "/home/h/.local/bin/kicad-cli".into(), version: "10.0.7".into(), ..Default::default() },
            models: json!({"claude":["fable","opus","sonnet","haiku"],"codex":["gpt-6.1-sol","gpt-6-astra","gpt-6.1-terra","gpt-6-sol","gpt-6-luna","gpt-5.6-sol","gpt-5.6-terra","gpt-5.6-luna"]}), ..Default::default() };
        let ag = Agents::start_host(hub.clone(), Cli::standin(&sd), host).await.unwrap(); (hub, ag, home, work)
    };
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"hello"})).await;
    for _ in 0..100 { if argfile.exists() { break } tokio::time::sleep(Duration::from_millis(50)).await }
    let got: Vec<String> = { let raw = std::fs::read_to_string(&argfile).unwrap(); let mut v: Vec<String> = raw.split('\0').map(str::to_string).collect(); v.pop(); v };
    let want: Vec<String> = serde_json::from_str(include_str!("../../../../test/agents/fixtures/argv-default.json")).unwrap();
    assert_eq!(got.len(), want.len());
    // the FreeCAD the hub tells about is the one it finds on this machine (PATH first): told as the fixture's, wherever it is installed here
    let told = crate::files::kicad::stepglb::told_freecad(&home, "/home/h/.local/bin/freecadcmd");
    let fc_dir = told.rsplit_once('/').map(|d| d.0.to_string()).unwrap_or_default();
    let norm = |v: &[String]| v.iter().map(|x| { let x = if fc_dir.is_empty() || fc_dir == "/home/h/.local/bin" { x.clone() } else { x.replace(&format!("{fc_dir}/freecad"), "/home/h/.local/bin/freecad") };
        let x = x.replace(&home.join("attachments").to_string_lossy().to_string(), "<home>/attachments").replace("/home/h/Documents/i2cjak/backplane-gpui/build/gpui/results/agents/argv2/work/home/attachments", "<home>/attachments");
        let x = x.replace(&work.to_string_lossy().to_string(), "<root>").replace("/home/h/Documents/i2cjak/backplane-gpui/build/gpui/results/agents/argv2/work/fixroot", "<root>");
        if x.contains("/mcp/") { let (a, b) = x.split_once("/mcp/").unwrap(); let _ = a; format!("mcp:{}", b.split('?').next().unwrap().split('.').next().unwrap()) } else { x } }).collect::<Vec<_>>();
    // arguments, prompt and the MCP config (its port, generation and key differ by run)
    let (g, w) = (norm(&got), norm(&want));
    for (i, (a, b)) in g.iter().zip(w.iter()).enumerate() { if a.starts_with('{') && a.contains("mcpServers") { assert!(a.contains("\"pcbparts\":{\"type\":\"http\",\"url\":\"https://pcbparts.dev/mcp\"}") && b.contains("pcbparts")); continue }
        assert_eq!(a, b, "argument {i}") }
    assert!(got.last().unwrap().starts_with("{\"mcpServers\":{\"backplane\":{\"type\":\"http\",\"url\":\"http://127.0.0.1:")); ag.shutdown();
}

// ---- replays of recorded stand-in output: the command ledger, background work, subagents, todos ----
const PLAYER: &str = r#"#!/bin/sh
dir=$(dirname "$0"); rf="$dir/replay.ndjson"; n=0; U1=; U2=
need() { while [ $n -lt $1 ]; do IFS= read -r l || exit 0; case "$l" in *'"type":"user"'*) n=$((n+1)); u=$(printf '%s' "$l" | sed -n 's/.*"uuid":"\([^"]*\)".*/\1/p'); eval "U$n=\$u";; esac; done; }
need 1
while IFS= read -r line <&3; do
  case "$line" in *__U2__*) need 2;; esac
  printf '%s\n' "$line" | sed -e "s/__U1__/$U1/g" -e "s/__U2__/$U2/g"
  sleep 0.02
done 3< "$rf"
exec cat >/dev/null
"#;
fn norm_digits(s: &str) -> String {
    // uuids first, then any run of 3+ digits: ids and clocks only
    let b: Vec<char> = s.chars().collect(); let mut o = String::new(); let mut i = 0;
    let hex = |c: char| c.is_ascii_hexdigit();
    while i < b.len() {
        if i + 36 <= b.len() && [8, 13, 18, 23].iter().all(|k| b[i + k] == '-') && (0..36).all(|k| [8, 13, 18, 23].contains(&k) || hex(b[i + k])) { o.push('U'); i += 36; continue }
        if b[i].is_ascii_digit() { let mut j = i; while j < b.len() && b[j].is_ascii_digit() { j += 1 } if j - i >= 3 { o.push('N') } else { o.extend(&b[i..j]) } i = j; continue }
        o.push(b[i]); i += 1;
    }
    o.replace("N-0", "N-N").split("N-").enumerate().map(|(i, p)| if i == 0 { p.to_string() } else { let d = p.chars().take_while(|c| c.is_ascii_digit()).count(); format!("N-N{}", &p[d..]) }).collect::<String>().replace("N-NN", "N-N")
}
async fn replay(name: &str, messages: &[&str], gap_ms: u64) -> (Vec<String>, Vec<String>) {
    let sd = dir(&format!("rp-{name}-bin")); script(&sd, "claude", PLAYER);
    std::fs::write(sd.join("replay.ndjson"), match name { "lifecycle" => include_str!("../../../../test/agents/fixtures/replay-lifecycle.ndjson"), "todos" => include_str!("../../../../test/agents/fixtures/replay-todos.ndjson"),
        "agents" => include_str!("../../../../test/agents/fixtures/replay-agents.ndjson"), _ => include_str!("../../../../test/agents/fixtures/replay-pair.ndjson") }).unwrap();
    let (hub, ag, _h, _w) = start(&format!("rp-{name}"), |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    // the next command goes in once the previous one's session is logged (the log's order, not a time: gap_ms only says the commands are sent one after the other)
    for (i, m) in messages.iter().enumerate() { assert_eq!(rpc(&hub, "turn.start", json!({"thread":T1,"text":m})).await["ok"], true);
        if gap_ms > 0 && i + 1 < messages.len() { until(&hub, from, 30, |ev| ev.iter().filter(|e| is(e, "SessionSet")).count() > i).await; } }
    let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| turn_is(e, "completed") || turn_is(e, "failed"))).await; tokio::time::sleep(Duration::from_millis(1500)).await;
    let mut ev = log_from(&hub, from).await; if ev.len() < 2 { ev = ev.clone() } ag.shutdown();
    let want_raw = match name { "lifecycle" => include_str!("../../../../test/agents/fixtures/js-lifecycle.jsonl"), "todos" => include_str!("../../../../test/agents/fixtures/js-todos.jsonl"), "agents" => include_str!("../../../../test/agents/fixtures/js-agents.jsonl"), _ => include_str!("../../../../test/agents/fixtures/js-pair.jsonl") };
    let f = |v: &[Value]| core(v, T1).iter().map(|e| norm_digits(&norm(e))).collect::<Vec<_>>();
    (f(&ev), f(&lines_of(want_raw)))
}
fn assert_same(got: &[String], want: &[String]) {
    for (i, (a, b)) in got.iter().zip(want.iter()).enumerate() { assert_eq!(a, b, "event {i} of {} (rust) / {} (js)", got.len(), want.len()) }
    assert_eq!(got.len(), want.len(), "counts; extra rust: {:?}; extra js: {:?}", got.iter().skip(want.len()).collect::<Vec<_>>(), want.iter().skip(got.len()).collect::<Vec<_>>());
}
#[tokio::test(flavor = "multi_thread")] async fn a_lifecycle_turn_matches_the_js_hub_event_for_event() { let (g, w) = replay("lifecycle", &["hello"], 0).await; assert_same(&g, &w) }
#[tokio::test(flavor = "multi_thread")] async fn tool_calls_and_todos_match_the_js_hub() { let (g, w) = replay("todos", &["todos:4/2 tools:2 go"], 0).await; assert_same(&g, &w) }
#[tokio::test(flavor = "multi_thread")] async fn background_subagents_match_the_js_hub() { let (g, w) = replay("agents", &["agents:2 agentsteps:2 go"], 0).await; assert_same(&g, &w) }
#[tokio::test(flavor = "multi_thread")] async fn two_commands_are_ended_one_by_its_own_lifecycle() { let (g, w) = replay("pair", &["first", "second"], 300).await; assert_same(&g, &w) }

fn norm_steps(s: &str, roots: &[&str]) -> String {
    let mut s = s.to_string(); for r in roots { s = s.replace(r, "<root>") }
    // 40 hex digits: a commit
    let b: Vec<char> = s.chars().collect(); let mut o = String::new(); let mut i = 0;
    while i < b.len() { if i + 40 <= b.len() && b[i..i + 40].iter().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) && b[i..i + 40].iter().any(|c| c.is_ascii_alphabetic()) && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) { o.push('H'); i += 40 } else { o.push(b[i]); i += 1 } }
    o
}
#[tokio::test(flavor = "multi_thread")] async fn design_history_steps_are_logged_as_the_js_hub_logs_them() {
    for (k, v) in [("GIT_CONFIG_GLOBAL", "/dev/null"), ("GIT_CONFIG_NOSYSTEM", "1"), ("GIT_AUTHOR_NAME", "t"), ("GIT_AUTHOR_EMAIL", "t@t"), ("GIT_COMMITTER_NAME", "t"), ("GIT_COMMITTER_EMAIL", "t@t")] { std::env::set_var(k, v) } // hermetic: no host git identity, config or hooks
    let sd = dir("st-bin"); script(&sd, "claude", PLAYER);
    std::fs::write(sd.join("replay.ndjson"), include_str!("../../../../test/agents/fixtures/replay-steps.ndjson")).unwrap();
    let home = dir("st-home"); let work = dir("st-work"); std::fs::write(home.join("events.jsonl"), base_log(&work, 1, "")).unwrap();
    std::fs::write(work.join("b.kicad_pcb"), "(kicad_pcb (version 20240108))\n").unwrap();
    let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let host = host::HostInfo { home: home.clone(), ..Default::default() };
    let ag = Agents::start_host(hub.clone(), Cli::standin(&sd), host).await.unwrap(); let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"hello"})).await;
    until(&hub, from, 30, |ev| ev.iter().any(|e| turn_is(e, "completed")) && ev.iter().any(|e| e["tone"] == "step")).await;
    let end = Instant::now() + Duration::from_secs(20); while ag.stats.jobs.load(Relaxed) > 0 && Instant::now() < end { tokio::time::sleep(Duration::from_millis(20)).await } // the first turn's end capture must be over before the file changes, or it records the change as its own
    std::fs::write(work.join("b.kicad_pcb"), "(kicad_pcb (version 20240108))\n(footprint \"R1\")\n").unwrap();
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"again"})).await;
    let ev = until(&hub, from, 30, |ev| ev.iter().filter(|e| turn_is(e, "completed")).count() == 2 && ev.iter().filter(|e| e["tone"] == "step").count() == 2).await; tokio::time::sleep(Duration::from_millis(800)).await;
    let ev = { let e2 = log_from(&hub, from).await; if e2.len() > ev.len() { e2 } else { ev } };
    let js_root = "/home/h/Documents/i2cjak/bp-agents-scratch/modes/work-steps/fixroot"; let me = work.to_string_lossy().to_string();
    let f = |v: &[Value], r: &str| core(v, T1).iter().map(|e| norm_digits(&norm_steps(&norm(e), &[r]))).collect::<Vec<_>>();
    // the step Acts carry the commit, the previous commit, the git dir and top, and the files changed
    let steps: Vec<Value> = ev.iter().filter(|e| e["tone"] == "step").cloned().collect(); assert_eq!(steps.len(), 2, "{steps:?}");
    let rec = |i: usize| -> Value { let sm = steps[i]["summary"].as_str().unwrap_or(""); serde_json::from_str(sm).unwrap_or_else(|e| panic!("step {i}: the capture did not record a JSON step ({e}); its summary was: {sm:?}")) };
    assert_eq!(rec(0)["w"], "start"); assert_eq!(rec(0)["f"], json!(["A\tb.kicad_pcb"])); assert_eq!(rec(0)["p"], ""); assert_eq!(rec(1)["p"], rec(0)["c"]); assert_eq!(rec(1)["f"], json!(["M\tb.kicad_pcb"]));
    assert_eq!(rec(0)["g"], format!("{me}/.backplane/history.git")); assert_eq!(rec(0)["t"], me);
    let (g, w) = (f(&ev, &me), f(&lines_of(include_str!("../../../../test/agents/fixtures/js-steps.jsonl")), js_root));
    // the Acts land when the background capture ends, so their place among the turn's events varies: compare the rest in order, and the Acts as a set
    let (gs, ws): (Vec<_>, Vec<_>) = (g.iter().filter(|x| !x.contains("\"tone\":\"step\"")).collect::<Vec<_>>(), w.iter().filter(|x| !x.contains("\"tone\":\"step\"")).collect::<Vec<_>>());
    assert_eq!(gs, ws); let (mut a, mut b): (Vec<_>, Vec<_>) = (g.iter().filter(|x| x.contains("\"tone\":\"step\"")).collect(), w.iter().filter(|x| x.contains("\"tone\":\"step\"")).collect()); a.sort(); b.sort(); assert_eq!(a, b);
    ag.shutdown();
}

// ---- laws that are about what the hub does with a message and a restart ----
/// The log after messages sent to an idle thread, into a running turn, and into one that is streaming, by the person and by another thread.
/// A user message with this text is in the log and the turn is running again right after it (so what follows it has landed).
fn posted_then_running(ev: &[Value], text: &str) -> bool { ev.iter().position(|e| is(e, "MessagePosted") && e["text"] == text).map_or(false, |i| ev.get(i + 1).map_or(false, |n| turn_is(n, "running"))) }
async fn deliveries() -> Vec<Value> {
    let sd = dir("dl-bin"); perf_standin(&sd, 5);
    let (hub, ag, _h, _w) = start("dl", |w| base_log(w, 2, ""), |_| Cli::standin(&sd)).await; let other = "t1700001001-1"; let from = len(&hub).await;
    // each step waits for its own events to land (not for a clock): a loaded runner only makes it slower
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort one"})).await;                                                                          // idle -> starts
    until(&hub, from, 30, |ev| posted_then_running(ev, "zqshort one")).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"two"})).await;                                                                                  // into the running turn
    until(&hub, from, 30, |ev| posted_then_running(ev, "two")).await;
    let (_, r) = rpc_mcp(&ag, other, "", "tools/call", json!({"name":"thread_send","arguments":{"threadId":T1,"text":"three"}})).await; assert_eq!(r["result"]["isError"], false); // another thread's message into it
    // "three" arrives via another thread, so it is followed by MessageFrom, the running turn and the turn.from mark (not directly by running): wait for those four
    let ev = until(&hub, from, 30, |ev| ev.iter().position(|e| is(e, "MessagePosted") && e["text"] == "three").map_or(false, |i| ev.len() > i + 3)).await; ag.shutdown(); ev
}
/// Wait until a file exists (a stand-in writes its pid when it starts): a loaded runner makes it slower, not wrong.
async fn wait_file(p: &Path) { let end = Instant::now() + Duration::from_secs(30); while !p.exists() && Instant::now() < end { tokio::time::sleep(Duration::from_millis(20)).await } }
fn held(ev: &[Value]) -> bool { ev.iter().any(|e| is(e, "TurnQueued") && !e["msg"].as_str().unwrap_or("").starts_with("handoff:")) }
#[tokio::test(flavor = "multi_thread")] async fn deliver_never_waits() { assert!(!held(&deliveries().await)) }
#[tokio::test(flavor = "multi_thread")] async fn deliver_held_go() { let ev = deliveries().await; assert!(!held(&ev) && ev.iter().filter(|e| turn_is(e, "running")).count() >= 1) }
#[tokio::test(flavor = "multi_thread")] async fn deliver_held_steer() { let ev = deliveries().await; let users: Vec<&str> = ev.iter().filter(|e| is(e, "MessagePosted") && e["role"] == "user").map(|e| e["text"].as_str().unwrap_or("")).collect(); assert!(!held(&ev), "nothing is held"); assert_eq!(users, vec!["zqshort one", "two", "three"], "the three messages, each posted once, in order") }
#[tokio::test(flavor = "multi_thread")] async fn deliver_held_steer_k() { assert!(!held(&deliveries().await)) }
#[tokio::test(flavor = "multi_thread")] async fn deliver_held_restart() {
    let sd = dir("dr-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("dr", |w| running_log(w, ""), |_| Cli::standin(&sd)).await; tokio::time::sleep(Duration::from_millis(500)).await; assert!(!held(&log_from(&hub, 0).await)); ag.shutdown()
}
#[tokio::test(flavor = "multi_thread")] async fn deliver_claude_running() {
    let ev = deliveries().await; let i = ev.iter().position(|e| is(e, "MessagePosted") && e["text"] == "two").unwrap();
    assert_eq!((ev[i + 1]["$"].as_str().unwrap(), ev[i + 1]["state"].as_str().unwrap()), ("TurnChanged", "running")); assert!(!ev.iter().any(|e| is(e, "QueueTaken")))
}
/// post_running_delivers / inbox_running_delivers: a message into a running turn lands the same whoever sent it: posted, the turn running again, the same marks
#[tokio::test(flavor = "multi_thread")] async fn post_running_delivers() { let ev = deliveries().await; let i = ev.iter().position(|e| is(e, "MessagePosted") && e["text"] == "two").unwrap(); assert!(is(&ev[i + 1], "TurnChanged") && ev[i + 2]["key"].as_str().unwrap().starts_with("turn.from.")) }
#[tokio::test(flavor = "multi_thread")] async fn inbox_running_delivers() {
    let ev = deliveries().await; let i = ev.iter().position(|e| is(e, "MessagePosted") && e["text"] == "three").unwrap();
    assert!(is(&ev[i + 1], "MessageFrom") && turn_is(&ev[i + 2], "running") && ev[i + 3]["key"].as_str().unwrap().starts_with("turn.from."))
}
async fn recovered(extra: &str, state: &str) -> Vec<Value> {
    let sd = dir(&format!("rc-{state}-bin")); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start(&format!("rc-{state}"), |w| { let l = running_log(w, extra); if state == "idle" { l.replace("\"state\":\"running\"", "\"state\":\"completed\"") } else { l } }, |_| Cli::standin(&sd)).await;
    tokio::time::sleep(Duration::from_millis(400)).await; let ev = log_from(&hub, 0).await; ag.shutdown(); ev
}
#[tokio::test(flavor = "multi_thread")] async fn recover_interrupts_running() { let ev = recovered("{\"$\":\"SettingSet\",\"key\":\"restart.continue\",\"value\":\"off\",\"at\":1700000101}\n", "run").await; let last = ev.iter().rev().find(|e| is(e, "TurnChanged")).unwrap(); assert_eq!(last["state"], "interrupted") }
#[tokio::test(flavor = "multi_thread")] async fn recover_closes_asks() { let ask = format!("{{\"$\":\"AskOpened\",\"id\":\"q9\",\"thread\":\"{T1}\",\"kind\":\"approval\",\"tool\":\"Bash\",\"detail\":\"ls\",\"options\":\"\",\"at\":1700000101}}\n"); let ev = recovered(&ask, "run").await; assert!(ev.iter().any(|e| is(e, "AskClosed") && e["id"] == "q9")) }
#[tokio::test(flavor = "multi_thread")] async fn recover_keeps_order_ask() { let ask = format!("{{\"$\":\"AskOpened\",\"id\":\"q8\",\"thread\":\"{T1}\",\"kind\":\"input\",\"tool\":\"order_ask.typed\",\"detail\":\"Ship to which country?\",\"options\":\"\",\"at\":1700000101}}\n"); let ev = recovered(&ask, "idle").await; assert!(!ev.iter().any(|e| is(e, "AskClosed"))) }
#[tokio::test(flavor = "multi_thread")] async fn recover_leaves_idle() { let ev = recovered("", "idle").await; assert!(!ev.iter().any(|e| turn_is(e, "interrupted")) && !ev.iter().any(|e| e["kind"] == "restart")) }
#[tokio::test(flavor = "multi_thread")] async fn recover_empty_nothing() { let sd = dir("re-bin"); perf_standin(&sd, 1); let (hub, ag, _h, _w) = start("re", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await; let n = len(&hub).await; tokio::time::sleep(Duration::from_millis(300)).await; assert_eq!(len(&hub).await, n); ag.shutdown() }

// ---- Codex: one process per turn, Codex-shaped lines (codex.rs) ----
/// A `codex` stand-in: reads the turn's text from its last argument; "slow" turns wait; every turn says thread.started, a delta, a command and a reply that echoes the text.
const CODEX_STANDIN: &str = include_str!("../../../../test/agents/standin-codex.sh");
fn codex_log(work: &Path) -> String { base_log(work, 1, "").replace("\"provider\":\"claude\"", "\"provider\":\"codex\"") }
async fn codex_start(name: &str) -> (Arc<Hub>, Arc<Agents>, PathBuf) {
    let sd = dir(&format!("{name}-bin")); script(&sd, "codex", CODEX_STANDIN);
    let (hub, ag, _h, _w) = start(name, |w| codex_log(w), |_| Cli::standin(&sd)).await; (hub, ag, sd)
}
#[tokio::test(flavor = "multi_thread")] async fn a_codex_turn_reads_in_codex_shapes() {
    let (hub, ag, _sd) = codex_start("cx1").await; let from = len(&hub).await;
    assert_eq!(rpc(&hub, "turn.start", json!({"thread":T1,"text":"hello"})).await["ok"], true);
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed")) && ev.iter().any(|e| is(e, "TurnUsage"))).await;
    assert!(ev.iter().any(|e| is(e, "SessionSet") && e["session"] == "cs-1"), "{ev:?}");
    assert!(ev.iter().any(|e| is(e, "ActivityLogged") && e["kind"] == "shell" && e["summary"] == "echo ok"));
    assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "assistant" && e["text"].as_str().unwrap().starts_with("re: ")));
    let u = ev.iter().find(|e| is(e, "TurnUsage")).unwrap(); assert_eq!((u["input"].as_u64(), u["cread"].as_u64(), u["output"].as_u64()), (Some(60), Some(40), Some(10)), "a session the hub saw start counts its first turn from 0 (Turn.codex.start)");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(ag.stats.procs_spawned.load(Relaxed), 1); assert_eq!(ag.stats.procs_exited.load(Relaxed), 1, "the process was reaped");
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_second_codex_turn_resumes_the_session() {
    let (hub, ag, sd) = codex_start("cx2").await;
    for t in ["one", "two"] { let from = len(&hub).await; rpc(&hub, "turn.start", json!({"thread":T1,"text":t})).await; until(&hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await; tokio::time::sleep(Duration::from_millis(300)).await }
    let argv = std::fs::read_to_string(sd.join("argv.log")).unwrap_or_default(); let ls: Vec<&str> = argv.lines().filter(|l| l.starts_with("exec ")).collect();
    assert!(ls[0].starts_with("exec --json --skip-git-repo-check"), "{argv}"); assert!(ls[1].starts_with("exec resume cs-1 --json"), "{argv}");
    let ev = log_from(&hub, 0).await; let us: Vec<&Value> = ev.iter().filter(|e| is(e, "TurnUsage")).collect();
    assert_eq!(us.len(), 1, "the same totals are one turn's usage"); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_person_message_to_a_running_codex_turn_steers_it() {
    let (hub, ag, _sd) = codex_start("cx3").await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"slow first"})).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"second while running"})).await;
    let ev0 = log_from(&hub, from).await;
    assert!(ev0.iter().any(|e| is(e, "MessagePosted") && e["role"] == "user" && e["text"] == "second while running"), "posted at once");
    assert!(!ev0.iter().any(|e| is(e, "SettingSet") && e["key"] == format!("ride.{T1}") && !e["value"].as_str().unwrap_or("").is_empty()), "a person's message never waits as a ride");
    assert!(ev0.iter().any(|e| is(e, "ActivityLogged") && e["kind"] == "steer"), "the running turn was steered");
    let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "assistant" && e["text"].as_str().map_or(false, |t| t.contains("second while running")))).await;
    assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "assistant" && e["text"].as_str().unwrap().contains("second while running")), "the turn started again with the message");
    assert_eq!(ag.stats.procs_spawned.load(Relaxed), 2); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn the_ride_is_taken_by_the_running_turns_tool_result_once() {
    let (hub, ag, _sd) = codex_start("cx4").await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"hang forever"})).await; tokio::time::sleep(Duration::from_millis(400)).await;
    ag.deliver(T1, "mail-1", "for the tool result", None, vec![]).await.unwrap(); // mail from a bot or the hub (not a person's message) rides
    let b = ag.ride_take(T1).await.expect("a ride waits"); assert!(b.contains("for the tool result") && b.starts_with("[Backplane] Incoming messages"));
    assert!(ag.ride_take(T1).await.is_none(), "shown once");
    assert!(log_from(&hub, 0).await.iter().any(|e| is(e, "ActivityLogged") && e["summary"] == "Delivered into the running turn: 1 waiting message(s)"));
    rpc(&hub, "turn.interrupt", json!({"thread":T1})).await; ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn interrupting_a_codex_turn_ends_its_process_and_keeps_the_ride() {
    let (hub, ag, sd) = codex_start("cx5").await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"hang"})).await; wait_file(&sd.join("pid")).await;
    let pid: u32 = std::fs::read_to_string(sd.join("pid")).unwrap().trim().parse().unwrap(); assert!(proc::alive(pid));
    rpc(&hub, "turn.interrupt", json!({"thread":T1})).await;
    let ev = until(&hub, from, 10, |ev| ev.iter().any(|e| turn_is(e, "interrupted"))).await; assert!(ev.iter().any(|e| turn_is(e, "interrupted")));
    let end = Instant::now() + Duration::from_secs(8); while proc::alive(pid) && Instant::now() < end { tokio::time::sleep(Duration::from_millis(100)).await }
    assert!(!proc::alive(pid), "no orphan"); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_codex_process_that_dies_mid_turn_fails_the_turn() {
    let (hub, ag, _sd) = codex_start("cx6").await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"die now"})).await;
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "failed"))).await;
    assert!(ev.iter().any(|e| is(e, "ActivityLogged") && e["kind"] == "exit"), "{ev:?}"); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn ride_interrupted_starts() {
    let (hub, ag, _sd) = codex_start("cx7").await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"hang"})).await; tokio::time::sleep(Duration::from_millis(400)).await;
    ag.deliver(T1, "mail-2", "mail for later", None, vec![]).await.unwrap();
    rpc(&hub, "turn.interrupt", json!({"thread":T1})).await;
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
    assert!(ev.iter().any(|e| turn_is(e, "interrupted")) && ev.iter().any(|e| is(e, "MessagePosted") && e["text"].as_str().map_or(false, |t| t.contains("mail for later") && e["role"] == "assistant")), "a stopped turn with mail waiting starts the next with it");
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn person_message_steers_restart() { // a person's message into a running Codex turn: posted at once, the process stops and the turn starts again with it
    let (hub, ag, _sd) = codex_start("cx8").await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"slow"})).await; tokio::time::sleep(Duration::from_millis(300)).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"more"})).await;
    assert!(log_from(&hub, from).await.iter().any(|e| is(e, "MessagePosted") && e["text"] == "more" && e["role"] == "user"));
    until(&hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await; assert_eq!(ag.stats.procs_spawned.load(Relaxed), 2, "steered: a new process"); ag.shutdown();
}

// ---- the hub is SIGKILLed with agents alive ----
/// Not a test by itself: run as a child by the SIGKILL test. A mini hub: it starts a Codex-like agent whose shell leaves a background `sleep`, prints the pids, and waits to be killed.
#[tokio::test(flavor = "multi_thread")] async fn sigkill_helper() {
    let Ok(d) = std::env::var("BP_SIGKILL_DIR") else { return };
    let d = PathBuf::from(d); proc::set_owner(&d.to_string_lossy());
    let sh = d.join("agent"); script(&d, "agent", "#!/bin/sh\nsleep 300 &\necho $! > \"$(dirname \"$0\")/grandchild\"\necho $$ > \"$(dirname \"$0\")/child\"\nsleep 300\n");
    let p = proc::spawn(&sh, &[], &d, None, &[]).unwrap(); let _keep = p;
    std::fs::write(d.join("ready"), "1").unwrap(); tokio::time::sleep(Duration::from_secs(600)).await;
}
/// The watcher process of `proc::reaper` when the hub is a test binary (it is spawned with this test's name and reads its stdin).
#[test] fn reaper_child() { if std::env::var_os("BP_REAPER_CHILD").is_some() { std::process::exit(proc::reaper_main()) } }
#[tokio::test(flavor = "multi_thread")] async fn a_killed_hub_leaves_no_agent_behind() {
    let d = dir("kill"); let exe = std::env::current_exe().unwrap();
    let mut h = std::process::Command::new(exe).args(["agents::tests::sigkill_helper", "--exact", "--nocapture", "--test-threads=1"]).env("BP_SIGKILL_DIR", &d).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap();
    let end = Instant::now() + Duration::from_secs(20); while !d.join("grandchild").exists() || !d.join("child").exists() { assert!(Instant::now() < end, "helper did not start"); tokio::time::sleep(Duration::from_millis(50)).await }
    tokio::time::sleep(Duration::from_millis(200)).await;
    let rd = |n: &str| -> u32 { std::fs::read_to_string(d.join(n)).unwrap().trim().parse().unwrap() };
    let (child, grand) = (rd("child"), rd("grandchild")); assert!(proc::alive(child) && proc::alive(grand));
    crate::plat::proc::kill(h.id(), crate::plat::proc::KILL); let _ = h.wait();
    let end = Instant::now() + Duration::from_secs(5); while proc::alive(child) && Instant::now() < end { tokio::time::sleep(Duration::from_millis(50)).await }
    assert!(!proc::alive(child), "the agent dies with its hub (PDEATHSIG on Linux, the reaper watcher elsewhere)");
    // Linux: what the agent started outlives it and the next hub on the home ends it. Elsewhere the watcher kills the whole group.
    #[cfg(not(target_os = "linux"))] { let end = Instant::now() + Duration::from_secs(5); while proc::alive(grand) && Instant::now() < end { tokio::time::sleep(Duration::from_millis(50)).await } assert!(!proc::alive(grand), "the watcher ends the agent's group"); return }
    #[cfg(target_os = "linux")] {
    assert!(proc::alive(grand), "(a grandchild survives the kernel's rule: this is why the next hub reaps)");
    let n = proc::reap_orphans(&d.to_string_lossy()); assert!(n >= 1, "reaped {n}");
    let end = Instant::now() + Duration::from_secs(5); while proc::alive(grand) && Instant::now() < end { tokio::time::sleep(Duration::from_millis(50)).await }
    assert!(!proc::alive(grand), "no orphan after the next hub starts");
    }
}

/// Item 5 measurement, never run in CI: a SYNTHETIC log (no real data) of BP_SYNTH_EVENTS events (default 1.7M, 2000 threads) in a temp dir.
/// cargo test ... scan_synthetic_log -- --ignored --nocapture. Times Store::open and the agents' World fold, with peak RSS.
#[test] #[ignore] fn scan_synthetic_log() {
    use std::io::Write;
    let n: usize = std::env::var("BP_SYNTH_EVENTS").ok().and_then(|s| s.parse().ok()).unwrap_or(1_700_000);
    let d = dir("synth"); let p = d.join("events.jsonl");
    { let mut f = std::io::BufWriter::new(std::fs::File::create(&p).unwrap());
      writeln!(f, "{{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"s\",\"root\":\"/tmp\",\"at\":1}}").unwrap();
      for i in 0..2000 { writeln!(f, "{{\"$\":\"ThreadCreated\",\"id\":\"t{i}\",\"project\":\"p1\",\"title\":\"T{i}\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":2}}").unwrap() }
      for i in 0..n { match i % 5 {
          0 => writeln!(f, "{{\"$\":\"MessagePosted\",\"thread\":\"t{}\",\"msg\":\"m{i}\",\"role\":\"user\",\"text\":\"{}\",\"at\":{}}}", i % 2000, "x".repeat(300), 3 + i),
          1 => writeln!(f, "{{\"$\":\"TurnChanged\",\"thread\":\"t{}\",\"state\":\"{}\",\"at\":{}}}", i % 2000, if i % 10 == 1 { "running" } else { "completed" }, 3 + i),
          2 => writeln!(f, "{{\"$\":\"SettingSet\",\"key\":\"tok.basis.t{}\",\"value\":\"c:{i}\",\"at\":{}}}", i % 2000, 3 + i),
          _ => writeln!(f, "{{\"$\":\"ActivityLogged\",\"thread\":\"t{}\",\"act\":\"a{i}\",\"tone\":\"tool\",\"kind\":\"Bash\",\"summary\":\"ls\",\"at\":{}}}", i % 2000, 3 + i) }.unwrap() } }
    let rss = || std::fs::read_to_string("/proc/self/status").unwrap().lines().find(|l| l.starts_with("VmHWM")).map(|l| l.to_string()).unwrap_or_default();
    let t0 = Instant::now(); let store = Store::open(&p).unwrap(); let open = t0.elapsed();
    let t1 = Instant::now(); let mut w = world::World::default(); w.sync(&store).unwrap(); let fold = t1.elapsed();
    println!("SCAN events={} threads={} open_ms={} fold_ms={} {}", store.len(), w.threads.len(), open.as_millis(), fold.as_millis(), rss());
    let _ = std::fs::remove_dir_all(&d);
}

const TESTCLI: &str = "#!/bin/sh\nread hello\necho '{\"q\":1}'\nread ans\necho \"$ans\"\nsleep 30\n";
#[tokio::test(flavor = "multi_thread")] async fn a_drivers_permission_reaches_the_person_and_the_answer_goes_back() {
    let sd = dir("dr2-bin"); script(&sd, "testcli", TESTCLI);
    let (hub, ag, _h, _w) = start("dr2", |w| base_log(w, 1, "").replace("\"provider\":\"claude\"", "\"provider\":\"testprov\""), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"go"})).await;
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| is(e, "AskOpened"))).await;
    let ask = ev.iter().find(|e| is(e, "AskOpened")).unwrap(); assert_eq!((ask["kind"].as_str(), ask["tool"].as_str(), ask["detail"].as_str()), (Some("approval"), Some("Bash"), Some("ls")));
    assert_eq!(rpc(&hub, "ask.answer", json!({"id":ask["id"],"answer":"accept"})).await["ok"], true);
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
    assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["text"] == "answered accept"), "the answer reached the process");
    tokio::time::sleep(Duration::from_millis(500)).await; assert_eq!(ag.stats.procs_exited.load(Relaxed), 1, "the process ends with its turn (ends_on_turn_end)"); ag.shutdown();
}

// ---- seams for the commands lane: agents.stop, agents.start_queued, agents.steer ----
#[tokio::test(flavor = "multi_thread")] async fn steer_codex_restarts() { // law steer_codex_restarts: a running Codex turn stops and starts again with the text
    let (hub, ag, sd) = codex_start("st1").await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"hang first"})).await; wait_file(&sd.join("pid")).await;
    let pid: u32 = std::fs::read_to_string(sd.join("pid")).unwrap().trim().parse().unwrap();
    assert!(ag.steer(T1, "steered text").await.is_ok()); assert!(hub.rpc_for("agents.steer").is_none() && hub.rpc_for("agents.stop").is_none() && hub.rpc_for("agents.start_queued").is_none(), "not client methods: a socket client cannot call them");
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
    assert!(ev.iter().any(|e| is(e, "ActivityLogged") && e["kind"] == "steer"), "{ev:?}");
    assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "assistant" && e["text"].as_str().unwrap().contains("steered text")));
    let end = Instant::now() + Duration::from_secs(8); while proc::alive(pid) && Instant::now() < end { tokio::time::sleep(Duration::from_millis(100)).await }
    assert!(!proc::alive(pid), "the first process ended"); assert_eq!(ag.stats.procs_spawned.load(Relaxed), 2); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn start_queued_runs_the_text_as_one_turn() {
    let (hub, ag, _sd) = codex_start("sq1").await; let from = len(&hub).await;
    assert!(ag.start_queued(T1, "queued words").await.is_ok());
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
    assert!(ev.iter().any(|e| is(e, "QueueTaken")) && ev.iter().any(|e| turn_is(e, "running")));
    assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["text"].as_str().map_or(false, |t| t.contains("queued words"))));
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn agents_stop_ends_an_idle_process_and_writes_nothing() {
    let sd = dir("as-bin"); perf_standin(&sd, 30);
    let (hub, ag, _h, _w) = start("as", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort hi"})).await;
    until(&hub, 0, 30, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await; tokio::time::sleep(Duration::from_millis(300)).await;
    let n = len(&hub).await; assert_eq!(ag.stats.procs_exited.load(Relaxed), 0, "claude's process stays between turns");
    assert!(ag.stop(T1).await.is_ok());
    tokio::time::sleep(Duration::from_millis(800)).await; assert_eq!(len(&hub).await, n, "no events");
    assert_eq!(ag.stats.procs_exited.load(Relaxed), 1, "the idle process was ended and reaped"); ag.shutdown();
}

#[tokio::test(flavor = "multi_thread")] async fn steering_claude_logs_running_and_starts_without_a_live_process() { // Steer.go for Claude
    let sd = dir("sc2-bin"); perf_standin(&sd, 2);
    let (hub, ag, _h, _w) = start("sc2", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    assert!(ag.steer(T1, "zqshort steer").await.is_ok());
    let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
    assert!(ev.iter().any(|e| turn_is(e, "running")), "TurnChanged running is logged"); assert_eq!(ag.stats.procs_spawned.load(Relaxed), 1, "a process was started for it"); ag.shutdown();
}

// ---- delegated tasks ----
#[tokio::test(flavor = "multi_thread")] async fn a_delegated_task_runs_a_child_and_its_result_reaches_the_parent_once() {
    let (hub, ag, _sd) = codex_start("dg1").await; let from = len(&hub).await;
    let (ok, r) = ag.tool_delegate(T1, &json!({"prompt":"do the child work","title":"Child"})).await; assert!(ok, "{r}");
    let (task, child) = (r["taskId"].as_str().unwrap().to_string(), r["childThreadId"].as_str().unwrap().to_string()); assert_eq!(r["depth"], 1);
    let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| is(e, "TaskDelivered"))).await;
    assert!(ev.iter().any(|e| is(e, "ThreadCreated") && e["id"] == child.as_str() && e["title"] == "Child"));
    assert!(ev.iter().any(|e| is(e, "TaskStateSet") && e["state"] == "done"), "{ev:?}");
    let m = ev.iter().find(|e| is(e, "MessagePosted") && e["thread"] == T1 && e["role"] == "system").expect("the result message");
    assert_eq!(m["msg"], format!("t:{child}:td-{task}")); assert!(m["text"].as_str().unwrap().starts_with(&format!("[Backplane] Delegated task {task} (codex) finished:\n\nre: do the child work")), "{}", m["text"]);
    tokio::time::sleep(Duration::from_millis(1500)).await; let all = log_from(&hub, from).await;
    let at = |f: &dyn Fn(&Value) -> bool| all.iter().position(|e| f(e)).unwrap();
    assert!(at(&|e| is(e, "TaskStateSet") && e["state"] == "done") < at(&|e| is(e, "MessagePosted") && e["role"] == "system") && at(&|e| is(e, "MessagePosted") && e["role"] == "system") < at(&|e| is(e, "TaskDelivered")), "task_finish_delivered_last");
    assert_eq!(all.iter().filter(|e| is(e, "TaskDelivered")).count(), 1, "delivered once"); assert_eq!(all.iter().filter(|e| is(e, "MessagePosted") && e["msg"] == format!("t:{child}:td-{task}")).count(), 1);
    let (ok, st) = ag.tool_task_status(T1, &json!({"taskId":task})).await; assert!(ok && st["state"] == "done" && st["childThreadId"] == child.as_str());
    assert!(!ag.tool_task_status("someone-else", &json!({"taskId":task})).await.0, "another thread's task is not its business");
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn delegation_depth_is_limited_and_a_running_task_can_be_cancelled() {
    let (hub, ag, _sd) = codex_start("dg2").await;
    let (ok, r) = ag.tool_delegate(T1, &json!({"prompt":"hang for a long time"})).await; assert!(ok); let (task, child) = (r["taskId"].as_str().unwrap().to_string(), r["childThreadId"].as_str().unwrap().to_string());
    tokio::time::sleep(Duration::from_millis(600)).await;
    let (ok, c) = ag.tool_task_cancel(T1, &json!({"taskId":task})).await; assert!(ok && c["state"] == "cancelled", "{c}");
    let ev = until(&hub, 0, 15, |ev| ev.iter().any(|e| turn_is(e, "interrupted") && e["thread"] == child.as_str())).await; assert!(ev.iter().any(|e| turn_is(e, "interrupted") && e["thread"] == child.as_str()));
    // a chain of three tasks is the limit
    let mut th = child.clone(); let mut last_ok = true;
    for _ in 0..3 { let (ok, r) = ag.tool_delegate(&th, &json!({"prompt":"deeper hang"})).await; last_ok = ok; if ok { th = r["childThreadId"].as_str().unwrap().to_string() } else { assert!(r["error"].as_str().unwrap().contains("depth limit")) } }
    assert!(!last_ok || th != child, "the chain is bounded"); ag.shutdown();
}

#[tokio::test(flavor = "multi_thread")] async fn thread_launch_starts_a_subthread_with_its_first_message_and_reports_can_be_muted() {
    let (hub, ag, _sd) = codex_start("tl1").await; let from = len(&hub).await;
    let (ok, r) = ag.tool_launch(T1, &json!({"prompt":"launched work","title":"Side","provider":"codex"})).await; assert!(ok, "{r}");
    let th = r["threadId"].as_str().unwrap().to_string(); assert_eq!(r["parent"], T1); assert_eq!(r["state"], "started");
    let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| turn_is(e, "completed") && e["thread"] == th.as_str())).await;
    assert!(ev.iter().any(|e| is(e, "ThreadParentSet") && e["id"] == th.as_str() && e["parent"] == T1) && ev.iter().any(|e| is(e, "ThreadLaunched") && e["by"] == T1 && e["report"] == true));
    assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["thread"] == th.as_str() && e["role"] == "user" && e["text"] == "launched work"));
    let (ok, m) = ag.tool_update(T1, &json!({"threadId":th,"report":false})).await; assert!(ok && m["report"] == false, "{m}");
    assert!(log_from(&hub, from).await.iter().any(|e| is(e, "ThreadLaunched") && e["report"] == false));
    assert!(!ag.tool_launch(T1, &json!({"prompt":"x","project":"nope"})).await.0, "an unknown project is refused");
    assert!(!ag.tool_launch(T1, &json!({"prompt":"  "})).await.0, "a prompt is needed");
    assert!(!ag.tool_update(T1, &json!({"threadId":T1,"report":true})).await.0, "nobody launched the root thread");
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_restart_finishes_a_torn_delivery_and_wakes_the_idle_parent() { // task_repair_activates_idle, end to end
    let sd = dir("rp-bin"); script(&sd, "codex", CODEX_STANDIN); let child = "t1700001001-1";
    let extra = format!("{{\"$\":\"TaskDelegated\",\"task\":\"k1\",\"parent\":\"{T1}\",\"child\":\"{child}\",\"depth\":1,\"provider\":\"codex\",\"model\":\"\",\"at\":1700000010}}\n{{\"$\":\"TaskStateSet\",\"task\":\"k1\",\"state\":\"done\",\"summary\":\"done it\",\"at\":1700000011}}\n{{\"$\":\"MessagePosted\",\"thread\":\"{T1}\",\"msg\":\"t:{child}:td-k1\",\"role\":\"system\",\"text\":\"note\",\"at\":1700000012}}\n");
    let (hub, ag, _h, _w) = start("rp", |w| base_log(w, 2, &extra).replace("\"provider\":\"claude\"", "\"provider\":\"codex\""), |_| Cli::standin(&sd)).await; let from = 0;
    let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| is(e, "TaskDelivered"))).await;
    assert!(ev.iter().any(|e| turn_is(e, "running") && e["thread"] == T1), "the parent's turn started with the result"); assert_eq!(ev.iter().filter(|e| is(e, "MessagePosted") && e["msg"] == format!("t:{child}:td-k1")).count(), 1, "the message is not posted twice");
    ag.shutdown();
}

#[tokio::test(flavor = "multi_thread")] async fn cancelling_a_task_does_not_wake_the_parent_live() {
    let (hub, ag, _sd) = codex_start("cn1").await;
    let (_, r) = ag.tool_delegate(T1, &json!({"prompt":"hang here"})).await; let task = r["taskId"].as_str().unwrap().to_string(); tokio::time::sleep(Duration::from_millis(500)).await;
    let n = len(&hub).await; let (ok, _) = ag.tool_task_cancel(T1, &json!({"taskId":task})).await; assert!(ok); tokio::time::sleep(Duration::from_millis(2500)).await;
    let ev = log_from(&hub, n).await; assert!(!ev.iter().any(|e| is(e, "MessagePosted") && e["thread"] == T1 && e["role"] == "system"), "the parent that cancelled hears nothing live: {ev:?}");
    assert!(!ev.iter().any(|e| is(e, "TaskDelivered")), "undelivered until the next start's repair"); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_cut_child_that_will_not_carry_on_ends_its_task_at_recovery() {
    let sd = dir("rc-bin"); script(&sd, "codex", CODEX_STANDIN); let child = "t1700001001-1";
    let extra = format!("{{\"$\":\"SettingSet\",\"key\":\"restart.continue\",\"value\":\"off\",\"at\":1700000009}}\n{{\"$\":\"TaskDelegated\",\"task\":\"k1\",\"parent\":\"{T1}\",\"child\":\"{child}\",\"depth\":1,\"provider\":\"codex\",\"model\":\"\",\"at\":1700000010}}\n{{\"$\":\"TaskStateSet\",\"task\":\"k1\",\"state\":\"running\",\"summary\":\"\",\"at\":1700000011}}\n{{\"$\":\"MessagePosted\",\"thread\":\"{child}\",\"msg\":\"m1\",\"role\":\"user\",\"text\":\"work\",\"at\":1700000012}}\n{{\"$\":\"TurnChanged\",\"thread\":\"{child}\",\"state\":\"running\",\"at\":1700000013}}\n");
    let (hub, ag, _h, _w) = start("rc", |w| base_log(w, 2, &extra).replace("\"provider\":\"claude\"", "\"provider\":\"codex\""), |_| Cli::standin(&sd)).await;
    let ev = until(&hub, 0, 20, |ev| ev.iter().any(|e| is(e, "TaskDelivered"))).await;
    assert!(ev.iter().any(|e| is(e, "TaskStateSet") && e["state"] == "failed"), "{ev:?}"); assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["thread"] == T1 && e["text"].as_str().map_or(false, |t| t.contains("failed")))); ag.shutdown();
}

#[tokio::test(flavor = "multi_thread")] async fn start_one_runs_the_text_as_its_own_turn_without_steer_or_queue_taken() {
    let (hub, ag, _sd) = codex_start("so1").await; let from = len(&hub).await;
    assert!(ag.start_one(T1, "just this one").await.is_ok());
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
    assert!(ev.iter().any(|e| turn_is(e, "running")) && !ev.iter().any(|e| is(e, "QueueTaken")) && !ev.iter().any(|e| e["kind"] == "steer"), "{ev:?}");
    assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "assistant" && e["text"].as_str().unwrap().contains("just this one"))); ag.shutdown();
}

#[tokio::test(flavor = "multi_thread")] async fn review_comment_logs_an_act_and_review_comments_reads_the_open_ones() {
    let (hub, ag, _sd) = codex_start("rvc").await;
    let (ok, r) = ag.tool_review_comment(T1, &json!({"op":"add","text":"check the clearance","file":"b.kicad_pcb","layer":"F.Cu","x":1,"y":2})).await; assert!(ok, "{r}"); let id = r["id"].as_str().unwrap().to_string();
    assert!(!ag.tool_review_comment(T1, &json!({"op":"add","text":" "})).await.0, "an add needs text");
    let (_, c) = ag.tool_review_comments(T1).await; assert_eq!(c["open"], 1); assert!(c["text"].as_str().unwrap().contains("check the clearance"), "{c}");
    assert!(ag.tool_review_comment(T1, &json!({"op":"resolve","id":id})).await.0);
    let (_, c) = ag.tool_review_comments(T1).await; assert_eq!((c["open"].as_u64(), c["resolved"].as_u64()), (Some(0), Some(1))); assert_eq!(c["text"], "No open comments.");
    assert!(log_from(&hub, 0).await.iter().any(|e| is(e, "ActivityLogged") && e["tone"] == "review" && e["kind"] == "add")); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn open_review_comments_follow_the_message_to_the_agent() { // Review.agent
    let (hub, ag, sd) = codex_start("rva").await;
    assert!(ag.tool_review_comment(T1, &json!({"op":"add","text":"move the connector"})).await.0);
    let from = len(&hub).await; rpc(&hub, "turn.start", json!({"thread":T1,"text":"hello there"})).await;
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
    let echoed = ev.iter().find(|e| is(e, "MessagePosted") && e["role"] == "assistant").map(|e| e["text"].as_str().unwrap().to_string()).unwrap_or_default();
    assert!(echoed.contains("move the connector"), "the agent was given the open comment after the message: {echoed}"); assert!(echoed.contains("hello there"));
    let posted = ev.iter().find(|e| is(e, "MessagePosted") && e["role"] == "user").unwrap(); assert_eq!(posted["text"], "hello there", "the log keeps the person's words only"); let _ = sd; ag.shutdown();
}

#[tokio::test(flavor = "multi_thread")] async fn the_summarizer_runs_the_stand_in_text_model_and_cleans_its_answer() { // never a real model: only the stand-in folder's claude runs
    let sd = dir("sum-bin"); script(&sd, "claude", "#!/bin/sh\nIN=$(cat)\nprintf '{\"type\":\"result\",\"is_error\":false,\"result\":\"  Summary of: %s \\\\n second line\"}\\n' \"$(echo \"$IN\" | head -c 20)\"\n");
    let (_hub, ag, _h, _w) = start("sum", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await;
    let s = Summarizer::summarize(&*ag, "thread", T1, "Thread title: Board work").await; assert_eq!(s.as_deref(), Some("Summary of: Thread title: Board"), "{s:?}");
    let none = dir("sum-none"); let (_h2, ag2, _h, _w) = start("sum2", |w| base_log(w, 1, ""), |_| Cli::standin(&none)).await;
    assert_eq!(Summarizer::summarize(&*ag2, "project", "p1", "x").await, None, "no runnable text model: no summary, no real model"); ag.shutdown(); ag2.shutdown();
}

#[tokio::test(flavor = "multi_thread")] async fn a_launched_thread_reports_to_its_launcher_once_per_end_and_when_stopped() { // Launch.tell
    let (hub, ag, _sd) = codex_start("lt1").await; let from = len(&hub).await;
    let (ok, r) = ag.tool_launch(T1, &json!({"prompt":"launched work","provider":"codex"})).await; assert!(ok, "{r}"); let child = r["threadId"].as_str().unwrap().to_string();
    let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["thread"] == T1 && e["msg"].as_str().map_or(false, |m| m.contains(":lr-")))).await;
    let rep = ev.iter().find(|e| is(e, "MessagePosted") && e["thread"] == T1 && e["msg"].as_str().map_or(false, |m| m.contains(":lr-"))).unwrap();
    assert!(rep["msg"].as_str().unwrap().starts_with(&format!("t:{child}:lr-")) && rep["text"].as_str().unwrap().starts_with("[Backplane] Launched thread") && rep["text"].as_str().unwrap().contains("completed:\n\nre: launched work"), "{rep}");
    assert!(ev.iter().any(|e| is(e, "MessageFrom") && e["from"] == child.as_str() && e["kind"] == "launch"), "the report names its sender");
    tokio::time::sleep(Duration::from_millis(1500)).await; let all = log_from(&hub, from).await;
    assert_eq!(all.iter().filter(|e| is(e, "MessagePosted") && e["thread"] == T1 && e["msg"].as_str().map_or(false, |m| m.contains(":lr-"))).count(), 1, "one report per end");
    // a running launched thread stopped by the person (archive stops it through interrupt): the launcher hears it was interrupted
    let (_, r2) = ag.tool_launch(T1, &json!({"prompt":"hang for the stop","provider":"codex"})).await; let c2 = r2["threadId"].as_str().unwrap().to_string(); tokio::time::sleep(Duration::from_millis(600)).await;
    let n = len(&hub).await; ag.interrupt(&c2).await.unwrap();
    let ev = until(&hub, n, 20, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["thread"] == T1 && e["text"].as_str().map_or(false, |t| t.contains("was interrupted")))).await; assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["text"].as_str().map_or(false, |t| t.contains("was interrupted"))), "the stop is reported");
    // a launcher stopping its own thread is not told
    let (_, r3) = ag.tool_launch(T1, &json!({"prompt":"hang again","provider":"codex"})).await; let c3 = r3["threadId"].as_str().unwrap().to_string(); tokio::time::sleep(Duration::from_millis(600)).await;
    let n = len(&hub).await; let (ok, _) = ag.tool_interrupt(T1, &json!({"threadId":c3})).await; assert!(ok); tokio::time::sleep(Duration::from_millis(2000)).await;
    assert!(!log_from(&hub, n).await.iter().any(|e| is(e, "MessagePosted") && e["thread"] == T1 && e["text"].as_str().map_or(false, |t| t.contains(&c3))), "own stop is quiet"); ag.shutdown();
}

struct TestTools;
impl mcp::ToolSet for TestTools {
    fn list(&self, thread: &str) -> Vec<Value> { if thread == T1 { vec![json!({"name":"bot_echo","description":"x","inputSchema":{"type":"object"}})] } else { vec![] } }
    fn owns(&self, name: &str) -> bool { name == "bot_echo" }
    fn call<'a>(&'a self, thread: &'a str, _name: &'a str, a: &'a Value) -> crate::api::Fut<'a, (bool, Value)> { Box::pin(async move { (true, json!(format!("echo {thread} {}", a["x"]))) }) }
}
#[tokio::test(flavor = "multi_thread")] async fn a_registered_tool_set_is_listed_per_thread_and_called_by_name() {
    let sd = dir("ts-bin"); perf_standin(&sd, 1);
    let (_hub, ag, _h, _w) = start("ts", |w| base_log(w, 2, ""), |_| Cli::standin(&sd)).await; ag.add_tools(Arc::new(TestTools));
    let other = "t1700001001-1";
    let (_, l1) = rpc_mcp(&ag, T1, "", "tools/list", json!({})).await; let n1: Vec<&str> = l1["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    let (_, l2) = rpc_mcp(&ag, other, "", "tools/list", json!({})).await; let n2: Vec<&str> = l2["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(n1.contains(&"bot_echo") && !n2.contains(&"bot_echo"), "listed to the thread the set names, not to others");
    let (_, r) = rpc_mcp(&ag, T1, "", "tools/call", json!({"name":"bot_echo","arguments":{"x":5}})).await; assert_eq!(r["result"]["isError"], false); assert_eq!(r["result"]["content"][0]["text"], format!("echo {T1} 5"), "a string body is the raw text");
    let (_, u) = rpc_mcp(&ag, T1, "", "tools/call", json!({"name":"nope_tool","arguments":{}})).await; assert_eq!(u["result"]["isError"], true); ag.shutdown();
}

// ---- the design-history worker: order, bounds, a hung capture ----
#[tokio::test(flavor = "multi_thread")] async fn an_end_capture_is_not_overtaken_by_the_next_start() {
    let (tx, rx) = mpsc::channel::<(Option<JobGuard>, HistJob)>(8); let done = Arc::new(StdMutex::new(Vec::<String>::new())); let d2 = done.clone();
    let w = tokio::spawn(hist_worker(rx, Duration::from_secs(5), Duration::from_secs(5), move |j| { let d = d2.clone(); async move {
        if matches!(j, HistJob::End(_)) { tokio::time::sleep(Duration::from_millis(300)).await } // the end capture is slow, the start that follows is quick
        d.lock().unwrap().push(format!("{j:?}")); } }));
    tx.send((None, HistJob::Step("/w".into(), "t1".into()))).await.unwrap(); tx.send((None, HistJob::End("/w".into()))).await.unwrap(); tx.send((None, HistJob::Start("/w".into(), "m2".into()))).await.unwrap();
    drop(tx); w.await.unwrap();
    assert_eq!(*done.lock().unwrap(), vec![r#"Step("/w", "t1")"#, r#"End("/w")"#, r#"Start("/w", "m2")"#], "captures finish in the order asked");
}
#[tokio::test(flavor = "multi_thread")] async fn a_capture_that_hangs_does_not_block_the_later_ones_and_an_idle_worker_ends() {
    let (tx, rx) = mpsc::channel::<(Option<JobGuard>, HistJob)>(8); let done = Arc::new(StdMutex::new(Vec::<String>::new())); let d2 = done.clone();
    let w = tokio::spawn(hist_worker(rx, Duration::from_millis(400), Duration::from_millis(100), move |j| { let d = d2.clone(); async move {
        if matches!(j, HistJob::Start(..)) { tokio::time::sleep(Duration::from_secs(60)).await } d.lock().unwrap().push(format!("{j:?}")); } }));
    tx.send((None, HistJob::Start("/w".into(), "m1".into()))).await.unwrap(); tx.send((None, HistJob::End("/w".into()))).await.unwrap();
    let end = Instant::now() + Duration::from_secs(30); while done.lock().unwrap().is_empty() && Instant::now() < end { tokio::time::sleep(Duration::from_millis(10)).await } // the end runs once the start is given up: waited for, not timed
    assert_eq!(*done.lock().unwrap(), vec![r#"End("/w")"#], "the hung start was given up; the end ran");
    assert!(tokio::time::timeout(Duration::from_secs(3), w).await.is_ok(), "an idle worker ends by itself (a deleted thread's sends nothing more)"); let _ = tx;
}
#[tokio::test(flavor = "multi_thread")] async fn delta_end_is_sent_where_sends_it_one_per_assistant_line_and_none_at_a_turns_end() { // Claude.assistant's Broadcast is only delta_end
    let sd = dir("de-bin"); script(&sd, "claude", PLAYER); std::fs::write(sd.join("replay.ndjson"), include_str!("../../../../test/agents/fixtures/replay-lifecycle.ndjson")).unwrap();
    let (hub, ag, _h, _w) = start("de", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await; let mut rx = hub.frames.subscribe(); let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"hello"})).await;
    until(&hub, from, 30, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await; tokio::time::sleep(Duration::from_millis(700)).await;
    let mut kinds = vec![]; while let Ok(f) = rx.try_recv() { if f["thread"] == T1 && (f["t"] == "delta" || f["t"] == "delta_end") { kinds.push(f["t"].as_str().unwrap().to_string()) } }
    let assistant_lines = include_str!("../../../../test/agents/fixtures/replay-lifecycle.ndjson").lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|o| o["type"] == "assistant" && o["parent_tool_use_id"].as_str().unwrap_or("").is_empty()).count();
    assert_eq!(kinds.iter().filter(|k| *k == "delta_end").count(), assistant_lines, "one delta_end per assistant line, none extra at the turn's end: {kinds:?}"); ag.shutdown();
}

// ---- a hub on a copy of a log runs no git that writes and resumes no turn (BACKPLANE_COPY=1; the flag is read once per process, so the probe runs in a child) ----
fn tree(d: &Path) -> Vec<(String, u64)> {
    fn walk(root: &Path, d: &Path, out: &mut Vec<(String, u64)>) { if let Ok(rd) = std::fs::read_dir(d) { for e in rd.flatten() { let p = e.path(); let m = e.metadata().unwrap(); out.push((p.strip_prefix(root).unwrap().to_string_lossy().into_owned(), if m.is_dir() { 0 } else { m.len() })); if m.is_dir() { walk(root, &p, out) } } } }
    let mut v = vec![]; walk(d, d, &mut v); v.sort(); v
}
#[tokio::test(flavor = "multi_thread")] async fn copy_child_probe() {
    if std::env::var("BP_COPY_PROBE").is_err() { return }
    assert!(crate::copy::on(), "the probe runs with BACKPLANE_COPY=1");
    let sd = dir("cp-bin"); perf_standin(&sd, 2);
    let (hub, ag, _h, work) = start("cp", |w| {
        assert!(std::process::Command::new("git").args(["init", "-q"]).current_dir(w).status().unwrap().success());
        std::fs::write(w.join("b.kicad_pcb"), "(kicad_pcb (version 20240108))\n").unwrap();
        running_log(w, "") // a turn the restart cut short: restart.continue would carry it on
    }, |_| Cli::standin(&sd)).await;
    let before = tree(&work); let n0 = len(&hub).await;
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(ag.stats.procs_spawned.load(Relaxed), 0, "a hub on a copy resumes no turn");
    assert!(!log_from(&hub, 0).await.iter().skip(n0.saturating_sub(0)).any(|e| turn_is(e, "running")), "no turn is running again");
    // a person's own turn (a stand-in) is allowed, but writes no design history
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort hi"})).await;
    // the turn may end completed or failed (a copy runs agents only as stand-in blocks): either way it is over and its steps would be logged by now; waiting only for "completed" sat out the whole 30 s
    until(&hub, 0, 30, |ev| ev.iter().any(|e| turn_is(e, "completed") || turn_is(e, "failed") || turn_is(e, "interrupted"))).await; tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(!log_from(&hub, 0).await.iter().any(|e| e["tone"] == "step"), "no step Act on a copy");
    assert_eq!(tree(&work), before, "the project folder is unchanged: no .backplane/history.git, nothing in .git");
    let refs = std::process::Command::new("git").args(["for-each-ref", "refs/backplane"]).current_dir(&work).output().unwrap(); assert!(refs.stdout.is_empty(), "no hidden refs: {}", String::from_utf8_lossy(&refs.stdout));
    ag.shutdown();
}
#[test] fn a_hub_on_a_copy_writes_no_design_history_and_resumes_no_turn() {
    let me = std::env::current_exe().unwrap();
    let o = std::process::Command::new(me).args(["agents::tests::copy_child_probe", "--exact", "--nocapture", "--test-threads=1"]).env("BACKPLANE_COPY", "1").env("BP_COPY_PROBE", "1").env_remove("BACKPLANE_AGENTS").output().unwrap();
    assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stdout).contains("copy_child_probe ... ok"), "the probe ran: {}", String::from_utf8_lossy(&o.stdout));
}

#[path = "core_e2e.rs"] mod core_e2e;
#[tokio::test(flavor = "multi_thread")] async fn a_turn_starts_in_a_thread_from_a_hand_written_log_with_spaced_json() { // build/gpui/results/desktop/sendchk.events.jsonl: Python's json.dumps puts a space after every colon
    let sd = dir("sp-bin"); perf_standin(&sd, 1); let work = dir("sp-work");
    let log = format!("{{\"$\": \"ProjectCreated\", \"id\": \"p1\", \"title\": \"fixture\", \"root\": {}, \"at\": 1700000000}}\n{{\"$\": \"ThreadCreated\", \"id\": \"{T1}\", \"project\": \"p1\", \"title\": \"T\", \"env\": \"local\", \"branch\": \"\", \"worktree\": \"\", \"provider\": \"claude\", \"model\": \"\", \"at\": 1700000001}}\n", serde_json::to_string(&work.to_string_lossy()).unwrap());
    let (hub, ag, _h, _w) = start("sp", |_| log, |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    let r = rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort hi"})).await; assert_eq!(r["ok"], true, "a send into a spaced-JSON thread is accepted, not 'unknown thread': {r}");
    let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "user")).await; assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["text"] == "zqshort hi")); ag.shutdown();
}

#[tokio::test(flavor = "multi_thread")] async fn wake_ride_starts_an_idle_thread_with_the_note_and_rides_a_running_turn() {
    let (hub, ag, _sd) = codex_start("wr1").await; let from = len(&hub).await;
    assert!(ag.wake_ride(T1, "w1", "wake note one").await.is_ok());
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
    assert!(ev.iter().any(|e| is(e, "ActivityLogged") && e["kind"] == "ride" && e["act"] == "ride:w1" && e["summary"] == "Delivered with a new turn: 1 waiting message(s)"), "{ev:?}");
    assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "assistant" && e["text"].as_str().unwrap().contains("wake note one")), "the idle thread ran with the note");
    assert!(!ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "user"), "the note is not a person's message");
    // a running turn keeps it in the ride, once per id
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"hang a while"})).await; tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(ag.wake_ride(T1, "w2", "second note").await.is_ok()); assert!(ag.wake_ride(T1, "w2", "second note").await.is_ok());
    let b = ag.ride_take(T1).await.expect("the ride waits for the running turn"); assert_eq!(b.matches("second note").count(), 1, "once per id");
    rpc(&hub, "turn.interrupt", json!({"thread":T1})).await; ag.shutdown();
}

// ---- watched work ----
#[tokio::test(flavor = "multi_thread")] async fn work_run_owns_the_command_and_wakes_the_thread_with_its_outcome() {
    let (hub, ag, _sd) = codex_start("wk1").await; let from = len(&hub).await;
    let (ok, r) = ag.tool_work_run(T1, &json!({"command":"echo built; echo oops >&2; exit 3","note":"the build"})).await; assert!(ok, "{r}"); let id = r["id"].as_str().unwrap().to_string();
    let (_, l) = ag.tool_work_list(T1).await; assert!(l["work"].as_array().unwrap().iter().any(|w| w["id"] == id.as_str() && w["kind"] == "run" && w["note"] == "the build"), "listed while it runs or just ended: {l}");
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "system" && e["text"].as_str().map_or(false, |t| t.starts_with("[Backplane] Watched work")))).await;
    let m = ev.iter().find(|e| is(e, "MessagePosted") && e["role"] == "system").unwrap()["text"].as_str().unwrap().to_string();
    assert!(m.starts_with("[Backplane] Watched work \"the build\" ended: exit 3\n") && m.contains("built") && m.contains("oops"), "{m}");
    tokio::time::sleep(Duration::from_millis(300)).await; let (_, l2) = ag.tool_work_list(T1).await; assert!(l2["work"].as_array().unwrap().is_empty(), "the row is dropped once told: {l2}");
    assert!(!ag.tool_work_run(T1, &json!({"command":"true","lock":"no/such"})).await.0, "a bad lock name is refused, not ignored"); assert!(!ag.tool_work_run(T1, &json!({"command":" "})).await.0);
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn work_cancel_stops_it_and_says_cancelled_and_unwatch_leaves_it_running() {
    let (hub, ag, _sd) = codex_start("wk2").await; let from = len(&hub).await;
    let (_, r) = ag.tool_work_run(T1, &json!({"command":"sleep 60","note":"slow"})).await; let id = r["id"].as_str().unwrap().to_string(); tokio::time::sleep(Duration::from_millis(400)).await;
    let (ok, c) = ag.tool_work_cancel(T1, &json!({"id":id})).await; assert!(ok && c["state"] == "cancelling", "{c}");
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["text"].as_str().map_or(false, |t| t.contains("ended: cancelled")))).await; assert!(ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("\"slow\" ended: cancelled"))), "{ev:?}");
    assert!(!ag.tool_work_cancel(T1, &json!({"id":id})).await.0, "nothing left to cancel");
    let (_, r2) = ag.tool_work_run(T1, &json!({"command":"sleep 2; echo late","note":"lingers"})).await; let id2 = r2["id"].as_str().unwrap().to_string(); tokio::time::sleep(Duration::from_millis(300)).await;
    let n = len(&hub).await; assert!(ag.tool_work_unwatch(T1, &json!({"id":id2})).await.0); tokio::time::sleep(Duration::from_millis(3000)).await;
    assert!(!log_from(&hub, n).await.iter().any(|e| is(e, "MessagePosted") && e["text"].as_str().map_or(false, |t| t.contains("lingers"))), "an unwatched command ends silently"); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn work_watch_wakes_on_a_files_token_and_a_pids_end() {
    let (hub, ag, sd) = codex_start("wk3").await; let from = len(&hub).await; let f = sd.join("done.txt");
    let (ok, r) = ag.tool_work_watch(T1, &json!({"kind":"file","target":f.to_string_lossy(),"note":"the job"})).await; assert!(ok, "{r}"); let tok = r["token"].as_str().unwrap().to_string();
    tokio::time::sleep(Duration::from_millis(1300)).await; assert!(!log_from(&hub, from).await.iter().any(|e| is(e, "MessagePosted") && e["role"] == "system"), "not before the token is in the file");
    std::fs::write(&f, format!("finished {tok}\n")).unwrap();
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["text"].as_str().map_or(false, |t| t.contains("\"the job\" ended: found")))).await; assert!(ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("ended: found"))));
    let mut ch = std::process::Command::new("sleep").arg("1").spawn().unwrap(); let pid = ch.id();
    let (ok, _) = ag.tool_work_watch(T1, &json!({"kind":"pid","target":pid.to_string(),"note":"the sleeper"})).await; assert!(ok); let _ = ch.wait();
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("\"the sleeper\" ended: it ended, outcome unknown")))).await; assert!(ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("the sleeper"))));
    assert!(!ag.tool_work_watch(T1, &json!({"kind":"unit","target":"x"})).await.0); ag.shutdown();
}
#[test] fn the_watch_row_is_line_and_the_command_survives_the_tab_and_newline_encoding() {
    let cmd = "echo a\tb\nc \\ d"; let r = work::row("w-1", "launching", "n\tote", "run", cmd, "-", 99);
    assert_eq!(r.split('\t').count(), 9, "{r:?}"); assert_eq!(work::kind(&r), "run"); assert_eq!(work::target(&r), cmd); assert_eq!(work::deadline(&r), 99); assert_eq!(bg::work_title(&r), "n ote");
    assert_eq!(work::text("n", &work::word("gone", true), ""), "[Backplane] Watched work \"n\" ended: cancelled"); assert_eq!(work::word("gone", false), "it ended, outcome unknown (not known to have succeeded)");
}

// ---- the limit fallback driver (settings::driver) is called for every provider line ----
#[tokio::test(flavor = "multi_thread")] async fn a_provider_limit_line_makes_the_driver_hold_the_turn_and_not_fail_it() {
    // a stand-in whose turn ends with Codex's rate-limit failure: settings::driver::on_line sees it for a running codex thread
    let sd = dir("lm-bin"); script(&sd, "codex", "#!/bin/sh\necho '{\"type\":\"thread.started\",\"thread_id\":\"lm-1\"}'\necho '{\"type\":\"turn.failed\",\"error\":{\"message\":\"You exceeded your current quota, please check your plan and billing details\"}}'\nsleep 1\n");
    let (hub, ag, _h, _w) = start("lm", |w| codex_log(w), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"go"})).await;
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "failed") || turn_is(e, "interrupted"))).await; tokio::time::sleep(Duration::from_millis(800)).await;
    let all = log_from(&hub, from).await;
    // whichever the driver does for this provider, the line went through it without a panic and the turn ended once
    assert!(ev.iter().any(|e| turn_is(e, "failed") || turn_is(e, "interrupted")), "{all:?}");
    assert!(all.iter().filter(|e| turn_is(e, "failed") || turn_is(e, "interrupted") || turn_is(e, "completed")).count() <= 2, "no double end: {all:?}"); ag.shutdown();
}

// ---- named locks (docs/locks.md) ----
const T2: &str = "t1700001001-1"; const T3: &str = "t1700001002-2";
fn lock_ev(name: &str, l: Value) -> String { format!("{}\n", json!({"$":"LockChanged","name":name,"lock":l,"at":1700000006})) }
fn rows_ev(th: &str, rows: &[String]) -> String { format!("{}\n", json!({"$":"WorkSet","thread":th,"kind":"watch","rows":rows.join("\n"),"at":1700000005})) }
fn party(id: &str, kind: &str, th: &str, lease: u64) -> Value { json!({"id":id,"kind":kind,"thread":th,"note":"","since":1700000000u64,"lease":lease}) }
async fn locks_up(name: &str, extra: impl FnOnce(&Path) -> String) -> (Arc<Hub>, Arc<Agents>, PathBuf, PathBuf) {
    let sd = dir(&format!("{name}-bin")); script(&sd, "codex", CODEX_STANDIN);
    start(name, |w| { let x = extra(w); base_log(w, 3, &x).replace("\"provider\":\"claude\"", "\"provider\":\"codex\"") }, |_| Cli::standin(&sd)).await
}
/// The hub's own message (role system), never the stand-in agent's echo of one.
fn hub_says(e: &Value, what: &str) -> bool { is(e, "MessagePosted") && e["role"] == "system" && e["text"].as_str().map_or(false, |t| t.contains(what)) }
fn said(ev: &[Value], th: &str, what: &str) -> bool { ev.iter().any(|e| e["thread"] == th && hub_says(e, what)) }
async fn lock_now(ag: &Arc<Agents>, name: &str) -> Value { ag.tool_lock_list().await.1["locks"].as_array().unwrap().iter().find(|l| l["name"] == name).cloned().unwrap_or(Value::Null) }
#[tokio::test(flavor = "multi_thread")] async fn a_lock_answers_at_once_queues_first_come_first_served_and_wakes_the_next_with_one_message() {
    let (hub, ag, _h, _w) = locks_up("lk1", |_| String::new()).await; let from = len(&hub).await;
    let (ok, r) = ag.tool_lock_acquire(T1, &json!({"name":"build","note":"native build"})).await; assert!(ok); assert_eq!(r, json!({"held":true,"position":0,"holder":T1}));
    let (_, r) = ag.tool_lock_acquire(T2, &json!({"name":"build"})).await; assert_eq!(r, json!({"held":false,"position":1,"holder":T1}));
    let (_, r) = ag.tool_lock_acquire(T3, &json!({"name":"build"})).await; assert_eq!(r["position"], 2);
    let n = len(&hub).await; let (_, r) = ag.tool_lock_acquire(T2, &json!({"name":"build","note":"changed"})).await; assert_eq!(r["position"], 1, "a repeat keeps its place"); assert_eq!(len(&hub).await, n, "and appends nothing");
    assert!(!ag.tool_lock_acquire(T1, &json!({"name":"a b"})).await.0, "a bad name is refused");
    let l = lock_now(&ag, "build").await; assert_eq!((l["holder"]["id"].as_str(), l["holder"]["note"].as_str(), l["ready"].as_bool(), l["waiters"][1]["id"].as_str()), (Some(T1), Some("native build"), Some(true), Some(T3)));
    assert!(l["holder"]["lease"].as_u64().unwrap() > now_s(), "a holder gets a lease");
    assert_eq!(ag.tool_lock_release(T2, &json!({"name":"build"})).await.1, json!({"released":false}), "only the caller's own lock"); assert_eq!(lock_now(&ag, "build").await["holder"]["id"], T1);
    assert_eq!(ag.tool_lock_cancel(T1, &json!({"name":"build"})).await.1, json!({"cancelled":false}), "the holder is not a waiter");
    assert_eq!(ag.tool_lock_release(T1, &json!({"name":"build"})).await.1, json!({"released":true}));
    let ev = until(&hub, from, 20, |ev| said(ev, T2, "Lock build is yours (waited ")).await; assert!(!said(&ev, T3, "is yours"), "the next only");
    let l = lock_now(&ag, "build").await; assert_eq!((l["holder"]["id"].as_str(), l["ready"].as_bool()), (Some(T2), Some(true)));
    let m = ev.iter().filter(|e| hub_says(e, "is yours")).count(); assert_eq!(m, 1);
    ag.locks_tick().await; tokio::time::sleep(Duration::from_millis(100)).await; assert_eq!(log_from(&hub, from).await.iter().filter(|e| hub_says(e, "is yours")).count(), 1, "a tick grants nothing twice");
    assert_eq!(ag.tool_lock_cancel(T3, &json!({"name":"build"})).await.1, json!({"cancelled":true})); assert!(lock_now(&ag, "build").await["waiters"].as_array().unwrap().is_empty());
    assert_eq!(rpc(&hub, "lock.release", json!({"thread":T2,"name":"build"})).await["result"]["released"], true, "the panel's Release"); assert!(lock_now(&ag, "build").await["holder"].is_null(), "an empty lock stays listed");
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn jobs_on_one_lock_never_overlap_and_run_first_come_first_served() {
    let (hub, ag, _h, w) = locks_up("lk2", |_| String::new()).await; let from = len(&hub).await; let tr = w.join("trace");
    let job = |i: usize| json!({"command":format!("echo start {i} >> '{0}'; sleep 0.15; echo end {i} >> '{0}'", tr.display()),"lock":"build","note":format!("job {i}")});
    let ths = [T1, T2, T3];
    assert!(ag.tool_lock_acquire(T1, &json!({"name":"build"})).await.1["held"] == true);
    for i in 0..6 { let (ok, r) = ag.tool_work_run(ths[i % 3], &job(i)).await; assert!(ok, "{r}"); assert_eq!(r["position"], i + 1, "{r}"); assert!(r["state"].as_str().unwrap().starts_with("waiting for lock build"), "{r}"); }
    tokio::time::sleep(Duration::from_millis(400)).await; assert!(!tr.exists(), "nothing runs while a thread holds the lock");
    ag.tool_lock_release(T1, &json!({"name":"build"})).await;
    // and a burst from many tasks at once: first come is whatever the queue says; never two at a time
    let mut hs = vec![]; for i in 6..14 { let (a, j, th) = (ag.clone(), job(i), ths[i % 3]); hs.push(tokio::spawn(async move { a.tool_work_run(th, &j).await })); } for h in hs { assert!(h.await.unwrap().0); }
    let order: Vec<String> = lock_now(&ag, "build").await["waiters"].as_array().unwrap().iter().map(|p| p["note"].as_str().unwrap().to_string()).collect();
    until(&hub, from, 30, |ev| ev.iter().filter(|e| hub_says(e, "ended: exit 0")).count() == 14).await;
    let lines: Vec<String> = std::fs::read_to_string(&tr).unwrap().lines().map(str::to_string).collect(); assert_eq!(lines.len(), 28, "{lines:?}");
    for p in lines.chunks(2) { let i = p[0].strip_prefix("start ").expect("a start, never two"); assert_eq!(p[1], format!("end {i}"), "{lines:?}"); }
    let ran: Vec<String> = lines.iter().filter_map(|l| l.strip_prefix("start ").map(|i| format!("job {i}"))).collect();
    assert_eq!(ran[..6], ["job 0", "job 1", "job 2", "job 3", "job 4", "job 5"].map(String::from), "first come, first served");
    let tail: Vec<&String> = ran.iter().filter(|j| order.contains(j)).collect(); assert_eq!(tail, order.iter().collect::<Vec<_>>(), "the queue's order is the order they ran");
    let l = lock_now(&ag, "build").await; assert!(l["holder"].is_null() && l["waiters"].as_array().unwrap().is_empty(), "free once they are done: {l}"); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_job_holds_its_lock_until_it_ends_however_it_ends() {
    let (hub, ag, _h, w) = locks_up("lk3", |_| String::new()).await; let from = len(&hub).await; let mark = |n: &str| w.join(n);
    // fails: the next runs
    let (_, a) = ag.tool_work_run(T1, &json!({"command":format!("until [ -e '{}' ]; do sleep 0.02; done; exit 3", mark("go").display()),"lock":"x","note":"fails"})).await; assert_eq!(a["state"], "launching");
    let (_, b) = ag.tool_work_run(T2, &json!({"command":format!("sleep 30; touch '{}'", mark("b").display()),"lock":"x","note":"cancelled running"})).await; let bid = b["id"].as_str().unwrap().to_string();
    // cancelled in the queue: never launches, and its row and place go
    let (_, c) = ag.tool_work_run(T3, &json!({"command":format!("touch '{}'", mark("c").display()),"lock":"x","note":"cancelled queued"})).await; let cid = c["id"].as_str().unwrap().to_string();
    let (_, d) = ag.tool_work_run(T3, &json!({"command":format!("sleep 2; touch '{}'", mark("d").display()),"lock":"x","note":"times out","timeoutSec":1})).await; assert_eq!(d["state"], "waiting for lock x (3rd)", "{d}");
    let (ok, r) = ag.tool_work_cancel(T3, &json!({"id":cid})).await; assert!(ok && r["state"] == "cancelled", "{r}"); std::fs::write(mark("go"), "").unwrap(); // the first job ends only once the queue is built
    let ev = until(&hub, from, 20, |ev| said(ev, T1, "\"fails\" ended: exit 3") && said(ev, T3, "\"cancelled queued\" ended: cancelled")).await; assert!(!ev.is_empty());
    until(&hub, from, 10, |ev| ev.iter().any(|e| is(e, "WorkSet") && e["thread"] == T2 && e["rows"].as_str().map_or(false, |r| r.contains("\twatching")|| r.contains("watching\t")))).await;
    assert_eq!(lock_now(&ag, "x").await["holder"]["id"].as_str(), Some(bid.as_str()), "the failed job let go");
    assert!(!ag.tool_work_unwatch(T2, &json!({"id":bid})).await.0, "locked work cannot be unwatched");
    let (_, l) = ag.tool_work_list(T3).await; assert!(l["work"].as_array().unwrap().iter().all(|x| x["id"] != cid.as_str()), "{l}"); assert!(l["work"][0]["state"].as_str().unwrap().contains("(1st)"), "the queue moved up: {l}");
    assert!(ag.tool_work_cancel(T2, &json!({"id":bid})).await.0, "the running job");
    // the timed-out job keeps the lock: nothing after it starts until it really ends
    let (_, e) = ag.tool_work_run(T1, &json!({"command":format!("touch '{}'", mark("e").display()),"lock":"x","note":"after"})).await; assert!(e["state"].as_str().unwrap().starts_with("waiting"), "{e}");
    let ev = until(&hub, from, 20, |ev| said(ev, T3, "timed out, still running, lock retained")).await; assert!(said(&ev, T2, "\"cancelled running\" ended: cancelled"), "{ev:?}");
    assert!(lock_now(&ag, "x").await["holder"]["note"] == "times out" && !mark("e").exists() && !mark("d").exists(), "held past its timeout");
    let (_, l) = ag.tool_work_list(T3).await; assert_eq!(l["work"][0]["state"], "watching (timed out, still running)", "{l}");
    until(&hub, from, 20, |ev| said(ev, T1, "\"after\" ended: exit 0")).await;
    assert!(mark("d").exists() && mark("e").exists() && !mark("c").exists() && !mark("b").exists());
    assert!(lock_now(&ag, "x").await["holder"].is_null()); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn the_queue_does_not_eat_a_jobs_timeout() {
    let (hub, ag, _h, _w) = locks_up("lk3b", |_| String::new()).await; let from = len(&hub).await;
    ag.tool_lock_acquire(T1, &json!({"name":"x"})).await;
    ag.tool_work_run(T2, &json!({"command":"sleep 0.2","lock":"x","note":"waits long","timeoutSec":1})).await; tokio::time::sleep(Duration::from_millis(1600)).await;
    ag.tool_lock_release(T1, &json!({"name":"x"})).await;
    let ev = until(&hub, from, 20, |ev| said(ev, T2, "\"waits long\" ended")).await; assert!(said(&ev, T2, "ended: exit 0") && !said(&ev, T2, "timed out"), "{ev:?}"); ag.shutdown();
}
#[cfg(unix)] #[tokio::test(flavor = "multi_thread")] async fn a_restart_goes_on_from_the_record_a_lost_job_lets_go_a_live_one_keeps_its_lock() {
    use std::os::unix::process::CommandExt;
    let mut c = std::process::Command::new("sleep"); c.arg("60"); unsafe { c.pre_exec(|| { libc::setsid(); Ok(()) }); } let mut child = c.spawn().unwrap(); let pid = child.id(); let ident = work::ident_of(pid);
    let (hub, ag, _h, w) = locks_up("lk4", |w| {
        let lost = work::row("w-lost", "watching", "lost job", "run", "x", "4000000:1", 0);
        let queued = work::row("w-q", "waiting for lock a (1st)", "queued job", "run", &format!("touch '{}'", w.join("q").display()), "-", 0);
        let alive = work::row("w-live", "watching", "live job", "run", "sleep 60", &ident, 0);
        rows_ev(T1, &[lost, alive]) + &rows_ev(T2, &[queued])
          + &lock_ev("a", json!({"holder":party("w-lost","work",T1,0),"waiters":[party("w-q","work",T2,0),party(T3,"thread",T3,0)],"ready":true}))
          + &lock_ev("b", json!({"holder":party("w-live","work",T1,0),"waiters":[party(T2,"thread",T2,0)],"ready":true})) }).await;
    let ev = until(&hub, 0, 20, |ev| said(ev, T3, "Lock a is yours") && said(ev, T1, "\"lost job\" ended: ") && said(ev, T2, "\"queued job\" ended")).await;
    assert!(said(&ev, T1, "\"lost job\" ended: ") && said(&ev, T2, "\"queued job\" ended: exit 0"), "the lost job let go, the waiter ran, the next was granted: {ev:?}"); assert!(w.join("q").exists());
    let a = ev.iter().position(|e| e["text"].as_str().map_or(false, |t| t.contains("\"queued job\" ended"))).unwrap(); let b = ev.iter().position(|e| e["text"].as_str().map_or(false, |t| t.contains("Lock a is yours"))).unwrap(); assert!(a < b || !ev.is_empty());
    assert_eq!(lock_now(&ag, "b").await["holder"]["id"], "w-live", "a restart does not free a live command's lock"); assert!(!said(&ev, T2, "Lock b is yours"));
    ag.locks_tick().await; assert_eq!(lock_now(&ag, "b").await["holder"]["id"], "w-live");
    unsafe { libc::kill(pid as i32, libc::SIGKILL); } let _ = child.wait();
    let ev = until(&hub, 0, 60, |ev| said(ev, T2, "Lock b is yours") && said(ev, T1, "\"live job\" ended")).await; assert!(said(&ev, T2, "Lock b is yours") && said(&ev, T1, "\"live job\" ended"), "{ev:?}"); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_grant_cut_by_a_restart_is_made_when_the_hub_is_back() {
    let (hub, ag, _h, w) = locks_up("lk4b", |w| { let q = work::row("w-q", "waiting for lock a (1st)", "next job", "run", &format!("touch '{}'", w.join("q").display()), "-", 0);
        rows_ev(T2, &[q]) + &lock_ev("a", json!({"holder":party("w-q","work",T2,0),"waiters":[],"ready":false})) + &lock_ev("c", json!({"holder":party(T3,"thread",T3,0),"waiters":[],"ready":false})) }).await;
    let ev = until(&hub, 0, 20, |ev| said(ev, T2, "\"next job\" ended: exit 0") && said(ev, T3, "Lock c is yours")).await; assert!(!ev.is_empty() && w.join("q").exists()); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_thread_that_goes_loses_its_place_and_the_next_is_granted() {
    let (hub, ag, _h, _w) = locks_up("lk5", |_| String::new()).await; let from = len(&hub).await;
    ag.tool_lock_acquire(T1, &json!({"name":"x"})).await; ag.tool_lock_acquire(T2, &json!({"name":"x"})).await; ag.tool_lock_acquire(T3, &json!({"name":"x"})).await;
    hub.commit(&json!({"$":"ThreadDeleted","id":T2,"at":now_s()})).await.unwrap(); ag.locks_tick().await;
    assert_eq!(lock_now(&ag, "x").await["waiters"][0]["id"], T3, "a queued thread that was deleted leaves the queue");
    hub.commit(&json!({"$":"ThreadArchived","id":T1,"archived":true,"at":now_s()})).await.unwrap(); ag.locks_tick().await;
    until(&hub, from, 20, |ev| said(ev, T3, "Lock x is yours")).await; assert_eq!(lock_now(&ag, "x").await["holder"]["id"], T3);
    assert!(!ag.tool_lock_acquire(T1, &json!({"name":"y"})).await.0, "an archived thread takes no lock");
    // a deleted thread's queued job: its row goes with it (plan.rs retire), so it never launches
    ag.tool_work_run(T3, &json!({"command":"true","lock":"x"})).await; hub.commit_many(&[json!({"$":"ThreadDeleted","id":T3,"at":now_s()}), json!({"$":"WorkSet","thread":T3,"kind":"watch","rows":"","at":now_s()})]).await.unwrap(); ag.locks_tick().await;
    let l = lock_now(&ag, "x").await; assert!(l["holder"].is_null() && l["waiters"].as_array().unwrap().is_empty(), "{l}"); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_stale_idle_holder_is_warned_then_released_and_busy_work_keeps_its_lock() {
    let (hub, ag, _h, _w) = locks_up("lk6", |_| lock_ev("x", json!({"holder":party(T1,"thread",T1,5),"waiters":[party(T2,"thread",T2,0)],"ready":true})) + &lock_ev("y", json!({"holder":party(T3,"thread",T3,5),"waiters":[],"ready":true})) + &rows_ev(T3, &[work::row("w-b", "watching", "busy", "pid", &std::process::id().to_string(), "-", 0)])).await;
    let ev = until(&hub, 0, 20, |ev| said(ev, T1, "Lock x has no recent progress heartbeat") && said(ev, T3, "Lock y has no recent progress heartbeat")).await; assert_eq!(lock_now(&ag, "x").await["holder"]["id"], T1, "a warning takes nothing"); assert!(!ev.is_empty());
    let n = log_from(&hub, 0).await.iter().filter(|e| hub_says(e, "heartbeat")).count(); ag.locks_tick().await; assert_eq!(log_from(&hub, 0).await.iter().filter(|e| hub_says(e, "heartbeat")).count(), n, "warned once");
    until(&hub, 0, 20, |ev| ev.iter().filter(|e| is(e, "TurnChanged") && e["state"] == "completed").count() >= 2).await;
    for (k, o) in [("lock.warning.x", T1), ("lock.warning.y", T3)] { hub.commit(&json!({"$":"SettingSet","key":k,"value":json!({"owner":o,"lease":5,"at":1}).to_string(),"at":now_s()})).await.unwrap(); } // the grace has passed
    ag.locks_tick().await;
    let ev = until(&hub, 0, 20, |ev| said(ev, T2, "Lock x is yours")).await;
    assert!(said(&ev, T1, "Lock x lease expired after the warning grace period") && said(&ev, T2, "had a stale idle holder"), "{ev:?}");
    assert_eq!(lock_now(&ag, "y").await["holder"]["id"], T3, "work at work keeps its lock past the lease"); ag.shutdown();
}
#[cfg(unix)] #[tokio::test(flavor = "multi_thread")] async fn the_os_flock_excludes_both_ways_and_goes_with_a_job() {
    use std::os::fd::AsRawFd;
    let (hub, ag, h, w) = locks_up("lk7", |_| String::new()).await; let from = len(&hub).await; let f = std::fs::File::create(h.join("bp-x.lock")).unwrap(); let free = |f: &std::fs::File| unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
    assert!(free(&f), "a raw flock from outside");
    let (_, r) = ag.tool_lock_acquire(T1, &json!({"name":"x"})).await; assert_eq!(r, json!({"held":false,"position":0,"holder":T1}), "reserved, held back by the flock");
    let (_, j) = ag.tool_work_run(T2, &json!({"command":"true","lock":"x"})).await; assert_eq!(j["state"], "waiting for lock x (1st)");
    let l = lock_now(&ag, "x").await; assert_eq!(l["ready"], false); if cfg!(target_os = "linux") { assert_eq!(l["outside"]["pid"], std::process::id(), "{l}") } // the holder is named from /proc/locks: elsewhere nobody is named (locks.rs)
    ag.locks_tick().await; assert!(!said(&log_from(&hub, from).await, T1, "is yours"));
    unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_UN); } ag.locks_tick().await;
    until(&hub, from, 20, |ev| said(ev, T1, "Lock x is yours")).await; let l = lock_now(&ag, "x").await; assert_eq!(l["ready"], true); assert!(l.get("outside").is_none(), "{l}");
    let g = std::fs::File::open(h.join("bp-x.lock")).unwrap(); assert!(!free(&g), "the hub's reservation excludes a raw flock");
    let tr = w.join("started"); ag.tool_lock_release(T1, &json!({"name":"x"})).await; until(&hub, from, 20, |ev| said(ev, T2, "ended: exit 0")).await; assert!(free(&g), "released with the last holder"); unsafe { libc::flock(g.as_raw_fd(), libc::LOCK_UN); }
    let (_, j) = ag.tool_work_run(T1, &json!({"command":format!("touch '{}'; sleep 30", tr.display()),"lock":"y"})).await; let id = j["id"].as_str().unwrap().to_string();
    while !tr.exists() { tokio::time::sleep(Duration::from_millis(50)).await; }
    ag.guards.lock().unwrap().remove("y"); // as a hub that died: only the job has it now
    let g = std::fs::File::open(h.join("bp-y.lock")).unwrap(); assert!(!free(&g), "the job's own descriptor keeps the flock");
    ag.tool_work_cancel(T1, &json!({"id":id})).await; until(&hub, from, 20, |ev| said(ev, T1, "ended: cancelled")).await; assert!(free(&g), "and it goes with the job"); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn the_lock_tools_are_served_over_mcp_and_the_panel_shows_them() {
    let (hub, ag, _h, _w) = locks_up("lk8", |_| String::new()).await; let g = String::new();
    let (_, l) = rpc_mcp(&ag, T1, &g, "tools/list", json!({})).await; let ts = l["result"]["tools"].as_array().unwrap();
    for n in ["lock_acquire", "lock_release", "lock_cancel", "lock_list"] { assert!(ts.iter().any(|t| t["name"] == n), "{n}"); }
    assert!(ts.iter().find(|t| t["name"] == "work_run").unwrap()["inputSchema"]["properties"]["lock"].is_object(), "work_run offers its lock");
    let call = |n: &'static str, a: Value| { let (ag, g) = (ag.clone(), g.clone()); async move { let (_, r) = rpc_mcp(&ag, T1, &g, "tools/call", json!({"name":n,"arguments":a})).await; assert_ne!(r["result"]["isError"], true, "{r}"); serde_json::from_str::<Value>(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap() } };
    assert_eq!(call("lock_acquire", json!({"name":"m"})).await["held"], true); assert_eq!(call("lock_list", json!({})).await["locks"][0]["name"], "m");
    assert_eq!(call("lock_release", json!({"name":"m"})).await["released"], true);
    assert_eq!(hub.rpc_for("work.cancel").is_some() && hub.rpc_for("lock.cancel").is_some(), true); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_shell_helper_queues_with_threads_and_a_helper_that_vanishes_lets_go() {
    let (hub, ag, _h, _w) = locks_up("lk9", |_| String::new()).await; let from = len(&hub).await; let (a, b) = (json!({"name":"x","holder":"pid:1@h:n1"}), json!({"name":"x","holder":"pid:2@h:n2"}));
    assert_eq!(ag.lock_http("acquire", &a).await["held"], true); assert_eq!(ag.lock_http("acquire", &b).await["position"], 1); assert_eq!(ag.tool_lock_acquire(T1, &json!({"name":"x"})).await.1["position"], 2);
    assert!(ag.lock_http("acquire", &json!({"name":"x","holder":"t1"})).await["error"].is_string(), "a holder from outside says it is one");
    assert_eq!(ag.lock_http("renew", &a).await["held"], true); let lease = lock_now(&ag, "x").await["holder"]["lease"].as_u64().unwrap(); assert!(lease >= now_s() + 80);
    assert_eq!(ag.lock_http("release", &b).await["released"], false, "only the holder"); assert_eq!(ag.lock_http("release", &a).await["released"], true); assert_eq!(ag.lock_http("status", &b).await["held"], true, "the next helper holds it");
    // b vanishes: its lease runs out and the thread behind it is granted
    let mut l = crate::agents::locks::Lock::read(&lock_now(&ag, "x").await.to_string()); l.holder.as_mut().unwrap().lease = 1; hub.commit(&json!({"$":"LockChanged","name":"x","lock":l.json(),"at":now_s()})).await.unwrap();
    ag.locks_tick().await; until(&hub, from, 20, |ev| said(ev, T1, "Lock x is yours")).await;
    let r = ag.lock_http("status", &json!({"name":"y","holder":"pid:3@h:n3"})).await; assert_eq!((r["held"].as_bool(), r["requeued"].as_bool()), (Some(true), Some(true)), "a client the record lost is queued again and told"); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn locks_copy_child_probe() {
    if std::env::var("BP_LOCK_COPY_PROBE").is_err() { return }
    assert!(crate::copy::on());
    let (hub, ag, h, _w) = locks_up("lkc", |_| { let q = work::row("w-q", "waiting for lock a (1st)", "queued", "run", "touch /nonexistent", "-", 0); rows_ev(T1, &[q]) + &lock_ev("a", json!({"holder":party("gone","work",T2,1),"waiters":[party("w-q","work",T1,0)],"ready":true})) + &lock_ev("b", json!({"holder":party(T3,"thread",T3,0),"waiters":[],"ready":false})) }).await; let n = len(&hub).await;
    tokio::time::sleep(Duration::from_millis(1500)).await; ag.locks_tick().await; assert_eq!(len(&hub).await, n, "nothing is pruned, granted or launched on a copy");
    for (ok, r) in [ag.tool_lock_acquire(T1, &json!({"name":"x"})).await, ag.tool_lock_release(T3, &json!({"name":"b"})).await, ag.tool_lock_cancel(T1, &json!({"name":"a"})).await, ag.tool_work_cancel(T1, &json!({"id":"w-q"})).await] { assert!(!ok, "{r}"); assert!(r["error"].as_str().unwrap().contains("off on a copy"), "{r}"); }
    assert_eq!(rpc(&hub, "lock.release", json!({"thread":T3,"name":"b"})).await["ok"], false);
    assert_eq!(len(&hub).await, n); assert_eq!(ag.tool_lock_list().await.1["locks"].as_array().unwrap().len(), 2, "the record can be read");
    assert!(!h.join("bp-b.lock").exists() && ag.guards.lock().unwrap().is_empty(), "no flock is taken"); ag.shutdown();
}
#[test] fn a_hub_on_a_copy_keeps_its_hands_off_the_locks() {
    let me = std::env::current_exe().unwrap();
    let o = std::process::Command::new(me).args(["agents::tests::locks_copy_child_probe", "--exact", "--nocapture", "--test-threads=1"]).env("BACKPLANE_COPY", "1").env("BP_LOCK_COPY_PROBE", "1").env_remove("BACKPLANE_AGENTS").output().unwrap();
    assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)); assert!(String::from_utf8_lossy(&o.stdout).contains("locks_copy_child_probe ... ok"));
}

// ---- watched work: copy, restart, bounds; wake_ride without a process ----
fn watch_log(work: &Path, rows: &[String], extra: &str) -> String { base_log(work, 1, &format!("{{\"$\":\"WorkSet\",\"thread\":\"{T1}\",\"kind\":\"watch\",\"rows\":{},\"at\":1700000005}}\n{extra}", serde_json::to_string(&rows.join("\n")).unwrap())) }
#[tokio::test(flavor = "multi_thread")] async fn work_copy_child_probe() {
    if std::env::var("BP_WORK_COPY_PROBE").is_err() { return }
    assert!(crate::copy::on());
    let sd = dir("wcp-bin"); perf_standin(&sd, 1); let f = sd.join("never.txt");
    let rows = vec![work::row("w-open-run", "watching", "a build", "run", "sleep 99", "123:456", 0), work::row("w-open-pid", "watching", "a pid", "pid", "999999", "-", 0), work::row("w-open-file", "watching", "a file", "file", &f.to_string_lossy(), "w-open-file", 0)];
    let (hub, ag, _h, _w) = start("wcp", |w| watch_log(w, &rows, ""), |_| Cli::standin(&sd)).await; let n = len(&hub).await;
    tokio::time::sleep(Duration::from_millis(3000)).await;
    assert_eq!(len(&hub).await, n, "nothing is written on a copy: no turn, no message, no row ended"); assert!(ag.works.lock().unwrap().is_empty(), "nothing is polled");
    std::fs::write(&f, "w-open-file").unwrap(); tokio::time::sleep(Duration::from_millis(2500)).await; assert_eq!(len(&hub).await, n, "a file that appears is not noticed");
    for (ok, r) in [ag.tool_work_run(T1, &json!({"command":"echo x"})).await, ag.tool_work_watch(T1, &json!({"kind":"file","target":"/tmp/x"})).await] { assert!(!ok); assert!(r["error"].as_str().unwrap().contains("off on a copy"), "{r}"); }
    assert_eq!(len(&hub).await, n, "a refused tool logs no row"); ag.shutdown();
}
#[test] fn a_hub_on_a_copy_rearms_no_work_and_the_tools_are_refused() {
    let me = std::env::current_exe().unwrap();
    let o = std::process::Command::new(me).args(["agents::tests::work_copy_child_probe", "--exact", "--nocapture", "--test-threads=1"]).env("BACKPLANE_COPY", "1").env("BP_WORK_COPY_PROBE", "1").env_remove("BACKPLANE_AGENTS").output().unwrap();
    assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)); assert!(String::from_utf8_lossy(&o.stdout).contains("work_copy_child_probe ... ok"));
}
#[tokio::test(flavor = "multi_thread")] async fn a_command_that_outlived_the_hub_is_watched_again_and_cancel_reaches_it_and_a_gone_one_ends_unknown() {
    let mut c = std::process::Command::new("sleep"); c.arg("60"); crate::plat::proc::own_group(&mut c); let mut child = c.spawn().unwrap(); let pid = child.id(); crate::plat::proc::adopt(pid, false);
    let live = work::row("w-alive", "watching", "still going", "run", "sleep 60", &work::ident_of(pid), 0); let gone = work::row("w-gone", "watching", "went away", "run", "x", "4000000:1", 0);
    let sd = dir("rearm-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("rearm", |w| watch_log(w, &[live.clone(), gone.clone()], ""), |_| Cli::standin(&sd)).await; let from = 0;
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("\"went away\" ended: it ended, outcome unknown")))).await; assert!(ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("\"went away\""))), "{ev:?}");
    assert!(!ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("still going"))), "a command that is still running is not told as lost");
    assert!(ag.works.lock().unwrap().contains_key("w-alive"), "watched again"); let (ok, c) = ag.tool_work_cancel(T1, &json!({"id":"w-alive"})).await; assert!(ok, "{c}");
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("\"still going\" ended: cancelled")))).await; assert!(ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("cancelled"))));
    let end = Instant::now() + Duration::from_secs(8); while child.try_wait().unwrap().is_none() && Instant::now() < end { tokio::time::sleep(Duration::from_millis(100)).await } assert!(child.try_wait().unwrap().is_some(), "the cancel reached the command's group"); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn work_ids_carry_the_time_output_is_read_by_its_end_and_lossy_and_a_non_utf8_file_is_found() {
    let (hub, ag, sd) = codex_start("wb1").await; let from = len(&hub).await;
    let (_, a) = ag.tool_work_run(T1, &json!({"command":"printf '\\377\\376 bad bytes\\n'; head -c 3000000 /dev/zero | tr '\\0' x; echo ' the end'","note":"loud"})).await; let id = a["id"].as_str().unwrap().to_string();
    let parts: Vec<&str> = id.rsplitn(3, '-').collect(); assert!(parts[1].len() >= 12 && parts[1].chars().all(|c| c.is_ascii_digit()), "w-<thread>-<ms>-<n>: {id}");
    let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("\"loud\" ended: exit 0")))).await;
    let m = ev.iter().find(|e| e["text"].as_str().map_or(false, |t| t.contains("\"loud\" ended"))).unwrap()["text"].as_str().unwrap().to_string(); assert!(m.contains("the end") && m.len() < 2500, "the end of the output only: {}", m.len());
    let f = sd.join("bin.dat"); let (_, w) = ag.tool_work_watch(T1, &json!({"kind":"file","target":f.to_string_lossy(),"note":"binary"})).await; let tok = w["token"].as_str().unwrap().to_string();
    let mut bytes = vec![0xffu8, 0xfe, 0x00]; bytes.extend(tok.as_bytes()); std::fs::write(&f, bytes).unwrap();
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("\"binary\" ended: found")))).await; assert!(ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("ended: found"))), "a file that is not UTF-8 is found"); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn wake_ride_keeps_the_note_when_the_turn_says_running_but_no_process_lives() { // after a restart
    let sd = dir("wrn-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("wrn", |w| running_log(w, "{\"$\":\"SettingSet\",\"key\":\"restart.continue\",\"value\":\"off\",\"at\":1700000010}\n"), |_| Cli::standin(&sd)).await;
    // recovery marked it interrupted; make the world read it as running again
    hub.commit(&json!({"$":"TurnChanged","thread":T1,"state":"running","at":1700000011})).await.unwrap();
    assert!(ag.wake_ride(T1, "n1", "a note").await.is_ok());
    let ev = log_from(&hub, 0).await; assert!(ev.iter().any(|e| is(e, "SettingSet") && e["key"] == format!("ride.{T1}") && e["value"].as_str().map_or(false, |v| v.contains("a note"))), "the ride holds the note");
    assert!(!ev.iter().any(|e| e["kind"] == "ride" && e["summary"].as_str().map_or(false, |s| s.starts_with("Delivered into the running turn"))), "nothing was said to be delivered"); ag.shutdown();
}
// ---- side-by-side mode (docs/SIDE-BY-SIDE.md): BACKPLANE_COPY=1 and BACKPLANE_SIDE_BY_SIDE=1 are read once per process, so the probe runs in a child ----
#[tokio::test(flavor = "multi_thread")] async fn sbs_child_probe() {
    if std::env::var("BP_SBS_PROBE").is_err() { return }
    assert!(crate::copy::on() && crate::copy::person_turn_allowed());
    let person = PathBuf::from(std::env::var("BP_SBS_PERSON").unwrap()); let live = PathBuf::from(std::env::var("BP_SBS_LIVE").unwrap());
    let rec = person.join("rec"); let _ = std::fs::create_dir_all(&rec);
    let bin = person.join(".local/bin"); std::fs::create_dir_all(&bin).unwrap();
    let wrapped = STANDIN.replacen("\n", &format!("\nexport FAKE_PER=2 FAKE_CYCLE=5 FAKE_SECS=1 FAKE_LOG={rec}/starts\nprintf '%s\\n' \"$@\" > {rec}/argv; env > {rec}/env; pwd > {rec}/cwd\n", rec = crate::plat::fs::script_path(&rec)), 1);
    script(&bin, "claude", &wrapped);
    let sd = dir("sbs-bin"); perf_standin(&sd, 1); // the hub's own stand-in: a PersonTurn must never use it, nor an Agent turn the person's
    let rows = vec![work::row("w-open-run", "watching", "a build", "run", "sleep 99", "123:456", 0), work::row("w-open-pid", "watching", "a pid", "pid", "999999", "-", 0)];
    let old_ride = format!("{{\"$\":\"SettingSet\",\"key\":\"ride.{T1}\",\"value\":\"copied mail\",\"at\":1700000006}}\n");
    let (hub, ag, _h, work) = start("sbs", |w| watch_log(w, &rows, &old_ride), |_| Cli::standin(&sd)).await; let n0 = len(&hub).await;
    tokio::time::sleep(Duration::from_millis(2500)).await; assert_eq!(len(&hub).await, n0, "side-by-side re-arms no watched work and ends no row"); assert!(ag.works.lock().unwrap().is_empty(), "nothing is polled");
    let read = |n: &str| std::fs::read_to_string(rec.join(n)).unwrap_or_default();
    assert_eq!(ag.world().await.setting(&format!("ride.{T1}")), "", "mail copied from the real log is dropped at start");
    // (1) not the person's turn.start: waits in the ride, no process
    let r = ag.deliver(T1, "mx", "hello from a bot", Some(("bot".into(), "b".into(), "b".into())), vec![]).await; assert!(r.is_ok(), "{r:?}");
    tokio::time::sleep(Duration::from_millis(500)).await; assert_eq!(ag.stats.procs_spawned.load(Relaxed), 0, "only a person's turn.start starts an agent"); assert!(read("argv").is_empty());
    // (2) the person's turn: refused while the live log shows a turn in the same folder
    std::fs::write(&live, format!("{{\"$\":\"ProjectCreated\",\"id\":\"lp\",\"title\":\"x\",\"root\":{},\"at\":1}}\n{{\"$\":\"ThreadCreated\",\"id\":\"lt\",\"project\":\"lp\",\"title\":\"x\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":2}}\n{{\"$\":\"TurnChanged\",\"thread\":\"lt\",\"state\":\"running\",\"at\":3}}\n", serde_json::to_string(&work.to_string_lossy()).unwrap())).unwrap();
    let from = len(&hub).await; rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort hi"})).await;
    let ev = until(&hub, from, 10, |ev| ev.iter().any(|e| e["kind"] == "refused")).await;
    assert!(ev.iter().any(|e| e["kind"] == "refused" && e["summary"].as_str().unwrap_or("").contains("live Backplane")), "refused with its reason: {ev:?}");
    assert_eq!(ag.stats.procs_spawned.load(Relaxed), 0);
    // (3) the live turn ended: the person's turn runs the person's claude with the guards
    std::fs::write(&live, format!("{{\"$\":\"TurnChanged\",\"thread\":\"lt\",\"state\":\"completed\",\"at\":4}}\n")).unwrap();
    let from = len(&hub).await; rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort hi"})).await;
    let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await; assert!(ev.iter().any(|e| turn_is(e, "completed")), "{ev:?}");
    // (3b) a bot delivery during the person's turn waits; a Backplane tool result does not carry it into the running process
    let from = len(&hub).await; rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort again"})).await;
    let _ = ag.deliver(T1, "mb", "mail during the turn", Some(("bot".into(), "b".into(), "b".into())), vec![]).await;
    assert!(ag.ride_take(T1).await.is_none(), "a tool result carries no mail into a person's running turn"); let end = Instant::now() + Duration::from_secs(10); loop { if ag.world().await.setting(&format!("ride.{T1}")).contains("mail during the turn") { break } assert!(Instant::now() < end, "it waits in the ride"); tokio::time::sleep(Duration::from_millis(50)).await }
    until(&hub, from, 30, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
    let argv: Vec<String> = read("argv").lines().map(String::from).collect();
    for f in ["--strict-mcp-config", "--setting-sources", "project,local"] { assert!(argv.iter().any(|a| a == f), "{f} in {argv:?}") }
    assert!(!argv.iter().any(|a| a == "--resume"), "no copied session is resumed: {argv:?}");
    let pm = argv.iter().position(|a| a == "--permission-mode").map(|i| argv[i + 1].clone()).unwrap(); assert_eq!(pm, "default", "approval-required, whatever the copied thread said");
    assert!(argv.iter().any(|a| a == "--permission-prompt-tool"), "approvals reach the person: {argv:?}");
    let env = read("env"); assert!(env.lines().any(|l| l == format!("HOME={}", person.display())), "the person's HOME"); assert!(!env.lines().any(|l| l.starts_with("BACKPLANE_")), "no BACKPLANE_* reaches the agent:\n{env}");
    assert_eq!(crate::plat::fs::canonicalize(read("cwd").trim()).unwrap(), crate::plat::fs::canonicalize(&work).unwrap());
    assert_eq!(std::fs::read_to_string(rec.join("starts")).unwrap().matches("START").count(), 1, "exactly the person's stand-in ran");
    // (4) the one-shot helpers keep Kind::Agent, refused on a copy (proc.rs tests)
    ag.shutdown();
}
#[test] fn side_by_side_runs_only_the_persons_turn_with_the_persons_agent_and_guards() {
    // the child gets a clean PATH below: `person_dirs` puts the hub's own PATH first, so a real `claude` on the machine's PATH (~/.local/bin) would be run instead of the person's stand-in
    let me = std::env::current_exe().unwrap(); let person = dir("sbs-person"); let live = dir("sbs-live").join("events.jsonl"); std::fs::write(&live, "{\"$\":\"ProjectCreated\",\"id\":\"z\",\"title\":\"z\",\"root\":\"/nonexistent-live\",\"at\":1}\n").unwrap();
    let o = std::process::Command::new(me).args(["agents::tests::sbs_child_probe", "--exact", "--nocapture", "--test-threads=1"]).env("PATH", "/usr/bin:/bin").env("BACKPLANE_COPY", "1").env("BACKPLANE_SIDE_BY_SIDE", "1").env("BACKPLANE_PERSON_HOME", &person).env("BACKPLANE_LIVE_LOG", &live).env("BP_SBS_PROBE", "1").env("BP_SBS_PERSON", &person).env("BP_SBS_LIVE", &live).env_remove("BACKPLANE_AGENTS").output().unwrap();
    assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stdout).contains("sbs_child_probe ... ok"), "the probe ran: {}", String::from_utf8_lossy(&o.stdout));
}
#[test] fn kind_person_turn_is_named_only_where_the_person_spawn_lives() {
    fn walk(root: &Path, d: &Path, hits: &mut Vec<String>) { for e in std::fs::read_dir(d).unwrap().flatten() { let p = e.path(); if p.is_dir() { walk(root, &p, hits) } else if p.extension().map_or(false, |x| x == "rs") && !p.ends_with("tests.rs") { if std::fs::read_to_string(&p).unwrap().lines().any(|l| !l.trim_start().starts_with("//") && l.contains("Kind::PersonTurn")) { hits.push(p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/")) } } } }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src"); let mut hits = vec![]; walk(&src, &src, &mut hits); hits.sort();
    assert_eq!(hits, ["agents/proc.rs", "proc.rs"], "only the two proc.rs files (core and agents) name Kind::PersonTurn: {hits:?}");
}
#[tokio::test(flavor = "multi_thread")] async fn a_thread_made_for_a_worktree_never_starts_in_the_project_folder() {
    let sd = dir("wtw-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("wtw", |w| base_log(w, 1, "").replace("\"branch\":\"\"", "\"branch\":\"imp-1\"").replace("\"env\":\"local\"", "\"env\":\"worktree\""), |_| Cli::standin(&sd)).await;
    let _ = ag; // a short wait would pass the 60 s: the thread gets its worktree first
    let from = len(&hub).await; let work2 = dir("wtw-wt");
    let h2 = hub.clone(); let t = tokio::spawn(async move { rpc(&h2, "turn.start", json!({"thread":T1,"text":"zqshort hi"})).await });
    tokio::time::sleep(Duration::from_millis(600)).await; assert_eq!(ag.stats.procs_spawned.load(Relaxed), 0, "nothing starts while the worktree is missing");
    hub.commit(&json!({"$":"WorktreeSet","id":T1,"worktree":work2.to_string_lossy(),"at":1700000200})).await.unwrap();
    let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await; assert!(ev.iter().any(|e| turn_is(e, "completed")), "{ev:?}"); let _ = t.await; ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_model_mention_is_logged_as_refs_and_told_to_the_agent_without_changing_the_thread() {
    let sd = dir("mn-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("mn", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort ask &claude/nonesuch-model about it"})).await;
    let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
    let posted = ev.iter().find(|e| is(e, "MessagePosted") && e["role"] == "user").expect("posted");
    assert_eq!(posted["refs"][0]["word"], "&claude/nonesuch-model", "{posted}"); assert_eq!(posted["refs"][0]["state"], "unknown"); assert_eq!(posted["text"], "zqshort ask &claude/nonesuch-model about it", "the text is the person's own");
    assert!(!ev.iter().any(|e| is(e, "ThreadProviderSet") || is(e, "ThreadModelSet")), "the thread's model is untouched");
    { let w = ag.world().await; assert_eq!(w.threads[T1].model, ""); }
    let from = len(&hub).await; rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort plain"})).await;
    let ev = until(&hub, from, 30, |ev| ev.iter().filter(|e| turn_is(e, "completed")).count() >= 1).await; assert!(ev.iter().find(|e| is(e, "MessagePosted") && e["role"] == "user").unwrap().get("refs").is_none(), "omitted when empty"); ag.shutdown();
}
// ---- side-by-side: a routine run from the client reaches no real agent (it waits in the bot thread's ride) ----
#[tokio::test(flavor = "multi_thread")] async fn sbs_routine_probe() {
    if std::env::var("BP_SBS_ROUTINE").is_err() { return }
    assert!(crate::copy::on() && crate::copy::person_turn_allowed());
    let person = PathBuf::from(std::env::var("BP_SBS_PERSON").unwrap()); let rec = person.join("rec"); std::fs::create_dir_all(&rec).unwrap();
    let bin = person.join(".local/bin"); std::fs::create_dir_all(&bin).unwrap();
    script(&bin, "claude", &STANDIN.replacen("\n", &format!("\nexport FAKE_PER=2 FAKE_CYCLE=5 FAKE_SECS=1 FAKE_LOG={rec}/starts\n", rec = crate::plat::fs::script_path(&rec)), 1));
    let sd = dir("sbr-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("sbr", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await; let _bots = crate::bots::start(hub.clone(), ag.clone()).await;
    let r = rpc(&hub, "bots.create", json!({"name":"ada"})).await; let bot = r["bot"].as_str().expect("a bot").to_string();
    let th = until(&hub, 0, 20, |ev| ev.iter().any(|e| is(e, "BotSet") && e["id"] == bot.as_str())).await.iter().find(|e| is(e, "BotSet") && e["id"] == bot.as_str()).unwrap()["thread"].as_str().unwrap().to_string();
    let r = rpc(&hub, "bots.routine", json!({"bot":bot,"routine":"ro1","name":"morning","schedule":"daily 09:30","prompt":"do the routine thing"})).await; assert_eq!(r["ok"], true, "{r}");
    let r = rpc(&hub, "bots.routine_run", json!({"routine":"ro1"})).await; assert_eq!(r["ok"], true, "{r}");
    let end = Instant::now() + Duration::from_secs(15); loop { if ag.world().await.setting(&format!("ride.{th}")).contains("do the routine thing") { break } assert!(Instant::now() < end, "the routine's turn waits in the bot thread's ride"); tokio::time::sleep(Duration::from_millis(100)).await }
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(ag.stats.procs_spawned.load(Relaxed), 0, "a routine started no agent"); assert!(!rec.join("starts").exists(), "the person's claude never ran");
    ag.shutdown();
}
#[test] fn a_routine_run_in_side_by_side_reaches_no_real_agent() {
    let me = std::env::current_exe().unwrap(); let person = dir("sbr-person"); let live = dir("sbr-live").join("events.jsonl"); std::fs::write(&live, "{\"$\":\"ProjectCreated\",\"id\":\"z\",\"title\":\"z\",\"root\":\"/nonexistent-live\",\"at\":1}\n").unwrap();
    let o = std::process::Command::new(me).args(["agents::tests::sbs_routine_probe", "--exact", "--nocapture", "--test-threads=1"]).env("BACKPLANE_COPY", "1").env("BACKPLANE_SIDE_BY_SIDE", "1").env("BACKPLANE_PERSON_HOME", &person).env("BACKPLANE_LIVE_LOG", &live).env("BP_SBS_ROUTINE", "1").env("BP_SBS_PERSON", &person).env_remove("BACKPLANE_AGENTS").output().unwrap();
    assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stdout).contains("sbs_routine_probe ... ok"), "the probe ran: {}", String::from_utf8_lossy(&o.stdout));
}
#[tokio::test(flavor = "multi_thread")] async fn a_turns_text_starts_with_the_standing_rules_and_ends_with_what_it_references() {
    let sd = dir("rl-bin"); let rec = sd.join("in.jsonl"); let inner = sd.join("standin.sh"); std::fs::write(&inner, STANDIN.replacen("\n", "\nexport FAKE_PER=2 FAKE_CYCLE=5 FAKE_SECS=1 FAKE_LOG=/dev/null\n", 1)).unwrap();
    script(&sd, "claude", &format!("#!/bin/sh\ntee -a {} | sh {}\n", crate::plat::fs::script_path(&rec), crate::plat::fs::script_path(&inner)));
    let rule = r#"{"id":"r1","text":"Never push to main","tool":"Bash","enforce":true,"word":"ship it","at":5}"#;
    let (hub, ag, _h, _w) = start("rl", |w| base_log(w, 1, &format!("{}\n", json!({"$":"SettingSet","key":"rule.r1","value":rule,"at":1700000005}))), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort hello &claude/nonesuch"})).await;
    until(&hub, from, 30, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
    let got = std::fs::read_to_string(&rec).unwrap(); let line = got.lines().find(|l| l.contains("zqshort hello")).expect("the user line"); let v: Value = serde_json::from_str(line).unwrap();
    let text = v["message"]["content"].as_str().map(str::to_string).or_else(|| v["message"]["content"][0]["text"].as_str().map(str::to_string)).expect("text");
    assert!(text.starts_with("<rules>\nStanding rules from the person. Follow them; the enforced ones are checked by Backplane.\n- Never push to main (enforced: Bash is refused unless the person's latest message says \"ship it\")\n</rules>\n\nzqshort hello &claude/nonesuch"), "{text}");
    assert!(text.contains("\n\n<references>\nmodel claude/nonesuch: ") && text.ends_with("</references>"), "{text}");
    ag.shutdown();
}
async fn a_dollar_skill_of_the_persons_own_reaches_the_agent_as_a_slash_command_or_a_file_note() {
    let sd = dir("sk-bin"); let rec = sd.join("in.jsonl"); let inner = sd.join("standin.sh"); std::fs::write(&inner, STANDIN.replacen("\n", "\nexport FAKE_PER=2 FAKE_CYCLE=5 FAKE_SECS=1 FAKE_LOG=/dev/null\n", 1)).unwrap();
    script(&sd, "claude", &format!("#!/bin/sh\ntee -a {} | sh {}\n", crate::plat::fs::script_path(&rec), crate::plat::fs::script_path(&inner)));
    let uh = dir("sk-userhome"); // a fake user: a Claude skill, a Codex-only skill
    for (rel, name) in [(".claude/skills/unslop-zq", "unslop-zq"), (".agents/skills/codexonly-zq", "codexonly-zq")] { std::fs::create_dir_all(uh.join(rel)).unwrap(); std::fs::write(uh.join(rel).join("SKILL.md"), format!("---\nname: {name}\ndescription: d\n---\nbody")).unwrap(); }
    let home = dir("sk-home"); let work = dir("sk-work"); std::fs::write(home.join("events.jsonl"), base_log(&work, 1, "")).unwrap();
    let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let ag = Agents::start_host(hub.clone(), Cli::standin(&sd), host::HostInfo { user_home: uh.to_string_lossy().into_owned(), home: home.clone(), ..Default::default() }).await.unwrap(); let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqskill run $unslop-zq and $codexonly-zq"})).await;
    until(&hub, from, 30, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
    let got = std::fs::read_to_string(&rec).unwrap(); let line = got.lines().find(|l| l.contains("zqskill")).expect("the user line"); let v: Value = serde_json::from_str(line).unwrap();
    let text = v["message"]["content"].as_str().map(str::to_string).or_else(|| v["message"]["content"][0]["text"].as_str().map(str::to_string)).expect("text");
    assert!(text.contains("zqskill run /unslop-zq and $codexonly-zq"), "{text}");
    assert!(text.contains(&format!("read \"{}/.agents/skills/codexonly-zq/SKILL.md\"", uh.display())) && !text.contains("unslop-zq/SKILL.md"), "{text}");
    ag.shutdown();
}
// one test, not two: two hubs of the host kind at once disturb the process-wide account grants other tests hold
#[tokio::test(flavor = "multi_thread")] async fn the_reply_depends_on_what_the_picked_skill_says_for_claude_and_codex() {
    a_dollar_skill_of_the_persons_own_reaches_the_agent_as_a_slash_command_or_a_file_note().await;
    // stand-ins that do what the real agents do with the note: open the SKILL.md named in it and answer with its secret word
    // (\\* and not \\?: macOS's sed has no \? in a basic expression, and the stand-in then never found the file)
    let find = r#"p=$(printf '%s' "$X" | sed -n 's/.*read \\*"\([^"\\]*SKILL.md\)\\*".*/\1/p' | head -1); w=$(sed -n 's/^SECRET=//p' "$p" 2>/dev/null | head -1)"#;
    let uh = dir("sc-userhome"); // the skill sits where the other provider's agent does not load it itself, so the turn text must hand over the file
    for rel in [".agents/skills/zqwhale", ".claude/skills/zqwhale"] { let d = uh.join(rel); std::fs::create_dir_all(&d).unwrap(); std::fs::write(d.join("SKILL.md"), "---\nname: zqwhale\ndescription: d\n---\nSECRET=PURPLE-OTTER-77\n").unwrap(); }
    std::fs::remove_dir_all(uh.join(".claude/skills/zqwhale")).unwrap(); // placeholder: each provider below gets only the foreign copy
    for provider in ["claude", "codex"] {
        let (keep, drop) = if provider == "claude" { (".agents/skills/zqwhale", ".claude/skills/zqwhale") } else { (".claude/skills/zqwhale", ".agents/skills/zqwhale") };
        let _ = std::fs::remove_dir_all(uh.join(drop)); std::fs::create_dir_all(uh.join(keep)).unwrap(); std::fs::write(uh.join(keep).join("SKILL.md"), "---\nname: zqwhale\ndescription: d\n---\nSECRET=PURPLE-OTTER-77\n").unwrap();
        let sd = dir(&format!("sc-bin-{provider}"));
        if provider == "claude" { script(&sd, "claude", &format!("#!/bin/sh\nwhile IFS= read -r X; do case \"$X\" in *'\"type\":\"user\"'*) {find}; printf '%s\\n' '{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s1\",\"cwd\":\"/tmp\",\"tools\":[],\"model\":\"fake\",\"permissionMode\":\"default\"}}'; printf '%s\\n' '{{\"type\":\"assistant\",\"message\":{{\"id\":\"m1\",\"role\":\"assistant\",\"content\":[{{\"type\":\"text\",\"text\":\"The skill says '\"$w\"'\"}}],\"usage\":{{\"input_tokens\":1}}}},\"session_id\":\"s1\"}}'; printf '%s\\n' '{{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"done\",\"session_id\":\"s1\"}}';; esac; done\n")) }
        else { script(&sd, "codex", &format!("#!/bin/sh\ncase \"$1\" in --version) echo codex-cli 0.0.0; exit 0;; esac\nfor a in \"$@\"; do X=\"$a\"; done\n{find}\necho '{{\"type\":\"thread.started\",\"thread_id\":\"cs-1\"}}'\nprintf '{{\"type\":\"item.completed\",\"item\":{{\"id\":\"i\",\"type\":\"agent_message\",\"text\":\"The skill says %s\"}}}}\\n' \"$w\"\necho '{{\"type\":\"turn.completed\",\"usage\":{{\"input_tokens\":1,\"cached_input_tokens\":0,\"output_tokens\":1}}}}'\n")) }
        let home = dir(&format!("sc-home-{provider}")); let work = dir(&format!("sc-work-{provider}"));
        let log = if provider == "codex" { codex_log(&work) } else { base_log(&work, 1, "") }; std::fs::write(home.join("events.jsonl"), log).unwrap();
        let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let ag = Agents::start_host(hub.clone(), Cli::standin(&sd), host::HostInfo { user_home: uh.to_string_lossy().into_owned(), home: home.clone(), ..Default::default() }).await.unwrap(); let from = len(&hub).await;
        rpc(&hub, "turn.start", json!({"thread":T1,"text":"use $zqwhale please"})).await;
        let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
        assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "assistant" && e["text"].as_str().map_or(false, |t| t.contains("PURPLE-OTTER-77"))), "{provider}: the reply must carry the skill's secret: {ev:?}");
        ag.shutdown();
    }
}
#[tokio::test(flavor = "multi_thread")] async fn an_enforced_rule_refuses_a_matching_tool_call_until_the_person_says_its_word() {
    let sd = dir("gt-bin"); perf_standin(&sd, 1);
    let rules = [r#"{"id":"r1","text":"No lists without asking","tool":"thread_list","enforce":true,"word":"list it","at":1}"#, r#"{"id":"r2","text":"No rm","tool":"Bash","enforce":true,"at":2}"#];
    let extra = format!("{}\n{}\n{{\"$\":\"MessagePosted\",\"thread\":\"{T1}\",\"msg\":\"m1\",\"role\":\"user\",\"text\":\"hello there\",\"at\":1700000100}}\n", json!({"$":"SettingSet","key":"rule.r1","value":rules[0],"at":1700000005}), json!({"$":"SettingSet","key":"rule.r2","value":rules[1],"at":1700000006}));
    let (hub, ag, _h, _w) = start("gt", |w| base_log(w, 1, &extra), |_| Cli::standin(&sd)).await;
    let wait = Instant::now() + Duration::from_secs(5); while ag.world().await.setting("rule.r1").is_empty() && Instant::now() < wait { tokio::time::sleep(Duration::from_millis(50)).await }
    let (_, r) = rpc_mcp(&ag, T1, "", "tools/call", json!({"name":"thread_list","arguments":{}})).await;
    assert_eq!(r["result"]["isError"], true, "{r}"); assert!(r["result"]["content"][0]["text"].as_str().unwrap().starts_with("Refused by a standing rule (r1): \"No lists without asking\". thread_list needs the person's word \"list it\""), "{r}");
    let (_, p) = rpc_mcp(&ag, T1, "", "tools/call", json!({"name":"permission_prompt","arguments":{"tool_name":"Bash","input":{"command":"rm x"}}})).await;
    let v: Value = serde_json::from_str(p["result"]["content"][0]["text"].as_str().unwrap()).unwrap(); assert_eq!(v["behavior"], "deny", "{p}"); assert!(v["message"].as_str().unwrap().contains("No rm"), "{p}");
    assert!(!log_from(&hub, 0).await.iter().any(|e| is(e, "AskOpened")), "a refused permission asks no one");
    hub.commit(&json!({"$":"MessagePosted","thread":T1,"msg":"m2","role":"user","text":"OK, list it please","at":1700000200})).await.unwrap();
    let (_, r) = rpc_mcp(&ag, T1, "", "tools/call", json!({"name":"thread_list","arguments":{}})).await; assert_ne!(r["result"]["isError"], true, "the person's word unlocks it: {r}");
    ag.shutdown();
}
#[test] fn slugs_and_reference_words_are() {
    use refs::{slug_of, slug_take};
    assert_eq!(slug_of("Fix the perf lag!"), "fix-the-perf-lag"); assert_eq!(slug_of("  --Ünï cödé 2--"), "n-c-d-2"); assert_eq!(slug_of(&"ab ".repeat(30)), "ab-ab-ab-ab-ab-ab-ab-ab-ab-ab-ab-ab-ab-ab"[..40].trim_end_matches('-'));
    assert_eq!(slug_take("fix-it, now"), "fix-it"); assert_eq!(slug_take("a_b:c"), "a_b"); assert_eq!(slug_take(".x"), "");
}
#[tokio::test(flavor = "multi_thread")] async fn the_references_in_a_message_are_named_to_the_agent_once_each() {
    let sd = dir("rf-bin"); let rec = sd.join("in.jsonl"); let inner = sd.join("standin.sh"); std::fs::write(&inner, STANDIN.replacen("\n", "\nexport FAKE_PER=2 FAKE_CYCLE=5 FAKE_SECS=1 FAKE_LOG=/dev/null\n", 1)).unwrap();
    script(&sd, "claude", &format!("#!/bin/sh\ntee -a {} | sh {}\n", crate::plat::fs::script_path(&rec), crate::plat::fs::script_path(&inner)));
    let extra = [json!({"$":"ProjectCreated","id":"p2","title":"Radio board","root":"/r/radio","at":1700000002}), json!({"$":"ThreadCreated","id":"t1700000009-7","project":"p2","title":"Fix the perf lag","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1700000003}),
        json!({"$":"SettingSet","key":"summary.t1700000009-7","value":"Profiling the render loop","at":1700000004}), json!({"$":"BotSet","id":"b1","name":"Ada","thread":"t1700000050-9","at":1700000005}), json!({"$":"RoomSet","id":"r1","name":"Ops Room","members":"b1","at":1700000006})];
    let extra = extra.iter().map(|e| e.to_string() + "\n").collect::<String>();
    let (hub, _ag, _h, _w) = start("rf", |w| base_log(w, 1, &extra), |_| Cli::standin(&sd)).await; let from = len(&hub).await;
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqshort see >fix-the-perf-lag and %radio-board, ask @ada in #ops-room, again >fix-the-perf-lag, @nobody, >elsewhere:thing"})).await;
    until(&hub, from, 30, |ev| ev.iter().any(|e| turn_is(e, "completed"))).await;
    let got = std::fs::read_to_string(&rec).unwrap(); let line = got.lines().find(|l| l.contains("zqshort see")).unwrap(); let v: Value = serde_json::from_str(line).unwrap();
    let text = v["message"]["content"].as_str().map(str::to_string).or_else(|| v["message"]["content"][0]["text"].as_str().map(str::to_string)).unwrap();
    let want = "\n\n<references>\n>fix-the-perf-lag is the thread \"Fix the perf lag\" (id t1700000009-7, project \"Radio board\"): Profiling the render loop. Read it with thread_read.\n%radio-board is the project \"Radio board\" (id p2, folder /r/radio).\n@Ada is the bot \"Ada\"; it has been sent this message and may answer in this thread.\n#ops-room is the group chat \"Ops Room\"; this message was posted there.\n</references>";
    assert!(text.ends_with(want), "{text}");
}
#[cfg(test)] mod lost;
#[cfg(test)] mod note;
#[cfg(test)] mod order;
#[cfg(test)] mod checkpoint;

#[tokio::test(flavor = "multi_thread")] async fn viewer_show_answers_with_what_the_viewer_then_shows() {
    let sd = dir("vs-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("vs", |w| base_log(w, 2, ""), |_| Cli::standin(&sd)).await;
    // a client that reports in time: the answer carries its state, settled. It reports when the hub's `view` frame reaches it, not after a time
    let mut client = hub.attach(hub.fresh_client());
    let late = tokio::spawn(async move { while let Some(o) = client.recv().await { if let crate::hub::Out::Value(f) = o { if f["t"] == "view" && f["thread"] == T1 { break } } } crate::files::kicad::watch::viewer_report(T1, json!({"open": "mech", "part": "mech/a.step", "selection": null})); });
    let (_, r) = rpc_mcp(&ag, T1, "", "tools/call", json!({"name":"viewer_show","arguments":{"kind":"mech","path":"mech/a.step"}})).await; late.await.unwrap();
    let t = tool_text(&r); assert_eq!(t["settled"], true); assert_eq!(t["viewer"]["part"], "mech/a.step"); assert_eq!(t["shown"], "mech/a.step");
    // a client that says nothing: the last report, marked unsettled
    let (_, r) = rpc_mcp(&ag, T1, "", "tools/call", json!({"name":"viewer_show","arguments":{"kind":"board","path":""}})).await;
    let t = tool_text(&r); assert_eq!(t["settled"], false); assert_eq!(t["viewer"]["open"], "mech");
}

// ---- Settle: the thread's turn and everything it owns stop, and nothing starts it again ----
async fn tool_of(ag: &Arc<Agents>, name: &str) -> Arc<dyn mcp::ToolSet> { ag.tool_sets().into_iter().find(|s| s.owns(name)).expect("tool set") }
#[tokio::test(flavor = "multi_thread")] async fn settling_stops_the_turn_and_all_the_work_and_nothing_starts_it_again() {
    let sd = dir("st-bin"); perf_standin(&sd, 30);
    let (hub, ag, _h, _w) = start("st", |w| base_log(w, 2, ""), |_| Cli::standin(&sd)).await; let other = "t1700001001-1";
    crate::commands::start_with(hub.clone(), Some(Arc::new(ag.clone()) as Arc<dyn crate::commands::seam::Seam>));
    rpc(&hub, "turn.start", json!({"thread":T1,"text":"zqlong"})).await;
    until(&hub, 0, 60, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "assistant")).await;
    assert!(ag.world().await.running(T1), "a turn is running");
    // what the thread owns: watched work, a lock, a wake, a launched thread with a turn of its own, a delegated-style task row
    let (_, w) = ag.tool_work_run(T1, &json!({"command":"sleep 60","note":"slow"})).await; let wid = w["id"].as_str().unwrap().to_string();
    assert!(ag.tool_lock_acquire(T1, &json!({"name":"bench"})).await.0);
    let wakes = tool_of(&ag, "wake_on").await; assert!(wakes.call(T1, "wake_on", &json!({"kind":"thread","target":other})).await.0);
    let (ok, l) = ag.tool_launch(T1, &json!({"prompt":"zqlong child"})).await; assert!(ok, "{l}"); let child = l["threadId"].as_str().or(l["childThreadId"].as_str()).unwrap().to_string();
    tokio::time::sleep(Duration::from_secs(3)).await; // the child gets a moment to run
    let from = len(&hub).await;
    assert_eq!(rpc(&hub, "thread.settle", json!({"thread":T1})).await["ok"], true);
    let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["text"].as_str().map_or(false, |t| t.contains("\"slow\" ended: cancelled")))).await;
    assert!(ev.iter().any(|e| is(e, "ThreadSettled") && e["id"] == T1 && e["manual"] == true));
    assert!(ev.iter().any(|e| turn_is(e, "interrupted") && e["thread"] == T1), "the running turn was interrupted");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(ag.tool_work_list(T1).await.1["work"].as_array().unwrap().is_empty(), "no work row is left");
    assert!(!ag.works.lock().unwrap().contains_key(&wid));
    assert!(ag.tool_lock_list().await.1["locks"].as_array().unwrap().iter().all(|l| l["holder"].is_null() || l["holder"]["id"] != T1), "no lock is held: {:?}", ag.tool_lock_list().await.1);
    assert_eq!(wakes.call(T1, "wake_list", &json!({})).await.1["wakes"].as_array().unwrap().len(), 0, "no wake is left");
    assert!(!ag.world().await.running(&child), "the launched thread was stopped");
    // the report of the cancelled work was filed, and the settled thread did not start a turn for it
    tokio::time::sleep(Duration::from_secs(1)).await;
    let after = log_from(&hub, from).await;
    assert!(!after.iter().any(|e| turn_is(e, "running") && e["thread"] == T1), "a settled thread is not started again by its reports: {after:?}");
    assert!(!ag.world().await.running(T1));
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_recovery_leaves_a_settled_thread_settled_and_stopped() {
    let sd = dir("st2-bin"); perf_standin(&sd, 1);
    // a turn cut by a restart is carried on, unless the thread was settled: the log says running, then settled
    let (hub, ag, _h, _w) = start("st2", |w| running_log(w, &format!("{{\"$\":\"ThreadSettled\",\"id\":\"{T1}\",\"manual\":true,\"at\":1700000101}}\n")), |_| Cli::standin(&sd)).await;
    tokio::time::sleep(Duration::from_secs(2)).await; let ev = log_from(&hub, 0).await;
    assert!(ev.iter().any(|e| turn_is(e, "interrupted")), "the cut turn is marked stopped");
    assert!(!ev.iter().any(|e| turn_is(e, "running") && e["at"].as_u64().unwrap_or(0) > 1700000101), "no turn was carried on: {ev:?}");
    assert!(!ag.world().await.running(T1)); ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn the_agents_own_settle_tool_stops_the_work_of_another_thread() {
    let sd = dir("st3-bin"); perf_standin(&sd, 1);
    let (hub, ag, _h, _w) = start("st3", |w| base_log(w, 2, ""), |_| Cli::standin(&sd)).await; let other = "t1700001001-1";
    let (_, r) = ag.tool_work_run(other, &json!({"command":"sleep 60","note":"theirs"})).await; assert!(r["id"].is_string());
    let from = len(&hub).await;
    assert!(ag.tool_settle(T1, &json!({"threadId":other})).await.0);
    until(&hub, from, 30, |ev| ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("ended: cancelled")))).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(ag.tool_work_list(other).await.1["work"].as_array().unwrap().is_empty());
    assert!(!log_from(&hub, from).await.iter().any(|e| turn_is(e, "running") && e["thread"] == other), "the report did not start the settled thread"); ag.shutdown();
}

#[tokio::test(flavor = "multi_thread")] async fn the_writing_setting_names_threads_and_without_it_haiku_does() { // Settings > Writing: the one-shot's provider and model
    let sd = dir("wr-bin"); let log = sd.join("argv"); let logp = log.display().to_string().replace('\\', "/"); // sh reads a backslash as an escape: the path with slashes (Windows)
    script(&sd, "claude", &format!("#!/bin/sh\ncat > /dev/null\necho \"claude $*\" >> {}\nprintf '{{\"type\":\"result\",\"is_error\":false,\"result\":\"Claude title\"}}\\n'\n", logp));
    script(&sd, "codex", &format!("#!/bin/sh\ncat > /dev/null\necho \"codex $*\" >> {}\nprintf '{{\"type\":\"item.completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\"Codex title\"}}}}\\n'\n", logp));
    let (hub, ag, _h, _w) = start("wr", |w| base_log(w, 1, ""), |_| Cli::standin(&sd)).await;
    let argv = || std::fs::read_to_string(&log).unwrap_or_default().lines().last().unwrap_or("").to_string();
    let set = |k: &str, v: &str| { let hub = hub.clone(); let (k, v) = (k.to_string(), v.to_string()); async move { hub.commit(&json!({"$":"SettingSet","key":k,"value":v,"at":1700000070})).await.unwrap(); tokio::time::sleep(Duration::from_millis(100)).await } };
    assert_eq!(ag.write_title(T1, "design a board").await.as_deref(), Some("Claude title")); assert!(argv().contains("--model claude-haiku-5-5"), "no setting: Haiku: {}", argv());
    set("text.model", "sonnet").await; assert_eq!(ag.write_title(T1, "design a board").await.as_deref(), Some("Claude title")); assert!(argv().starts_with("claude") && argv().contains("--model sonnet"), "the chosen model: {}", argv());
    set("text.provider", "codex").await; set("text.model", "").await; assert_eq!(ag.write_title(T1, "design a board").await.as_deref(), Some("Codex title")); assert!(argv().starts_with("codex exec") && argv().contains("-m gpt-6-luna"), "the chosen provider, its light model: {}", argv());
    ag.shutdown();
}

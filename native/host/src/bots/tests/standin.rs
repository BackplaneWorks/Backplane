//! The uberbot end to end on a temp home: the real hub, agents and bots (`bots::start`, as main.rs does) with a stand-in `claude` that reaches its tools over the MCP URL it was started with.
//! The person's message gives Bub a turn, a tool call saves a memory, the next turn starts with that memory, and a routine that comes due gives its run a turn of its own.
use super::*;
use crate::agents::{proc::Cli, Agents};
use std::{path::Path, time::{Duration, Instant}};

/// `@LOG@`: a file; each turn's input and each tool answer go there. A message with `zqsave` saves a memory, a run's brief (`<charter>`) saves another from the run's own thread.
const AGENT: &str = r#"#!/bin/sh
url=""
while [ $# -gt 0 ]; do
  if [ "$1" = --mcp-config ]; then cfg="$2"; [ -f "$cfg" ] && cfg=$(cat "$cfg"); url=$(printf %s "$cfg" | sed -E 's/.*"backplane":\{"type":"http","url":"([^"]*)".*/\1/'); fi
  shift
done
sid="fake-$$"; n=0
tool() { k=0; r=""; while [ -z "$r" ] && [ $k -lt 100 ]; do k=$((k + 1)); r=$(curl -s -X POST -H 'content-type: application/json' --data "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"$1\",\"arguments\":$2}}" "$url"); [ -z "$r" ] && sleep 0.1; done; printf 'TOOL %s %s\n' "$1" "$r" >> @LOG@; } # an empty answer is a connection that did not come through: asked again (the hub's MCP endpoint under a loaded run), never taken as done
say() { printf '{"type":"assistant","message":{"id":"m%s-%s","role":"assistant","content":[{"type":"text","text":"%s"}],"usage":{"input_tokens":1,"output_tokens":1}},"session_id":"%s"}\n' "$$" "$n" "$1" "$sid"; }
while IFS= read -r line; do
  case "$line" in *'"type":"user"'*)
    n=$((n + 1)); printf 'TURN %s\n' "$line" >> @LOG@
    own=$(printf %s "$line" | sed 's/<chat>.*<\/chat>//') # the turn's own words: not its memory view (bots/recall)
    printf '{"type":"system","subtype":"init","session_id":"%s","cwd":"/tmp","tools":["Bash"],"model":"fake","permissionMode":"default"}\n' "$sid"
    case "$own" in
      *'<charter>'*) tool memory_save '{"key":"run-note","kind":"note","text":"the run saw the morning routine"}'; say "planned the day" ;;
      *zqwait*) while [ ! -f "@HOLD@" ]; do sleep 0.05; done; say "answered the person" ;;
      *'You were restarted'*) say "" ;;
      *zqhold*) tool thread_launch '{"project":"p1","prompt":"child work please"}'; while [ ! -f "@HOLD@" ]; do sleep 0.05; done; say "launched it" ;;
      *zqlaunch*) tool thread_launch '{"project":"p1","prompt":"child work please"}'; say "launched it" ;;
      *'Launched thread'*) say "heard the report" ;;
      *'child work please'*) while [ ! -f "@GATE@" ]; do sleep 0.05; done; say "child done" ;;
      *zqsave*) tool memory_save '{"key":"favourite-part","kind":"fact","text":"likes the 0603 footprint"}'; say "saved it" ;;
      *) say "hello" ;;
    esac
    printf '{"type":"result","subtype":"success","is_error":false,"result":"done","session_id":"%s","modelUsage":{"fake":{"contextWindow":200000}}}\n' "$sid" ;;
  esac
done
"#;
async fn wait_for(hub: &Arc<Hub>, what: &str, f: impl Fn(&[Value]) -> bool) {
    let end = Instant::now() + Duration::from_secs(120); // a bound for a starved full-suite run, never a pause: the loop leaves as soon as the event is there
    while Instant::now() < end { if f(&log(hub)) { return } tokio::time::sleep(Duration::from_millis(50)).await }
    let timeline: Vec<String> = log(hub).iter().filter(|e| !matches!(e["$"].as_str(), Some("SettingSet" | "UsageSet" | "SessionSet"))).map(|e| format!("{} {} {} {}", e["$"].as_str().unwrap_or(""), e["thread"].as_str().or(e["id"].as_str()).unwrap_or(""), e["state"].as_str().or(e["role"].as_str()).unwrap_or(""), e["text"].as_str().unwrap_or("").chars().take(60).collect::<String>())).collect();
    panic!("{what} never came; the log in order:\n{}", timeline.join("\n"));
}
/// What the stand-in's `thread_launch` call answered (its `TOOL` line): the child exists only if the tool answered, so a test says what the tool said when no child came
fn launch_answer(out: &Path) -> String { std::fs::read_to_string(out).unwrap_or_default().lines().find(|l| l.starts_with("TOOL thread_launch")).unwrap_or("no thread_launch call was logged").to_string() }
fn said(l: &[Value], th: &str, text: &str) -> bool { l.iter().any(|e| e["$"] == "MessagePosted" && e["thread"] == th && e["role"] == "assistant" && e["text"] == text) }
fn write_agent(d: &Path, out: &Path) { use crate::plat::fs::PermissionsExt; let p = d.join("claude"); std::fs::write(&p, AGENT.replace("@LOG@", &crate::plat::fs::script_path(out)).replace("@HOLD@", &crate::plat::fs::script_path(&out.with_file_name("hold"))).replace("@GATE@", &crate::plat::fs::script_path(&out.with_file_name("gate")))).unwrap(); std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); }

#[tokio::test(flavor = "multi_thread")] async fn the_uberbot_answers_remembers_and_wakes_for_a_routine_with_a_stand_in_agent() {
    std::env::set_var("TZ", "Etc/UTC");
    let root = crate::plat::tmp::dir("uber-e2e");
    let (home, bin) = (root.join("home"), root.join("bin")); std::fs::create_dir_all(&home).unwrap(); std::fs::create_dir_all(&bin).unwrap();
    let out = root.join("agent.log"); write_agent(&bin, &out);
    std::fs::write(home.join("events.jsonl"), "{\"$\":\"SettingSet\",\"key\":\"machine.id\",\"value\":\"m1\",\"at\":1}\n").unwrap();
    let hub = Hub::start(Cfg { home, version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let ag = Agents::start_with(hub.clone(), Cli::standin(&bin)).await.unwrap();
    let bots = crate::bots::start(hub.clone(), ag.clone()).await;

    // the person makes Bub the uberbot
    let r = rpc(&hub, "bots.create", json!({"name":"bub","uber":true})).await; let bub = r["bot"].as_str().unwrap_or_else(|| panic!("{r}")).to_string();
    wait_for(&hub, "the promotion", |l| l.iter().any(|e| e["$"] == "UberSet") && l.iter().any(|e| e["$"] == "WorktreeSet")).await;
    assert!(bots.uber_init(now_s()).await > 0, "the uberbot's first ops");
    let chat = log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == bub.as_str()).unwrap()["thread"].as_str().unwrap().to_string();

    // the person's message is a turn in Bub's thread, and a tool call saves a memory
    assert_eq!(rpc(&hub, "bots.say", json!({"bot":bub,"text":"zqsave what I like","msg":"m1"})).await["ok"], true);
    wait_for(&hub, "Bub's reply", |l| said(l, &chat, "saved it")).await;
    wait_for(&hub, "the memory", |l| l.iter().any(|e| e["$"] == "MemorySaved" && e["bot"] == bub.as_str() && e["key"] == "favourite-part")).await;
    let agent = std::fs::read_to_string(&out).unwrap();
    assert!(agent.contains("TOOL memory_save") && agent.contains("\"isError\":false"), "the tool answered over MCP: {agent}");
    wait_for(&hub, "the turn's end", |l| l.iter().rev().find(|e| e["$"] == "TurnChanged" && e["thread"] == chat.as_str()).map_or(false, |e| e["state"] == "completed")).await;

    // the next turn starts with what was saved (a fact rides when the message's words pick it: mem_block)
    assert_eq!(rpc(&hub, "bots.say", json!({"bot":bub,"text":"which footprint again?","msg":"m2"})).await["ok"], true);
    wait_for(&hub, "the second reply", |l| said(l, &chat, "hello")).await;
    let agent = std::fs::read_to_string(&out).unwrap();
    let turn = agent.lines().filter(|l| l.starts_with("TURN ")).last().unwrap();
    assert!(turn.contains("which footprint again?") && turn.contains("likes the 0603 footprint"), "the memory rides the turn: {turn}");

    // a routine comes due: the tick fires it, it is filed, and a run of its own gets a turn with the tools of the bot; the person's chat stays out of it
    assert_eq!(rpc(&hub, "bots.routine", json!({"bot":bub,"routine":"ro1","name":"morning","schedule":"@hourly","prompt":"plan the day"})).await["ok"], true);
    let chat_lines = |hub: &Arc<Hub>| log(hub).iter().filter(|e| e["$"] == "MessagePosted" && e["thread"] == chat.as_str()).count();
    let before = chat_lines(&hub);
    assert_eq!(bots.tick_at(now_s() + 7200).await, 1, "the routine fired");
    wait_for(&hub, "the routine's run", |l| l.iter().any(|e| e["$"] == "RunSet" && e["bot"] == bub.as_str() && e["conv"] == "routine:ro1")).await;
    let run = log(&hub).iter().find(|e| e["$"] == "RunSet").unwrap()["thread"].as_str().unwrap().to_string();
    wait_for(&hub, "the run's reply", |l| said(l, &run, "planned the day")).await;
    wait_for(&hub, "the run's memory", |l| l.iter().any(|e| e["$"] == "MemorySaved" && e["bot"] == bub.as_str() && e["key"] == "run-note")).await;
    wait_for(&hub, "the run's end", |l| l.iter().any(|e| e["$"] == "TurnChanged" && e["thread"] == run.as_str() && e["state"] == "completed")).await;
    let agent = std::fs::read_to_string(&out).unwrap();
    assert!(agent.lines().any(|l| l.starts_with("TURN ") && l.contains("<charter>") && l.contains("plan the day")), "the brief carries the routine's prompt");
    assert_eq!(chat_lines(&hub), before, "nothing in the person's conversation");
    ag.shutdown();
    let _ = std::fs::remove_dir_all(&root);
}

/// #1239: a thread a bot launched ends its turn: the report wakes the bot (a turn of its own, with the report as its input), as a person's launcher is woken
#[tokio::test(flavor = "multi_thread")] async fn a_report_from_a_thread_a_bot_launched_wakes_the_bot_with_the_report_as_input() {
    let root = crate::plat::tmp::dir("bot-launch-e2e");
    let (home, bin, work) = (root.join("home"), root.join("bin"), root.join("work")); for d in [&home, &bin, &work] { std::fs::create_dir_all(d).unwrap(); }
    let out = root.join("agent.log"); write_agent(&bin, &out);
    std::fs::write(home.join("events.jsonl"), format!("{{\"$\":\"SettingSet\",\"key\":\"machine.id\",\"value\":\"m1\",\"at\":1}}\n{{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"toy\",\"root\":{},\"at\":1}}\n", serde_json::to_string(&work.to_string_lossy()).unwrap())).unwrap();
    let hub = Hub::start(Cfg { home, version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let ag = Agents::start_with(hub.clone(), Cli::standin(&bin)).await.unwrap();
    let _bots = crate::bots::start(hub.clone(), ag.clone()).await;
    let r = rpc(&hub, "bots.create", json!({"name":"ada"})).await; let ada = r["bot"].as_str().unwrap_or_else(|| panic!("{r}")).to_string();
    let chat = { let end = Instant::now() + Duration::from_secs(60); loop { if let Some(e) = log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == ada.as_str() && e["thread"].as_str().map_or(false, |t| !t.is_empty())) { break e["thread"].as_str().unwrap().to_string() } assert!(Instant::now() < end, "the bot's thread"); tokio::time::sleep(Duration::from_millis(50)).await } };
    assert_eq!(rpc(&hub, "bots.say", json!({"bot":ada,"text":"zqlaunch it","msg":"m1"})).await["ok"], true);
    // the bot's turn is over and its thread has settled (it idles) while the child works
    wait_for(&hub, "the bot's first turn", |l| said(l, &chat, "launched it") && l.iter().rev().find(|e| e["$"] == "TurnChanged" && e["thread"] == chat.as_str()).map_or(false, |e| e["state"] == "completed")).await;
    assert!(launch_answer(&out).contains("\"isError\":false"), "the bot's thread_launch: {}", launch_answer(&out));
    hub.commit(&json!({"$":"ThreadSettled","id":chat,"manual":false,"at":now_s()})).await.unwrap();
    std::fs::write(out.with_file_name("gate"), "go").unwrap(); // only now may the child finish: the report always meets a settled thread, whatever the load
    wait_for(&hub, "the report in the bot's thread", |l| l.iter().any(|e| e["$"] == "MessagePosted" && e["thread"] == chat.as_str() && e["text"].as_str().map_or(false, |t| t.starts_with("[Backplane] Launched thread")))).await;
    wait_for(&hub, "the bot's turn on the report", |l| said(l, &chat, "heard the report")).await;
    let agent = std::fs::read_to_string(&out).unwrap();
    assert!(agent.lines().any(|l| l.starts_with("TURN ") && l.contains("Launched thread") && l.contains("completed")), "the report is the bot's input: {agent}");
    ag.shutdown();
    let _ = std::fs::remove_dir_all(&root);
}

/// #1239: the child finishes while the bot's own turn is still running (a loaded machine): the report still reaches the bot, as its next turn's input
#[tokio::test(flavor = "multi_thread")] async fn a_report_that_comes_while_the_bot_turn_still_runs_reaches_the_bot() {
    let root = crate::plat::tmp::dir("bot-launch-running");
    let (home, bin, work) = (root.join("home"), root.join("bin"), root.join("work")); for d in [&home, &bin, &work] { std::fs::create_dir_all(d).unwrap(); }
    let out = root.join("agent.log"); write_agent(&bin, &out); std::fs::write(out.with_file_name("gate"), "go").unwrap(); // the child finishes at once
    std::fs::write(home.join("events.jsonl"), format!("{{\"$\":\"SettingSet\",\"key\":\"machine.id\",\"value\":\"m1\",\"at\":1}}\n{{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"toy\",\"root\":{},\"at\":1}}\n", serde_json::to_string(&work.to_string_lossy()).unwrap())).unwrap();
    let hub = Hub::start(Cfg { home, version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let ag = Agents::start_with(hub.clone(), Cli::standin(&bin)).await.unwrap();
    let _bots = crate::bots::start(hub.clone(), ag.clone()).await;
    let r = rpc(&hub, "bots.create", json!({"name":"ada"})).await; let ada = r["bot"].as_str().unwrap_or_else(|| panic!("{r}")).to_string();
    let chat = { let end = Instant::now() + Duration::from_secs(60); loop { if let Some(e) = log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == ada.as_str() && e["thread"].as_str().map_or(false, |t| !t.is_empty())) { break e["thread"].as_str().unwrap().to_string() } assert!(Instant::now() < end, "the bot's thread"); tokio::time::sleep(Duration::from_millis(50)).await } };
    assert_eq!(rpc(&hub, "bots.say", json!({"bot":ada,"text":"zqhold it","msg":"m1"})).await["ok"], true);
    // the bot's turn waits on its hold file; the child ends meanwhile and its report comes to the running turn
    wait_for(&hub, "the bot's thread_launch call", |_| std::fs::read_to_string(&out).unwrap_or_default().contains("TOOL thread_launch")).await;
    assert!(launch_answer(&out).contains("\"isError\":false"), "the bot's thread_launch: {}", launch_answer(&out));
    wait_for(&hub, "the report while the bot's turn runs", |l| l.iter().any(|e| e["$"] == "MessagePosted" && e["thread"] == chat.as_str() && e["text"].as_str().map_or(false, |t| t.starts_with("[Backplane] Launched thread")))).await;
    std::fs::write(out.with_file_name("hold"), "go").unwrap();
    wait_for(&hub, "the bot's turn on the report", |l| said(l, &chat, "heard the report")).await;
    let agent = std::fs::read_to_string(&out).unwrap();
    assert!(agent.lines().any(|l| l.starts_with("TURN ") && l.contains("Launched thread") && l.contains("completed")), "the report is the bot's input: {agent}");
    ag.shutdown();
    let _ = std::fs::remove_dir_all(&root);
}

/// #1246: the restart's wake waited for nobody. Thirty seconds after every start (a fresh install's first start too) it went into the uberbot's chat as a system
/// message; when the person had just asked something, it steered their turn (a Codex turn starts again with only the wake's text) and told the agent the turn was
/// private and to say nothing, so the person's question was never answered. The wake now waits for the turn in the chat to end, and only then has a turn of its own.
#[tokio::test(flavor = "multi_thread")] async fn the_restart_wake_waits_for_the_persons_turn_in_the_uberbots_chat() {
    let root = crate::plat::tmp::dir("uber-wake-idle");
    let (home, bin) = (root.join("home"), root.join("bin")); std::fs::create_dir_all(&home).unwrap(); std::fs::create_dir_all(&bin).unwrap();
    let out = root.join("agent.log"); write_agent(&bin, &out);
    std::fs::write(home.join("events.jsonl"), "{\"$\":\"SettingSet\",\"key\":\"machine.id\",\"value\":\"m1\",\"at\":1}\n").unwrap();
    let hub = Hub::start(Cfg { home, version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let ag = Agents::start_with(hub.clone(), Cli::standin(&bin)).await.unwrap();
    let bots = crate::bots::start(hub.clone(), ag.clone()).await;
    let r = rpc(&hub, "bots.create", json!({"name":"bub","uber":true})).await; let bub = r["bot"].as_str().unwrap_or_else(|| panic!("{r}")).to_string();
    wait_for(&hub, "the promotion", |l| l.iter().any(|e| e["$"] == "UberSet") && l.iter().any(|e| e["$"] == "WorktreeSet")).await;
    assert!(bots.uber_init(now_s()).await > 0, "the uberbot's first ops");
    let chat = log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == bub.as_str()).unwrap()["thread"].as_str().unwrap().to_string();
    let woke = |l: &[Value]| l.iter().any(|e| e["$"] == "MessagePosted" && e["thread"] == chat.as_str() && e["role"] == "system" && e["text"].as_str().map_or(false, |t| t.starts_with("You were restarted")));

    // the person asks; their turn is still running when the wake comes
    assert_eq!(rpc(&hub, "bots.say", json!({"bot":bub,"text":"zqwait how do I start?","msg":"m1"})).await["ok"], true);
    wait_for(&hub, "the person's turn in the agent", |_| std::fs::read_to_string(&out).unwrap_or_default().contains("zqwait")).await;
    let b2 = bots.clone(); let wake = tokio::spawn(async move { b2.wake_uber_within("update", Duration::from_secs(60), Duration::from_millis(100)).await });
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(!woke(&log(&hub)), "the wake did not go into the person's running turn");
    assert!(!log(&hub).iter().any(|e| e["$"] == "ActivityLogged" && e["thread"] == chat.as_str() && e["kind"] == "steer"), "nothing steered the person's turn");
    assert!(!wake.is_finished(), "the wake waits");

    // the person's turn ends with its answer, and then the wake has its own turn
    std::fs::write(out.with_file_name("hold"), "go").unwrap();
    wait_for(&hub, "the answer to the person", |l| said(l, &chat, "answered the person")).await;
    assert_eq!(tokio::time::timeout(Duration::from_secs(60), wake).await.expect("the wake goes once the chat is idle").unwrap(), Ok(true));
    let l = log(&hub);
    let at = |f: &dyn Fn(&Value) -> bool| l.iter().position(|e| f(e)).unwrap();
    let answer = at(&|e| e["$"] == "MessagePosted" && e["thread"] == chat.as_str() && e["text"] == "answered the person");
    let wake_msg = at(&|e| e["$"] == "MessagePosted" && e["thread"] == chat.as_str() && e["role"] == "system" && e["text"].as_str().map_or(false, |t| t.starts_with("You were restarted")));
    assert!(answer < wake_msg, "the person was answered before the wake was posted");
    let turns = std::fs::read_to_string(&out).unwrap();
    let person_turn = turns.lines().find(|l| l.starts_with("TURN ") && l.contains("zqwait")).unwrap();
    assert!(!person_turn.contains("You were restarted"), "the person's turn never carried the wake: {person_turn}");
    ag.shutdown();
    let _ = std::fs::remove_dir_all(&root);
}

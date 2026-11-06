//! The uberbot end to end on a temp home: the real hub, agents and bots (`bots::start`, as main.rs does) with a stand-in `claude` that reaches its tools over the MCP URL it was started with.
//! The person's message gives Bub a turn, a tool call saves a memory, the next turn starts with that memory, and a routine that comes due gives its run a turn of its own.
use super::*;
use crate::agents::{proc::Cli, Agents};
use std::{path::Path, time::{Duration, Instant}};

/// `@LOG@`: a file; each turn's input and each tool answer go there. A message with `zqsave` saves a memory, a run's brief (`<charter>`) saves another from the run's own thread.
const AGENT: &str = r#"#!/bin/sh
url=""
while [ $# -gt 0 ]; do
  if [ "$1" = --mcp-config ]; then url=$(printf %s "$2" | sed -E 's/.*"backplane":\{"type":"http","url":"([^"]*)".*/\1/'); fi
  shift
done
sid="fake-$$"; n=0
tool() { r=$(curl -s -X POST -H 'content-type: application/json' --data "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"$1\",\"arguments\":$2}}" "$url"); printf 'TOOL %s %s\n' "$1" "$r" >> @LOG@; }
say() { printf '{"type":"assistant","message":{"id":"m%s-%s","role":"assistant","content":[{"type":"text","text":"%s"}],"usage":{"input_tokens":1,"output_tokens":1}},"session_id":"%s"}\n' "$$" "$n" "$1" "$sid"; }
while IFS= read -r line; do
  case "$line" in *'"type":"user"'*)
    n=$((n + 1)); printf 'TURN %s\n' "$line" >> @LOG@
    printf '{"type":"system","subtype":"init","session_id":"%s","cwd":"/tmp","tools":["Bash"],"model":"fake","permissionMode":"default"}\n' "$sid"
    case "$line" in
      *'<charter>'*) tool memory_save '{"key":"run-note","kind":"note","text":"the run saw the morning routine"}'; say "planned the day" ;;
      *zqsave*) tool memory_save '{"key":"favourite-part","kind":"fact","text":"likes the 0603 footprint"}'; say "saved it" ;;
      *) say "hello" ;;
    esac
    printf '{"type":"result","subtype":"success","is_error":false,"result":"done","session_id":"%s","modelUsage":{"fake":{"contextWindow":200000}}}\n' "$sid" ;;
  esac
done
"#;
async fn wait_for(hub: &Arc<Hub>, what: &str, f: impl Fn(&[Value]) -> bool) {
    let end = Instant::now() + Duration::from_secs(30);
    while Instant::now() < end { if f(&log(hub)) { return } tokio::time::sleep(Duration::from_millis(50)).await }
    panic!("{what} never came; the log's kinds: {:?}", kinds(hub, 0));
}
fn said(l: &[Value], th: &str, text: &str) -> bool { l.iter().any(|e| e["$"] == "MessagePosted" && e["thread"] == th && e["role"] == "assistant" && e["text"] == text) }
fn write_agent(d: &Path, out: &Path) { use std::os::unix::fs::PermissionsExt; let p = d.join("claude"); std::fs::write(&p, AGENT.replace("@LOG@", &out.to_string_lossy())).unwrap(); std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); }

#[tokio::test(flavor = "multi_thread")] async fn the_uberbot_answers_remembers_and_wakes_for_a_routine_with_a_stand_in_agent() {
    std::env::set_var("TZ", "Etc/UTC");
    let root = std::env::temp_dir().join(format!("bp-uber-e2e-{}", std::process::id())); let _ = std::fs::remove_dir_all(&root);
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

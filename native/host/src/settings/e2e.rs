//! End to end: a planted secret must not appear in the log, any client frame, an agent line (to or from) or an MCP answer.
//! A real hub (temp home, canned presenter), the real agents actor with a stand-in `claude`, the real MCP server, the real vault.
//! Stand-in agents only: no model runs.
use crate::{agents::{proc::Cli, Agents}, api::Caller, hub::{Cfg, Hub}, presenter::Canned, settings};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::{Arc, Mutex}, time::Duration};

const SECRET: &str = "PLANTED-s3cr3t-Zk9vQ27x";
const API_SECRET: &str = "apikey-VALUE-Qw83Lm5t";
const T1: &str = "t1700000001-2";

fn dir(name: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-set-e2e-{}-{}", std::process::id(), name)); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }

/// A `claude` that records every line it is sent and says the secret in every way an agent can: streamed text, a message, a tool call,
/// its own stderr.
fn standin(sd: &Path, got: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let body = format!(r#"#!/bin/sh
while IFS= read -r line; do
  printf '%s\n' "$line" >> '{got}'
  case "$line" in
    *'"type":"user"'*)
      printf '%s\n' '{{"type":"system","subtype":"init","session_id":"s1","cwd":"/tmp","tools":["Bash"],"model":"fake","permissionMode":"default"}}'
      printf '%s\n' '{{"type":"stream_event","event":{{"type":"message_start","message":{{"id":"m1","role":"assistant","content":[],"usage":{{"input_tokens":1}}}}}},"session_id":"s1"}}'
      printf '%s\n' '{{"type":"stream_event","event":{{"type":"content_block_start","index":0,"content_block":{{"type":"text","text":""}}}},"session_id":"s1"}}'
      printf '%s\n' '{{"type":"stream_event","event":{{"type":"content_block_delta","index":0,"delta":{{"type":"text_delta","text":"streamed {s} and {a} "}}}},"session_id":"s1"}}'
      printf '%s\n' '{{"type":"assistant","message":{{"id":"m1","role":"assistant","content":[{{"type":"text","text":"message {s} and {a}"}}],"usage":{{"input_tokens":1,"output_tokens":1}}}},"session_id":"s1"}}'
      printf '%s\n' '{{"type":"assistant","message":{{"id":"m2","role":"assistant","content":[{{"type":"tool_use","id":"tu1","name":"Bash","input":{{"command":"echo {s}","description":"tool call {a}"}}}}],"usage":{{"input_tokens":1,"output_tokens":2}}}},"session_id":"s1"}}'
      echo "stderr {s}" >&2
      printf '%s\n' '{{"type":"result","subtype":"success","is_error":false,"result":"done {s}","session_id":"s1","modelUsage":{{"fake":{{"contextWindow":200000}}}}}}' ;;
  esac
done
"#, got = got.display(), s = SECRET, a = API_SECRET);
    let p = sd.join("claude"); std::fs::write(&p, body).unwrap(); std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
}

async fn post(port: u16, path: &str, body: &str) -> Value {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    s.write_all(format!("POST {path} HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
    let mut out = String::new(); s.read_to_string(&mut out).await.unwrap();
    serde_json::from_str(out.split("\r\n\r\n").nth(1).unwrap_or("null")).unwrap_or(Value::Null)
}
async fn tool(ag: &Arc<Agents>, name: &str, args: Value) -> String {
    let m = ag.mcp.get().unwrap(); let url = m.url(T1, ""); let path = format!("/{}", url.splitn(4, '/').nth(3).unwrap());
    let r = post(m.port, &path, &json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}}).to_string()).await;
    r["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_planted_secret_is_in_no_log_line_no_client_frame_no_agent_line_and_no_mcp_answer() {
    let _l = settings::vault::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("BACKPLANE_VAULT_KEYRING", "file");
    let home = dir("home"); let work = dir("work"); let sd = dir("bin"); let got = sd.join("received.txt");
    standin(&sd, &got);
    std::fs::write(home.join("events.jsonl"), format!(
        "{{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"fixture\",\"root\":{},\"at\":1700000000}}\n{{\"$\":\"ThreadCreated\",\"id\":\"{T1}\",\"project\":\"p1\",\"title\":\"Fixture\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":1700000001}}\n",
        serde_json::to_string(&work.to_string_lossy()).unwrap())).unwrap();
    // the secrets: one in the vault, one in api.env
    settings::open(&home);
    assert_eq!(settings::vault::put("planted", SECRET, "e2e", "person"), "ok");
    settings::keys::add(&home, "E2E_KEY", API_SECRET).unwrap();
    let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let ag = Agents::start_with(hub.clone(), Cli::standin(&sd)).await.unwrap();
    settings::start(hub.clone(), Some(ag.clone()));

    // every frame the hub sends a client
    let frames: Arc<Mutex<Vec<String>>> = Default::default();
    let mut rx = hub.frames.subscribe();
    let f2 = frames.clone(); let collector = tokio::spawn(async move { while let Ok(v) = rx.recv().await { f2.lock().unwrap().push(v.to_string()) } });
    let c = Caller { gen: "1".into(), client: 1 };
    async fn call(hub: &Arc<Hub>, c: &Caller, m: &'static str, p: Value) -> Vec<Value> { let h = hub.rpc_for(m).expect(m); h.call(hub, c, &json!(7), m, &p).await }
    // replies are frames too
    let mut replies: Vec<String> = vec![];

    // 1. the person puts the secret in a message, a setting and a thread title; the agent says it back every way it can
    let before = hub.store.len();
    for r in call(&hub, &c, "turn.start", json!({"thread":T1,"text":format!("please use {SECRET} and {API_SECRET} now")})).await { replies.push(r.to_string()) }
    for r in call(&hub, &c, "setting.set", json!({"key":"note.x","value":format!("keep {SECRET}")})).await { replies.push(r.to_string()) }
    hub.live_delta(T1, &format!("live text {SECRET}")).await.unwrap();
    hub.commit(&json!({"$":"SettingSet","key":format!("note.{SECRET}"),"value":format!("act {SECRET}"),"at":2})).await.unwrap();
    let end = std::time::Instant::now() + Duration::from_secs(30);
    loop { let ev = hub.store.range(before, usize::MAX).unwrap(); if ev.iter().any(|l| l.contains("\"completed\"")) || std::time::Instant::now() > end { break } tokio::time::sleep(Duration::from_millis(50)).await }
    tokio::time::sleep(Duration::from_millis(300)).await;

    // 2. through the secret box: set, list, del; and the tools an agent calls
    for r in call(&hub, &c, "secret.set", json!({"name":"second","value":"second-VALUE-Hn4Pq8","purpose":"p"})).await { replies.push(r.to_string()) }
    for r in call(&hub, &c, "secret.list", json!({})).await { replies.push(r.to_string()) }
    let mut answers = vec![
        tool(&ag, "secret_list", json!({})).await,
        tool(&ag, "secret_request", json!({"name":"planted","purpose":"x"})).await,
        tool(&ag, "secret_run", json!({"command":"echo \"v=$T\"; cat \"$F\"; echo \"e=$T\" >&2","env":{"T":"planted"},"files":{"F":"planted"}})).await,
        tool(&ag, "secret_run", json!({"command":format!("echo literal {SECRET} and {API_SECRET}")})).await,
        tool(&ag, "thread_read", json!({"threadId":T1,"limit":50})).await,
        tool(&ag, "thread_list", json!({})).await,
    ];
    assert!(answers[2].contains("\\u2022") || answers[2].contains('\u{2022}'), "secret_run output is masked, not dropped: {}", answers[2]);
    assert!(answers[4].contains("\u{2022}"), "thread_read shows the masked message: {}", answers[4]);
    for r in call(&hub, &c, "secret.del", json!({"name":"second"})).await { replies.push(r.to_string()) }
    tokio::time::sleep(Duration::from_millis(200)).await;
    answers.extend(replies);

    // 3. the log, byte for byte
    let log = std::fs::read(home.join("events.jsonl")).unwrap();
    let text = String::from_utf8_lossy(&log).to_string();
    for s in [SECRET, API_SECRET, "second-VALUE-Hn4Pq8"] {
        assert!(!text.contains(s), "the log holds {s}");
        assert!(!hub.store.range(0, usize::MAX).unwrap().iter().any(|l| l.contains(s)), "the store holds {s}");
        let frames = frames.lock().unwrap(); assert!(!frames.iter().any(|f| f.contains(s)), "a client frame holds {s}");
        assert!(!hub.live_snapshot().iter().any(|(_, t)| t.contains(s)), "the live text holds {s}");
        assert!(!hub.live_frame().to_string().contains(s), "the live frame holds {s}");
        for a in &answers { assert!(!a.contains(s), "an MCP answer or reply holds {s}: {a}") }
        // the model the clients' views are built from
    }
    assert!(text.contains("\u{2022}"), "the secret is covered, not removed");
    // 4. the agent saw masked lines only
    let received = std::fs::read_to_string(&got).unwrap_or_default();
    assert!(!received.is_empty(), "the stand-in ran");
    assert!(!received.contains(SECRET) && !received.contains(API_SECRET), "a line to the agent holds a secret: {received}");
    assert!(received.contains("\u{2022}"));
    // 5. the stand-in's own output (stdout lines, stderr) reached the log masked
    assert!(text.contains("streamed ") || text.contains("message "), "the agent's words arrived");
    // 6. the prompt names the vault's secrets and never a value
    let prompt = ag.prompt_for(T1, &work.to_string_lossy()).await;
    assert!(prompt.contains("`planted`") && !prompt.contains(SECRET) && !prompt.contains(API_SECRET), "{prompt}");
    collector.abort(); ag.shutdown();
}

/// Launch arguments, stdin and stderr of a process are masked too (Spawn.agent, Agent.write, the exit tail).
#[tokio::test(flavor = "multi_thread")]
async fn a_process_is_launched_fed_and_read_through_the_masks() {
    let _l = settings::vault::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("BACKPLANE_VAULT_KEYRING", "file");
    let home = dir("proc-home"); settings::open(&home); assert_eq!(settings::vault::put("p", SECRET, "", "person"), "ok");
    let mut p = crate::agents::proc::spawn(Path::new("/bin/sh"), &["-c".into(), "echo \"arg=$1\"; read l; echo \"in=$l\"; echo \"err=$1\" >&2".into(), "sh".into(), SECRET.into()], &home, None, &[]).unwrap();
    p.input.send(format!("typed {SECRET}")).await.unwrap();
    let mut out = vec![]; while let Some(l) = p.lines.recv().await { out.push(l) }
    let exit = p.exit.await.unwrap();
    let all = format!("{out:?} {}", exit.stderr);
    assert!(!all.contains(SECRET), "{all}"); assert!(out.iter().any(|l| l.starts_with("arg=\u{2022}")) && out.iter().any(|l| l.starts_with("in=typed \u{2022}")) && exit.stderr.contains("err=\u{2022}"), "{all}");
}

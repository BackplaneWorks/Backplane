//! A Claude session the CLI no longer has ("No conversation found with session ID"): the thread's id is cleared and the same message runs in a fresh session with the
//! recent conversation handed over, said once, one retry at most. Every path that starts a Claude turn ends in `claude_send`; each is driven here.
use super::*;

/// A stand-in claude that has only the sessions named `fresh-*`: any other `--resume` id is "lost" (an error result and the same words on stderr, exit 1).
/// FAKE_MODE=stderr: only stderr and the exit; FAKE_MODE=nofresh: even a start with no resume says it (the loop check). Every start and user line goes to $FAKE_LOG.
pub(super) const LOST_CLAUDE: &str = r#"#!/bin/sh
log=${FAKE_LOG:-/dev/null}; prev=; res=
for a in "$@"; do [ "$prev" = --resume ] && res=$a; prev=$a; done
echo "START resume=$res" >> "$log"
lost=
[ -n "$res" ] && case "$res" in fresh-*) ;; *) lost=1 ;; esac
[ "$FAKE_MODE" = nofresh ] && lost=1
if [ -n "$lost" ]; then
  [ "$FAKE_MODE" != stderr ] && printf '%s\n' '{"type":"result","subtype":"error_during_execution","is_error":true,"errors":["No conversation found with session ID: '"$res"'"],"session_id":"x"}'
  echo "No conversation found with session ID: $res" >&2
  exit 1
fi
sid="fresh-$$"
while IFS= read -r line; do
  case "$line" in *'"type":"user"'*)
    printf 'USER %s\n' "$line" >> "$log"
    printf '%s\n' '{"type":"system","subtype":"init","session_id":"'"$sid"'","cwd":"/tmp","tools":["Bash"],"model":"fake","permissionMode":"default"}'
    printf '%s\n' '{"type":"assistant","message":{"id":"m'"$$"'","role":"assistant","content":[{"type":"text","text":"ok"}]},"session_id":"'"$sid"'"}'
    printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"result":"ok","session_id":"'"$sid"'","modelUsage":{"fake":{"contextWindow":200000}}}' ;;
  esac
done
"#;
const GONE: &str = "gone-session";

pub(super) struct Rig { pub(super) hub: Arc<Hub>, pub(super) ag: Arc<Agents>, rec: PathBuf }
impl Rig {
    pub(super) fn starts(&self) -> Vec<String> { std::fs::read_to_string(&self.rec).unwrap_or_default().lines().filter(|l| l.starts_with("START")).map(str::to_string).collect() }
    pub(super) fn users(&self) -> Vec<String> { std::fs::read_to_string(&self.rec).unwrap_or_default().lines().filter(|l| l.starts_with("USER ")).map(str::to_string).collect() }
}
/// A thread with a session the CLI does not have and a little conversation before it.
pub(super) async fn rig(name: &str, mode: &str, extra: &str) -> Rig {
    let sd = dir(&format!("{name}-bin")); let rec = sd.join("rec");
    script(&sd, "claude", &LOST_CLAUDE.replacen("\n", &format!("\nexport FAKE_MODE={mode} FAKE_LOG={}\n", rec.display()), 1));
    let extra = extra.to_string();
    let (hub, ag, _h, _w) = start(name, move |w| base_log(w, 1, &format!("{{\"$\":\"SessionSet\",\"id\":\"{T1}\",\"session\":\"{GONE}\",\"at\":1700000050}}\n{{\"$\":\"MessagePosted\",\"thread\":\"{T1}\",\"msg\":\"m0\",\"role\":\"user\",\"text\":\"earlier question\",\"at\":1700000051}}\n{{\"$\":\"MessagePosted\",\"thread\":\"{T1}\",\"msg\":\"m0a\",\"role\":\"assistant\",\"text\":\"earlier answer\",\"at\":1700000052}}\n{extra}")), |_| Cli::standin(&sd)).await;
    Rig { hub, ag, rec }
}
fn lost_say(ev: &[Value]) -> Vec<&Value> { ev.iter().filter(|e| is(e, "ActivityLogged") && e["kind"] == "session").collect() }
fn failed(ev: &[Value]) -> bool { ev.iter().any(|e| turn_is(e, "failed")) }
/// The one outcome every path must reach: completed, never failed, said once, fresh session recorded, resumed once then started afresh, the conversation handed over.
async fn expect_recovered(r: &Rig, from: usize, text: &str) {
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed") || turn_is(e, "failed"))).await;
    assert!(!failed(&ev), "the turn must not fail: {ev:?}");
    assert!(ev.iter().any(|e| turn_is(e, "completed")), "{ev:?}");
    assert_eq!(lost_say(&ev).len(), 1, "said once: {ev:?}");
    assert_eq!(lost_say(&ev)[0]["tone"], "info");
    assert!(ev.iter().any(|e| is(e, "SessionSet") && e["session"].as_str().map_or(false, |s| s.starts_with("fresh-"))), "the new session is recorded: {ev:?}");
    assert_eq!(r.starts(), vec![format!("START resume={GONE}"), "START resume=".to_string()], "one resume, one fresh start");
    let u = r.users(); assert_eq!(u.len(), 1, "{u:?}");
    assert!(u[0].contains(text), "the same message runs: {}", u[0]);
    assert!(u[0].contains("earlier question") && u[0].contains("earlier answer"), "the recent conversation goes with it: {}", u[0]);
    assert!(!ev.iter().any(|e| e["$"] == "SettingSet" && e["key"].as_str().map_or(false, |k| k.starts_with("failed."))), "no failed marker");
    assert_eq!(r.ag.world().await.threads.get(T1).unwrap().session.starts_with("fresh-"), true);
}

#[tokio::test(flavor = "multi_thread")] async fn a_persons_turn_with_a_lost_session_runs_afresh_with_the_conversation() {
    let r = rig("ls-person", "result", "").await; let from = len(&r.hub).await;
    rpc(&r.hub, "turn.start", json!({"thread":T1,"text":"zqshort hello again"})).await;
    expect_recovered(&r, from, "zqshort hello again").await;
    // the next turn resumes the new session and does not say it again
    let from = len(&r.hub).await; rpc(&r.hub, "turn.start", json!({"thread":T1,"text":"zqshort and then"})).await;
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed") || turn_is(e, "failed"))).await;
    assert!(ev.iter().any(|e| turn_is(e, "completed")) && lost_say(&ev).is_empty(), "{ev:?}");
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn the_stderr_alone_and_the_exit_are_enough() {
    let r = rig("ls-stderr", "stderr", "").await; let from = len(&r.hub).await;
    rpc(&r.hub, "turn.start", json!({"thread":T1,"text":"zqshort hello again"})).await;
    expect_recovered(&r, from, "zqshort hello again").await; r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_bots_chat_turn_is_recovered_too() {
    let r = rig("ls-bot", "result", "").await; let from = len(&r.hub).await;
    assert!(r.ag.deliver(T1, "mb1", "zqshort from a bot", Some(("bot".into(), "b".into(), "b".into())), vec![]).await.is_ok());
    expect_recovered(&r, from, "zqshort from a bot").await; r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_queued_message_is_recovered_too() {
    let r = rig("ls-queue", "result", "").await; let from = len(&r.hub).await;
    assert!(r.ag.start_queued(T1, "zqshort queued words").await.is_ok());
    expect_recovered(&r, from, "zqshort queued words").await; r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_turn_the_restart_cut_continues_in_a_fresh_session() {
    let sd = dir("ls-cont-bin"); let rec = sd.join("rec");
    script(&sd, "claude", &LOST_CLAUDE.replacen("\n", &format!("\nexport FAKE_MODE=result FAKE_LOG={}\n", rec.display()), 1));
    let (hub, ag, _h, _w) = start("ls-cont", |w| base_log(w, 1, &format!("{{\"$\":\"SessionSet\",\"id\":\"{T1}\",\"session\":\"{GONE}\",\"at\":1700000050}}\n{{\"$\":\"MessagePosted\",\"thread\":\"{T1}\",\"msg\":\"m1\",\"role\":\"user\",\"text\":\"build it\",\"at\":1700000100}}\n{{\"$\":\"TurnChanged\",\"thread\":\"{T1}\",\"state\":\"running\",\"at\":1700000100}}\n")), |_| Cli::standin(&sd)).await;
    let ev = until(&hub, 0, 20, |ev| ev.iter().any(|e| turn_is(e, "completed") || (turn_is(e, "failed")))).await;
    assert!(!failed(&ev) && ev.iter().any(|e| turn_is(e, "completed")), "{ev:?}");
    assert_eq!(lost_say(&ev).len(), 1, "{ev:?}");
    let starts: Vec<String> = std::fs::read_to_string(&rec).unwrap_or_default().lines().filter(|l| l.starts_with("START")).map(str::to_string).collect();
    assert_eq!(starts, vec![format!("START resume={GONE}"), "START resume=".to_string()]);
    ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_fresh_session_that_says_it_too_fails_the_turn_and_does_not_loop() {
    let r = rig("ls-loop", "nofresh", "").await; let from = len(&r.hub).await;
    rpc(&r.hub, "turn.start", json!({"thread":T1,"text":"zqshort hello"})).await;
    let ev = until(&r.hub, from, 20, |ev| failed(ev) || ev.iter().any(|e| turn_is(e, "completed"))).await;
    assert!(failed(&ev), "the second refusal is an ordinary failure: {ev:?}");
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert_eq!(r.starts().len(), 2, "one retry at most: {:?}", r.starts()); r.ag.shutdown();
}

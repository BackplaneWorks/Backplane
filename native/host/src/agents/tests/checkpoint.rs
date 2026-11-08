//! A turn's checkpoint is the files as that turn left them. The next turn may start the moment one ends (a queued message does), so its writes must never land
//! inside the earlier checkpoint: reverting to checkpoint 1 removes what turn 2 wrote.
use super::*;

/// A stand-in claude that writes `<word after "write ">` into its working folder as soon as a user line arrives, then answers.
const WRITER: &str = r#"#!/bin/sh
sid="w-$$"
while IFS= read -r line; do
  case "$line" in *'"type":"user"'*)
    f=$(printf '%s' "$line" | sed -n 's/.*write \([a-z0-9.]*\).*/\1/p'); [ -n "$f" ] && echo "by turn" > "$f"
    printf '%s\n' '{"type":"system","subtype":"init","session_id":"'"$sid"'","cwd":"/tmp","tools":["Bash"],"model":"fake","permissionMode":"default"}'
    printf '%s\n' '{"type":"assistant","message":{"id":"m'"$$"'","role":"assistant","content":[{"type":"text","text":"ok"}]},"session_id":"'"$sid"'"}'
    printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"result":"ok","session_id":"'"$sid"'","modelUsage":{"fake":{"contextWindow":200000}}}' ;;
  esac
done
"#;
fn git(d: &Path, a: &[&str]) { let o = std::process::Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(a).current_dir(d).output().unwrap(); assert!(o.status.success(), "git {a:?}: {}", String::from_utf8_lossy(&o.stderr)); }

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_turn_started_the_moment_the_last_one_ends_is_not_inside_the_earlier_checkpoint() {
    let bin = dir("ckpt-bin"); script(&bin, "claude", WRITER);
    let (hub, ag, _home, work) = start("ckpt", |w| { std::fs::write(w.join("a.txt"), "one\n").unwrap();
        // a work tree big enough that a capture takes a while: the earlier turn's end capture is likely still running when the next message is sent. (The hub never runs a repo's own clean
        // filters, so a slow filter cannot make it slow; the wait is proved without git below.) Whether the capture was still running is machine speed: the revert assertions hold either way.
        std::fs::create_dir_all(w.join("bulk")).unwrap(); for i in 0..6000 { std::fs::write(w.join(format!("bulk/f{i}.txt")), format!("{i}\n")).unwrap() } git(w, &["init", "-q", "-b", "main"]); git(w, &["add", "."]); git(w, &["commit", "-qm", "init"]); base_log(w, 1, "") }, |_| Cli::standin(&bin)).await;
    let wp = work.to_string_lossy().into_owned();
    for (n, file) in ["first.txt", "second.txt"].iter().enumerate() {
        let from = len(&hub).await;
        // no pause between the end of one turn and the start of the next: the first turn's end capture is still waiting when this one is sent
        assert_eq!(rpc(&hub, "turn.start", json!({"thread":T1,"text":format!("please write {file}")})).await["ok"], true);
        until(&hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed") || turn_is(e, "failed"))).await;
        assert!(work.join(file).exists(), "turn {} wrote {file}", n + 1);
    }
    for _ in 0..100 { if crate::files::git::checkpoint_count(&wp, T1).await >= 3 { break } tokio::time::sleep(Duration::from_millis(100)).await }
    assert!(crate::files::git::checkpoint_count(&wp, T1).await >= 3, "checkpoints 0, 1 and 2 exist");
    assert_eq!(crate::files::git::checkpoint_revert(&wp, T1, 1).await, "");
    assert!(work.join("first.txt").exists(), "checkpoint 1 has what turn 1 wrote");
    assert!(!work.join("second.txt").exists(), "checkpoint 1 does not have what turn 2 wrote");
    ag.shutdown();
}

/// The wait itself, without git: an end capture still counted as running holds the next turn, which says so in the thread once it has waited `say`, and goes on the moment the capture ends.
#[tokio::test(flavor = "multi_thread")]
async fn a_turn_waiting_for_the_earlier_checkpoint_says_so_and_goes_on_when_it_ends() {
    let bin = dir("ckpt-wait-bin"); script(&bin, "claude", WRITER);
    let (hub, ag, _home, _work) = start("ckpt-wait", |w| base_log(w, 1, ""), |_| Cli::standin(&bin)).await;
    let ends = Arc::new(super::super::HistEnds::default()); ends.n.fetch_add(1, Relaxed);                    // one end capture "running"
    let from = len(&hub).await;
    let waiter = tokio::spawn(super::super::ends_settled(ends.clone(), ag.clone(), T1.to_string(), Duration::from_millis(50)));
    let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| is(e, "ActivityLogged") && e["summary"].as_str().map_or(false, |s| s.contains("Saving the last turn's checkpoint")))).await;
    assert!(ev.iter().any(|e| is(e, "ActivityLogged") && e["thread"] == T1 && e["kind"] == "checkpoint"), "the waiting turn says so in its thread: {ev:?}");
    assert!(!waiter.is_finished(), "it still waits while the capture runs");
    ends.n.fetch_sub(1, Relaxed); ends.done.notify_waiters();                                                // the capture ends
    tokio::time::timeout(Duration::from_secs(10), waiter).await.expect("it goes on once the capture ends").unwrap();
    // a turn that finds nothing running does not say anything
    let from = len(&hub).await; super::super::ends_settled(Arc::new(super::super::HistEnds::default()), ag.clone(), T1.to_string(), Duration::from_millis(50)).await;
    tokio::time::sleep(Duration::from_millis(200)).await; assert!(!log_from(&hub, from).await.iter().any(|e| is(e, "ActivityLogged") && e["kind"] == "checkpoint"), "nothing was waited for: nothing said");
    ag.shutdown();
}

/// A turn that waits for the earlier turn's end capture holds back only its own start: the person's message is answered at once (its events are durable), a setting written from another
/// client and another thread's turn go through meanwhile, and the waiting turn starts, once, after the capture.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_turn_waiting_for_the_checkpoint_holds_back_nothing_else_and_starts_after_it() {
    let bin = dir("ckpt-hold-bin"); script(&bin, "claude", WRITER);
    let (hub, ag, _home, work) = start("ckpt-hold", |w| base_log(w, 2, ""), |_| Cli::standin(&bin)).await;
    let t2 = "t1700001001-1";
    hub.register_rpc(Arc::new(crate::settings::set::SettingSet { agents: Some(ag.clone()) }));    // the settings handler of a real hub, which the agents' own tests do not register
    let held = ag.ends_of(T1); held.n.fetch_add(1, Relaxed);                                     // thread 1's earlier end capture is still running
    let from = len(&hub).await; let t = Instant::now();
    let r = tokio::time::timeout(Duration::from_secs(5), rpc(&hub, "turn.start", json!({"thread":T1,"text":"please write held.txt"}))).await.expect("the message is answered while its turn waits");
    assert_eq!(r["ok"], true); assert!(t.elapsed() < Duration::from_secs(1), "answered at once, not after the capture: {:?}", t.elapsed());
    until(&hub, from, 10, |ev| ev.iter().any(|e| is(e, "ActivityLogged") && e["kind"] == "checkpoint")).await;     // it says it waits (after CKPT_SAY)
    // another client's setting and another thread's turn, meanwhile
    let t = Instant::now(); let s = tokio::time::timeout(Duration::from_secs(5), rpc(&hub, "setting.set", json!({"key":"restart.continue","value":"off"}))).await.expect("setting.set is answered while a turn waits");
    assert_eq!(s["ok"], true); assert!(t.elapsed() < Duration::from_secs(1), "setting.set took {:?} while a turn waited for a checkpoint", t.elapsed());
    assert_eq!(hub.setting("restart.continue").await.as_deref(), Some("off"), "and the setting was written");
    let t = Instant::now(); assert_eq!(rpc(&hub, "turn.start", json!({"thread":t2,"text":"please write other.txt"})).await["ok"], true); assert!(t.elapsed() < Duration::from_secs(1));
    for _ in 0..100 { if work.join("other.txt").exists() { break } tokio::time::sleep(Duration::from_millis(50)).await }
    assert!(work.join("other.txt").exists(), "another thread's turn runs meanwhile");
    assert!(!work.join("held.txt").exists(), "the waiting turn has not started its agent yet");
    held.n.fetch_sub(1, Relaxed); held.done.notify_waiters();                                         // the capture ends
    for _ in 0..100 { if work.join("held.txt").exists() { break } tokio::time::sleep(Duration::from_millis(50)).await }
    assert!(work.join("held.txt").exists(), "the waiting turn starts once the capture is over");
    let ev = until(&hub, from, 20, |ev| ev.iter().filter(|e| turn_is(e, "completed")).count() >= 2).await;
    assert_eq!(ev.iter().filter(|e| is(e, "MessagePosted") && e["role"] == "user" && e["text"] == "please write held.txt").count(), 1, "the message is posted once");
    ag.shutdown();
}

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
        // a work tree big enough that a capture takes longer than the test's polling: the earlier turn's end capture is still running when the next message is sent
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

//! The desk's side of alerts: the window tells the hub which thread it shows and whether
//! it has the focus (`desk.look`) when either changes and at most once a minute while focused; a `{"t":"notify"}` frame the hub sends to the desk used
//! last becomes a desktop notification (`notify-send`, or the program BACKPLANE_GPUI_NOTIFY_CMD names: a test sets it to record the call).
use crate::{hub::Command, Backplane};
use gpui::{Context, Window};
use serde_json::{json, Value};
use std::sync::Mutex;

pub const BEAT_S: u64 = 60;
static LAST: Mutex<(String, u64)> = Mutex::new((String::new(), 0));

pub fn said(thread: &str, focused: bool) -> String { format!("{} {}", if focused { "1" } else { "0" }, thread) }
/// Desk.due: when the thread or the focus changed since it last spoke (or never has), or a beat has passed with the focus.
pub fn due(last: &str, last_at: u64, thread: &str, focused: bool, now: u64) -> bool { last.is_empty() || last != said(thread, focused) || (focused && now >= last_at + BEAT_S) }

fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) }

/// Called while drawing.
pub fn tick(this: &mut Backplane, window: &mut Window, _cx: &mut Context<Backplane>) {
    if !this.usable { return }
    let (thread, focused, t) = (this.selected.clone(), window.is_window_active(), now());
    let mut l = LAST.lock().unwrap();
    if !due(&l.0, l.1, &thread, focused, t) { return }
    if this.tx.try_send(Command::Rpc("desk.look".into(), crate::screens::settings::Secret(json!({"thread": thread, "focused": focused})), "desk")).is_ok() { *l = (said(&thread, focused), t); }
}

/// The notify frame's command line: program and arguments.
pub fn command(f: &Value) -> (String, Vec<String>) {
    let title = f["title"].as_str().unwrap_or("Backplane"); let body = f["body"].as_str().unwrap_or("");
    let prog = std::env::var("BACKPLANE_GPUI_NOTIFY_CMD").unwrap_or_else(|_| "notify-send".into());
    (prog, vec!["--app-name=Backplane".into(), "--".into(), title.chars().take(200).collect(), body.chars().take(400).collect()])
}
/// Shows the notification; a missing notify-send is not an error.
pub fn show(f: &Value) {
    let (prog, args) = command(f);
    eprintln!("GPUI_LOG {}", json!({"notify": f["kind"], "thread": f["thread"]}));
    let _ = std::process::Command::new(prog).args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map(|mut c| { std::thread::spawn(move || { let _ = c.wait(); }); });
}

#[cfg(test)] mod tests { use super::*;
    #[test] fn speaks_on_a_change_and_each_beat_only_while_focused() {
        let s = said("t1", true);
        assert!(due("", 0, "t1", true, 10), "never spoke");
        assert!(!due(&s, 10, "t1", true, 50)); assert!(due(&s, 10, "t1", true, 70), "a beat");
        assert!(due(&s, 10, "t2", true, 11), "another thread"); assert!(due(&s, 10, "t1", false, 11), "lost the focus");
        assert!(!due(&said("t1", false), 10, "t1", false, 500), "unfocused: no beat");
    }
    #[test] fn the_command_carries_title_and_body() { let (_, a) = command(&json!({"title":"T","body":"B"})); assert_eq!(a[2..], ["T".to_string(), "B".to_string()]); }
}

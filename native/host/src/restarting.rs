//! A restart every client can see (docs/HUB-API.md, Restarts). The hub says it is going before it goes, and why, so a window or a phone shows
//! "Restarting Backplane…" rather than a dead connection; a hub that comes back from a crash says so to the clients that join just after.
//! - `{"t":"hub.restarting","reason","build","text","at"}` to every socket, from the signal that ends the hub (SIGTERM: the guard's restart,
//!   fresh-watch.sh's relaunch, a person's stop). `reason` comes from `<home>/restart.next`, which whoever restarts the hub on purpose writes
//!   first (`{"reason":"build","build":"<build id>"}`: fresh-watch.sh); without it `restart`. A client keeps `text` up while it has no hub, and
//!   drops it as soon as it is connected again.
//! - info `hub.back` (a JSON string `{reason,text,at}`, `""` for none) for the first `BACK_FOR` after a start that followed a crash (the guard's
//!   `BACKPLANE_PREV_END=crash`), shown once by a client that joins then.
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock, Weak};

/// What the one who restarts the hub on purpose leaves in the home for the hub to say.
pub const NEXT: &str = "restart.next";
/// How long after a start that followed a crash a joining client is told so.
pub const BACK_FOR: std::time::Duration = std::time::Duration::from_secs(120);
/// How long the ending hub gives its sockets to send the frame.
const FLUSH: std::time::Duration = std::time::Duration::from_millis(300);

static HUB: OnceLock<Weak<crate::hub::Hub>> = OnceLock::new();

/// A build id (`20261120-000620-2c25023cc9`) as a person reads it: its commit.
pub fn short(build: &str) -> &str { let b = build.trim(); b.rsplit('-').next().unwrap_or(b) }
/// The words a client shows while the hub is away (English: clients translate it as hub text).
pub fn text(reason: &str, build: &str) -> String {
    match reason {
        "build" if !short(build).is_empty() => format!("Restarting Backplane… (new build {})", short(build)),
        "stop" => "Backplane stopped".into(),
        _ => "Restarting Backplane…".into(),
    }
}
/// The frame for a restart, from what `restart.next` holds (anything else, or nothing: a plain restart).
pub fn frame(next: &str, at_ms: u64) -> Value {
    let v: Value = serde_json::from_str(next.trim()).unwrap_or(Value::Null);
    let reason = match v["reason"].as_str() { Some(r @ ("build" | "restart" | "stop")) => r, _ => "restart" };
    let build: String = v["build"].as_str().unwrap_or("").chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(64).collect();
    json!({"t":"hub.restarting","reason":reason,"build":build,"text":text(reason, &build),"at":at_ms})
}
/// The `hub.back` value for a start (`prev`: the guard's BACKPLANE_PREV_END), "" when there is nothing to say.
pub fn back(prev: &str, at_ms: u64) -> String {
    match prev { "crash" => json!({"reason":"crash","text":"Backplane is back after it stopped unexpectedly","at":at_ms}).to_string(), _ => String::new() }
}

/// At the start: the hub the signal will speak for, and the crash note for clients that join soon.
pub async fn start(hub: &Arc<crate::hub::Hub>) {
    let _ = HUB.set(Arc::downgrade(hub));
    let _ = std::fs::remove_file(hub.cfg.home.join(NEXT)); // said by the hub that went; a stale one must not name the next restart
    let b = back(&std::env::var("BACKPLANE_PREV_END").unwrap_or_default(), crate::hub::now_ms());
    if b.is_empty() { return }
    hub.set_info("hub.back", &b).await;
    let h = Arc::downgrade(hub);
    tokio::spawn(async move { tokio::time::sleep(BACK_FOR).await; if let Some(h) = h.upgrade() { h.set_info("hub.back", "").await; } });
}

/// From the signal that ends the hub, before anything else goes: every socket is told, and given a moment to send it.
pub fn announce() {
    let Some(hub) = HUB.get().and_then(Weak::upgrade) else { return };
    let next = std::fs::read_to_string(hub.cfg.home.join(NEXT)).unwrap_or_default();
    let f = frame(&next, crate::hub::now_ms());
    eprintln!("backplane-hub: telling {} client(s): {}", hub.clients().len(), f["text"].as_str().unwrap_or(""));
    hub.push_all(&f);
    std::thread::sleep(FLUSH);
}

/// What runs on SIGTERM, SIGINT, SIGHUP (and Ctrl+C on Windows): the clients told, then the KiCad sessions ended.
pub fn on_exit() { announce(); crate::files::kicad::embed::shutdown_all(); }

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn a_new_build_is_named_by_its_commit() {
        let f = frame(r#"{"reason":"build","build":"20261120-000620-2c25023cc9"}"#, 7);
        assert_eq!((f["t"].as_str(), f["reason"].as_str(), f["build"].as_str(), f["at"].as_u64()), (Some("hub.restarting"), Some("build"), Some("20261120-000620-2c25023cc9"), Some(7)));
        assert_eq!(f["text"], "Restarting Backplane… (new build 2c25023cc9)");
    }
    #[test] fn nothing_or_nonsense_is_a_plain_restart() {
        for n in ["", "{", r#"{"reason":"reboot"}"#, r#"{"reason":"build"}"#] { assert_eq!(frame(n, 1)["text"], "Restarting Backplane…", "{n:?}"); }
        assert_eq!(frame(r#"{"reason":"stop"}"#, 1)["text"], "Backplane stopped");
        assert_eq!(frame(r#"{"reason":"build","build":"x\"; rm"}"#, 1)["build"], "xrm", "only an id's characters");
    }
    #[test] fn only_a_crash_is_said_after_the_start() {
        let b: Value = serde_json::from_str(&back("crash", 5)).unwrap();
        assert_eq!((b["reason"].as_str(), b["at"].as_u64()), (Some("crash"), Some(5)));
        assert_eq!(back("", 5), ""); assert_eq!(back("restart", 5), "");
    }
}

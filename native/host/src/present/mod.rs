//! The Rust presenter (docs/PRESENTER-PORT.md). `Routed` is a `Presenter` that asks each ported `Screen` first and falls to the helper
//! for what is not ported yet; a screen takes the same params the helper takes and returns what the helper's V returns.
//! `BACKPLANE_PRESENTER_DUMP=<dir>` writes every `(params, helper output)` pair to `<dir>/views.jsonl` (0600): the input of the differential test.
//! The dump holds message text, drafts and `session`: SYNTHETIC logs only. It is written only by a copy run, only while the helper is running, and only for a log under
//! `copy::BIG_LOG` (a copy of a real store is far larger); `frames.jsonl` (the fold stream) and a frame index per view are the next step and not written yet.
pub mod keys;
pub mod panels;
pub mod tokstats;
pub mod picker;
pub mod thread;
pub mod viewer;
pub mod settings;
pub mod onboard;
pub mod botforms;
pub mod ready;
pub mod badge;
mod refusal;
pub mod tz;
pub mod window;
use crate::presenter::{Fut, Presenter};
use anyhow::Result;
use serde_json::{json, Value};
use std::{io::Write, path::PathBuf, sync::{Arc, Mutex}};
pub mod screen;
mod native_sidebar;

/// One ported part of the presentation. `None`: not mine, ask the next one (finally the helper).
pub trait Screen: Send + Sync {
    fn name(&self) -> &'static str;
    fn view(&self, params: &Value) -> Option<Value>;
}

pub struct Routed { helper: Arc<dyn Presenter>, screens: Vec<Box<dyn Screen>>, dump: Option<Mutex<std::fs::File>> }

impl Routed {
    pub fn new(helper: Arc<dyn Presenter>, screens: Vec<Box<dyn Screen>>, home: &std::path::Path, helper_running: bool) -> Routed {
        let log = std::fs::metadata(home.join("events.jsonl")).map(|m| m.len()).unwrap_or(0);
        Routed { helper, screens, dump: Self::open_dump(std::env::var_os("BACKPLANE_PRESENTER_DUMP").map(PathBuf::from), crate::copy::on(), helper_running, log) }
    }
    /// Refused (said once) unless: a copy run, the helper is running (nothing to record otherwise), and the log is small (`copy::BIG_LOG`): a dump of a real-size copy would hold real content.
    fn open_dump(dir: Option<PathBuf>, copy: bool, helper_running: bool, log_bytes: u64) -> Option<Mutex<std::fs::File>> {
        use crate::plat::fs::{DirBuilderExt, OpenOptionsExt};
        let dir = dir?;
        if !copy || !helper_running || log_bytes > crate::copy::BIG_LOG {
            eprintln!("BACKPLANE_PRESENTER_DUMP ignored: it needs a copy run (BACKPLANE_COPY=1), a running helper and a log under {} bytes (synthetic logs only)", crate::copy::BIG_LOG);
            return None
        }
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir).ok()?;
        match std::fs::OpenOptions::new().create(true).append(true).mode(0o600).open(dir.join("views.jsonl")) {
            Ok(f) => Some(Mutex::new(f)),
            Err(e) => { eprintln!("BACKPLANE_PRESENTER_DUMP: {e}"); None }
        }
    }
    fn dump(&self, params: &Value, out: &Value) {
        if let Some(f) = &self.dump { if let Ok(mut f) = f.lock() { let _ = writeln!(f, "{}", json!({"params": params, "out": out})); } }
    }
}

impl Presenter for Routed {
    fn name(&self) -> &'static str { "routed (rust screens, then the helper)" }
    fn fold<'a>(&'a self, frame: &'a Value) -> Fut<'a, ()> { self.helper.fold(frame) }
    fn view<'a>(&'a self, params: &'a Value) -> Fut<'a, Value> {
        Box::pin(async move {
            // a request carrying the hub's own answers (reads: git.diff, mech.list, review pages...) is the helper's: only its Ui applies them, and a Rust screen would draw the
            // thread as it was and drop them
            // (a `fab.look` answer only feeds the header's Buy it, and the thread must not stop answering because the hub looked: every action that moves a thread sends one)
            // (with no helper there is nothing to send the others to: the Rust screens apply them themselves (session/pages.rs), and a view that carries one is answered like any other)
            let reads = params.get("reads").and_then(Value::as_array).map_or(false, |r| r.iter().any(|x| x["kind"] != "fab.look")) && !self.helper.absent();
            if !reads { for s in &self.screens { if let Some(v) = s.view(params) { return Ok(v) } } }
            let out = self.helper.view(params).await?;
            self.dump(params, &out);
            Ok(out)
        })
    }
    fn ping(&self) -> Fut<'_, ()> { self.helper.ping() }
    fn wants_log(&self) -> bool { self.helper.wants_log() }
    fn is_down(&self) -> bool { self.helper.is_down() }
    fn restart(&self) -> Fut<'_, ()> { self.helper.restart() }
    fn why_down(&self) -> Option<String> { self.helper.why_down() }
    fn absent(&self) -> bool { self.helper.absent() }
}

/// The helper-less hub's base presenter: a view nothing ported answers says so (the session turns the error into the retryable PRESENTER_UNAVAILABLE) and
/// nothing is folded into it, so it holds nothing and the hub does not feed it the log (`wants_log` is false).
pub struct NotPorted;
impl Presenter for NotPorted {
    fn name(&self) -> &'static str { "not ported (no helper)" }
    fn fold<'a>(&'a self, _frame: &'a Value) -> Fut<'a, ()> { Box::pin(async { Ok(()) }) }
    fn view<'a>(&'a self, params: &'a Value) -> Fut<'a, Value> {
        Box::pin(async move { anyhow::bail!("not ported: {} {}", params["target"]["kind"].as_str().unwrap_or("?"), params["action"].as_str().unwrap_or("")) })
    }
    fn ping(&self) -> Fut<'_, ()> { Box::pin(async { Ok(()) }) }
    fn wants_log(&self) -> bool { false }
    fn absent(&self) -> bool { true }
}

#[allow(dead_code)]
type _R = Result<()>;

#[cfg(test)]
mod tests {
    use super::*; use crate::presenter::Canned;
    struct Mine; impl Screen for Mine {
        fn name(&self) -> &'static str { "mine" }
        fn view(&self, p: &Value) -> Option<Value> { if p["target"]["kind"] == "thread" { Some(json!({"shown":{"by":"rust"}})) } else { None } }
    }
    fn helper() -> Arc<dyn Presenter> { Arc::new(Canned { reply: json!({"shown":{"by":"helper"}}) }) }
    fn tmp(n: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("bp-present-{n}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); d }

    #[tokio::test] async fn a_screen_answers_what_it_has_and_the_helper_the_rest() {
        let r = Routed { helper: helper(), screens: vec![Box::new(Mine)], dump: None };
        assert_eq!(r.view(&json!({"target":{"kind":"thread","id":"t"}})).await.unwrap()["shown"]["by"], "rust");
        assert_eq!(r.view(&json!({"target":{"kind":"recent"}})).await.unwrap()["shown"]["by"], "helper");
    }
    #[tokio::test] async fn without_a_helper_an_unported_view_says_so_and_the_log_is_not_fed() {
        let r = Routed { helper: Arc::new(NotPorted), screens: vec![Box::new(Mine)], dump: None };
        assert_eq!(r.view(&json!({"target":{"kind":"thread","id":"t"}})).await.unwrap()["shown"]["by"], "rust", "a ported screen still answers");
        let e = r.view(&json!({"target":{"kind":"settings"},"action":"x"})).await.unwrap_err().to_string();
        assert!(e.contains("not ported") && e.contains("settings"), "{e}");
        assert!(!r.wants_log() && r.ping().await.is_ok() && !r.is_down());
    }
    #[tokio::test] async fn no_screens_is_the_helper_unchanged() {
        let r = Routed { helper: helper(), screens: vec![], dump: None };
        assert_eq!(r.view(&json!({})).await.unwrap(), json!({"shown":{"by":"helper"}}));
    }
    // the hub's explanation of a dead helper must pass through the router (the supervisor prints it before each restart)
    #[tokio::test] async fn why_down_is_forwarded_to_the_helper() {
        struct H; impl Presenter for H {
            fn name(&self) -> &'static str { "h" }
            fn fold<'a>(&'a self, _f: &'a Value) -> Fut<'a, ()> { Box::pin(async { Ok(()) }) }
            fn view<'a>(&'a self, _p: &'a Value) -> Fut<'a, Value> { Box::pin(async { Ok(json!({})) }) }
            fn ping(&self) -> Fut<'_, ()> { Box::pin(async { Ok(()) }) }
            fn why_down(&self) -> Option<String> { Some("the policy helper died: killed by SIGKILL".into()) }
        }
        let r = Routed { helper: Arc::new(H), screens: vec![], dump: None };
        assert_eq!(r.why_down().as_deref(), Some("the policy helper died: killed by SIGKILL"));
        assert!(Routed { helper: helper(), screens: vec![], dump: None }.why_down().is_none(), "a helper with nothing to say says nothing");
    }
    #[tokio::test] async fn the_dump_records_the_helper_pairs_and_only_on_a_copy_run() {
        let d = tmp("on");
        let r = Routed { helper: helper(), screens: vec![Box::new(Mine)], dump: Routed::open_dump(Some(d.clone()), true, true, 1000) };
        r.view(&json!({"target":{"kind":"recent"}})).await.unwrap();   // helper answered: recorded
        r.view(&json!({"target":{"kind":"thread","id":"t"}})).await.unwrap(); // rust answered: not recorded (nothing to compare with)
        let text = std::fs::read_to_string(d.join("views.jsonl")).unwrap();
        assert_eq!(text.lines().count(), 1, "{text}");
        let l: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(l["params"]["target"]["kind"], "recent"); assert_eq!(l["out"]["shown"]["by"], "helper");
        let d2 = tmp("off");
        assert!(Routed::open_dump(Some(d2.clone()), false, true, 1000).is_none() && !d2.exists(), "a non-copy run writes no dump");
        assert!(Routed::open_dump(Some(d2.clone()), true, false, 1000).is_none() && !d2.exists(), "no helper, nothing to record");
        assert!(Routed::open_dump(Some(d2.clone()), true, true, crate::copy::BIG_LOG + 1).is_none() && !d2.exists(), "a log of real size is never dumped");
        use crate::plat::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(d.join("views.jsonl")).unwrap().permissions().mode() & 0o777, 0o600, "the dump is private");
    }
    #[tokio::test] async fn a_fab_look_answer_does_not_send_the_view_to_the_helper_but_another_read_does() {
        let r = Routed { helper: helper(), screens: vec![Box::new(Mine)], dump: None };
        let p = |k: &str| json!({"target":{"kind":"thread","id":"t"},"reads":[{"kind":k,"id":1,"ok":true,"result":{}}]});
        assert_eq!(r.view(&p("fab.look")).await.unwrap()["shown"]["by"], "rust");
        assert_eq!(r.view(&p("git.diff")).await.unwrap()["shown"]["by"], "helper");
    }
}

//! The Rust presenter (docs/gpui/PRESENTER-PORT.md). `Routed` is a `Presenter` that asks each ported `Screen` first and falls to the helper
//! for what is not ported yet; a screen takes the same params the helper takes and returns what the helper's V returns.
//! `BACKPLANE_PRESENTER_DUMP=<dir>` (copy runs only) writes every `(params, helper output)` pair to `<dir>/views.jsonl`: the input of the differential test.
use crate::presenter::{Fut, Presenter};
use anyhow::Result;
use serde_json::{json, Value};
use std::{io::Write, path::PathBuf, sync::{Arc, Mutex}};

/// One ported part of the presentation. `None`: not mine, ask the next one (finally the helper).
pub trait Screen: Send + Sync {
    fn name(&self) -> &'static str;
    fn view(&self, params: &Value) -> Option<Value>;
}

pub struct Routed { helper: Arc<dyn Presenter>, screens: Vec<Box<dyn Screen>>, dump: Option<Mutex<std::fs::File>> }

impl Routed {
    pub fn new(helper: Arc<dyn Presenter>, screens: Vec<Box<dyn Screen>>) -> Routed {
        Routed { helper, screens, dump: Self::open_dump(std::env::var_os("BACKPLANE_PRESENTER_DUMP").map(PathBuf::from), crate::copy::on()) }
    }
    /// The dump holds hub content, so it is written only by a copy run (a throwaway home); anything else ignores the variable and says so.
    fn open_dump(dir: Option<PathBuf>, copy: bool) -> Option<Mutex<std::fs::File>> {
        let dir = dir?;
        if !copy { eprintln!("BACKPLANE_PRESENTER_DUMP ignored: only a copy run (BACKPLANE_COPY=1) may write one"); return None }
        std::fs::create_dir_all(&dir).ok()?;
        match std::fs::OpenOptions::new().create(true).append(true).open(dir.join("views.jsonl")) {
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
            for s in &self.screens { if let Some(v) = s.view(params) { return Ok(v) } }
            let out = self.helper.view(params).await?;
            self.dump(params, &out);
            Ok(out)
        })
    }
    fn ping(&self) -> Fut<'_, ()> { self.helper.ping() }
    fn wants_log(&self) -> bool { self.helper.wants_log() }
    fn is_down(&self) -> bool { self.helper.is_down() }
    fn restart(&self) -> Fut<'_, ()> { self.helper.restart() }
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
    #[tokio::test] async fn no_screens_is_the_helper_unchanged() {
        let r = Routed { helper: helper(), screens: vec![], dump: None };
        assert_eq!(r.view(&json!({})).await.unwrap(), json!({"shown":{"by":"helper"}}));
    }
    #[tokio::test] async fn the_dump_records_the_helper_pairs_and_only_on_a_copy_run() {
        let d = tmp("on");
        let r = Routed { helper: helper(), screens: vec![Box::new(Mine)], dump: Routed::open_dump(Some(d.clone()), true) };
        r.view(&json!({"target":{"kind":"recent"}})).await.unwrap();   // helper answered: recorded
        r.view(&json!({"target":{"kind":"thread","id":"t"}})).await.unwrap(); // rust answered: not recorded (nothing to compare with)
        let text = std::fs::read_to_string(d.join("views.jsonl")).unwrap();
        assert_eq!(text.lines().count(), 1, "{text}");
        let l: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(l["params"]["target"]["kind"], "recent"); assert_eq!(l["out"]["shown"]["by"], "helper");
        let d2 = tmp("off");
        assert!(Routed::open_dump(Some(d2.clone()), false).is_none() && !d2.exists(), "a non-copy run writes no dump");
    }
}

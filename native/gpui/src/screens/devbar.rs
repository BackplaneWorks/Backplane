//! Test builds: a thin strip at the top of the window while a build is being made or waits to relaunch the app.
//! The writers (scripts/dev-build.sh, fresh-watch.sh) keep one JSON line in `$BACKPLANE_DEV_STATUS` (default `~/.backplane-gpui-dev/STATUS`):
//! `{"phase":"building|ready|waiting|relaunching","text":"...","done":n,"total":m}`. They rewrite it at least every few seconds while they work and remove it when done, so
//! a file nobody has touched for `STALE` seconds is a dead writer's and shows nothing.
use crate::{theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::{path::PathBuf, time::{Duration, SystemTime}};

const STALE: u64 = 20;
const POLL: Duration = Duration::from_millis(1000);

#[derive(Clone, Debug, PartialEq)]
pub struct Bar { pub phase: String, pub text: String, pub frac: Option<f32>, /// the watcher can relaunch on request (it is waiting for turns to end)
    pub restart: bool, /// turns that a relaunch would stop
    pub turns: u64, /// a haiku on what the build adds (may be empty)
    pub haiku: String }

/// The watcher's status (STATUS: a build waits to relaunch the app, can be asked to) and the builder's (BUILD: a build is being made, failed): the watcher's is shown first.
fn paths() -> Vec<PathBuf> {
    if let Some(p) = std::env::var_os("BACKPLANE_DEV_STATUS") { return vec![PathBuf::from(p)] }
    let d = crate::plat::user_home().unwrap_or_default().join(".backplane-gpui-dev");
    /* i18n-ok: file names */ vec![d.join("STATUS"), d.join("BUILD")]
}
fn path() -> PathBuf { paths().remove(0) }

/// The bar a status line stands for (None: nothing to show).
pub fn parse(line: &str) -> Option<Bar> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let phase = v["phase"].as_str()?.to_string();
    let text = v["text"].as_str().unwrap_or("").to_string();
    if phase.is_empty() || text.is_empty() { return None }
    let (done, total) = (v["done"].as_f64().unwrap_or(0.0), v["total"].as_f64().unwrap_or(0.0));
    Some(Bar { phase, text, frac: (total > 0.0).then(|| (done / total).clamp(0.0, 1.0) as f32), restart: v["action"].as_str() == Some("restart"), turns: v["turns"].as_u64().unwrap_or(0), haiku: v["haiku"].as_str().unwrap_or("").to_string() })
}

fn read_one(p: &PathBuf) -> Option<Bar> {
    let age = SystemTime::now().duration_since(std::fs::metadata(p).ok()?.modified().ok()?).unwrap_or_default();
    if age.as_secs() > STALE { return None }
    parse(&std::fs::read_to_string(p).ok()?)
}
fn read() -> Option<Bar> { paths().iter().find_map(read_one) }

/// Look at the status file once a second (one small read) and redraw when it changed.
pub fn watch(cx: &mut Context<Backplane>) {
    cx.spawn(async move |weak, cx| loop {
        cx.background_executor().timer(POLL).await;
        let now = read();
        if weak.update(cx, |this, cx| {
            let timed_out = this.screens.devbar_confirm.map_or(false, |t| t.elapsed() > CONFIRM);
            if this.screens.devbar != now || timed_out || (now.is_none() && this.screens.devbar_confirm.is_some()) { this.screens.devbar = now; if timed_out || this.screens.devbar.is_none() { this.screens.devbar_confirm = None; } cx.notify(); }
        }).is_err() { break; }
    }).detach();
}

/// Where a relaunch request goes: a file beside the status file, which the watcher takes (and removes) as a press of Enter.
/* i18n-ok: file names */ pub fn request_path() -> PathBuf { path().with_file_name("RESTART") }
pub fn request_restart() { let _ = std::fs::write(request_path(), b"restart\n"); }
/// How long the confirming click stays asked for.
pub const CONFIRM: Duration = Duration::from_secs(6);

/// The strip: a fill as long as the progress, the words over it. When the watcher is waiting for turns to end it is green and offers "Restart now"; that button asks for a second click
/// (saying what stops), and the second one makes the request.
pub fn strip(cx: &mut Context<Backplane>, bar: &Bar, light: bool, confirming: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let ready = bar.restart;
    let hue = match bar.phase.as_str() { "building" => c("accent"), "failed" => c("bad"), _ => c("ok") };
    let mut el = div().h(crate::scale::tpx(22.)).flex_shrink_0().relative().overflow_hidden().bg(c("raise")).border_b_1().border_color(c("line"))
        .child(div().absolute().top_0().left_0().bottom_0().w(relative(bar.frac.unwrap_or(1.0))).bg(hue).opacity(if ready || bar.phase == "failed" { 0.5 } else { 0.35 }))
        .child(div().absolute().inset_0().flex().items_center().px_3().text_size(crate::scale::tpx(11.)).text_color(c("text")).child(bar.text.clone()).when(!bar.haiku.is_empty(), |d| d.child(div().ml_3().opacity(0.75).italic().child(bar.haiku.clone()))));
    if ready {
        /* i18n-ok: the developer build bar, for dev builds only */ let label = if confirming { format!("Click again: stops {} running turn{}", bar.turns, if bar.turns == 1 { "" } else { "s" }) } else { "Restart now".to_string() };
        el = el.child(div().absolute().top_0().right_0().bottom_0().flex().items_center().pr_2().child(
            div().id("devbar-restart").px_2().h(px(16.)).flex().items_center().cursor_pointer().text_size(crate::scale::tpx(11.)).font_weight(FontWeight::BOLD)
                .bg(c(if confirming { "bad" } else { "ok" })).text_color(c("on-accent")).child(label)
                .on_click(cx.listener(|this, _, _, cx| { this.devbar_click(cx); }))));
    }
    el.into_any_element()
}

impl Backplane {
    /// The bar's Restart now: the first click asks to be sure, the second (within CONFIRM) requests the relaunch.
    pub(crate) fn devbar_click(&mut self, cx: &mut Context<Self>) {
        match self.screens.devbar_confirm.take() { Some(t) if t.elapsed() <= CONFIRM => request_restart(), _ => self.screens.devbar_confirm = Some(std::time::Instant::now()) }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test;
    #[test] fn a_status_line_becomes_a_bar() {
        let b = parse(r#"{"phase":"building","text":"Building 50/200","done":50,"total":200}"#).unwrap();
        assert_eq!((b.phase.as_str(), b.text.as_str(), b.frac), ("building", "Building 50/200", Some(0.25)));
        assert_eq!(parse(r#"{"phase":"waiting","text":"2 turns running"}"#).unwrap().frac, None);
        assert_eq!(parse(r#"{"phase":"building","text":"x","done":9,"total":3}"#).unwrap().frac, Some(1.0));
        let w = parse(r#"{"phase":"waiting","text":"x","action":"restart","turns":3}"#).unwrap(); assert!(w.restart && w.turns == 3 && !b.restart && w.haiku.is_empty());
        assert_eq!(parse(r#"{"phase":"waiting","text":"x","haiku":"a / b / c"}"#).unwrap().haiku, "a / b / c");
        assert!(parse("").is_none() && parse("{}").is_none() && parse(r#"{"phase":"x","text":""}"#).is_none());
    }
}

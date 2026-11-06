//! Test builds: a thin strip at the top of the window while a build is being made or waits to relaunch the app.
//! The writers (scripts/gpui/dev-build.sh, fresh-watch.sh) keep one JSON line in `$BACKPLANE_DEV_STATUS` (default `~/.backplane-gpui-dev/STATUS`):
//! `{"phase":"building|ready|waiting|relaunching","text":"...","done":n,"total":m}`. They rewrite it at least every few seconds while they work and remove it when done, so
//! a file nobody has touched for `STALE` seconds is a dead writer's and shows nothing.
use crate::{theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::{path::PathBuf, time::{Duration, SystemTime}};

const STALE: u64 = 20;
const POLL: Duration = Duration::from_millis(1000);

#[derive(Clone, Debug, PartialEq)]
pub struct Bar { pub phase: String, pub text: String, pub frac: Option<f32> }

fn path() -> PathBuf {
    std::env::var_os("BACKPLANE_DEV_STATUS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".backplane-gpui-dev/STATUS"))
}

/// The bar a status line stands for (None: nothing to show).
pub fn parse(line: &str) -> Option<Bar> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let phase = v["phase"].as_str()?.to_string();
    let text = v["text"].as_str().unwrap_or("").to_string();
    if phase.is_empty() || text.is_empty() { return None }
    let (done, total) = (v["done"].as_f64().unwrap_or(0.0), v["total"].as_f64().unwrap_or(0.0));
    Some(Bar { phase, text, frac: (total > 0.0).then(|| (done / total).clamp(0.0, 1.0) as f32) })
}

fn read() -> Option<Bar> {
    let p = path();
    let age = SystemTime::now().duration_since(std::fs::metadata(&p).ok()?.modified().ok()?).unwrap_or_default();
    if age.as_secs() > STALE { return None }
    parse(&std::fs::read_to_string(p).ok()?)
}

/// Look at the status file once a second (one small read) and redraw when it changed.
pub fn watch(cx: &mut Context<Backplane>) {
    cx.spawn(async move |weak, cx| loop {
        cx.background_executor().timer(POLL).await;
        let now = read();
        if weak.update(cx, |this, cx| { if this.screens.devbar != now { this.screens.devbar = now; cx.notify(); } }).is_err() { break; }
    }).detach();
}

/// The strip: a fill as long as the progress, the words over it.
pub fn strip(bar: &Bar, light: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let hue = if bar.phase == "building" { c("accent") } else { c("ok") };
    div().h(px(20.)).flex_shrink_0().relative().overflow_hidden().bg(c("raise")).border_b_1().border_color(c("line"))
        .child(div().absolute().top_0().left_0().bottom_0().w(relative(bar.frac.unwrap_or(1.0))).bg(hue).opacity(0.35))
        .child(div().absolute().inset_0().flex().items_center().px_3().text_size(px(11.)).text_color(c("text")).child(bar.text.clone()))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test;
    #[test] fn a_status_line_becomes_a_bar() {
        let b = parse(r#"{"phase":"building","text":"Building 50/200","done":50,"total":200}"#).unwrap();
        assert_eq!((b.phase.as_str(), b.text.as_str(), b.frac), ("building", "Building 50/200", Some(0.25)));
        assert_eq!(parse(r#"{"phase":"waiting","text":"2 turns running"}"#).unwrap().frac, None);
        assert_eq!(parse(r#"{"phase":"building","text":"x","done":9,"total":3}"#).unwrap().frac, Some(1.0));
        assert!(parse("").is_none() && parse("{}").is_none() && parse(r#"{"phase":"x","text":""}"#).is_none());
    }
}

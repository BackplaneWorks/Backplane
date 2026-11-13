//! The Fab panel (screens/mgmt.rs hosts it; the hub's fab/panel.rs publishes `mgmt.fab.<digest>`): the project's fab and tier, whether its
//! limits are in the project's DRC rules (and a press that writes them), the board setup minimums under the fab's (Apply, on the person's
//! press), the project's own rules that allow less (conflicts), what changed since the project last looked (Seen), and the limits with
//! the day each was read: the date opens its source page. Every decision is the hub's; pure rules first, tested.
use crate::{hub::Command, i18n::{tf, trs}, screens::settings::Secret, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
/// The DRC section's state in words, and the button that writes it (None when it is in sync or there is no KiCad project).
pub fn drc_words(state: &str) -> (&'static str, Option<&'static str>) {
    match state {
        "current" => ("The fab's limits are in the DRC rules", None), // i18n-ok: English the draw passes through trs
        "stale" => ("The DRC rules have older fab limits", Some("Update")), // i18n-ok: English the draw passes through trs
        "none" => ("The fab's limits are not in the DRC rules", Some("Write")), // i18n-ok: English the draw passes through trs
        _ => ("No KiCad project for DRC rules", None), // i18n-ok: English the draw passes through trs
    }
}
/// The tier in words.
pub fn tier_words(t: &str) -> &'static str { if t == "extra" { "costs-more limits" } else { "base-price limits" } } // i18n-ok: English the draw passes through trs
/// A suggestion as a line: "copper to copper spacing: board setup 0 mm, the fab 0.09 mm".
pub fn suggestion_line(x: &Value) -> String {
    let n = |k: &str| x[k].as_f64().map(|f| format!("{f}")).unwrap_or_default();
    tf("gpui.mgmt.fab.suggestion", &[("what", s(x, "label").to_string().into()), ("project", n("project").into()), ("fab", n("fab").into())])
}

/// One of the panel's buttons: `fab.profile {pro, action, key?}`; the hub publishes the panel again after it.
pub fn press(app: &mut Backplane, action: &str, key: &str, cx: &mut Context<Backplane>) {
    let pro = app.screens.mgmt.project.clone(); if pro.is_empty() { return }
    let mut p = json!({"pro": pro, "action": action}); if !key.is_empty() { p["key"] = json!(key) }
    eprintln!("GPUI_LOG {}", json!({"event": "fab_press", "action": action, "key": key}));
    let _ = app.tx.try_send(Command::Rpc("fab.profile".into(), Secret(p), "mgmt"));
    app.ui_epoch += 1; cx.notify();
}
/// The proposed stackup taken: `fab.profile {pro, action: "stackup", id}`.
pub fn press_stackup(app: &mut Backplane, id: &str, cx: &mut Context<Backplane>) {
    let pro = app.screens.mgmt.project.clone(); if pro.is_empty() { return }
    let _ = app.tx.try_send(Command::Rpc("fab.profile".into(), Secret(json!({"pro": pro, "action": "stackup", "id": id})), "mgmt"));
    app.ui_epoch += 1; cx.notify();
}
/// The e2e harness's controls: the DRC button, each Apply, Seen and Refresh (`mgmt:fab:<what>`).
pub fn controls(m: &super::mgmt::Mgmt) -> Vec<(String, String, String)> {
    if m.peek()["open"] != "fab" { return vec![] }
    let d = m.data("fab"); let mut v: Vec<(String, String, String)> = vec![];
    if drc_words(s(&d["drc"], "state")).1.is_some() { v.push(("mgmt:fab:drc".into(), "DRC".into(), "fab:drc".into())) } // i18n-ok: English the draw passes through trs
    for x in d["suggestions"].as_array().into_iter().flatten() { v.push((format!("mgmt:fab:apply:{}", s(x, "key")), s(x, "key").into(), format!("fab:apply:{}", s(x, "key")))) }
    if d["changed"].as_array().map_or(false, |a| !a.is_empty()) { v.push(("mgmt:fab:seen".into(), "Seen".into(), "fab:seen".into())) } // i18n-ok: ctl ids
    v.push(("mgmt:fab:refresh".into(), "Refresh".into(), "fab:refresh".into())); // i18n-ok: ctl ids
    if d["review"]["running"] != true { v.push(("mgmt:fab:review".into(), "Review".into(), "fab:review".into())) } // i18n-ok: ctl ids
    if !d["review"]["at"].is_null() { v.push(("mgmt:fab:checks".into(), "Findings".into(), "fab:checks".into())) } // i18n-ok: ctl ids
    v
}
pub fn act(app: &mut Backplane, a: &str, cx: &mut Context<Backplane>) {
    match a { "drc" | "seen" | "refresh" | "review" => press(app, a, "", cx), "checks" => crate::screens::mgmt::toggle(app, "checks", cx), _ => if let Some(k) = a.strip_prefix("apply:") { press(app, "apply", k, cx) } }
}

fn btn(cx: &mut Context<Backplane>, light: bool, id: String, label: &str, on: impl Fn(&mut Backplane, &mut Context<Backplane>) + 'static) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let rect = id.clone();
    crate::ui::with_rect(div().id(SharedString::from(id)), move || rect).flex_shrink_0().h(px(22.)).px_2().flex().items_center().text_size(px(11.)).cursor_pointer()
        .bg(c("raise")).hover(|s| s.bg(c("line"))).child(trs(label)).on_click(cx.listener(move |this, _, _, cx| { on(this, cx); cx.notify(); })).into_any_element()
}

pub fn body(app: &mut Backplane, d: &Value, light: bool, cx: &mut Context<Backplane>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    if d.is_null() { return div().p_3().text_size(px(12.)).text_color(c("faint")).child(trs("Reading the fab's profile…")).into_any_element() }
    if !s(d, "error").is_empty() { return div().p_3().text_size(px(12.)).text_color(c("warn")).child(s(d, "error").to_string()).into_any_element() }
    // the top row: the fab, its revision and day, the board it is for, the tier, Refresh
    let read = tf("gpui.mgmt.fab.read", &[("rev", d["rev"].as_u64().unwrap_or(0).into()), ("day", s(d, "updated").to_string().into())]);
    let top = div().flex_shrink_0().min_h(px(super::head::ROW_H)).flex().items_center().gap_2().px_2().border_b_1().border_color(c("rule")).text_size(px(11.))
        .child(div().flex_shrink_0().text_color(c("text")).child(trs("The fab")))
        .child(div().flex_shrink_0().text_color(c("dim")).child(read))
        .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().text_color(c("faint")).child(format!("{} · {}", s(d, "board"), trs(tier_words(s(d, "tier"))))))
        .child(btn(cx, light, "mgmt:fab:refresh".into(), "Refresh", |a, cx| press(a, "refresh", "", cx))); // i18n-ok: English the draw passes through trs
    let mut list = div().id("fab-rows").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().text_size(px(12.));
    // the design review: its last run's verdict and counts; its findings are in the Checks list
    let rv = &d["review"];
    let running = rv["running"] == true;
    let line = if running { trs("Reviewing the design…") } else if rv["at"].is_null() { trs("No design review yet") } else {
        tf("gpui.mgmt.fab.review", &[("must", rv["counts"]["error"].as_u64().unwrap_or(0).into()), ("should", rv["counts"]["warning"].as_u64().unwrap_or(0).into()), ("notes", rv["counts"]["info"].as_u64().unwrap_or(0).into()), ("scope", s(rv, "scope").to_string().into())])
    };
    let bad = rv["counts"]["error"].as_u64().unwrap_or(0) > 0;
    let mut row = div().flex_shrink_0().flex().items_center().gap_2().px_2().h(px(28.)).border_b_1().border_color(c("line"))
        .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().text_color(c(if bad { "bad" } else { "dim" })).child(line));
    if !rv["at"].is_null() { row = row.child(btn(cx, light, "mgmt:fab:checks".into(), "Findings", |a, cx| crate::screens::mgmt::toggle(a, "checks", cx))) } // i18n-ok: English the draw passes through trs
    if !running { row = row.child(btn(cx, light, "mgmt:fab:review".into(), "Review", |a, cx| press(a, "review", "", cx))) } // i18n-ok: English the draw passes through trs
    list = list.child(row);
    // the DRC rules
    let (words, button) = drc_words(s(&d["drc"], "state"));
    let mut drc = div().flex_shrink_0().flex().items_center().gap_2().px_2().h(px(28.)).border_b_1().border_color(c("line"))
        .child(div().flex_1().min_w_0().text_color(c(if button.is_some() { "warn" } else { "dim" })).child(trs(words)));
    if let Some(b) = button { drc = drc.child(btn(cx, light, "mgmt:fab:drc".into(), b, |a, cx| press(a, "drc", "", cx))) }
    list = list.child(drc);
    // what needs the person: conflicts, board setup suggestions, changes
    for x in d["conflicts"].as_array().into_iter().flatten() { list = list.child(div().flex_shrink_0().px_2().py_1().border_b_1().border_color(c("line")).text_color(c("bad")).child(x.as_str().unwrap_or("").to_string())) }
    for x in d["suggestions"].as_array().cloned().unwrap_or_default() {
        let key = s(&x, "key").to_string();
        list = list.child(div().flex_shrink_0().flex().items_center().gap_2().px_2().h(px(28.)).border_b_1().border_color(c("line"))
            .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().text_color(c("warn")).child(suggestion_line(&x)))
            .child(btn(cx, light, format!("mgmt:fab:apply:{key}"), "Apply", move |a, cx| press(a, "apply", &key, cx)))); // i18n-ok: English the draw passes through trs
    }
    // the stackup: the one picked, or the one proposed with Use
    let st = &d["stackup"];
    if let Some(id) = st["chosen"].as_str() {
        let id = id.to_string(); let proposed = st["by"] == "proposed";
        let line = tf("gpui.mgmt.fab.stackup", &[("id", id.clone().into()), ("w", st["outer_50_mm"].as_f64().map(|w| format!("{w}")).unwrap_or_default().into())]);
        let mut row = div().flex_shrink_0().flex().items_center().gap_2().px_2().h(px(28.)).border_b_1().border_color(c("line"))
            .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().text_color(c(if proposed { "warn" } else { "dim" })).child(if proposed { format!("{} {line}", trs("Proposed:")) } else { line })); // i18n-ok: English the draw passes through trs
        if proposed { row = row.child(btn(cx, light, "mgmt:fab:stackup".into(), "Use", move |a, cx| press_stackup(a, &id, cx))) } // i18n-ok: English the draw passes through trs
        list = list.child(row);
    }
    let changed: Vec<Value> = d["changed"].as_array().cloned().unwrap_or_default();
    if !changed.is_empty() {
        let mut box_ = div().flex_shrink_0().flex().flex_col().px_2().py_1().gap_1().border_b_1().border_color(c("line"))
            .child(div().flex().items_center().gap_2().child(div().flex_1().text_color(c("warn")).child(trs("Changed since you last looked"))).child(btn(cx, light, "mgmt:fab:seen".into(), "Seen", |a, cx| press(a, "seen", "", cx)))); // i18n-ok: English the draw passes through trs
        for l in changed.iter().take(12) { box_ = box_.child(div().text_size(px(11.)).text_color(c("dim")).child(l.as_str().unwrap_or("").to_string())) }
        list = list.child(box_);
    }
    // the limits: what it is, free, for more, the day read (opens the source)
    let w = |i: usize| -> Div { let d = div().flex_shrink_0().overflow_hidden().whitespace_nowrap().text_ellipsis(); match i { 0 => d.flex_1().min_w_0(), 1 | 2 => d.w(px(72.)), _ => d.w(px(74.)) } };
    list = list.child(div().flex_shrink_0().flex().items_center().gap_2().px_2().h(px(22.)).border_b_1().border_color(c("line")).text_size(px(11.)).text_color(c("dim"))
        .child(w(0).child(trs("Limit"))).child(w(1).child(trs("Base price"))).child(w(2).child(trs("Costs more"))).child(w(3).child(trs("Read"))));
    for (i, l) in d["limits"].as_array().cloned().unwrap_or_default().into_iter().enumerate() {
        let side = if l["free"].is_object() { &l["free"] } else { &l["extra"] };
        let (url, day, derived) = (s(side, "source").to_string(), s(side, "fetched").to_string(), s(side, "confidence") != "published");
        let mark = if l["changed"] == true { "• " } else { "" };
        let label = format!("{mark}{}", s(&l, "label"));
        let cell = |v: &Value| v["text"].as_str().map(str::to_string).unwrap_or_else(|| "—".into());
        list = list.child(div().id(SharedString::from(format!("fab-l-{i}"))).flex_shrink_0().flex().items_center().gap_2().px_2().h(px(22.)).border_b_1().border_color(c("line"))
            .child(w(0).text_color(c(if l["changed"] == true { "warn" } else if l["inherited"] == true { "dim" } else { "text" })).child(label))
            .child(w(1).child(cell(&l["free"]))).child(w(2).text_color(c("dim")).child(cell(&l["extra"])))
            .child(w(3).id(SharedString::from(format!("fab-src-{i}"))).text_color(c("faint")).cursor_pointer().hover(|x| x.text_color(c("text")))
                .child(if derived { format!("{day}*") } else { day }).on_click(move |_, _, cx| { if url.starts_with("https://") { cx.open_url(&url) } })));
    }
    list = list.child(div().p_2().text_size(px(11.)).text_color(c("faint")).child(trs("* worked out from the fab's page (a mil in mm, a ring from a diameter). Dimmed: the rigid board's value, where this board's page says nothing else. A date opens the page it was read from.")));
    div().size_full().flex().flex_col().child(top).child(list).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn the_drc_state_says_what_to_press() {
        assert_eq!(drc_words("current").1, None); assert_eq!(drc_words("stale").1, Some("Update")); assert_eq!(drc_words("none").1, Some("Write")); assert_eq!(drc_words("noproject").1, None);
        assert_eq!(tier_words("extra"), "costs-more limits"); assert_eq!(tier_words(""), "base-price limits");
    }
    #[test] fn the_controls_follow_the_hubs_data() {
        let mut m = super::super::mgmt::Mgmt::default(); m.digest = "d".into();
        m.info("mgmt.state.d", &json!({"open": "fab"}).to_string());
        m.info("mgmt.fab.d", &json!({"drc": {"state": "stale"}, "suggestions": [{"key": "min_clearance"}], "changed": ["x"]}).to_string());
        assert_eq!(controls(&m).iter().map(|x| x.2.as_str()).collect::<Vec<_>>(), ["fab:drc", "fab:apply:min_clearance", "fab:seen", "fab:refresh", "fab:review"]);
        m.info("mgmt.fab.d", &json!({"drc": {"state": "current"}}).to_string()); assert_eq!(controls(&m).len(), 2);
    }
}

//! The Plan panel (screens/mgmt.rs hosts it; the hub's plan/panel.rs publishes `mgmt.plan.<digest>`): the routing plan per layer, each
//! corridor and plane region with its state in the floorplanner's language (proposed: translucent, dashed; edited by the person: solid, ✎;
//! approved: solid, ✓; routing: n/m; routed: faded; blocked: red, ⚠), net count and reference plane; Approve or Reject an item, a layer or
//! all (the person's decision: `plan.approve`), Stop routing, Hide done; a row press selects and zooms its nets in KiCad (`plan.focus`).
//! Pure rules first, tested.
use crate::{hub::Command, i18n::{tf, trs}, screens::settings::Secret, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};

static HIDE_DONE: AtomicBool = AtomicBool::new(false);
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
/// Whether an item counts as done (Hide done leaves it out).
pub fn done(i: &Value) -> bool { matches!(s(i, "state"), "routed" | "verified") }
/// The state's mark beside an item's name: ✓ approved, ✎ edited, n/m routing, ✓✓ verified, ⚠ n blocked.
pub fn mark(i: &Value) -> String {
    match s(i, "state") {
        "approved" => "✓".into(), "edited" => "✎".into(), "verified" => "✓✓".into(), "routed" => "100%".into(), "rejected" => "✗".into(), // i18n-ok: marks
        "routing" => match (i["routed"].as_u64(), i["total"].as_u64()) { (Some(r), Some(t)) if t > 0 => format!("{r}/{t}"), _ => "…".into() },
        "blocked" => format!("⚠ {}", i["problems"].as_u64().unwrap_or(0).max(1)),
        _ => String::new(),
    }
}
/// The rows a layer shows (Hide done leaves out routed and verified items).
pub fn shown(l: &Value, hide_done: bool) -> Vec<Value> { l["items"].as_array().into_iter().flatten().filter(|i| !(hide_done && done(i))).cloned().collect() }
fn hex(c: &str) -> u32 { u32::from_str_radix(c.trim_start_matches('#'), 16).unwrap_or(0x8a8a8a) }

fn send(app: &mut Backplane, m: &str, mut p: Value, cx: &mut Context<Backplane>) {
    let pro = app.screens.mgmt.project.clone(); if pro.is_empty() { return }
    p["pro"] = json!(pro);
    eprintln!("GPUI_LOG {}", json!({"event": "plan_press", "method": m, "params": p}));
    let _ = app.tx.try_send(Command::Rpc(m.into(), Secret(p), "mgmt"));
    app.ui_epoch += 1; cx.notify();
}
pub fn approve(app: &mut Backplane, layer: &str, ids: &[String], yes: bool, cx: &mut Context<Backplane>) { send(app, "plan.approve", json!({"layer": layer, "ids": ids, "approve": yes}), cx) }
/// The e2e harness's controls (`mgmt:plan:<what>`).
pub fn controls(m: &super::mgmt::Mgmt) -> Vec<(String, String, String)> {
    if m.peek()["open"] != "plan" { return vec![] }
    let d = m.data("plan"); let mut v: Vec<(String, String, String)> = vec![];
    if d["awaiting"].as_u64().unwrap_or(0) > 0 { v.push(("mgmt:plan:approve-all".into(), "Approve all".into(), "plan:all".into())) } // i18n-ok: ctl ids
    for l in d["layers"].as_array().into_iter().flatten() { for i in l["items"].as_array().into_iter().flatten() {
        let id = s(i, "id"); v.push((format!("mgmt:plan:row:{id}"), id.into(), format!("plan:focus:{id}")));
        if matches!(s(i, "state"), "proposed" | "edited") { v.push((format!("mgmt:plan:approve:{id}"), id.into(), format!("plan:approve:{id}"))); v.push((format!("mgmt:plan:reject:{id}"), id.into(), format!("plan:reject:{id}"))) }
    } }
    if d["routing"] == true { v.push(("mgmt:plan:stop".into(), "Stop".into(), "plan:stop".into())) } // i18n-ok: ctl ids
    v
}
pub fn act(app: &mut Backplane, a: &str, cx: &mut Context<Backplane>) {
    if a == "all" { approve(app, "", &[], true, cx) }
    else if a == "stop" { send(app, "plan.stop", json!({}), cx) }
    else if let Some(id) = a.strip_prefix("approve:") { approve(app, "", &[id.to_string()], true, cx) }
    else if let Some(id) = a.strip_prefix("reject:") { approve(app, "", &[id.to_string()], false, cx) }
    else if let Some(id) = a.strip_prefix("focus:") { send(app, "plan.focus", json!({"id": id}), cx) }
}

fn btn(cx: &mut Context<Backplane>, light: bool, id: String, label: &str, on: impl Fn(&mut Backplane, &mut Context<Backplane>) + 'static) -> AnyElement {
    let c = |n: &str| theme::color(light, n); let rect = id.clone();
    crate::ui::with_rect(div().id(SharedString::from(id)), move || rect).flex_shrink_0().h(px(20.)).px_2().flex().items_center().text_size(px(11.)).cursor_pointer()
        .bg(c("raise")).hover(|s| s.bg(c("line"))).child(trs(label)).on_click(cx.listener(move |this, _, _, cx| { on(this, cx); cx.notify(); })).into_any_element()
}

pub fn body(app: &mut Backplane, d: &Value, light: bool, cx: &mut Context<Backplane>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    if d.is_null() { return div().p_3().text_size(px(12.)).text_color(c("faint")).child(trs("Reading the routing plan…")).into_any_element() }
    if d["empty"] == true { return div().p_3().text_size(px(12.)).text_color(c("faint")).child(trs("No routing plan yet. An agent proposes one before routing (plan_propose); you steer and approve it here or in KiCad.")).into_any_element() }
    let hide = HIDE_DONE.load(Ordering::Relaxed);
    let awaiting = d["awaiting"].as_u64().unwrap_or(0);
    let mut top = div().flex_shrink_0().min_h(px(super::head::ROW_H)).flex().items_center().gap_2().px_2().border_b_1().border_color(c("rule")).text_size(px(11.))
        .child(div().flex_1().min_w_0().text_color(c(if awaiting > 0 { "warn" } else { "dim" })).child(if awaiting > 0 { tf("gpui.mgmt.plan.awaiting", &[("n", awaiting.into())]) } else { trs("Nothing awaits your approval") }));
    if !d["live"].as_bool().unwrap_or(false) { top = top.child(div().flex_shrink_0().text_color(c("faint")).child(trs("saved plan"))) }
    if awaiting > 0 { top = top.child(btn(cx, light, "mgmt:plan:approve-all".into(), "Approve all", |a, cx| approve(a, "", &[], true, cx))) } // i18n-ok: English the draw passes through trs
    if d["routing"] == true { top = top.child(btn(cx, light, "mgmt:plan:stop".into(), "Stop routing", |a, cx| send(a, "plan.stop", json!({}), cx))) } // i18n-ok: English the draw passes through trs
    top = top.child(btn(cx, light, "mgmt:plan:hide".into(), if hide { "Show done" } else { "Hide done" }, |_, cx| { HIDE_DONE.fetch_xor(true, Ordering::Relaxed); cx.notify(); })); // i18n-ok: English the draw passes through trs
    let mut list = div().id("plan-rows").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().text_size(px(12.));
    for l in d["layers"].as_array().cloned().unwrap_or_default() {
        let layer = s(&l, "layer").to_string(); let total = l["items"].as_array().map_or(0, Vec::len);
        let lid = layer.clone(); let lid2 = layer.clone();
        let mut head = div().flex_shrink_0().flex().items_center().gap_2().px_2().h(px(24.)).bg(c("panel")).border_b_1().border_color(c("line")).text_size(px(11.))
            .child(div().flex_shrink_0().text_color(c("text")).child(layer.clone()))
            .child(div().flex_shrink_0().text_color(c("dim")).child(format!("{} · {}/{} {}", s(&l, "role"), l["done"].as_u64().unwrap_or(0), total, trs("done"))))
            .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().text_color(c("faint")).child(s(&l, "stackup").to_string()));
        if s(&l, "status") != "approved" && total > 0 {
            head = head.child(btn(cx, light, format!("mgmt:plan:layer-ok:{layer}"), "Approve layer", move |a, cx| approve(a, &lid, &[], true, cx))) // i18n-ok: English the draw passes through trs
                .child(btn(cx, light, format!("mgmt:plan:layer-no:{layer}"), "Reject", move |a, cx| approve(a, &lid2, &[], false, cx))); // i18n-ok: English the draw passes through trs
        }
        list = list.child(head);
        for i in shown(&l, hide) {
            let id = s(&i, "id").to_string(); let col = hex(s(&i, "color")); let stt = s(&i, "state").to_string();
            let swatch = match stt.as_str() {
                "proposed" => div().w(px(14.)).h(px(10.)).bg(rgba((col << 8) | 0x40)).border_1().border_dashed().border_color(rgb(col)),
                "edited" => div().w(px(14.)).h(px(10.)).bg(rgba((col << 8) | 0x60)).border_1().border_color(rgb(col)),
                "routed" | "verified" => div().w(px(14.)).h(px(10.)).bg(rgba((col << 8) | 0x30)).border_1().border_color(rgba((col << 8) | 0x80)),
                "blocked" => div().w(px(14.)).h(px(10.)).bg(rgba((col << 8) | 0x60)).border_2().border_color(c("bad")),
                "rejected" => div().w(px(14.)).h(px(10.)).bg(c("raise")).border_1().border_color(c("line")),
                _ => div().w(px(14.)).h(px(10.)).bg(rgb(col)).border_1().border_color(rgb(col)),
            };
            let name = format!("{}{}", s(&i, "name"), if i["locked"] == true { " 🔒" } else { "" }); // i18n-ok: a mark
            let detail = if s(&i, "kind") == "plane" { i["reference"].as_str().filter(|r| !r.is_empty()).map(|r| tf("gpui.mgmt.plan.reference", &[("layer", r.to_string().into())])).unwrap_or_default() } else { tf("gpui.mgmt.plan.nets", &[("n", i["nets"].as_u64().unwrap_or(0).into())]) };
            let tone = match stt.as_str() { "blocked" => "bad", "edited" => "warn", "rejected" | "routed" | "verified" => "faint", _ => "text" };
            let rid = id.clone();
            let mut row = crate::ui::with_rect(div().id(SharedString::from(format!("plan-row-{id}"))), { let id = id.clone(); move || format!("mgmt:plan:row:{id}") })
                .flex_shrink_0().flex().items_center().gap_2().pl(px(14.)).pr_2().h(px(24.)).border_b_1().border_color(c("line")).cursor_pointer().hover(|x| x.bg(c("panel")))
                .child(swatch)
                .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().text_color(c(tone)).child(name))
                .child(div().flex_shrink_0().text_size(px(11.)).text_color(c("dim")).child(detail))
                .child(div().flex_shrink_0().w(px(44.)).text_size(px(11.)).text_color(c(if stt == "blocked" { "bad" } else { "dim" })).child(mark(&i)))
                .on_click(cx.listener(move |this, _, _, cx| send(this, "plan.focus", json!({"id": rid}), cx)));
            if matches!(stt.as_str(), "proposed" | "edited") {
                let (a, b) = (id.clone(), id.clone());
                row = row.child(btn(cx, light, format!("mgmt:plan:approve:{id}"), "✓", move |x, cx| approve(x, "", &[a.clone()], true, cx))) // i18n-ok: a mark
                    .child(btn(cx, light, format!("mgmt:plan:reject:{id}"), "✗", move |x, cx| approve(x, "", &[b.clone()], false, cx))); // i18n-ok: a mark
            }
            list = list.child(row);
        }
    }
    div().size_full().flex().flex_col().child(top).child(list).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn marks_and_hide_done() {
        assert_eq!(mark(&json!({"state": "routing", "routed": 9, "total": 14})), "9/14"); assert_eq!(mark(&json!({"state": "approved"})), "✓");
        assert_eq!(mark(&json!({"state": "blocked", "problems": 2})), "⚠ 2"); assert_eq!(mark(&json!({"state": "proposed"})), "");
        let l = json!({"items": [{"id": "a", "state": "routed"}, {"id": "b", "state": "proposed"}, {"id": "c", "state": "verified"}]});
        assert_eq!(shown(&l, true).iter().map(|i| s(i, "id").to_string()).collect::<Vec<_>>(), ["b"]); assert_eq!(shown(&l, false).len(), 3);
        assert_eq!(hex("#e8a33d"), 0xe8a33d);
        assert!(super::super::mgmt::VIEWS.iter().any(|v| v.id == "plan"), "the host draws the Plan panel");
    }
}

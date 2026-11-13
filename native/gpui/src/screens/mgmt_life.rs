//! The Lifecycle panel (screens/mgmt.rs hosts it; the hub's kicad/lifepanel.rs publishes `mgmt.life.<digest>`): each part's lifecycle,
//! and stock and price over time, for the whole BOM and per part. History starts when Backplane first looked a part up; with one reading so
//! far the panel says so and shows the values it has. Graphs: thin lines on the panel's ground, no decoration; hover shows the value and day.
//! Pure rules first (dates, scaling, the nearest point), tested; the element code only lays out and paints.
use crate::{hub::Command, i18n::{tf, trs}, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::{cell::Cell, rc::Rc};

/// The tag of `mgmt.life.part`'s answer (Command::Ask).
pub const PART_TAG: u64 = 0x6d67_6c66;

#[derive(Default)]
pub struct Ui {
    /// the part shown in detail (its key) and its samples (Null while asked)
    pub sel: Option<String>, pub samples: Value,
    /// the point under the pointer: (chart id, index)
    pub hover: Option<(&'static str, usize)>,
    /// each chart's bounds as last painted (for the pointer's nearest point)
    pub bounds: std::collections::HashMap<&'static str, Rc<Cell<Bounds<Pixels>>>>,
}

/// A day (unix seconds) as YYYY-MM-DD (UTC; the proleptic Gregorian calendar: Howard Hinnant's civil_from_days).
pub fn day(at: u64) -> String {
    let z = (at / 86400) as i64 + 719468;
    let era = z.div_euclid(146097); let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400; let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1; let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}
/// Points scaled into a w x h box (x by time, y from a floor of 0 to the highest value, flat series in the middle).
pub fn scale(pts: &[(f64, f64)], w: f32, h: f32) -> Vec<(f32, f32)> {
    if pts.is_empty() { return vec![] }
    let (x0, x1) = (pts.first().unwrap().0, pts.last().unwrap().0);
    let top = pts.iter().map(|p| p.1).fold(0.0f64, f64::max);
    pts.iter().map(|(x, y)| {
        let fx = if x1 > x0 { ((x - x0) / (x1 - x0)) as f32 } else { 0.5 };
        let fy = if top > 0.0 { (*y / top) as f32 } else { 0.5 };
        (fx * w, h - fy * h)
    }).collect()
}
/// The y axis' top: the highest value (the floor is 0).
pub fn top_of(pts: &[(f64, f64)]) -> f64 { pts.iter().map(|p| p.1).fold(0.0, f64::max) }
/// The point nearest the pointer's x (px from the chart's left).
pub fn nearest(scaled: &[(f32, f32)], x: f32) -> Option<usize> { scaled.iter().enumerate().min_by(|a, b| (a.1 .0 - x).abs().total_cmp(&(b.1 .0 - x).abs())).map(|(i, _)| i) }
fn pairs(v: &Value) -> Vec<(f64, f64)> { v.as_array().into_iter().flatten().filter_map(|p| Some((p[0].as_f64()?, p[1].as_f64()?))).collect() }
/// A part's samples as (stock, price) series, the lifecycle readings (no stock of their own) left out.
pub fn series(samples: &Value) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
    let ss: Vec<&Value> = samples["samples"].as_array().into_iter().flatten().filter(|s| s["stock"].is_i64() && s["stock"].as_i64().unwrap_or(-1) >= 0).collect();
    (ss.iter().filter_map(|s| Some((s["at"].as_f64()?, s["stock"].as_f64()?))).collect(), ss.iter().filter_map(|s| Some((s["at"].as_f64()?, s["price"].as_f64()?))).collect())
}
pub fn money(v: f64) -> String { super::mgmt_bom::money(v) }
fn count(v: f64) -> String { super::mgmt_bom::abbrev(v.round() as i64) }
/// What a lifecycle reads as, and its tone.
pub fn life_word(l: &str) -> (String, &'static str) {
    match l { "active" => (trs("Active"), "ok"), "nrnd" => (trs("NRND"), "warn"), "eol" => (trs("EOL"), "bad"), _ => (trs("Unknown"), "faint") }
}

/// A line chart: one thin line and dots at its points, the hovered one's value and day over it.
fn chart(app: &mut Backplane, id: &'static str, title: String, pts: Vec<(f64, f64)>, fmt: fn(f64) -> String, light: bool, cx: &mut Context<Backplane>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let cell = app.screens.mgmt.life.bounds.entry(id).or_insert_with(|| Rc::new(Cell::new(Bounds::default()))).clone();
    let hover = app.screens.mgmt.life.hover.filter(|h| h.0 == id).map(|h| h.1).filter(|i| *i < pts.len());
    let shown = hover.or(pts.len().checked_sub(1));
    let label = shown.map(|i| format!("{} · {}", fmt(pts[i].1), day(pts[i].0 as u64))).unwrap_or_default();
    let (line, dot, mark) = (c("accent"), c("dim"), c("text"));
    let p2 = pts.clone(); let cell2 = cell.clone();
    let plot = canvas(move |b, _, _| { cell2.set(b); }, move |b: Bounds<Pixels>, _, w: &mut Window, _| {
        let (bw, bh) = (f32::from(b.size.width), f32::from(b.size.height));
        let sc = scale(&p2, bw - 8.0, bh - 8.0);
        let at = |p: (f32, f32)| point(b.origin.x + px(p.0 + 4.0), b.origin.y + px(p.1 + 4.0));
        // the base line, then the series
        let mut base = PathBuilder::stroke(px(1.0)); base.move_to(point(b.origin.x, b.origin.y + px(bh - 0.5))); base.line_to(point(b.origin.x + px(bw), b.origin.y + px(bh - 0.5)));
        if let Ok(p) = base.build() { w.paint_path(p, rgba(0x80808033)); }
        if sc.len() >= 2 { let mut pb = PathBuilder::stroke(px(1.25)); pb.move_to(at(sc[0])); for q in &sc[1..] { pb.line_to(at(*q)); } if let Ok(p) = pb.build() { w.paint_path(p, line); } }
        for (i, q) in sc.iter().enumerate() {
            let r = if Some(i) == hover { 3.0 } else { 1.5 };
            let o = at(*q); let mut pb = PathBuilder::fill(); let n = 12;
            for k in 0..=n { let a = k as f32 / n as f32 * std::f32::consts::TAU; let pt = point(o.x + px(r * a.cos()), o.y + px(r * a.sin())); if k == 0 { pb.move_to(pt) } else { pb.line_to(pt) } }
            if let Ok(p) = pb.build() { w.paint_path(p, if Some(i) == hover { mark } else { dot }); }
        }
    }).size_full();
    let n = pts.len(); let p3 = pts.clone();
    let (first_at, last_at) = (pts.first().map_or(0, |p| p.0 as u64), pts.last().map_or(0, |p| p.0 as u64));
    div().flex().flex_col().gap(px(2.)).px_2().py_1()
        .child(div().flex().items_center().gap_2().text_size(px(11.)).child(div().text_color(c("dim")).child(title)).child(div().flex_1()).child(div().text_color(c("text")).child(label)))
        .child(div().flex().gap_1().child(div().w(px(46.)).flex_shrink_0().h(px(64.)).flex().flex_col().justify_between().items_end().text_size(px(10.)).text_color(c("faint"))
                .child(fmt(top_of(&pts))).child(fmt(0.0)))
            .child(div().flex_1().min_w_0().flex().flex_col().child(div().id(id).h(px(64.)).w_full().relative().child(plot)
            .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
                let b = cell.get(); let bw = f32::from(b.size.width);
                let x = f32::from(e.position.x - b.origin.x) - 4.0;
                let i = nearest(&scale(&p3, bw - 8.0, 1.0), x);
                let now = i.map(|i| (id, i));
                if this.screens.mgmt.life.hover != now && n > 0 { this.screens.mgmt.life.hover = now; cx.notify(); }
            }))
            .on_hover(cx.listener(move |this, on: &bool, _, cx| { if !*on && this.screens.mgmt.life.hover.map_or(false, |h| h.0 == id) { this.screens.mgmt.life.hover = None; cx.notify(); } })))
            // the days the line spans
            .child(div().flex().justify_between().text_size(px(10.)).text_color(c("faint")).child(day(first_at)).child(day(last_at)))))
        .into_any_element()
}

/// A part pressed: its samples asked of the hub, its graphs shown.
pub fn select(app: &mut Backplane, key: &str, cx: &mut Context<Backplane>) {
    if app.screens.mgmt.life.sel.as_deref() == Some(key) { app.screens.mgmt.life.sel = None; cx.notify(); return }
    app.screens.mgmt.life.sel = Some(key.to_string()); app.screens.mgmt.life.samples = Value::Null;
    let _ = app.tx.try_send(Command::Ask("mgmt.life.part".into(), json!({"project": app.screens.mgmt.project, "key": key}), PART_TAG));
    cx.notify();
}
pub fn answered(app: &mut Backplane, reply: &Value, cx: &mut Context<Backplane>) {
    let r = if reply["result"].is_object() { reply["result"].clone() } else { reply.clone() };
    if app.screens.mgmt.life.sel.as_deref() == r["key"].as_str() { eprintln!("GPUI_LOG {}", json!({"event": "life_part", "key": r["key"], "n": r["samples"].as_array().map_or(0, Vec::len)})); app.screens.mgmt.life.samples = r; cx.notify(); }
}
/// The e2e harness's controls: a part each (`mgmt:life:<key>`).
pub fn controls(m: &super::mgmt::Mgmt) -> Vec<(String, String, String)> {
    if m.peek()["open"] != "life" { return vec![] }
    m.data("life")["parts"].as_array().into_iter().flatten().filter_map(|p| p["key"].as_str()).map(|k| (format!("mgmt:life:{k}"), k.to_string(), format!("life:part:{k}"))).collect()
}
pub fn act(app: &mut Backplane, a: &str, cx: &mut Context<Backplane>) { if let Some(k) = a.strip_prefix("part:") { select(app, k, cx) } }

pub fn body(app: &mut Backplane, d: &Value, light: bool, cx: &mut Context<Backplane>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    if d.is_null() { return div().p_3().text_size(px(12.)).text_color(c("faint")).child(trs("Reading the schematic…")).into_any_element() }
    let counts = &d["counts"];
    let mut top = div().flex_shrink_0().min_h(px(super::head::ROW_H)).flex().items_center().gap_3().px_2().border_b_1().border_color(c("rule")).text_size(px(11.));
    for k in ["active", "nrnd", "eol"] {
        let (w, tone) = life_word(k); let n = counts[k].as_u64().unwrap_or(0);
        top = top.child(div().flex().items_center().gap_1().child(div().text_color(c(if n > 0 { tone } else { "faint" })).child(w)).child(div().text_color(c(if n > 0 { "text" } else { "faint" })).child(n.to_string())));
    }
    // what is not known is said once, not on every row
    let unknown = counts["unknown"].as_u64().unwrap_or(0);
    if unknown > 0 {
        let unchecked = d["unchecked"].as_u64().unwrap_or(0);
        top = top.child(div().id("life-unknown").text_color(c("faint")).tooltip(crate::keys::tip(&tf("gpui.mgmt.life.unknown.tip", &[("a", unchecked.into()), ("b", unknown.saturating_sub(unchecked).into())]), "", light))
            .child(tf("gpui.mgmt.life.unknown.n", &[("n", unknown.into())])));
    }
    top = top.child(div().flex_1().min_w_0()).children(d["since"].as_u64().map(|s| div().text_color(c("faint")).child(tf("gpui.mgmt.life.since", &[("day", day(s).into()), ("n", d["samples"].as_u64().unwrap_or(0).into())]))));
    let mut col = div().id("life-body").flex_1().min_h_0().overflow_y_scroll().flex().flex_col();
    // the whole BOM over time
    let cost = pairs(&d["bom"]["cost"]); let instock = pairs(&d["bom"]["instock"]);
    let lines = d["bom"]["lines"].as_u64().unwrap_or(0);
    if cost.is_empty() {
        col = col.child(div().px_2().py_2().text_size(px(12.)).text_color(c("faint")).child(if lines == 0 { trs("No line has an MPN or a fab part number to follow yet.") } else { trs("No reading of the fab's stock yet: the BOM panel's lookups start it.") }));
    } else if cost.len() < 2 {
        let (at, v) = cost[0]; let n = instock.first().map(|p| p.1).unwrap_or(0.0);
        col = col.child(div().px_2().py_2().flex().flex_col().gap_1().text_size(px(12.))
            .child(div().text_color(c("dim")).child(tf("gpui.mgmt.life.one", &[("day", day(at as u64).into())])))
            .child(div().text_color(c("text")).child(tf("gpui.mgmt.life.now", &[("cost", money(v).into()), ("n", (n as u64).into()), ("lines", lines.into())]))));
    } else {
        col = col.child(chart(app, "life-cost", trs("Parts a board"), cost, money, light, cx))
            .child(chart(app, "life-instock", tf("gpui.mgmt.life.instock", &[("lines", lines.into())]), instock, count, light, cx));
    }
    // the parts: a press shows the part's own graphs
    let sel = app.screens.mgmt.life.sel.clone();
    let parts: Vec<Value> = d["parts"].as_array().cloned().unwrap_or_default();
    let mut list = div().flex().flex_col().text_size(px(12.)).border_t_1().border_color(c("rule"));
    // the lifecycle column only once some part has one; the MPN takes what the others leave, shortened in its middle
    let any_known = parts.iter().any(|p| p["lifecycle"] != "unknown");
    // narrow, the price goes (the part's own graphs say it) and the MPN keeps the room
    let show_price = app.screens.mgmt.panel_w >= 460.;
    let n_cols = 3. + if any_known { 1. } else { 0. } + if show_price { 1. } else { 0. };
    let fixed = 16. + 92. + 56. + if show_price { 56. } else { 0. } + if any_known { 56. } else { 0. } + 8. * (n_cols - 1.);
    let mpn_chars = super::mgmt_bom::chars_in(app.screens.mgmt.panel_w.max(240.) - fixed);
    for p in &parts {
        let key = p["key"].as_str().unwrap_or("").to_string(); let on = sel.as_deref() == Some(key.as_str());
        let (w, tone) = life_word(p["lifecycle"].as_str().unwrap_or(""));
        let refs_v: Vec<String> = p["refs"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
        let refs = super::mgmt_bom::refs_fit(&refs_v, 12);
        // the stock said in words where it is not plain (never colour alone)
        let stock_t = match (p["stock_status"].as_str(), p["stock"].as_i64()) { (Some(st @ ("out" | "low")), Some(n)) => super::mgmt_bom::stock_words(&json!({"status": st, "stock": n})).unwrap_or_default(), (_, Some(n)) => super::mgmt_bom::abbrev(n), _ => "—".into() };
        let stock_tone = match p["stock_status"].as_str() { Some("out") => "bad", Some("low") => "warn", _ => "dim" };
        list = list.child(crate::ui::with_rect(div().id(SharedString::from(format!("life-{key}"))), { let k = key.clone(); move || format!("mgmt:life:{k}") })
            .flex_shrink_0().flex().items_center().gap_2().px_2().h(px(24.)).border_b_1().border_color(c("line")).cursor_pointer().when(on, |d| d.bg(c("raise"))).hover(|d| d.bg(c("panel")))
            .child(div().w(px(92.)).flex_shrink_0().overflow_hidden().whitespace_nowrap().text_color(c("dim")).child(refs))
            .child({ let m = p["mpn"].as_str().unwrap_or("").to_string(); let t = super::mgmt_bom::mid_ellipsis(&m, mpn_chars);
                div().id(SharedString::from(format!("life-mpn-{key}"))).flex_1().min_w_0().overflow_hidden().whitespace_nowrap().when(t != m, |d| d.tooltip(crate::keys::tip(&m, "", light))).child(t) })
            .when(any_known, |r| r.child(div().w(px(56.)).flex_shrink_0().text_color(c(tone)).child(if p["lifecycle"] == "unknown" { String::new() } else { w })))
            .child(div().w(px(56.)).flex_shrink_0().flex().justify_end().text_color(c(stock_tone)).child(stock_t))
            .when(show_price, |r| r.child(div().w(px(56.)).flex_shrink_0().flex().justify_end().text_color(c("dim")).child(p["price"].as_f64().map(money).unwrap_or_else(|| "—".into()))))
            .on_click({ let k = key.clone(); cx.listener(move |this, _, _, cx| select(this, &k, cx)) }));
    }
    // the part pressed: its own graphs, above the list (in view, whatever row it was)
    let detail = sel.as_ref().and_then(|k| parts.iter().find(|p| p["key"].as_str() == Some(k.as_str()))).map(|p| {
        let name = format!("{} · {}", super::mgmt_bom::refs_fit(&p["refs"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>(), 20), p["mpn"].as_str().unwrap_or(""));
        div().flex().flex_col().border_t_1().border_color(c("rule")).child(div().px_2().pt_1().text_size(px(11.)).text_color(c("text")).child(name)).child(part_detail(app, light, cx))
    });
    div().size_full().flex().flex_col().child(top).child(col.children(detail).child(list)).into_any_element()
}

/// The selected part's graphs, or what there is of them.
fn part_detail(app: &mut Backplane, light: bool, cx: &mut Context<Backplane>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let s = app.screens.mgmt.life.samples.clone();
    let wrap = div().flex().flex_col().bg(c("panel")).border_b_1().border_color(c("rule")).text_size(px(12.));
    if s.is_null() { return wrap.child(div().px_2().py_2().text_color(c("faint")).child(trs("Reading its history…"))).into_any_element() }
    let (stock, price) = series(&s);
    match stock.len() {
        0 => wrap.child(div().px_2().py_2().text_color(c("faint")).child(trs("No reading of this part yet."))).into_any_element(),
        1 => wrap.child(div().px_2().py_2().flex().flex_col().gap_1()
                .child(div().text_color(c("dim")).child(tf("gpui.mgmt.life.one", &[("day", day(stock[0].0 as u64).into())])))
                .child(div().text_color(c("text")).child(tf("gpui.mgmt.life.part.now", &[("stock", count(stock[0].1).into()), ("price", price.first().map(|p| money(p.1)).unwrap_or_else(|| "—".into()).into())])))).into_any_element(),
        _ => wrap.child(chart(app, "life-part-stock", trs("Stock"), stock, count, light, cx)).child(chart(app, "life-part-price", trs("Unit price"), price, money, light, cx)).into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn days() { assert_eq!(day(0), "1970-01-01"); assert_eq!(day(1794522000), "2026-11-12"); assert_eq!(day(951782400), "2000-02-29"); }
    #[test] fn scaling_and_the_nearest_point() {
        let s = scale(&[(0.0, 0.0), (10.0, 5.0), (20.0, 10.0)], 100.0, 50.0);
        assert_eq!(s, vec![(0.0, 50.0), (50.0, 25.0), (100.0, 0.0)]);
        assert_eq!(scale(&[(5.0, 3.0)], 100.0, 50.0), vec![(50.0, 0.0)]); assert_eq!(scale(&[(1.0, 0.0), (2.0, 0.0)], 10.0, 10.0)[0].1, 5.0);
        assert_eq!(nearest(&s, 60.0), Some(1)); assert_eq!(nearest(&s, 99.0), Some(2)); assert_eq!(nearest(&[], 1.0), None);
    }
    #[test] fn a_parts_series_and_words() {
        let v = json!({"samples": [{"at": 1, "stock": 10, "price": 0.5}, {"at": 2, "lifecycle": "NRND"}, {"at": 3, "stock": 8, "price": 0.6}, {"at": 4, "stock": -1}]});
        let (st, pr) = series(&v); assert_eq!(st, vec![(1.0, 10.0), (3.0, 8.0)]); assert_eq!(pr, vec![(1.0, 0.5), (3.0, 0.6)]);
        assert_eq!(top_of(&[(1.0, 3.0), (2.0, 9.0)]), 9.0); assert_eq!(top_of(&[]), 0.0);
        assert_eq!(money(1.234), "$1.23"); assert_eq!(money(0.0029), "$0.0029"); assert_eq!(life_word("eol").1, "bad"); assert_eq!(life_word("x").1, "faint");
    }
}
